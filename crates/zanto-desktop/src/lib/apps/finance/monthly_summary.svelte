<script lang="ts">
  let { data }: { data: { month: string; total: number; by_category: { category: string; total: number }[] } } =
    $props();
  const cats = $derived(data?.by_category ?? []);
</script>

<div class="space-y-2">
  <div class="text-base font-medium">
    {data?.month} — total <span class="tabular-nums">{data?.total}</span>
  </div>
  <ul class="text-sm space-y-1">
    {#each cats as c}
      <li class="flex justify-between border-b border-gray-100 py-0.5">
        <span>{c.category}</span>
        <span class="tabular-nums">{c.total}</span>
      </li>
    {/each}
    {#if cats.length === 0}
      <li class="text-gray-400">No spending recorded for this month.</li>
    {/if}
  </ul>
</div>
