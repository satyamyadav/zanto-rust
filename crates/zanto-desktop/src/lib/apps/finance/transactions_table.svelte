<script lang="ts">
  let { data }: { data: { rows: any[] } } = $props();
  const rows = $derived(data?.rows ?? []);
</script>

<div class="overflow-x-auto">
  <table class="w-full border-collapse text-sm">
    <thead>
      <tr class="border-b border-border text-left">
        <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Date</th>
        <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Merchant</th>
        <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Category</th>
        <th scope="col" class="px-3 py-1.5 text-right font-medium text-muted-foreground">Amount</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as r}
        <tr class="border-b border-border/50">
          <td class="px-3 py-1.5 font-mono tabular-nums text-foreground">{r.date}</td>
          <td class="px-3 py-1.5 break-words text-foreground">{r.merchant}</td>
          <td class="px-3 py-1.5 text-muted-foreground">{r.category}</td>
          <td class="px-3 py-1.5 text-right font-mono tabular-nums text-foreground">{r.amount}</td>
        </tr>
      {/each}
      {#if rows.length === 0}
        <tr>
          <td colspan="4" class="px-3 py-4 text-center text-muted-foreground"
            >No transactions yet.</td
          >
        </tr>
      {/if}
    </tbody>
  </table>
</div>
