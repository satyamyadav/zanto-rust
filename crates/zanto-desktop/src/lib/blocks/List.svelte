<script lang="ts">
  let { data }: { data: { title?: string; items: string[] } } = $props();
  const items = $derived(data?.items ?? []);
</script>

<div class="space-y-1.5">
  {#if data.title}<div class="text-sm font-medium text-foreground">{data.title}</div>{/if}
  {#if items.length === 0}
    <div class="text-sm text-muted-foreground">No items.</div>
  {:else}
    <ul class="list-disc space-y-0.5 pl-5 text-sm text-foreground">
      {#each items as item}<li class="break-words">{item}</li>{/each}
    </ul>
  {/if}
</div>
