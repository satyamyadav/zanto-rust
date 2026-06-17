<script lang="ts">
  import { onMount } from "svelte";
  import { ipc, type FileEntry } from "$lib/ipc";
  import { send } from "$lib/stores/session.svelte";
  import { Folder, FileText, ArrowUp, Paperclip, RefreshCw } from "@lucide/svelte";

  // F3 — browse the configured roots / project dir (B1 `browse_dir`) and list
  // resource files (CSVs, statements). "Attach to chat" sends a prompt that
  // references the file path so the agent can read it with its fs tools.

  let entries = $state<FileEntry[]>([]);
  // `null` = listing the allowed roots; otherwise the directory being viewed.
  let cwd = $state<string | null>(null);
  // Breadcrumb of directory paths we descended through, for "up".
  let trail = $state<string[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function browse(path: string | null) {
    loading = true;
    error = null;
    try {
      entries = await ipc.browseDir(path ?? undefined);
      cwd = path;
    } catch (e) {
      error = `${e}`;
    } finally {
      loading = false;
    }
  }

  function openDir(e: FileEntry) {
    if (cwd !== null) trail = [...trail, cwd];
    browse(e.path);
  }

  function up() {
    if (trail.length === 0) {
      browse(null);
      return;
    }
    const prev = trail[trail.length - 1];
    trail = trail.slice(0, -1);
    browse(prev);
  }

  function attach(e: FileEntry) {
    send(
      `I want to use the file at ${e.path} as a reference for my finances. ` +
        `Please read it with your file tools and help me import or review its contents.`,
    );
  }

  const dirs = $derived(entries.filter((e) => e.isDir));
  const files = $derived(entries.filter((e) => !e.isDir));

  onMount(() => browse(null));
</script>

<div class="space-y-3">
  <div class="flex items-center justify-between">
    <div class="text-sm font-medium">Resource files</div>
    <button
      type="button"
      class="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-muted disabled:opacity-50"
      onclick={() => browse(cwd)}
      disabled={loading}
    >
      <RefreshCw class="size-3.5" /> Refresh
    </button>
  </div>

  <div class="flex items-center gap-2 text-xs text-muted-foreground">
    {#if cwd !== null}
      <button
        type="button"
        class="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 hover:bg-muted"
        onclick={up}
      >
        <ArrowUp class="size-3.5" /> Up
      </button>
    {/if}
    <span class="truncate">{cwd ?? "Allowed roots"}</span>
  </div>

  {#if error}
    <div class="text-sm text-destructive">Couldn't browse: {error}</div>
  {:else if loading}
    <div class="text-sm text-muted-foreground">Loading…</div>
  {:else if entries.length === 0}
    <div class="text-sm text-muted-foreground">
      No files here. Add a project directory in settings to browse statements.
    </div>
  {:else}
    <ul class="divide-y divide-border/50 rounded-lg border border-border">
      {#each dirs as d (d.path)}
        <li>
          <button
            type="button"
            class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-muted"
            onclick={() => openDir(d)}
          >
            <Folder class="size-4 text-muted-foreground" />
            <span class="truncate">{d.name}</span>
          </button>
        </li>
      {/each}
      {#each files as f (f.path)}
        <li class="flex items-center gap-2 px-3 py-2 text-sm">
          <FileText class="size-4 text-muted-foreground" />
          <span class="min-w-0 flex-1 truncate">{f.name}</span>
          <button
            type="button"
            class="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-muted"
            onclick={() => attach(f)}
          >
            <Paperclip class="size-3.5" /> Attach to chat
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
