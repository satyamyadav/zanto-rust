<script lang="ts">
  import { onMount } from "svelte";
  import * as Resizable from "$lib/components/ui/resizable";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Chat from "$lib/components/Chat.svelte";
  import Canvas from "$lib/components/Canvas.svelte";
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import ApprovalDialog from "$lib/ApprovalDialog.svelte";
  import { loadApps } from "$lib/stores/app.svelte";

  let settingsOpen = $state(false);

  onMount(loadApps);
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
