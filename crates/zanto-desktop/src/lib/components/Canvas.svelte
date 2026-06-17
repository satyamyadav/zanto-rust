<script lang="ts">
  import Block from "$lib/Block.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";
  import { appStore, activeApp } from "$lib/stores/app.svelte";
  import Dashboard from "$lib/apps/finance/Dashboard.svelte";
</script>

<div class="h-full overflow-auto bg-background p-4">
  {#if sessionStore.canvas}
    <Block block={sessionStore.canvas} />
  {:else if appStore.activeId === "finance"}
    <Dashboard />
  {:else}
    <div class="flex h-full items-center justify-center p-6">
      <div class="max-w-xs text-center font-sans">
        <p class="text-sm font-medium text-foreground">
          {activeApp()?.name ?? "Nothing open yet"}
        </p>
        <p class="mt-1 text-sm text-muted-foreground">
          Views and artifacts open here — ask zanto to show data as a table or chart.
        </p>
      </div>
    </div>
  {/if}
</div>
