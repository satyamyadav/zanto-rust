<script lang="ts">
  import { onMount } from "svelte";
  import * as Resizable from "$lib/components/ui/resizable";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Chat from "$lib/components/Chat.svelte";
  import Canvas from "$lib/components/Canvas.svelte";
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import { appStore, loadApps, mountApp } from "$lib/stores/app.svelte";
  import { newSession, initStreaming } from "$lib/stores/session.svelte";
  import { loadCatalogue } from "$lib/stores/artifacts.svelte";
  import { settingsStore, openSettings } from "$lib/stores/settings.svelte";

  onMount(async () => {
    initStreaming();
    await Promise.all([loadApps(), loadCatalogue()]);
    // Land in general Chat by default; fall back to the first app.
    const start = appStore.apps.find((a) => a.id === "chat") ?? appStore.apps[0];
    if (start) {
      await mountApp(start.id);
      await newSession();
    }
  });
</script>

<div class="h-screen w-screen bg-background text-foreground overflow-hidden">
  <Resizable.PaneGroup direction="horizontal" class="h-full">
    <Resizable.Pane defaultSize={20} minSize={14} maxSize={32}>
      <Sidebar onOpenSettings={() => openSettings()} />
    </Resizable.Pane>
    <Resizable.Handle />
    <Resizable.Pane defaultSize={52} minSize={30}>
      <Chat />
    </Resizable.Pane>
    <Resizable.Handle />
    <Resizable.Pane defaultSize={28} minSize={16}>
      <Canvas />
    </Resizable.Pane>
  </Resizable.PaneGroup>
</div>

<SettingsDialog bind:open={settingsStore.open} initialSection={settingsStore.section} />
