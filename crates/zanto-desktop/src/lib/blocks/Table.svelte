<script lang="ts">
  let { data }: { data: { title?: string; columns: string[]; rows: any[][] } } = $props();
</script>

<div class="space-y-1">
  {#if data.title}<div class="text-sm font-medium">{data.title}</div>{/if}
  <div class="overflow-auto">
    <table class="w-full text-sm border-collapse">
      <thead>
        <tr class="text-left border-b border-border">
          {#each data.columns as c}<th class="py-1 pr-3 font-medium">{c}</th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each data.rows as row}
          <tr class="border-b border-border/50">
            {#each row as cell}<td class="py-1 pr-3">{cell}</td>{/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>
