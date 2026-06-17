<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { send } from "$lib/stores/session.svelte";
  import Chart from "$lib/blocks/Chart.svelte";
  import Onboarding from "./Onboarding.svelte";
  import { Plus, Receipt, Wallet, TrendingDown } from "@lucide/svelte";

  type Category = { category: string; total: number };
  type Overview = {
    empty: boolean;
    balance?: number;
    month?: string;
    month_total?: number;
    transaction_count?: number;
    top_categories?: Category[];
    series?: { labels: string[]; data: number[] };
  };
  type Profile = {
    setup: boolean;
    currency?: string;
    monthly_income?: number | null;
    categories?: string[];
  };

  let overview = $state<Overview | null>(null);
  let error = $state<string | null>(null);
  // When there is no data, first-run onboarding takes over the empty state until
  // a profile exists or the user skips it for this mount.
  let needsOnboarding = $state(false);

  async function load() {
    overview = null;
    error = null;
    try {
      overview = await ipc.queryApp("finance", "overview");
      if (overview?.empty) {
        const profile: Profile = await ipc.queryApp("finance", "profile");
        needsOnboarding = !profile?.setup;
      } else {
        needsOnboarding = false;
      }
    } catch (e) {
      error = `${e}`;
    }
  }

  // After saving or skipping onboarding, dismiss it and refresh the overview.
  function onboardingDone() {
    needsOnboarding = false;
    load();
  }

  onMount(load);

  function money(v: number | undefined): string {
    return (v ?? 0).toLocaleString(undefined, { maximumFractionDigits: 2 });
  }

  const cats = $derived(overview?.top_categories ?? []);
</script>

<div class="h-full">
  {#if error}
    <div class="text-sm text-destructive">Couldn't load overview: {error}</div>
  {:else if !overview}
    <div class="text-sm text-muted-foreground">Loading overview…</div>
  {:else if overview.empty && needsOnboarding}
    <Onboarding onDone={onboardingDone} />
  {:else if overview.empty}
    <div class="flex h-full flex-col items-center justify-center gap-4 text-center">
      <div class="rounded-full bg-muted p-4">
        <Wallet class="size-8 text-muted-foreground" />
      </div>
      <div class="space-y-1">
        <div class="text-lg font-semibold">No transactions yet</div>
        <div class="max-w-xs text-sm text-muted-foreground">
          Add your first transaction or ask for a summary to get started.
        </div>
      </div>
      <div class="flex gap-2">
        <button
          class="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:opacity-90"
          onclick={() => send("Add a transaction")}
        >
          <Plus class="size-4" /> Add a transaction
        </button>
        <button
          class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-muted"
          onclick={() => send("Show me this month's spending summary")}
        >
          <Receipt class="size-4" /> This month's summary
        </button>
      </div>
    </div>
  {:else}
    <div class="space-y-4">
      <div class="grid grid-cols-2 gap-3">
        <div class="rounded-lg border border-border p-3">
          <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
            <Wallet class="size-3.5" /> Balance
          </div>
          <div class="mt-1 text-2xl font-semibold tabular-nums">{money(overview.balance)}</div>
        </div>
        <div class="rounded-lg border border-border p-3">
          <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
            <TrendingDown class="size-3.5" /> This month
          </div>
          <div class="mt-1 text-2xl font-semibold tabular-nums">{money(overview.month_total)}</div>
        </div>
      </div>

      {#if overview.series && overview.series.labels.length}
        <div class="rounded-lg border border-border p-3">
          <Chart
            data={{
              type: "bar",
              title: "Spending — last 6 months",
              labels: overview.series.labels,
              datasets: [{ label: "Spend", data: overview.series.data }],
            }}
          />
        </div>
      {/if}

      <div class="rounded-lg border border-border p-3">
        <div class="mb-2 text-sm font-medium">Top categories ({overview.month})</div>
        {#if cats.length}
          <table class="w-full border-collapse text-sm">
            <thead>
              <tr class="border-b border-border text-left text-muted-foreground">
                <th class="py-1 pr-3 font-medium">Category</th>
                <th class="py-1 text-right font-medium">Total</th>
              </tr>
            </thead>
            <tbody>
              {#each cats as c}
                <tr class="border-b border-border/50">
                  <td class="py-1 pr-3">{c.category}</td>
                  <td class="py-1 text-right tabular-nums">{money(c.total)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <div class="text-sm text-muted-foreground">No spending recorded this month.</div>
        {/if}
      </div>
    </div>
  {/if}
</div>
