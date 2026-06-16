<script lang="ts">
  let { data }: { data: { title?: string; items: string[] } } = $props();
</script>

<div class="space-y-1">
  {#if data.title}<div class="text-sm font-medium">{data.title}</div>{/if}
  <ul class="text-sm list-disc pl-5 space-y-0.5">
    {#each data.items as item}<li>{item}</li>{/each}
  </ul>
</div>
