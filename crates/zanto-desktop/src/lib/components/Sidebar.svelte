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
  import FolderOpenIcon from "@lucide/svelte/icons/folder-open";
  import LayersIcon from "@lucide/svelte/icons/layers";
  import ArtifactBrowser from "./ArtifactBrowser.svelte";
  import Workspace from "./Workspace.svelte";
  import { workspaceStore } from "$lib/stores/workspace.svelte";
  import { appStore, mountApp } from "$lib/stores/app.svelte";
  import {
    sessionStore,
    newSession,
    selectSession,
    deleteSession,
    renameSession,
    archiveSession,
    unarchiveSession,
    loadMoreSessions,
  } from "$lib/stores/session.svelte";

  let { onOpenSettings }: { onOpenSettings: () => void } = $props();

  let artifactsOpen = $state(false);

  let archivedOpen = $state(false);

  // True while an app switch is in flight: drives the loading affordance and
  // guards against concurrent switches racing the session list.
  let switching = $state(false);

  // The general "Chat" app is surfaced separately from the vertical apps.
  const chatApp = $derived(appStore.apps.find((a) => a.id === "chat"));
  const verticalApps = $derived(appStore.apps.filter((a) => a.id !== "chat"));

  async function switchTo(id: string) {
    // Race guard: ignore further clicks while a switch is already running, so
    // rapid switches can't run mountApp/newSession concurrently and leave the
    // session list mismatched with the active app.
    if (switching) return;
    switching = true;
    try {
      await mountApp(id);
      sessionStore.canvas = null;
      await newSession(); // clears the thread, opens a fresh chat, reloads the list
    } finally {
      switching = false;
    }
  }

  // Infinite scroll: when the session list nears the bottom and more pages
  // exist, fetch the next page.
  function onSessionsScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    if (!sessionStore.sessionsHasMore || sessionStore.loadingMoreSessions) return;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 120) loadMoreSessions();
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

  // Shared focus ring for bare clickable elements (buttons not built on the ui primitive).
  const focusRing =
    "outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-0";
</script>

