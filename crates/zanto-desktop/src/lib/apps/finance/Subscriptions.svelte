<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { formatCurrency } from "./format";

  let { currency }: { currency?: string } = $props();

  type Item = {
    merchant: string;
    amount: number;
    count: number;
    last_date: string;
    monthly_total: number;
  };

  let items = $state<Item[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      const res = await ipc.queryApp("finance", "recurring");
      items = (res?.items ?? []) as Item[];
    } catch (e) {
      error = `${e}`;
    } finally {
      loading = false;
    }
  }

  function money(v: number | undefined): string {
    return formatCurrency(v, currency);
  }

  const total = $derived(items.reduce((s, it) => s + (it.monthly_total ?? 0), 0));

  onMount(load);
</script>

<div class="space-y-3">
  <div class="flex items-center gap-2">
    <div class="text-sm font-medium">Subscriptions</div>
    {#if items.length}
      <div class="ml-auto font-mono tabular-nums text-sm text-muted-foreground">
        {money(total)}/mo
      </div>
    {/if}
  </div>

  {#if error}
    <div class="text-sm text-destructive">Couldn't load subscriptions: {error}. Try again.</div>
  {:else if loading}
    <div class="h-24 animate-pulse rounded-lg border border-border bg-muted/40"></div>
  {:else if items.length === 0}
    <div class="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
      No recurring charges detected yet. They appear once a charge repeats monthly for a few months.
    </div>
  {:else}
    <ul class="space-y-1">
      {#each items as it (it.merchant)}
        <li class="flex items-center justify-between gap-4 border-b border-border/50 py-2">
          <div class="min-w-0">
            <div class="break-words font-medium text-foreground">{it.merchant}</div>
            <div class="text-xs text-muted-foreground">monthly · last seen {it.last_date}</div>
          </div>
          <div class="font-mono tabular-nums text-foreground">{money(it.amount)}</div>
        </li>
      {/each}
    </ul>
  {/if}
</div>
