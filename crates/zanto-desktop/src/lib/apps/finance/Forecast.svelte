<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { formatCurrency } from "./format";

  type ForecastData = {
    month: string;
    projected_net_worth: number;
    expected_income: number;
    expected_expense: number;
    avg_monthly_expense: number;
    month_to_date_income: number;
    month_to_date_expense: number;
  };

  let { currency }: { currency?: string } = $props();

  let forecast = $state<ForecastData | null>(null);
  let error = $state<string | null>(null);

  function money(v: number | undefined): string {
    return formatCurrency(v, currency);
  }

  async function load() {
    error = null;
    try {
      forecast = (await ipc.queryApp("finance", "forecast")) as ForecastData;
    } catch (e) {
      error = `${e}`;
    }
  }

  onMount(load);
</script>

<div class="rounded-lg border border-border bg-card p-3">
  {#if error}
    <div class="text-sm text-destructive">Couldn't load the forecast: {error}.</div>
  {:else if !forecast}
    <div class="h-3 w-32 animate-pulse rounded bg-muted"></div>
    <div class="mt-2 h-7 w-28 animate-pulse rounded bg-muted"></div>
  {:else}
    <div class="text-xs text-muted-foreground">Projected end of {forecast.month}</div>
    <div class="mt-1 font-display text-2xl font-semibold tabular-nums">
      {money(forecast.projected_net_worth)}
    </div>
    <div class="mt-1 text-sm text-muted-foreground">
      ≈ {money(forecast.expected_income)} in, {money(forecast.expected_expense)} out this month
    </div>
    <div class="mt-1 text-xs text-muted-foreground">based on your run-rate + recent averages</div>
  {/if}
</div>
