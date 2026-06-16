<script lang="ts">
  let { data }: { data: { rows: any[] } } = $props();
  const rows = $derived(data?.rows ?? []);
</script>

<div class="overflow-auto">
  <table class="w-full text-sm border-collapse">
    <thead>
      <tr class="text-left border-b border-gray-300">
        <th class="py-1 pr-3">Date</th>
        <th class="py-1 pr-3">Merchant</th>
        <th class="py-1 pr-3">Category</th>
        <th class="py-1 pr-3 text-right">Amount</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as r}
        <tr class="border-b border-gray-100">
          <td class="py-1 pr-3">{r.date}</td>
          <td class="py-1 pr-3">{r.merchant}</td>
          <td class="py-1 pr-3">{r.category}</td>
          <td class="py-1 pr-3 text-right">{r.amount}</td>
        </tr>
      {/each}
      {#if rows.length === 0}
        <tr><td colspan="4" class="py-2 text-gray-400">No transactions yet.</td></tr>
      {/if}
    </tbody>
  </table>
</div>
