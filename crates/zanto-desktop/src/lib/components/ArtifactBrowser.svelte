<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
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
      toast.error(`${e}`);
    } finally {
      loading = false;
    }
  }

  async function preview(id: string) {
    try {
      selected = await ipc.readStoredArtifact(id);
    } catch (e) {
      toast.error(`${e}`);
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
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-3xl">
    <Dialog.Header>
      <Dialog.Title>Artifacts</Dialog.Title>
    </Dialog.Header>

    <div class="flex items-center gap-2 pb-2">
      {#each ["all", "project", "global"] as const as s}
        <button
          class="rounded-md px-2 py-1 text-xs capitalize {scope === s
            ? 'bg-primary text-primary-foreground'
            : 'bg-muted text-muted-foreground hover:text-foreground'}"
          onclick={() => (scope = s)}
        >
          {s}
        </button>
      {/each}
    </div>

    <div class="flex h-[60vh] gap-3">
      <!-- List -->
      <div class="w-1/3 min-w-48 overflow-auto rounded-md border border-border">
        {#if loading}
          <div class="p-3 text-sm text-muted-foreground">Loading…</div>
        {:else if items.length === 0}
          <div class="p-3 text-sm text-muted-foreground">No artifacts.</div>
        {:else}
          {#each items as a (a.id)}
            <button
              class="w-full border-b border-border px-3 py-2 text-left last:border-b-0 {selected?.id === a.id
                ? 'bg-accent'
                : 'hover:bg-accent/50'}"
              onclick={() => preview(a.id)}
            >
              <div class="truncate text-sm font-medium">{a.title || "Untitled"}</div>
              <div class="text-[10px] text-muted-foreground">
                {a.kind} · {a.scope} · {fmtDate(a.created_at)}
              </div>
            </button>
          {/each}
        {/if}
      </div>

      <!-- Preview -->
      <div class="flex-1 overflow-auto rounded-md border border-border p-3">
        {#if !selected}
          <div class="text-sm text-muted-foreground">Select an artifact to preview.</div>
        {:else if selected.is_image}
          <img
            src={`data:${selected.mime ?? "image/png"};base64,${selected.content}`}
            alt={selected.title}
            class="max-w-full"
          />
        {:else if selected.kind === "markdown"}
          <Markdown data={{ title: selected.title, content: selected.content }} />
        {:else}
          <pre class="overflow-auto whitespace-pre-wrap text-xs">{selected.content}</pre>
        {/if}
      </div>
    </div>
  </Dialog.Content>
</Dialog.Root>
