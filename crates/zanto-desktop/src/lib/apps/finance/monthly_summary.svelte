<script lang="ts">
  let { data }: { data: { month: string; total: number; by_category: { category: string; total: number }[] } } =
    $props();
  const cats = $derived(data?.by_category ?? []);
</script>

<div class="space-y-2">
  <div class="flex items-baseline justify-between gap-4">
    <span class="text-sm font-medium text-muted-foreground">{data?.month}</span>
    <span class="font-display text-xl font-semibold tabular-nums text-foreground">{data?.total}</span>
  </div>
  {#if cats.length === 0}
    <div class="text-sm text-muted-foreground">No spending recorded for this month.</div>
  {:else}
    <ul class="space-y-1 text-sm">
      {#each cats as c}
        <li class="flex justify-between gap-4 border-b border-border/50 py-1">
          <span class="break-words text-foreground">{c.category}</span>
          <span class="font-mono tabular-nums text-foreground">{c.total}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>
