<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { send } from "$lib/stores/session.svelte";
  import { Button } from "$lib/components/ui/button";
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
  // Working copy while editing — committed only when the builder saves, so the
  // rendered dashboard never updates mid-edit. Cancel discards it.
  let draftWidgets = $state<Widget[]>([]);
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

  function startEditing() {
    // Deep-copy so edits don't touch the live layout until saved.
    draftWidgets = widgets.map((w) => ({ ...w }));
    editing = true;
  }

  function cancelEditing() {
    editing = false;
    draftWidgets = [];
  }

  function onWidgetsSaved() {
    editing = false;
    draftWidgets = [];
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

  // Active tab gets the amber primary indicator; the rest are quiet.
  function tabClass(active: boolean): string {
    return [
      "inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring",
      active
        ? "bg-background text-foreground shadow-sm ring-1 ring-primary"
        : "text-muted-foreground hover:text-foreground",
    ].join(" ");
  }
</script>

<div class="h-full">
  {#if error}
    <div class="text-sm text-destructive">Couldn't load your finances: {error}. Try reopening the app.</div>
  {:else if !overview}
    <!-- KPI skeleton so cards don't pop in once data arrives. -->
    <div class="space-y-4">
      <div class="grid grid-cols-2 gap-3">
        {#each Array(4) as _, i (i)}
          <div class="rounded-lg border border-border p-3">
            <div class="h-3 w-20 animate-pulse rounded bg-muted"></div>
            <div class="mt-2 h-7 w-24 animate-pulse rounded bg-muted"></div>
          </div>
        {/each}
      </div>
      <div class="h-40 animate-pulse rounded-lg border border-border bg-muted/40"></div>
    </div>
  {:else if overview.empty && needsOnboarding}
    <Onboarding onDone={onboardingDone} />
  {:else if overview.empty}
    <div class="flex h-full flex-col items-center justify-center gap-4 text-center">
      <div class="rounded-full bg-accent p-4">
        <Wallet class="size-8 text-accent-foreground" />
      </div>
      <div class="space-y-1">
        <div class="font-display text-lg font-semibold">No transactions yet</div>
        <div class="max-w-xs text-sm text-muted-foreground">
          Add your first transaction, ask for a summary, or attach a statement to get started.
        </div>
      </div>
      <div class="flex flex-wrap justify-center gap-2">
        <Button onclick={() => send("Add a transaction")}>
          <Plus /> Add a transaction
        </Button>
        <Button variant="outline" onclick={() => send("Show me this month's spending summary")}>
          <Receipt /> This month's summary
        </Button>
        <Button variant="outline" onclick={() => (tab = "resources")}>
          <FolderOpen /> Browse files
        </Button>
      </div>
      {#if tab === "resources"}
        <div class="mt-2 w-full max-w-md text-left">
          <ResourcesPanel />
        </div>
      {/if}
    </div>
  {:else}
    <div class="space-y-4">
      <div class="flex items-center gap-2">
        <div
          role="tablist"
          aria-label="Finance views"
          class="inline-flex items-center gap-1 rounded-lg border border-border bg-muted/40 p-0.5"
        >
          <button
            type="button"
            role="tab"
            aria-selected={tab === "dashboard"}
            class={tabClass(tab === "dashboard")}
            onclick={() => (tab = "dashboard")}
          >
            <LayoutDashboard class="size-4" /> Dashboard
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "resources"}
            class={tabClass(tab === "resources")}
            onclick={() => (tab = "resources")}
          >
            <FolderOpen class="size-4" /> Resources
          </button>
        </div>

        {#if tab === "dashboard" && !editing}
          <Button class="ml-auto" variant="outline" size="sm" onclick={startEditing}>
            <Pencil /> Edit
          </Button>
        {:else if tab === "dashboard" && editing}
          <Button class="ml-auto" variant="ghost" size="sm" onclick={cancelEditing}>Cancel</Button>
        {/if}
      </div>

      {#if tab === "resources"}
        <ResourcesPanel />
      {:else}
        {#if editing}
          <WidgetBuilder bind:widgets={draftWidgets} onSaved={onWidgetsSaved} />
        {/if}

        {@const kpis = widgets.filter((w) => w.kind === "kpi")}
        {#if kpis.length}
          <div class="grid grid-cols-2 gap-3">
            {#each kpis as w, i (i)}
              {@const Icon = KPI_ICON[w.source] ?? Wallet}
              <div class="rounded-lg border border-border bg-card p-3">
                <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <Icon class="size-3.5" /> {w.title}
                </div>
                <div class="mt-1 font-display text-2xl font-semibold tabular-nums">
                  {kpiValue(w.source)}
                </div>
              </div>
            {/each}
          </div>
        {/if}

        {#each widgets.filter((w) => w.kind !== "kpi") as w, i (i)}
          {#if w.kind === "chart" && w.source === "series" && overview.series && overview.series.labels.length}
            <div class="rounded-lg border border-border bg-card p-3">
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
            <div class="rounded-lg border border-border bg-card p-3">
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
                        <td class="py-1 text-right font-mono tabular-nums">{money(c.total)}</td>
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
