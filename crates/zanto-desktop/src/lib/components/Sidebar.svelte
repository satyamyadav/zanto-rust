<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import MoreVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import MessageSquareIcon from "@lucide/svelte/icons/message-square";
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
  import ArchiveRestoreIcon from "@lucide/svelte/icons/archive-restore";
  import { appStore, mountApp } from "$lib/stores/app.svelte";
  import {
    sessionStore,
    newSession,
    selectSession,
    deleteSession,
    renameSession,
    archiveSession,
    unarchiveSession,
  } from "$lib/stores/session.svelte";

  let { onOpenSettings }: { onOpenSettings: () => void } = $props();

  let archivedOpen = $state(false);

  // The general "Chat" app is surfaced separately from the vertical apps.
  const chatApp = $derived(appStore.apps.find((a) => a.id === "chat"));
  const verticalApps = $derived(appStore.apps.filter((a) => a.id !== "chat"));

  async function switchTo(id: string) {
    await mountApp(id);
    sessionStore.canvas = null;
    await newSession(); // clears the thread, opens a fresh chat, reloads the list
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
  <!-- Chat + app switcher -->
  <div class="p-3 space-y-1">
    {#if chatApp}
      <button
        class="w-full flex items-center gap-2 text-left px-2 py-1.5 rounded-md text-sm transition-colors {appStore.activeId ===
        chatApp.id
          ? 'bg-sidebar-primary text-sidebar-primary-foreground'
          : 'hover:bg-sidebar-accent'}"
        onclick={() => switchTo(chatApp.id)}
      >
        <MessageSquareIcon class="size-4" /> Chat
      </button>
    {/if}

    {#if verticalApps.length > 0}
      <div class="text-[10px] uppercase tracking-wide text-muted-foreground mt-3 mb-1">Apps</div>
      {#each verticalApps as a}
        <button
          class="w-full text-left px-2 py-1.5 rounded-md text-sm transition-colors {appStore.activeId === a.id
            ? 'bg-sidebar-primary text-sidebar-primary-foreground'
            : 'hover:bg-sidebar-accent'}"
          onclick={() => switchTo(a.id)}
        >
          {a.name}
        </button>
      {/each}
    {/if}
  </div>

  <div class="border-t border-sidebar-border"></div>

  <!-- Chats (sessions for the active context) -->
  <div class="px-3 py-2 flex items-center justify-between">
    <div class="text-[10px] uppercase tracking-wide text-muted-foreground">Chats</div>
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
            <DropdownMenu.Item onclick={() => archiveSession(s.id)}>Archive</DropdownMenu.Item>
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

    <!-- Archived (collapsible) -->
    {#if sessionStore.archivedSessions.length > 0}
      <div class="pt-2">
        <button
          class="w-full flex items-center gap-1 px-2 py-1 text-[10px] uppercase tracking-wide text-muted-foreground hover:text-foreground"
          onclick={() => (archivedOpen = !archivedOpen)}
        >
          {#if archivedOpen}
            <ChevronDownIcon class="size-3" />
          {:else}
            <ChevronRightIcon class="size-3" />
          {/if}
          Archived ({sessionStore.archivedSessions.length})
        </button>
        {#if archivedOpen}
          {#each sessionStore.archivedSessions as s (s.id)}
            <div class="group flex items-center gap-1 rounded-md px-2 py-1.5 hover:bg-sidebar-accent">
              <div class="flex-1 min-w-0">
                <div class="truncate text-sm text-muted-foreground">{s.title || "Untitled"}</div>
                <div class="text-[10px] text-muted-foreground">
                  {relTime(s.updated_at)} · {s.message_count} msgs
                </div>
              </div>
              <Button
                variant="ghost"
                size="icon"
                class="size-6 opacity-0 group-hover:opacity-100 shrink-0"
                title="Unarchive"
                onclick={() => unarchiveSession(s.id)}
              >
                <ArchiveRestoreIcon class="size-4" />
              </Button>
            </div>
          {/each}
        {/if}
      </div>
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
