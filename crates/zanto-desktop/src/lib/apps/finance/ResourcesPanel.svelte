<script lang="ts">
  import { onMount } from "svelte";
  import { ipc, type FileEntry } from "$lib/ipc";
  import FileListItem from "$lib/components/FileListItem.svelte";
  import { send } from "$lib/stores/session.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Textarea } from "$lib/components/ui/textarea";
  import { Folder, FileText, ArrowUp, Paperclip, RefreshCw, Send, X } from "@lucide/svelte";

  // F3 — browse the configured roots / project dir (B1 `browse_dir`) and list
  // resource files (CSVs, statements). "Attach to chat" opens an editable prompt
  // that references the file path so the agent can read it with its fs tools.

  let entries = $state<FileEntry[]>([]);
  // `null` = listing the allowed roots; otherwise the directory being viewed.
  let cwd = $state<string | null>(null);
  // Breadcrumb of directory paths we descended through, for "up".
  let trail = $state<string[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);

  // Attach prompt the user can preview/edit before sending. `null` = closed.
  let draft = $state<string | null>(null);

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

  // At the allowed-roots level there's no parent to go up to.
  const atRoot = $derived(cwd === null);

  function attach(e: FileEntry) {
    draft =
      `I want to use the file at ${e.path} as a reference for my finances. ` +
      `Please read it with your file tools and help me import or review its contents.`;
  }

  function sendDraft() {
    if (draft && draft.trim()) send(draft.trim());
    draft = null;
  }

  // Extension label as a lightweight kind/size hint (byte size isn't exposed by
  // the browse IPC, so the file type is the most useful at-a-glance signal).
  function kind(name: string): string {
    const dot = name.lastIndexOf(".");
    if (dot <= 0 || dot === name.length - 1) return "file";
    return name.slice(dot + 1).toUpperCase();
  }

  // browse_dir already returns dirs-first, name-sorted; split for grouped rendering.
  const dirs = $derived(entries.filter((e) => e.isDir));
  const files = $derived(entries.filter((e) => !e.isDir));

  onMount(() => browse(null));
</script>

<div class="space-y-3">
  <div class="flex items-center justify-between">
    <div class="font-display text-sm font-semibold">Resource files</div>
    <Button variant="outline" size="xs" onclick={() => browse(cwd)} disabled={loading}>
      <RefreshCw /> Refresh
    </Button>
  </div>

  <div class="flex items-center gap-2 text-xs text-muted-foreground">
    <Button variant="outline" size="xs" onclick={up} disabled={atRoot} aria-label="Go up one level">
      <ArrowUp /> Up
    </Button>
    <span class="truncate font-mono">{cwd ?? "Allowed roots"}</span>
  </div>

  {#if error}
    <div class="text-sm text-destructive">Couldn't browse this folder: {error}. Try refreshing.</div>
  {:else if loading}
    <ul class="divide-y divide-border/50 rounded-lg border border-border">
      {#each Array(4) as _, i (i)}
        <li class="flex items-center gap-2 px-3 py-2">
          <div class="size-4 animate-pulse rounded bg-muted"></div>
          <div class="h-3.5 w-40 animate-pulse rounded bg-muted"></div>
        </li>
      {/each}
    </ul>
  {:else if entries.length === 0}
    <div class="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
      Nothing here yet. Add a project directory in settings to browse statements and CSVs.
    </div>
  {:else}
    <ul class="divide-y divide-border/50 rounded-lg border border-border">
      {#each dirs as d (d.path)}
        <li>
          <button
            type="button"
            class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
            onclick={() => openDir(d)}
          >
            <Folder class="size-4 text-muted-foreground" />
            <FileListItem name={d.name} path={d.path} isDir={true} />
            <span class="text-xs text-muted-foreground">Folder</span>
          </button>
        </li>
      {/each}
      {#each files as f (f.path)}
        <li class="flex items-center gap-2 px-3 py-2 text-sm">
          <FileText class="size-4 text-muted-foreground" />
          <FileListItem name={f.name} path={f.path} isDir={false} />
          <span class="font-mono text-xs text-muted-foreground">{kind(f.name)}</span>
          <Button variant="outline" size="xs" onclick={() => attach(f)}>
            <Paperclip /> Attach to chat
          </Button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if draft !== null}
    <div class="space-y-2 rounded-lg border border-border bg-card p-3">
      <div class="text-xs font-medium text-muted-foreground">Review the message before sending</div>
      <Textarea bind:value={draft} rows={4} class="text-sm" />
      <div class="flex items-center justify-end gap-2">
        <Button variant="ghost" size="sm" onclick={() => (draft = null)}>
          <X /> Cancel
        </Button>
        <Button size="sm" onclick={sendDraft} disabled={!draft.trim()}>
          <Send /> Send to chat
        </Button>
      </div>
    </div>
  {/if}
</div>
