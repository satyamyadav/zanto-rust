<script lang="ts">
  let { data }: { data: { label: string; value: any; unit?: string } } = $props();
</script>

<div>
  <div class="text-xs text-muted-foreground">{data.label}</div>
  <div class="text-2xl font-semibold tabular-nums">
    {data.value}{#if data.unit}<span class="text-sm text-muted-foreground ml-1">{data.unit}</span>{/if}
  </div>
</div>
