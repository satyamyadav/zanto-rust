<script lang="ts">
  let { data }: { data: { label: string; value: any; unit?: string } } = $props();
</script>

<div>
  <div class="text-xs font-medium text-muted-foreground">{data.label}</div>
  <div class="font-display text-2xl font-semibold tabular-nums text-foreground">
    {data.value}{#if data.unit}<span class="ml-1 font-sans text-sm font-normal text-muted-foreground"
        >{data.unit}</span
      >{/if}
  </div>
</div>
