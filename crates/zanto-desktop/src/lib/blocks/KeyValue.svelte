<script lang="ts">
  let { data }: { data: { title?: string; pairs: { key: string; value: any }[] } } = $props();
  const pairs = $derived(data?.pairs ?? []);
</script>

<div class="space-y-1.5">
  {#if data.title}<div class="text-sm font-medium text-foreground">{data.title}</div>{/if}
  {#if pairs.length === 0}
    <div class="text-sm text-muted-foreground">No entries.</div>
  {:else}
    <dl class="text-sm">
      {#each pairs as p}
        <div class="flex justify-between gap-4 border-b border-border/50 py-1">
          <dt class="text-muted-foreground">{p.key}</dt>
          <dd class="text-right break-words font-mono tabular-nums text-foreground">{p.value}</dd>
        </div>
      {/each}
    </dl>
  {/if}
</div>
