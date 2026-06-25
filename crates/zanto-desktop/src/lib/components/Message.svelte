<script lang="ts">
  import Block from "$lib/Block.svelte";
  import type { ChatEntry, ChatSegment } from "$lib/stores/session.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";
  import TextSegment from "./segments/TextSegment.svelte";
  import ToolCallSegment from "./segments/ToolCallSegment.svelte";
  import WorkflowGroup from "./segments/WorkflowGroup.svelte";
  import ThinkingBlock from "./segments/ThinkingBlock.svelte";
  import ErrorSegment from "./segments/ErrorSegment.svelte";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import CheckIcon from "@lucide/svelte/icons/check";
  import FileIcon from "@lucide/svelte/icons/file";
  import ImageIcon from "@lucide/svelte/icons/image";
  import SaveIcon from "@lucide/svelte/icons/save";
  import Trash2Icon from "@lucide/svelte/icons/trash-2";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import FolderOpenIcon from "@lucide/svelte/icons/folder-open";
  import { onDestroy } from "svelte";
  import ImageViewer from "$lib/components/ImageViewer.svelte";
  import { ipc } from "$lib/ipc";
  import { toast } from "svelte-sonner";

  // `isLast` marks the trailing entry — the only one that can be the live,
  // streaming turn whose trailing reasoning animates.
  let { entry, isLast = false }: { entry: ChatEntry; isLast?: boolean } = $props();

  type ToolCallSegmentData = Extract<ChatSegment, { kind: "tool_call" }>;

  // Pure-plumbing artifact reads: they produce no block and nothing the user needs
  // to see, so their tool-call card is always hidden. Block-rendering tools
  // (chart/render_artifact/finance views) are NOT listed here — they're hidden by
  // the authoritative `renders_as_block` segment flag instead of by name (B5-1).
  const PLUMBING_TOOL_CALLS = new Set(["list_artifacts", "get_artifact", "pin_artifact"]);

  // A tool-call segment whose card should be hidden: it rendered a block, or it's
  // internal plumbing.
  function isHiddenToolCall(seg: ChatSegment): boolean {
    return (
      seg.kind === "tool_call" &&
      (seg.renders_as_block === true || PLUMBING_TOOL_CALLS.has(seg.name))
    );
  }

  // Index of the LAST tool_call segment in document order (-1 if none). Text
  // BEFORE this index is the model's intermediate "working" narration (hoisted
  // into the Thinking block); text AT/AFTER it is the final answer (inline).
  const lastToolIdx = $derived.by(() => {
    let idx = -1;
    entry.segments.forEach((s, i) => {
      if (s.kind === "tool_call") idx = i;
    });
    return idx;
  });

  // A rendered item is one of: a workflow run (≥2 consecutive tool_calls →
  // WorkflowGroup) or a single segment. Reasoning segments AND pre-last-tool text
  // segments are NOT inline items — they are hoisted into ONE persistent
  // ThinkingBlock at the top of the turn (below). Walk the remaining segments in
  // document order, coalescing maximal runs of consecutive tool_call segments.
  // Tool calls / workflows / blocks / errors / final text appear inline,
  // interleaved, where they happened.
  type RenderItem =
    | { kind: "workflow"; steps: ToolCallSegmentData[] }
    | { kind: "single"; seg: ChatSegment };
  const items = $derived.by<RenderItem[]>(() => {
    const out: RenderItem[] = [];
    const lti = lastToolIdx;
    // Drop reasoning and pre-last-tool text (by document index) — those are
    // hoisted into the Thinking block. Everything else renders inline in order.
    const segs = entry.segments.filter((seg, idx) => {
      if (seg.kind === "reasoning") return false;
      if (seg.kind === "text" && idx < lti) return false;
      if (isHiddenToolCall(seg)) return false;
      return true;
    });
    let i = 0;
    while (i < segs.length) {
      const seg = segs[i];
      if (seg.kind === "tool_call") {
        let j = i;
        while (j < segs.length && segs[j].kind === "tool_call") j++;
        const run = segs.slice(i, j) as ToolCallSegmentData[];
        if (run.length >= 2) out.push({ kind: "workflow", steps: run });
        else out.push({ kind: "single", seg: run[0] });
        i = j;
      } else {
        out.push({ kind: "single", seg });
        i++;
      }
    }
    return out;
  });

  // The turn is live for the whole run (reasoning → tools → text) — driven by
  // `busy`, not just `streaming`, so the hoisted block stays live across tool
  // gaps and doesn't vanish the instant the first text chunk arrives.
  const live = $derived(isLast && entry.role === "assistant" && sessionStore.busy);

  // Hoisted "thinking/working" block inputs. The thinking content is the model's
  // working text: all reasoning segments PLUS the prose narration it writes
  // BEFORE the last tool call ("Let me check…", "It seems…"). The FINAL answer
  // (text at/after the last tool call, or all text when no tools) stays inline.
  // Concatenated in document order, blank line between distinct parts.
  const thinkingText = $derived(
    entry.segments
      .map((s, idx) => {
        if (s.kind === "reasoning") return s.text;
        if (s.kind === "text" && idx < lastToolIdx) return s.text;
        return "";
      })
      .filter((t) => t.trim().length > 0)
      .join("\n\n"),
  );
  const stepCount = $derived(
    entry.segments.filter((s) => s.kind === "tool_call" && !isHiddenToolCall(s)).length,
  );
  // Show the block when the turn produced working text OR any tool call — so
  // tool turns and narrating turns get a persistent affordance, but a trivial
  // pure-text turn does not.
  const showThinking = $derived(
    entry.role === "assistant" && (thinkingText.trim().length > 0 || stepCount > 0),
  );

  // Concatenated plain text of the message's text/markdown segments.
  const copyText = $derived(
    entry.segments
      .map((s) => (s.kind === "text" ? s.text : s.kind === "block" && s.block.kind === "markdown" ? s.block.text : ""))
      .filter((t) => t.length > 0)
      .join("\n\n"),
  );

  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  // Once a document is saved, its artifact id unlocks the file action bar
  // (Save a copy / Reveal / Delete). Local to this message component.
  let savedArtifactId = $state<string | null>(null);
  let confirmingDelete = $state(false);

  // Image viewer state.
  let viewerOpen = $state(false);
  let viewerIndex = $state(0);

  // Derived list of image attachments for the current entry.
  const imageAttachments = $derived(
    (entry.attachments ?? [])
      .filter((a) => a.isImage)
      .map((a) => ({ path: a.path, name: a.name })),
  );

  // Thumbnail data-URLs, keyed by path. Loaded lazily on first render.
  let thumbnails = $state<Record<string, string>>({});

  // Load thumbnails for all image attachments whenever the set changes.
  $effect(() => {
    for (const img of imageAttachments) {
      if (thumbnails[img.path]) continue;
      ipc.readImageDataUrl(img.path).then((url) => {
        thumbnails = { ...thumbnails, [img.path]: url };
      }).catch(() => {});
    }
  });

  function openViewer(idx: number) {
    viewerIndex = idx;
    viewerOpen = true;
  }

  function closeViewer() {
    viewerOpen = false;
  }

  async function copyMessage() {
    try {
      await navigator.clipboard.writeText(copyText);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1500);
    } catch {
      /* clipboard unavailable */
    }
  }

  // A "document" worth saving: an assistant message whose markdown is substantial
  // — has a heading, or is long enough not to be a throwaway reply. Keeps the
  // Save action off every one-line answer.
  const isDocument = $derived(
    entry.role === "assistant" &&
      (/^#{1,6}\s/m.test(copyText) || copyText.length >= 600),
  );

  // Title from the document's first heading, else "Untitled document".
  function documentTitle(text: string): string {
    const heading = text.split("\n").find((l) => /^#{1,6}\s/.test(l));
    return (
      (heading ?? "Untitled document").replace(/^#+\s*/, "").trim().slice(0, 80) ||
      "Untitled document"
    );
  }

  // Deliberate save: persist the message's document to the artifact store so it
  // appears (and upserts) in the Artifacts panel. Bumps the refresh signal so an
  // open browser reflects it.
  async function saveMessageDocument() {
    try {
      const ref = await ipc.storeDocumentArtifact(documentTitle(copyText), copyText);
      savedArtifactId = ref?.id ?? null;
      sessionStore.artifactsTick++;
      toast.success("Saved to Artifacts");
    } catch (e) {
      toast.error("Could not save the document", { description: `${e}` });
    }
  }

  // File actions on the saved document (id known after Save).
  async function saveCopyOfDocument() {
    if (!savedArtifactId) return;
    try {
      const saved = await ipc.saveArtifactCopy(savedArtifactId);
      if (saved) toast.success("Saved a copy");
    } catch (e) {
      toast.error("Could not save a copy", { description: `${e}` });
    }
  }

  async function revealDocument() {
    if (!savedArtifactId) return;
    try {
      await ipc.revealArtifact(savedArtifactId);
    } catch (e) {
      toast.error("Could not reveal the file", { description: `${e}` });
    }
  }

  async function deleteSavedDocument() {
    if (!savedArtifactId) return;
    try {
      await ipc.deleteStoredArtifact(savedArtifactId);
      savedArtifactId = null;
      confirmingDelete = false;
      sessionStore.artifactsTick++;
      toast.success("Deleted");
    } catch (e) {
      toast.error("Could not delete the document", { description: `${e}` });
    }
  }

  onDestroy(() => clearTimeout(copyTimer));

  // Per-code-block copy: a small copy button is overlaid on each rendered <pre>
  // (which lives inside the sanitized Block {@html}). Clicks are handled via
  // delegation on the message node. The button's "Copied" feedback is local and
  // self-resetting so it survives Block's streaming re-renders without any
  // reactive state pointing into the {@html} subtree.
  async function onContainerClick(e: MouseEvent) {
    const target = e.target as HTMLElement | null;
    const btn = target?.closest<HTMLButtonElement>("[data-code-copy]");
    if (!btn) return;
    e.preventDefault();
    const pre = btn.parentElement?.querySelector("pre");
    if (!pre) return;
    try {
      await navigator.clipboard.writeText(pre.innerText);
      btn.textContent = "Copied";
      window.setTimeout(() => {
        // Guard against a re-render that detached/replaced this button.
        if (btn.isConnected) btn.textContent = "Copy";
      }, 1500);
    } catch {
      /* clipboard unavailable */
    }
  }

  // Wrap each <pre> with a positioned container holding a copy button, and
  // listen for clicks on those buttons via delegation on the node itself.
  function decoratePre(node: HTMLElement) {
    node.addEventListener("click", onContainerClick);
    const decorate = () => {
      const pres = node.querySelectorAll<HTMLElement>("pre");
      pres.forEach((pre) => {
        const parent = pre.parentElement;
        if (!parent || parent.dataset.codeWrap === "1") return;
        const wrap = document.createElement("div");
        wrap.dataset.codeWrap = "1";
        wrap.style.position = "relative";
        pre.replaceWith(wrap);
        wrap.appendChild(pre);
        const btn = document.createElement("button");
        btn.type = "button";
        btn.dataset.codeCopy = "1";
        btn.setAttribute("aria-label", "Copy code");
        btn.className =
          "absolute right-1.5 top-1.5 rounded-md border border-border bg-background/80 px-1.5 py-1 text-xs text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100 focus:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
        btn.textContent = "Copy";
        wrap.appendChild(btn);
      });
    };
    decorate();
    const obs = new MutationObserver(decorate);
    obs.observe(node, { childList: true, subtree: true });
    return {
      destroy: () => {
        obs.disconnect();
        node.removeEventListener("click", onContainerClick);
      },
    };
  }

</script>

{#snippet renderItem(item: RenderItem)}
  {#if item.kind === "workflow"}
    <WorkflowGroup steps={item.steps} />
  {:else if item.seg.kind === "text"}
    <TextSegment text={item.seg.text} />
  {:else if item.seg.kind === "tool_call"}
    <ToolCallSegment name={item.seg.name} args={item.seg.args} output={item.seg.output} ok={item.seg.ok} />
  {:else if item.seg.kind === "block"}
    <Block block={item.seg.block} />
  {:else if item.seg.kind === "error"}
    <ErrorSegment message={item.seg.message} retryText={item.seg.retryText} />
  {/if}
{/snippet}

{#snippet assistantBody()}
  <!-- ONE persistent thinking/working block, hoisted above the inline items.
       Live across the whole turn (busy); collapses to a "Thought…" summary when
       done — it is never removed, so it doesn't vanish on the first chunk. -->
  {#if showThinking}
    <ThinkingBlock text={thinkingText} {stepCount} {live} />
  {/if}
  {#each items as item, i (i)}
    {@render renderItem(item)}
  {/each}
{/snippet}

{#if entry.role === "user"}
  <div class="flex justify-end">
    <div
      data-role="user"
      class="flex max-w-[85%] flex-col gap-1.5 rounded-2xl rounded-br-sm bg-muted px-4 py-2.5 text-sm leading-relaxed text-foreground shadow-sm"
    >
      {#each items as item, i (i)}
        {@render renderItem(item)}
      {/each}
      {#if entry.attachments && entry.attachments.length > 0}
        <div class="flex flex-wrap gap-1.5 pt-0.5">
          {#each entry.attachments as a, chipIdx (a.path)}
            {#if a.isImage}
              {@const imgIdx = imageAttachments.findIndex((i) => i.path === a.path)}
              <button
                type="button"
                onclick={() => openViewer(imgIdx >= 0 ? imgIdx : 0)}
                class="inline-flex items-center gap-1.5 rounded-md border border-border bg-background/50 px-1.5 py-1 text-xs text-muted-foreground hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                title={a.path}
                aria-label="View image {a.name}"
                data-attachment-chip
                data-image-chip
              >
                {#if thumbnails[a.path]}
                  <img
                    src={thumbnails[a.path]}
                    alt={a.name}
                    class="size-5 rounded object-cover"
                    data-thumbnail
                  />
                {:else}
                  <ImageIcon class="size-3.5 shrink-0" />
                {/if}
                <span class="max-w-40 truncate font-mono">{a.name}</span>
              </button>
            {:else}
              <span
                class="inline-flex items-center gap-1.5 rounded-md border border-border bg-background/50 px-2 py-1 text-xs text-muted-foreground"
                title={a.path}
                data-attachment-chip
              >
                <FileIcon class="size-3.5 shrink-0" />
                <span class="max-w-40 truncate font-mono">{a.name}</span>
              </span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  </div>
{:else}
  <div class="group flex justify-start">
    <div class="flex w-full max-w-[90%] flex-col gap-2.5 text-sm leading-relaxed text-foreground">
      <div use:decoratePre class="flex flex-col gap-2.5">
        {@render assistantBody()}
      </div>
      {#if copyText.length > 0}
        <div class="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          <button
            type="button"
            onclick={copyMessage}
            aria-label="Copy message"
            class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {#if copied}
              <CheckIcon class="size-3.5" />
              Copied
            {:else}
              <CopyIcon class="size-3.5" />
              Copy
            {/if}
          </button>
          {#if isDocument}
            {#if savedArtifactId === null}
              <button
                type="button"
                onclick={saveMessageDocument}
                aria-label="Save document to Artifacts"
                class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <SaveIcon class="size-3.5" />
                Save
              </button>
            {:else}
              <button
                type="button"
                onclick={saveCopyOfDocument}
                aria-label="Save a copy"
                class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <DownloadIcon class="size-3.5" />
                Save a copy…
              </button>
              <button
                type="button"
                onclick={revealDocument}
                aria-label="Reveal in folder"
                class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <FolderOpenIcon class="size-3.5" />
                Reveal in folder
              </button>
              {#if confirmingDelete}
                <button
                  type="button"
                  onclick={deleteSavedDocument}
                  aria-label="Confirm delete"
                  class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-destructive hover:bg-destructive/10 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <Trash2Icon class="size-3.5" />
                  Delete?
                </button>
                <button
                  type="button"
                  onclick={() => (confirmingDelete = false)}
                  aria-label="Cancel delete"
                  class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  Cancel
                </button>
              {:else}
                <button
                  type="button"
                  onclick={() => (confirmingDelete = true)}
                  aria-label="Delete document"
                  class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <Trash2Icon class="size-3.5" />
                  Delete
                </button>
              {/if}
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if viewerOpen && imageAttachments.length > 0}
  <ImageViewer images={imageAttachments} activeIndex={viewerIndex} onclose={closeViewer} />
{/if}