<div class="flex h-full flex-col bg-sidebar text-sidebar-foreground">
  <!-- Brand mark -->
  <div class="flex items-center gap-2 px-3 pt-3 pb-1">
    <span
      class="size-2 rounded-full bg-primary"
      aria-hidden="true"
    ></span>
    <span class="font-display text-sm font-semibold tracking-tight">zanto</span>
  </div>

  <!-- Chat + app switcher -->
  <div class="space-y-0.5 p-3 pt-2">
    {#if chatApp}
      {@const active = appStore.activeId === chatApp.id}
      <button
        class="relative flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-60 {focusRing} {active
          ? 'bg-sidebar-accent text-sidebar-accent-foreground'
          : 'hover:bg-sidebar-accent/60'}"
        aria-current={active ? "true" : undefined}
        disabled={switching}
        onclick={() => switchTo(chatApp.id)}
      >
        {#if active}
          <span
            class="absolute inset-y-1 left-0 w-0.5 rounded-full bg-primary"
            aria-hidden="true"
          ></span>
        {/if}
        <MessageSquareIcon class="size-4" /> Chat
      </button>
    {/if}

    {#if verticalApps.length > 0}
      <div class="mt-3 mb-1 px-2 font-display text-xs uppercase tracking-wide text-muted-foreground">
        Apps
      </div>
      {#each verticalApps as a}
        {@const active = appStore.activeId === a.id}
        <button
          class="relative w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-60 {focusRing} {active
            ? 'bg-sidebar-accent text-sidebar-accent-foreground'
            : 'hover:bg-sidebar-accent/60'}"
          aria-current={active ? "true" : undefined}
          disabled={switching}
          onclick={() => switchTo(a.id)}
        >
          {#if active}
            <span
              class="absolute inset-y-1 left-0 w-0.5 rounded-full bg-primary"
              aria-hidden="true"
            ></span>
          {/if}
          {a.name}
        </button>
      {/each}
    {/if}
  </div>

  <div class="border-t border-sidebar-border"></div>

  <!-- Chats (sessions for the active context) -->
  <div class="flex items-center justify-between px-3 py-2">
    <div class="font-display text-xs uppercase tracking-wide text-muted-foreground">Chats</div>
    <Button
      variant="ghost"
      size="icon"
      class="size-6"
      onclick={newSession}
      disabled={!appStore.activeId}
      title="New chat"
    >
      <PlusIcon class="size-4" />
    </Button>
  </div>

  <div
    class="flex-1 space-y-0.5 overflow-auto px-2 transition-opacity {switching
      ? 'pointer-events-none opacity-50'
      : ''}"
    onscroll={onSessionsScroll}
  >
    {#each sessionStore.sessions as s (s.id)}
      {@const active = sessionStore.activeSessionId === s.id}
      <div
        class="group relative flex items-center gap-1 rounded-md px-2 py-1.5 transition-colors {active
          ? 'bg-sidebar-accent'
          : 'hover:bg-sidebar-accent/60'}"
      >
        {#if active}
          <span
            class="absolute inset-y-1 left-0 w-0.5 rounded-full bg-primary"
            aria-hidden="true"
          ></span>
        {/if}
        <button
          class="min-w-0 flex-1 rounded-sm text-left {focusRing}"
          onclick={() => selectSession(s.id)}
        >
          <div class="truncate text-sm {active ? 'text-sidebar-accent-foreground' : ''}">
            {s.title || "Untitled"}
          </div>
          <div class="text-xs text-muted-foreground">
            {relTime(s.updated_at)} · {s.message_count} msgs
          </div>
        </button>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger
            class="shrink-0 rounded-sm p-0.5 text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100 group-focus-within:opacity-100 aria-expanded:opacity-100 {focusRing}"
            title="Chat actions"
          >
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
    {#if sessionStore.loadingMoreSessions}
      <div class="px-2 py-2 text-center text-xs text-muted-foreground">Loading…</div>
    {/if}
    {#if sessionStore.sessions.length === 0}
      <div class="px-2 py-6 text-center">
        <p class="text-sm text-foreground">No chats yet</p>
        <p class="mt-0.5 text-xs text-muted-foreground">Start a conversation to see it here.</p>
        <Button
          variant="outline"
          size="sm"
          class="mt-3"
          onclick={newSession}
          disabled={!appStore.activeId}
        >
          <PlusIcon class="size-4" /> New chat
        </Button>
      </div>
    {/if}

    <!-- Archived (collapsible) -->
    {#if sessionStore.archivedSessions.length > 0}
      <div class="pt-2">
        <button
          class="flex w-full items-center gap-1 rounded-sm px-2 py-1 font-display text-xs uppercase tracking-wide text-muted-foreground transition-colors hover:text-foreground {focusRing}"
          aria-expanded={archivedOpen}
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
            <div
              class="group flex items-center gap-1 rounded-md px-2 py-1.5 transition-colors hover:bg-sidebar-accent/60"
            >
              <div class="min-w-0 flex-1">
                <div class="truncate text-sm text-muted-foreground">{s.title || "Untitled"}</div>
                <div class="text-xs text-muted-foreground">
                  {relTime(s.updated_at)} · {s.message_count} msgs
                </div>
              </div>
              <Button
                variant="ghost"
                size="icon"
                class="size-6 shrink-0 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
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
  <div class="flex items-center justify-between gap-2 border-t border-sidebar-border p-2">
    <span class="truncate px-1 font-mono text-xs text-muted-foreground">{appStore.config?.model ?? ""}</span>
    <div class="flex shrink-0 items-center gap-1">
      <Button variant="ghost" size="icon" class="size-7" onclick={() => (workspaceStore.open = true)} title="Workspace">
        <LayersIcon class="size-4" />
      </Button>
      <Button variant="ghost" size="icon" class="size-7" onclick={() => (artifactsOpen = true)} title="Artifacts">
        <FolderOpenIcon class="size-4" />
      </Button>
      <Button variant="ghost" size="icon" class="size-7" onclick={onOpenSettings} title="Settings">
        <SettingsIcon class="size-4" />
      </Button>
    </div>
  </div>
</div>

<ArtifactBrowser bind:open={artifactsOpen} />
<Workspace bind:open={workspaceStore.open} />
