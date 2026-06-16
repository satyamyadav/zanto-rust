<script lang="ts">
  import { onMount } from "svelte";
  import * as Resizable from "$lib/components/ui/resizable";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Chat from "$lib/components/Chat.svelte";
  import Canvas from "$lib/components/Canvas.svelte";
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import ApprovalDialog from "$lib/ApprovalDialog.svelte";
  import { appStore, loadApps, mountApp } from "$lib/stores/app.svelte";
  import { loadSessions, newSession } from "$lib/stores/session.svelte";

  let settingsOpen = $state(false);

  onMount(async () => {
    await loadApps();
    // Land ready-to-chat: mount the first solution and open a fresh session.
    if (appStore.apps.length > 0) {
      await mountApp(appStore.apps[0].id);
      await loadSessions();
      await newSession();
    }
  });
</script>

<div class="h-screen w-screen bg-background text-foreground overflow-hidden">
  <Resizable.PaneGroup direction="horizontal" class="h-full">
    <Resizable.Pane defaultSize={20} minSize={14} maxSize={32}>
      <Sidebar onOpenSettings={() => (settingsOpen = true)} />
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

<SettingsDialog bind:open={settingsOpen} />
<ApprovalDialog />
