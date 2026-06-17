<script lang="ts">
  import Block from "$lib/Block.svelte";
  import type { ChatEntry, ChatSegment } from "$lib/stores/session.svelte";
  import TextSegment from "./segments/TextSegment.svelte";
  import ReasoningSegment from "./segments/ReasoningSegment.svelte";
  import ToolCallSegment from "./segments/ToolCallSegment.svelte";
  import WorkflowGroup from "./segments/WorkflowGroup.svelte";
  import ErrorSegment from "./segments/ErrorSegment.svelte";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import CheckIcon from "@lucide/svelte/icons/check";
  import { onDestroy } from "svelte";

  let { entry }: { entry: ChatEntry } = $props();

  type ToolCallSegmentData = Extract<ChatSegment, { kind: "tool_call" }>;
  // A rendered item is either a workflow run (≥2 consecutive tool_calls) or a
  // single segment. Walk segments, coalescing maximal runs of consecutive
  // tool_call segments; a run of length ≥2 becomes a WorkflowGroup, anything
  // else (including a lone tool_call) renders as a single segment as before.
  type RenderItem = { kind: "workflow"; steps: ToolCallSegmentData[] } | { kind: "single"; seg: ChatSegment };
  const items = $derived.by<RenderItem[]>(() => {
    const out: RenderItem[] = [];
    const segs = entry.segments;
    let i = 0;
    while (i < segs.length) {
      if (segs[i].kind === "tool_call") {
        let j = i;
        while (j < segs.length && segs[j].kind === "tool_call") j++;
        const run = segs.slice(i, j) as ToolCallSegmentData[];
        if (run.length >= 2) out.push({ kind: "workflow", steps: run });
        else out.push({ kind: "single", seg: run[0] });
        i = j;
      } else {
        out.push({ kind: "single", seg: segs[i] });
        i++;
      }
    }
    return out;
  });

  // Concatenated plain text of the message's text/markdown segments.
  const copyText = $derived(
    entry.segments
      .map((s) => (s.kind === "text" ? s.text : s.kind === "block" && s.block.kind === "markdown" ? s.block.text : ""))
      .filter((t) => t.length > 0)
      .join("\n\n"),
  );

  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

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
          "absolute right-1.5 top-1.5 rounded-md border border-border bg-background/80 px-1.5 py-1 text-xs text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100 focus:opacity-100";
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

{#snippet segments()}
  {#each items as item, i (i)}
    {#if item.kind === "workflow"}
      <WorkflowGroup steps={item.steps} />
    {:else if item.seg.kind === "text"}
      <TextSegment text={item.seg.text} />
    {:else if item.seg.kind === "reasoning"}
      <ReasoningSegment text={item.seg.text} />
    {:else if item.seg.kind === "tool_call"}
      <ToolCallSegment name={item.seg.name} args={item.seg.args} output={item.seg.output} ok={item.seg.ok} />
    {:else if item.seg.kind === "block"}
      <Block block={item.seg.block} />
    {:else if item.seg.kind === "error"}
      <ErrorSegment message={item.seg.message} retryText={item.seg.retryText} />
    {/if}
  {/each}
{/snippet}

{#if entry.role === "user"}
  <div class="flex justify-end">
    <div
      class="flex max-w-[85%] flex-col gap-1.5 rounded-2xl rounded-br-sm bg-primary px-4 py-2.5 text-sm leading-relaxed text-primary-foreground shadow-sm"
    >
      {@render segments()}
    </div>
  </div>
{:else}
  <div class="group flex justify-start">
    <div class="flex max-w-[90%] flex-col gap-2.5 text-sm leading-relaxed text-foreground">
      <div use:decoratePre class="flex flex-col gap-2.5">
        {@render segments()}
      </div>
      {#if copyText.length > 0}
        <div class="opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          <button
            type="button"
            onclick={copyMessage}
            aria-label="Copy message"
            class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            {#if copied}
              <CheckIcon class="size-3.5" />
              Copied
            {:else}
              <CopyIcon class="size-3.5" />
              Copy
            {/if}
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}
