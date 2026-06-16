<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { send } from "$lib/stores/session.svelte";

  type Action = { label: string; prompt: string; icon?: string };
  let { data }: { data: { title?: string; actions: Action[] } } = $props();

  function run(a: Action) {
    send(a.prompt);
  }
</script>

<div class="space-y-2">
  {#if data.title}<div class="text-sm font-medium">{data.title}</div>{/if}
  <div class="flex flex-wrap gap-2">
    {#each data.actions as a}
      <Button variant="outline" size="sm" onclick={() => run(a)}>{a.label}</Button>
    {/each}
  </div>
</div>
