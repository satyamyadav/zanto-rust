<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import MoreVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import { appStore, mountApp } from "$lib/stores/app.svelte";
  import {
    sessionStore,
    newSession,
    loadSessions,
    selectSession,
    deleteSession,
    renameSession,
  } from "$lib/stores/session.svelte";

  let { onOpenSettings }: { onOpenSettings: () => void } = $props();

  async function pickApp(id: string) {
    await mountApp(id);
    sessionStore.convo = [];
    sessionStore.canvas = null;
    await loadSessions();
  }

  function relTime(unixSecs: number): string {
    const diff = unixSecs - Math.floor(Date.now() / 1000);
    const abs = Math.abs(diff);
    const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
    if (abs < 60) return rtf.format(Math.round(diff), "second");
    if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
    if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
    return rtf.format(Math.round(diff / 86400), "day");
  }

  async function doRename(id: string, current: string) {
    const title = window.prompt("Rename session", current);
    if (title != null) await renameSession(id, title);
  }
</script>

<div class="flex h-full flex-col bg-sidebar text-sidebar-foreground">
  <!-- App switcher -->
  <div class="p-3 space-y-1">
    <div class="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">Solutions</div>
    {#each appStore.apps as a}
      <button
        class="w-full text-left px-2 py-1.5 rounded-md text-sm transition-colors {appStore.activeId === a.id
          ? 'bg-sidebar-primary text-sidebar-primary-foreground'
          : 'hover:bg-sidebar-accent'}"
        onclick={() => pickApp(a.id)}
      >
        {a.name}
      </button>
    {/each}
    {#if appStore.apps.length === 0}
      <div class="text-sm text-muted-foreground">No apps</div>
    {/if}
  </div>

  <div class="border-t border-sidebar-border"></div>

  <!-- Sessions -->
  <div class="px-3 py-2 flex items-center justify-between">
    <div class="text-[10px] uppercase tracking-wide text-muted-foreground">Sessions</div>
    <Button variant="ghost" size="icon" class="size-6" onclick={newSession} disabled={!appStore.activeId}>
      <PlusIcon class="size-4" />
    </Button>
  </div>

  <div class="flex-1 overflow-auto px-2 space-y-0.5">
    {#each sessionStore.sessions as s (s.id)}
      <div
        class="group flex items-center gap-1 rounded-md px-2 py-1.5 {sessionStore.activeSessionId === s.id
          ? 'bg-sidebar-accent'
          : 'hover:bg-sidebar-accent'}"
      >
        <button class="flex-1 min-w-0 text-left" onclick={() => selectSession(s.id)}>
          <div class="truncate text-sm">{s.title || "Untitled"}</div>
          <div class="text-[10px] text-muted-foreground">
            {relTime(s.updated_at)} · {s.message_count} msgs
          </div>
        </button>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger class="opacity-0 group-hover:opacity-100 shrink-0">
            <MoreVerticalIcon class="size-4" />
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end">
            <DropdownMenu.Item onclick={() => doRename(s.id, s.title)}>Rename</DropdownMenu.Item>
            <DropdownMenu.Item class="text-destructive" onclick={() => deleteSession(s.id)}>
              Delete
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
    {/each}
    {#if sessionStore.sessions.length === 0}
      <div class="px-2 text-sm text-muted-foreground">No sessions yet.</div>
    {/if}
  </div>

  <!-- Footer -->
  <div class="border-t border-sidebar-border p-2 flex items-center justify-between gap-2">
    <span class="text-[11px] text-muted-foreground truncate px-1">{appStore.config?.model ?? ""}</span>
    <Button variant="ghost" size="icon" class="size-7 shrink-0" onclick={onOpenSettings}>
      <SettingsIcon class="size-4" />
    </Button>
  </div>
</div>
