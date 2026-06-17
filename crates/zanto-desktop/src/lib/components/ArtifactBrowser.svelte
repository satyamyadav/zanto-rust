<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { toast } from "svelte-sonner";
  import Markdown from "$lib/blocks/Markdown.svelte";
  import {
    ipc,
    type StoredArtifactRef,
    type StoredArtifact,
    type ArtifactScope,
  } from "$lib/ipc";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let scope = $state<ArtifactScope | "all">("all");
  let items = $state<StoredArtifactRef[]>([]);
  let selected = $state<StoredArtifact | null>(null);
  let loading = $state(false);

  async function refresh() {
    loading = true;
    try {
      items = await ipc.listStoredArtifacts(scope === "all" ? undefined : scope);
    } catch (e) {
      toast.error("Could not load artifacts", { description: `${e}` });
    } finally {
      loading = false;
    }
  }

  async function preview(id: string) {
    try {
      selected = await ipc.readStoredArtifact(id);
    } catch (e) {
      toast.error("Could not open the artifact", { description: `${e}` });
    }
  }

  // Reload the list whenever the dialog opens or the scope filter changes.
  // Clear any open preview so a stale selection isn't shown across opens.
  $effect(() => {
    if (open) {
      void scope; // track the filter
      selected = null;
      refresh();
    }
  });

  function fmtDate(unixSecs: number): string {
    return new Date(unixSecs * 1000).toLocaleString();
  }

  const scopes: { value: ArtifactScope | "all"; label: string }[] = [
    { value: "all", label: "All" },
    { value: "project", label: "Project" },
    { value: "global", label: "Global" },
  ];

  // Empty-state heading, scoped to the active filter ("No project artifacts yet").
  const emptyHeading = $derived(
    scope === "all" ? "No artifacts yet" : `No ${scope} artifacts yet`
  );
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-3xl">
    <Dialog.Header>
      <Dialog.Title class="font-display">Artifacts</Dialog.Title>
    </Dialog.Header>

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

    <div class="flex h-[60vh] flex-col gap-3 sm:flex-row">
      <!-- List -->
      <div class="overflow-auto rounded-md border border-border sm:w-1/3 sm:min-w-48">
        {#if loading}
          <div class="p-3 text-sm text-muted-foreground">Loading…</div>
        {:else if items.length === 0}
          <div class="flex h-full flex-col items-center justify-center gap-2 p-4 text-center">
            <p class="text-sm font-medium">{emptyHeading}</p>
            <p class="text-xs text-muted-foreground">
              Ask the assistant to create a chart, document, or note — saved artifacts appear here.
            </p>
            {#if scope !== "all"}
              <Button size="sm" variant="outline" onclick={() => (scope = "all")}>Show all scopes</Button>
            {/if}
          </div>
        {:else}
          {#each items as a (a.id)}
            <button
              type="button"
              class="w-full border-b border-border px-3 py-2 text-left last:border-b-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring {selected?.id === a.id
                ? 'bg-accent'
                : 'hover:bg-accent/50'}"
              onclick={() => preview(a.id)}
            >
              <div class="truncate text-sm font-medium">{a.title || "Untitled"}</div>
              <div class="font-mono text-xs text-muted-foreground">
                {a.kind} · {a.scope} · {fmtDate(a.created_at)}
              </div>
            </button>
          {/each}
        {/if}
      </div>

      <!-- Preview -->
      <div class="flex-1 overflow-auto rounded-md border border-border p-3">
        {#if !selected}
          <div class="flex h-full items-center justify-center text-sm text-muted-foreground">
            Select an artifact to preview it.
          </div>
        {:else if selected.is_image}
          <img
            src={`data:${selected.mime ?? "image/png"};base64,${selected.content}`}
            alt={selected.title || "Artifact image preview"}
            class="max-w-full"
          />
        {:else if selected.kind === "markdown"}
          <Markdown data={{ title: selected.title, content: selected.content }} />
        {:else}
          <pre class="overflow-auto whitespace-pre-wrap font-mono text-xs select-text">{selected.content}</pre>
        {/if}
      </div>
    </div>
  </Dialog.Content>
</Dialog.Root>
