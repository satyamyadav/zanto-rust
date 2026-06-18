<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import Chart from "$lib/blocks/Chart.svelte";
  import { formatCurrency } from "./format";

  let { currency }: { currency?: string } = $props();

  type Trends = {
    months: string[];
    categories: { category: string; data: number[] }[];
    mom_delta?: number;
    mom_pct?: number;
  };

  let trends = $state<Trends | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      trends = (await ipc.queryApp("finance", "trends")) as Trends;
    } catch (e) {
      error = `${e}`;
    } finally {
      loading = false;
    }
  }

  function money(v: number | undefined): string {
    return formatCurrency(v, currency);
  }

  const categories = $derived(trends?.categories ?? []);

  onMount(load);
</script>

<div class="space-y-3">
  {#if error}
    <div class="text-sm text-destructive">Couldn't load trends: {error}. Try again.</div>
  {:else if loading}
    <div class="h-64 animate-pulse rounded-lg border border-border bg-muted/40"></div>
  {:else if trends}
    {#if trends.mom_delta !== undefined}
      <div
        class={[
          "text-sm",
          trends.mom_delta >= 0 ? "text-destructive" : "text-success",
        ].join(" ")}
      >
        Spending {trends.mom_delta >= 0 ? "↑" : "↓"}
        {money(Math.abs(trends.mom_delta))} ({Math.round((trends.mom_pct ?? 0) * 100)}%) vs last month
      </div>
    {/if}

    {#if categories.length}
      <div class="rounded-lg border border-border bg-card p-3">
        <Chart
          data={{
            type: "line",
            title: "Category trends",
            labels: trends.months ?? [],
            datasets: categories.map((c) => ({ label: c.category, data: c.data })),
          }}
        />
      </div>
    {:else}
      <div class="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
        Not enough history yet for trends.
      </div>
    {/if}
  {/if}
</div>
