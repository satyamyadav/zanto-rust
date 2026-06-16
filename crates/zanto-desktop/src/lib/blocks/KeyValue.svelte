<script lang="ts">
  let { data }: { data: { title?: string; pairs: { key: string; value: any }[] } } = $props();
</script>

<div class="space-y-1">
  {#if data.title}<div class="text-sm font-medium">{data.title}</div>{/if}
  <dl class="text-sm">
    {#each data.pairs as p}
      <div class="flex justify-between gap-4 border-b border-border/50 py-0.5">
        <dt class="text-muted-foreground">{p.key}</dt>
        <dd class="tabular-nums">{p.value}</dd>
      </div>
    {/each}
  </dl>
</div>
