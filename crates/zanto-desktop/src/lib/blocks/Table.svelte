<script lang="ts">
  let { data }: { data: { title?: string; columns: string[]; rows: any[][] } } = $props();
  const columns = $derived(data?.columns ?? []);
  const rows = $derived(data?.rows ?? []);
</script>

<div class="space-y-1.5">
  {#if data.title}<div class="text-sm font-medium text-foreground">{data.title}</div>{/if}
  <div class="overflow-x-auto">
    <table class="w-full border-collapse text-sm">
      <thead>
        <tr class="border-b border-border text-left">
          {#each columns as c}
            <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">{c}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each rows as row}
          <tr class="border-b border-border/50">
            {#each row as cell}
              <td class="px-3 py-1.5 align-top break-words text-foreground">{cell}</td>
            {/each}
          </tr>
        {/each}
        {#if rows.length === 0}
          <tr>
            <td
              colspan={Math.max(columns.length, 1)}
              class="px-3 py-3 text-center text-muted-foreground">No data.</td
            >
          </tr>
        {/if}
      </tbody>
    </table>
  </div>
</div>
