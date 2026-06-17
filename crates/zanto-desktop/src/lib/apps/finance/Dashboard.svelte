<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { send } from "$lib/stores/session.svelte";
  import Chart from "$lib/blocks/Chart.svelte";
  import Onboarding from "./Onboarding.svelte";
  import ResourcesPanel from "./ResourcesPanel.svelte";
  import WidgetBuilder, { type Widget } from "./WidgetBuilder.svelte";
  import { Plus, Receipt, Wallet, TrendingDown, FolderOpen, LayoutDashboard, Pencil } from "@lucide/svelte";

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
  // F4 — saved dashboard widget list (defaults to the fixed layout server-side).
  let widgets = $state<Widget[]>([]);
  // Top-level view: the dashboard or the F3 resources browser.
  let tab = $state<"dashboard" | "resources">("dashboard");
  // F4 edit toggle for the widget builder.
  let editing = $state(false);

  async function load() {
    overview = null;
    error = null;
    try {
      // overview and the widget layout are independent — fetch concurrently.
      const [ov, w] = await Promise.all([
        ipc.queryApp("finance", "overview"),
        ipc.queryApp("finance", "widgets"),
      ]);
      overview = ov;
      widgets = (w?.widgets ?? []) as Widget[];
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

  function onWidgetsSaved() {
    editing = false;
    load();
  }

  onMount(load);

  function money(v: number | undefined): string {
    return (v ?? 0).toLocaleString(undefined, { maximumFractionDigits: 2 });
  }

  // Resolve a widget's `source` against the overview data into a renderable shape.
  const KPI_ICON: Record<string, typeof Wallet> = {
    balance: Wallet,
    month_total: TrendingDown,
    transaction_count: Receipt,
  };

  function kpiValue(source: string): string {
    if (!overview) return "0";
    if (source === "transaction_count") return `${overview.transaction_count ?? 0}`;
    return money((overview as any)[source] as number | undefined);
  }
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
          Add your first transaction, ask for a summary, or attach a statement to get started.
        </div>
      </div>
      <div class="flex flex-wrap justify-center gap-2">
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
        <button
          class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-2 text-sm font-medium hover:bg-muted"
          onclick={() => (tab = "resources")}
        >
          <FolderOpen class="size-4" /> Browse files
        </button>
      </div>
      {#if tab === "resources"}
        <div class="mt-2 w-full max-w-md text-left">
          <ResourcesPanel />
        </div>
      {/if}
    </div>
  {:else}
    <div class="space-y-4">
      <div class="flex items-center gap-1">
        <button
          class="inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium {tab === 'dashboard' ? 'bg-muted' : 'hover:bg-muted'}"
          onclick={() => (tab = "dashboard")}
        >
          <LayoutDashboard class="size-4" /> Dashboard
        </button>
        <button
          class="inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium {tab === 'resources' ? 'bg-muted' : 'hover:bg-muted'}"
          onclick={() => (tab = "resources")}
        >
          <FolderOpen class="size-4" /> Resources
        </button>
        {#if tab === "dashboard"}
          <button
            class="ml-auto inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium {editing ? 'bg-muted' : 'hover:bg-muted'}"
            onclick={() => (editing = !editing)}
          >
            <Pencil class="size-4" /> {editing ? "Done" : "Edit"}
          </button>
        {/if}
      </div>

      {#if tab === "resources"}
        <ResourcesPanel />
      {:else}
        {#if editing}
          <WidgetBuilder bind:widgets onSaved={onWidgetsSaved} />
        {/if}

        {@const kpis = widgets.filter((w) => w.kind === "kpi")}
        {#if kpis.length}
          <div class="grid grid-cols-2 gap-3">
            {#each kpis as w, i (i)}
              {@const Icon = KPI_ICON[w.source] ?? Wallet}
              <div class="rounded-lg border border-border p-3">
                <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <Icon class="size-3.5" /> {w.title}
                </div>
                <div class="mt-1 text-2xl font-semibold tabular-nums">{kpiValue(w.source)}</div>
              </div>
            {/each}
          </div>
        {/if}

        {#each widgets.filter((w) => w.kind !== "kpi") as w, i (i)}
          {#if w.kind === "chart" && w.source === "series" && overview.series && overview.series.labels.length}
            <div class="rounded-lg border border-border p-3">
              <Chart
                data={{
                  type: "bar",
                  title: w.title,
                  labels: overview.series.labels,
                  datasets: [{ label: "Spend", data: overview.series.data }],
                }}
              />
            </div>
          {:else if w.kind === "table" && w.source === "top_categories"}
            {@const cats = overview.top_categories ?? []}
            <div class="rounded-lg border border-border p-3">
              <div class="mb-2 text-sm font-medium">{w.title} ({overview.month})</div>
              {#if cats.length}
                <table class="w-full border-collapse text-sm">
                  <thead>
                    <tr class="border-b border-border text-left text-muted-foreground">
                      <th class="py-1 pr-3 font-medium">Category</th>
                      <th class="py-1 text-right font-medium">Total</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each cats as c (c.category)}
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
          {/if}
        {/each}
      {/if}
    </div>
  {/if}
</div>
