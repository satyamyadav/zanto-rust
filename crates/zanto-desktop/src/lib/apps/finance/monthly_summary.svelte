<script lang="ts">
  let {
    data,
  }: {
    data: {
      month: string;
      income?: number;
      total: number;
      net?: number;
      by_category: { category: string; total: number }[];
    };
  } = $props();
  const cats = $derived(data?.by_category ?? []);

  function money(v: number | undefined): string {
    return (v ?? 0).toLocaleString(undefined, { maximumFractionDigits: 2 });
  }
</script>

<div class="space-y-2">
  <div class="text-sm font-medium text-muted-foreground">{data?.month}</div>
  <div class="grid grid-cols-3 gap-3">
    <div>
      <div class="text-xs text-muted-foreground">Income</div>
      <div class="font-display text-lg font-semibold tabular-nums text-success">{money(data?.income)}</div>
    </div>
    <div>
      <div class="text-xs text-muted-foreground">Spent</div>
      <div class="font-display text-lg font-semibold tabular-nums text-foreground">{money(data?.total)}</div>
    </div>
    <div>
      <div class="text-xs text-muted-foreground">Net</div>
      <div class="font-display text-lg font-semibold tabular-nums text-foreground">{money(data?.net)}</div>
    </div>
  </div>
  {#if cats.length === 0}
    <div class="text-sm text-muted-foreground">No spending recorded for this month.</div>
  {:else}
    <ul class="space-y-1 text-sm">
      {#each cats as c}
        <li class="flex justify-between gap-4 border-b border-border/50 py-1">
          <span class="break-words text-foreground">{c.category}</span>
          <span class="font-mono tabular-nums text-foreground">{money(c.total)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>
