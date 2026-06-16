<script lang="ts">
  import Block from "$lib/Block.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";
  import { appStore, activeApp } from "$lib/stores/app.svelte";
</script>

<div class="h-full overflow-auto p-4 bg-background">
  {#if sessionStore.canvas}
    <Block block={sessionStore.canvas} />
  {:else if appStore.activeId}
    <div class="text-sm text-muted-foreground">
      {activeApp()?.name} — views open here when the assistant presents them.
    </div>
  {:else}
    <div class="text-sm text-muted-foreground">Mount a solution to see its views.</div>
  {/if}
</div>
