<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { toast } from "svelte-sonner";
  import { FolderOpen, Download } from "@lucide/svelte";
  import Markdown from "$lib/blocks/Markdown.svelte";
  import Block from "$lib/Block.svelte";
  import { openWorkspace } from "$lib/stores/workspace.svelte";
  import {
    ipc,
    type StoredArtifactRef,
    type StoredArtifact,
    type PinnedArtifact,
    type ArtifactScope,
    type ChatBlock,
  } from "$lib/ipc";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  // Top-level backend tab: filesystem documents vs DB-pinned views.
  type Backend = "documents" | "views";
  let backend = $state<Backend>("documents");

  // Documents (filesystem) state.
  let scope = $state<ArtifactScope | "all">("all");
  let docItems = $state<StoredArtifactRef[]>([]);
  let selectedDoc = $state<StoredArtifact | null>(null);

  // Pinned views (DB) state.
  let viewItems = $state<PinnedArtifact[]>([]);
  let selectedView = $state<PinnedArtifact | null>(null);

  let loading = $state(false);
  let hasProjectDir = $state(true);

  async function refresh() {
    loading = true;
    try {
      if (backend === "documents") {
        docItems = await ipc.listStoredArtifacts(scope === "all" ? undefined : scope);
      } else {
        viewItems = await ipc.listPinnedArtifacts();
      }
    } catch (e) {
      toast.error("Could not load artifacts", { description: `${e}` });
    } finally {
      loading = false;
    }
  }

  async function previewDoc(id: string) {
    try {
      selectedDoc = await ipc.readStoredArtifact(id);
    } catch (e) {
      toast.error("Could not open the artifact", { description: `${e}` });
    }
  }

  // Save a copy of a document to a user-chosen path (native save dialog).
  // Documents only — pinned views live in the DB and have no file to copy.
  async function saveCopy(id: string) {
    try {
      const saved = await ipc.saveArtifactCopy(id);
      if (saved) toast.success("Saved a copy");
      else toast("Save cancelled");
    } catch (e) {
      toast.error("Could not save a copy", { description: `${e}` });
    }
  }

  // Reveal a document's file in the OS file manager.
  async function revealDoc(id: string) {
    try {
      await ipc.revealArtifact(id);
    } catch (e) {
      toast.error("Could not reveal the file", { description: `${e}` });
    }
  }

  async function refreshProjectDir() {
    try {
      hasProjectDir = Boolean((await ipc.getConfig()).project_dir);
    } catch {
      // Banner is non-blocking; assume configured on error so we don't nag.
      hasProjectDir = true;
    }
  }

  // Reload the list whenever the dialog opens or the backend/scope changes.
  // Clear any open preview so a stale selection isn't shown across opens.
  $effect(() => {
    if (open) {
      void backend; // track the backend tab
      void scope; // track the document filter
      selectedDoc = null;
      selectedView = null;
      refresh();
    }
  });

  // Refresh the working-dir banner whenever the dialog opens (config can change
  // between opens via the Workspace dialog).
  $effect(() => {
    if (open) refreshProjectDir();
  });

  function fmtDate(unixSecs: number): string {
    return new Date(unixSecs * 1000).toLocaleString();
  }

  // Build a renderable component block for a pinned view's preview.
  function viewBlock(v: PinnedArtifact): ChatBlock {
    return { kind: "component", component_id: v.component_id, data: v.data, target: "inline" };
  }

  const backends: { value: Backend; label: string }[] = [
    { value: "documents", label: "Documents" },
    { value: "views", label: "Pinned views" },
  ];

  const scopes: { value: ArtifactScope | "all"; label: string }[] = [
    { value: "all", label: "All" },
    { value: "project", label: "Project" },
    { value: "global", label: "Global" },
  ];

  // Empty-state heading, scoped to the active document filter.
  const emptyDocHeading = $derived(
    scope === "all" ? "No documents yet" : `No ${scope} documents yet`
  );
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-3xl">
    <Dialog.Header>
      <Dialog.Title class="font-display">Artifacts</Dialog.Title>
    </Dialog.Header>

    <!-- Backend tabs: filesystem documents vs DB-pinned views. -->
    <div class="flex items-center gap-4 border-b border-border pb-2" role="tablist" aria-label="Artifact backend">
      {#each backends as b (b.value)}
        <button
          type="button"
          role="tab"
          aria-selected={backend === b.value}
          class="-mb-px border-b-2 px-0.5 pb-1.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {backend === b.value
            ? 'border-primary text-foreground'
            : 'border-transparent text-muted-foreground hover:text-foreground'}"
          onclick={() => (backend = b.value)}
        >
          {b.label}
        </button>
      {/each}
    </div>

    {#if backend === "documents" && !hasProjectDir}
      <!-- Non-blocking nudge: documents fall back to the global store when no
           project dir is set. Opening the Workspace dialog lets the user set one. -->
      <div class="flex items-start gap-3 rounded-md border border-border bg-muted/40 p-3 text-xs">
        <FolderOpen class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <p class="flex-1 text-foreground">
          Documents save to the global store. Set a project folder to keep them with this project.
        </p>
        <Button size="sm" variant="outline" onclick={openWorkspace}>Set project folder</Button>
      </div>
    {/if}

    {#if backend === "documents"}
      <div class="flex items-center gap-4 border-b border-border pb-2" role="tablist" aria-label="Artifact scope">
        {#each scopes as s (s.value)}
          <button
            type="button"
            role="tab"
            aria-selected={scope === s.value}
            class="-mb-px border-b-2 px-0.5 pb-1.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {scope === s.value
              ? 'border-primary text-foreground'
              : 'border-transparent text-muted-foreground hover:text-foreground'}"
            onclick={() => (scope = s.value)}
          >
            {s.label}
          </button>
        {/each}
      </div>
    {/if}

    <div class="flex h-[60vh] flex-col gap-3 sm:flex-row">
      <!-- List -->
      <div class="overflow-auto rounded-md border border-border sm:w-1/3 sm:min-w-48">
        {#if loading}
          <div class="p-3 text-sm text-muted-foreground">Loading…</div>
        {:else if backend === "documents"}
          {#if docItems.length === 0}
            <div class="flex h-full flex-col items-center justify-center gap-2 p-4 text-center">
              <p class="text-sm font-medium">{emptyDocHeading}</p>
              <p class="text-xs text-muted-foreground">
                Ask the assistant to save a document or note — saved files appear here.
              </p>
              {#if scope !== "all"}
                <Button size="sm" variant="outline" onclick={() => (scope = "all")}>Show all scopes</Button>
              {/if}
            </div>
          {:else}
            {#each docItems as a (a.id)}
              <button
                type="button"
                class="w-full border-b border-border px-3 py-2 text-left last:border-b-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring {selectedDoc?.id === a.id
                  ? 'bg-accent'
                  : 'hover:bg-accent/50'}"
                onclick={() => previewDoc(a.id)}
              >
                <div class="truncate text-sm font-medium">{a.title || "Untitled"}</div>
                <div class="font-mono text-xs text-muted-foreground">
                  {a.kind} · {a.scope} · {fmtDate(a.created_at)}
                </div>
              </button>
            {/each}
          {/if}
        {:else if viewItems.length === 0}
          <div class="flex h-full flex-col items-center justify-center gap-2 p-4 text-center">
            <p class="text-sm font-medium">No pinned views yet</p>
            <p class="text-xs text-muted-foreground">
              Pin a chart, table, or other view to keep it here and reopen it later.
            </p>
          </div>
        {:else}
          {#each viewItems as v (v.id)}
            <button
              type="button"
              class="w-full border-b border-border px-3 py-2 text-left last:border-b-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring {selectedView?.id === v.id
                ? 'bg-accent'
                : 'hover:bg-accent/50'}"
              onclick={() => (selectedView = v)}
            >
              <div class="truncate text-sm font-medium">{v.title || "Untitled"}</div>
              <div class="font-mono text-xs text-muted-foreground">
                {v.component_id} · {fmtDate(v.created_at)}
              </div>
            </button>
          {/each}
        {/if}
      </div>

      <!-- Preview -->
      <div class="flex flex-1 flex-col overflow-hidden rounded-md border border-border">
        {#if backend === "documents" && selectedDoc}
          <!-- Document actions: Save a copy / Reveal in folder (filesystem docs
               only; not shown for pinned views). -->
          {@const docId = selectedDoc.id}
          <div class="flex items-center justify-end gap-2 border-b border-border px-3 py-2">
            <Button size="sm" variant="outline" onclick={() => saveCopy(docId)}>
              <Download class="size-4" />
              Save a copy…
            </Button>
            <Button size="sm" variant="outline" onclick={() => revealDoc(docId)}>
              <FolderOpen class="size-4" />
              Reveal in folder
            </Button>
          </div>
        {/if}
        <div class="flex-1 overflow-auto p-3">
        {#if backend === "documents"}
          {#if !selectedDoc}
            <div class="flex h-full items-center justify-center text-sm text-muted-foreground">
              Select a document to preview it.
            </div>
          {:else if selectedDoc.is_image}
            <img
              src={`data:${selectedDoc.mime ?? "image/png"};base64,${selectedDoc.content}`}
              alt={selectedDoc.title || "Artifact image preview"}
              class="max-w-full"
            />
          {:else if selectedDoc.kind === "markdown"}
            <Markdown data={{ title: selectedDoc.title, content: selectedDoc.content }} />
          {:else}
            <pre class="overflow-auto whitespace-pre-wrap font-mono text-xs select-text">{selectedDoc.content}</pre>
          {/if}
        {:else if !selectedView}
          <div class="flex h-full items-center justify-center text-sm text-muted-foreground">
            Select a pinned view to preview it.
          </div>
        {:else}
          {#key selectedView.id}
            <Block block={viewBlock(selectedView)} />
          {/key}
        {/if}
        </div>
      </div>
    </div>
  </Dialog.Content>
</Dialog.Root>
