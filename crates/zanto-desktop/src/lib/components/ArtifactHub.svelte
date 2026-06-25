<script lang="ts">
  // Artifact Hub: merges the explorer (Library) and the viewer (open tabs) into
  // one panel. The Library lists all stored documents; clicking one opens/focuses
  // its tab. New renders / saved chat documents open as tabs too. The active
  // tab's viewer carries the document toolbar (Save a copy / Reveal / Delete),
  // so the actions persist with the view instead of living on the chat bubble.
  import { Button } from "$lib/components/ui/button";
  import { toast } from "svelte-sonner";
  import { FolderOpen, Download, Trash2, X as XIcon, Save, Library } from "@lucide/svelte";
  import Markdown from "$lib/blocks/Markdown.svelte";
  import Block from "$lib/Block.svelte";
  import { openSettings } from "$lib/stores/settings.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";
  import {
    hubStore,
    openStored,
    openPinned,
    closeTab,
    focusTab,
    markDocSaved,
    removeStored,
    type HubTab,
  } from "$lib/stores/artifactHub.svelte";
  import {
    ipc,
    type StoredArtifactRef,
    type StoredArtifact,
    type PinnedArtifact,
    type ArtifactScope,
    type ChatBlock,
  } from "$lib/ipc";

  let { onClose }: { onClose: () => void } = $props();

  // ── Library (stored documents + pinned views) ─────────────────────────────
  type Backend = "documents" | "views";
  let backend = $state<Backend>("documents");
  let scope = $state<ArtifactScope | "all">("all");
  let docItems = $state<StoredArtifactRef[]>([]);
  let viewItems = $state<PinnedArtifact[]>([]);
  let loading = $state(false);
  let showLibrary = $state(true);

  async function refreshLibrary() {
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

  // Reload when the backend/scope changes or a doc is saved/deleted (artifactsTick).
  $effect(() => {
    void backend;
    void scope;
    void sessionStore.artifactsTick;
    refreshLibrary();
  });

  const backends: { value: Backend; label: string }[] = [
    { value: "documents", label: "Documents" },
    { value: "views", label: "Pinned views" },
  ];

  // ── Active tab content ────────────────────────────────────────────────────
  const activeTab = $derived<HubTab | null>(
    hubStore.open.find((t) => t.key === hubStore.activeKey) ?? null,
  );

  // For a "stored" tab, lazily fetch the full content. Keyed by id so switching
  // tabs refetches; cached in `loadedDoc` for the current active stored id.
  let loadedDoc = $state<StoredArtifact | null>(null);
  let loadingDoc = $state(false);
  let confirmingDelete = $state(false);

  $effect(() => {
    confirmingDelete = false;
    const t = activeTab;
    if (t && t.kind === "stored") {
      const id = t.id;
      loadingDoc = true;
      loadedDoc = null;
      ipc
        .readStoredArtifact(id)
        .then((d) => {
          // Guard against a race: only apply if this is still the active tab.
          if (hubStore.activeKey === `stored:${id}`) loadedDoc = d;
        })
        .catch((e) => toast.error("Could not open the artifact", { description: `${e}` }))
        .finally(() => (loadingDoc = false));
    } else {
      loadedDoc = null;
    }
  });

  // ── Toolbar actions (operate on the active stored artifact) ───────────────
  async function saveDocTab(tab: HubTab) {
    if (tab.kind !== "doc") return;
    try {
      const ref = await ipc.storeDocumentArtifact(tab.title, tab.text);
      markDocSaved(tab.key, ref.id, ref.title ?? tab.title);
      sessionStore.artifactsTick++;
      toast.success("Saved to Artifacts");
    } catch (e) {
      toast.error("Could not save the document", { description: `${e}` });
    }
  }

  async function saveCopy(id: string) {
    try {
      const saved = await ipc.saveArtifactCopy(id);
      if (saved) toast.success("Saved a copy");
    } catch (e) {
      toast.error("Could not save a copy", { description: `${e}` });
    }
  }

  async function revealDoc(id: string) {
    try {
      await ipc.revealArtifact(id);
    } catch (e) {
      toast.error("Could not reveal the file", { description: `${e}` });
    }
  }

  async function deleteDoc(id: string) {
    try {
      await ipc.deleteStoredArtifact(id);
      removeStored(id);
      confirmingDelete = false;
      sessionStore.artifactsTick++;
      toast.success("Deleted");
    } catch (e) {
      toast.error("Could not delete the document", { description: `${e}` });
    }
  }

  function fmtDate(unixSecs: number): string {
    return new Date(unixSecs * 1000).toLocaleString();
  }

  // A renderable component block for a pinned view (chart/table/etc).
  function pinnedBlock(componentId: string, data: unknown): ChatBlock {
    return { kind: "component", component_id: componentId, data, target: "inline" };
  }

  const scopes: { value: ArtifactScope | "all"; label: string }[] = [
    { value: "all", label: "All" },
    { value: "project", label: "Project" },
    { value: "global", label: "Global" },
  ];
</script>

<div class="flex h-full flex-col">
  <!-- Header: title + Library toggle + close -->
  <div class="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
    <h2 class="font-display text-sm font-medium text-foreground">Artifacts</h2>
    <Button
      variant={showLibrary ? "secondary" : "ghost"}
      size="sm"
      class="ml-1 h-7"
      onclick={() => (showLibrary = !showLibrary)}
      title="Toggle the library"
    >
      <Library class="size-3.5" />
      Library
    </Button>
    <div class="ml-auto"></div>
    <Button variant="ghost" size="icon" class="size-7" onclick={onClose} title="Close">
      <XIcon class="size-4" />
    </Button>
  </div>

  <div class="flex min-h-0 flex-1">
    <!-- Library list -->
    {#if showLibrary}
      <div class="flex w-56 shrink-0 flex-col border-r border-border">
        <!-- Backend: filesystem documents vs DB-pinned views. -->
        <div class="flex items-center gap-2 border-b border-border px-2 py-1.5" role="tablist" aria-label="Artifact backend">
          {#each backends as b (b.value)}
            <button
              type="button"
              role="tab"
              aria-selected={backend === b.value}
              class="rounded-md px-2 py-0.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {backend === b.value
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:text-foreground'}"
              onclick={() => (backend = b.value)}
            >
              {b.label}
            </button>
          {/each}
        </div>
        {#if backend === "documents"}
          <div class="flex items-center gap-1 border-b border-border px-2 py-1.5" role="tablist" aria-label="Artifact scope">
            {#each scopes as s (s.value)}
              <button
                type="button"
                role="tab"
                aria-selected={scope === s.value}
                class="rounded-md px-2 py-0.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {scope === s.value
                  ? 'bg-accent text-accent-foreground'
                  : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => (scope = s.value)}
              >
                {s.label}
              </button>
            {/each}
          </div>
        {/if}
        <div class="min-h-0 flex-1 overflow-auto rounded-md border border-border">
          {#if loading}
            <div class="p-3 text-sm text-muted-foreground">Loading…</div>
          {:else if backend === "documents"}
            {#if docItems.length === 0}
              <div class="flex flex-col items-center justify-center gap-1 p-4 text-center">
                <p class="text-sm font-medium">No documents yet</p>
                <p class="text-xs text-muted-foreground">Saved documents appear here.</p>
              </div>
            {:else}
              {#each docItems as a (a.id)}
                {@const isOpen = hubStore.open.some((t) => t.key === `stored:${a.id}`)}
                <button
                  type="button"
                  class="w-full border-b border-border px-3 py-2 text-left last:border-b-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring {isOpen
                    ? 'bg-accent/40'
                    : 'hover:bg-accent/50'}"
                  onclick={() => openStored(a.id, a.title || 'Untitled')}
                >
                  <div class="truncate text-sm font-medium">{a.title || "Untitled"}</div>
                  <div class="font-mono text-xs text-muted-foreground">
                    {a.kind} · {a.scope} · {fmtDate(a.created_at)}
                  </div>
                </button>
              {/each}
            {/if}
          {:else if viewItems.length === 0}
            <div class="flex flex-col items-center justify-center gap-1 p-4 text-center">
              <p class="text-sm font-medium">No pinned views yet</p>
              <p class="text-xs text-muted-foreground">Pin a chart or table to keep it here.</p>
            </div>
          {:else}
            {#each viewItems as v (v.id)}
              {@const isOpen = hubStore.open.some((t) => t.key === `pinned:${v.id}`)}
              <button
                type="button"
                class="w-full border-b border-border px-3 py-2 text-left last:border-b-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring {isOpen
                  ? 'bg-accent/40'
                  : 'hover:bg-accent/50'}"
                onclick={() => openPinned(v.id, v.component_id, v.data, v.title || 'Untitled')}
              >
                <div class="truncate text-sm font-medium">{v.title || "Untitled"}</div>
                <div class="font-mono text-xs text-muted-foreground">
                  {v.component_id} · {fmtDate(v.created_at)}
                </div>
              </button>
            {/each}
          {/if}
        </div>
      </div>
    {/if}

    <!-- Tabs + viewer -->
    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      {#if hubStore.open.length === 0}
        <div class="flex h-full items-center justify-center p-6 text-center">
          <p class="max-w-xs text-sm text-muted-foreground">
            Open a document from the Library, or ask the assistant to create one — it opens here as a tab.
          </p>
        </div>
      {:else}
        <!-- Tab strip -->
        <div class="flex shrink-0 items-stretch gap-1 overflow-x-auto border-b border-border px-1.5 py-1" role="tablist" aria-label="Open artifacts">
          {#each hubStore.open as t (t.key)}
            <div
              class="group flex items-center gap-1 rounded-md px-2 py-1 text-xs transition-colors {hubStore.activeKey === t.key
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:bg-muted/50'}"
            >
              <button
                type="button"
                role="tab"
                aria-selected={hubStore.activeKey === t.key}
                onclick={() => focusTab(t.key)}
                class="max-w-40 truncate focus-visible:outline-none"
              >
                {t.title}{t.kind === "doc" ? " •" : ""}
              </button>
              <button
                type="button"
                aria-label="Close tab"
                onclick={() => closeTab(t.key)}
                class="rounded opacity-60 hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <XIcon class="size-3" />
              </button>
            </div>
          {/each}
        </div>

        <!-- Toolbar for the active tab: documents get file actions; pinned views
             are DB records with no file, so no toolbar. -->
        {#if activeTab && activeTab.kind === "doc"}
          <div class="flex shrink-0 items-center justify-end gap-2 border-b border-border px-3 py-2">
            <Button size="sm" onclick={() => saveDocTab(activeTab)}>
              <Save class="size-4" />
              Save
            </Button>
          </div>
        {:else if activeTab && activeTab.kind === "stored"}
          {@const docId = activeTab.id}
          <div class="flex shrink-0 items-center justify-end gap-2 border-b border-border px-3 py-2">
            <Button size="sm" variant="outline" onclick={() => saveCopy(docId)}>
              <Download class="size-4" />
              Save a copy…
            </Button>
            <Button size="sm" variant="outline" onclick={() => revealDoc(docId)}>
              <FolderOpen class="size-4" />
              Reveal in folder
            </Button>
            {#if confirmingDelete}
              <Button size="sm" variant="destructive" onclick={() => deleteDoc(docId)}>Delete</Button>
              <Button size="sm" variant="ghost" onclick={() => (confirmingDelete = false)}>Cancel</Button>
            {:else}
              <Button size="sm" variant="outline" onclick={() => (confirmingDelete = true)}>
                <Trash2 class="size-4" />
                Delete
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Active viewer -->
        <div class="min-h-0 flex-1 overflow-auto p-3">
          {#if activeTab?.kind === "doc"}
            <Markdown data={{ title: activeTab.title, content: activeTab.text }} />
          {:else if activeTab?.kind === "pinned"}
            {#key activeTab.key}
              <Block block={pinnedBlock(activeTab.componentId, activeTab.data)} canPin={false} />
            {/key}
          {:else if loadingDoc}
            <div class="flex h-full items-center justify-center text-sm text-muted-foreground">Loading…</div>
          {:else if loadedDoc?.is_image}
            <img
              src={`data:${loadedDoc.mime ?? "image/png"};base64,${loadedDoc.content}`}
              alt={loadedDoc.title || "Artifact image"}
              class="max-w-full"
            />
          {:else if loadedDoc?.kind === "markdown"}
            <Markdown data={{ title: loadedDoc.title, content: loadedDoc.content }} />
          {:else if loadedDoc}
            <pre class="overflow-auto whitespace-pre-wrap font-mono text-xs select-text">{loadedDoc.content}</pre>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</div>
