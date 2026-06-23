<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { send } from "$lib/stores/session.svelte";
  import { Button } from "$lib/components/ui/button";
  import Chart from "$lib/blocks/Chart.svelte";
  import Onboarding from "./Onboarding.svelte";
  import ResourcesPanel from "./ResourcesPanel.svelte";
  import TransactionsView from "./TransactionsView.svelte";
  import Import from "./Import.svelte";
  import CategoryRules from "./CategoryRules.svelte";
  import Budgets from "./Budgets.svelte";
  import Accounts from "./Accounts.svelte";
  import AccountsEditor from "./AccountsEditor.svelte";
  import BudgetBars from "./BudgetBars.svelte";
  import Subscriptions from "./Subscriptions.svelte";
  import Trends from "./Trends.svelte";
  import Goals from "./Goals.svelte";
  import GoalsEditor from "./GoalsEditor.svelte";
  import Forecast from "./Forecast.svelte";
  import WidgetBuilder, { type Widget } from "./WidgetBuilder.svelte";
  import { formatCurrency } from "./format";
  import {
    Plus,
    Receipt,
    Wallet,
    TrendingDown,
    TrendingUp,
    Scale,
    FolderOpen,
    LayoutDashboard,
    ListChecks,
    AlertCircle,
    AlertTriangle,
    Pencil,
    Repeat,
    LineChart,
    Upload,
    Landmark,
    Target,
  } from "@lucide/svelte";

  type Category = { category: string; total: number };
  type BudgetStatus = {
    category: string;
    limit: number;
    spent: number;
    pct: number;
    over: boolean;
  };
  type OverBudget = { category: string; limit: number; spent: number; by: number };
  type GoalStatus = {
    name: string;
    kind: "savings" | "debt";
    account: string;
    target: number;
    current?: number;
    owed?: number;
    progress: number;
    remaining?: number;
    complete: boolean;
  };
  type PaceWarning = { category: string; limit: number; spent: number; projected: number };
  type Overview = {
    empty: boolean;
    balance?: number;
    month?: string;
    month_total?: number;
    income?: number;
    net_cash_flow?: number;
    uncategorized_count?: number;
    transaction_count?: number;
    top_categories?: Category[];
    series?: { labels: string[]; data: number[] };
    budget_status?: BudgetStatus[];
    over_budget?: OverBudget[];
    mom_delta?: number;
    mom_pct?: number;
    accounts?: { name: string; type: string; balance: number }[];
    net_worth?: number;
    goal_status?: GoalStatus[];
    projected_net_worth?: number;
    pace_warnings?: PaceWarning[];
  };
  type Profile = {
    setup: boolean;
    currency?: string;
    monthly_income?: number | null;
    categories?: string[];
  };

  let overview = $state<Overview | null>(null);
  // Separate from `overview` so a refetch (after an edit/save) keeps the current
  // dashboard on screen instead of tearing down to the skeleton mid-edit (B4-1).
  let loading = $state(false);
  let error = $state<string | null>(null);
  // When there is no data, first-run onboarding takes over the empty state until
  // a profile exists or the user skips it for this mount.
  let needsOnboarding = $state(false);
  // F4 — saved dashboard widget list (defaults to the fixed layout server-side).
  let widgets = $state<Widget[]>([]);
  // Working copy while editing — committed only when the builder saves, so the
  // rendered dashboard never updates mid-edit. Cancel discards it.
  let draftWidgets = $state<Widget[]>([]);
  // Top-level view: the dashboard, the editable transactions surface, or the F3
  // resources browser.
  let tab = $state<
    | "dashboard"
    | "transactions"
    | "accounts"
    | "import"
    | "subscriptions"
    | "trends"
    | "goals"
    | "resources"
  >("dashboard");
  // F4 edit toggle for the widget builder.
  let editing = $state(false);
  // Currency ISO code from the profile, for currency-aware formatting.
  let currency = $state<string | undefined>(undefined);
  // Categories from the profile, passed to the transactions/rules editors.
  let profileCategories = $state<string[]>([]);
  // Filter the TransactionsView opens with (set when reviewing uncategorized).
  let txFilter = $state<"all" | "uncategorized">("all");

  async function load() {
    // Keep the existing `overview` on screen while refetching; swap on success.
    loading = true;
    error = null;
    try {
      // overview and the widget layout are independent — fetch concurrently.
      const [ov, w] = await Promise.all([
        ipc.queryApp("finance", "overview"),
        ipc.queryApp("finance", "widgets"),
      ]);
      overview = ov;
      widgets = (w?.widgets ?? []) as Widget[];
      // Always fetch the profile so currency + categories are available.
      const profile: Profile = await ipc.queryApp("finance", "profile");
      currency = profile?.currency;
      profileCategories = profile?.categories ?? [];
      if (overview?.empty) {
        needsOnboarding = !profile?.setup;
      } else {
        needsOnboarding = false;
      }
    } catch (e) {
      error = `${e}`;
    } finally {
      loading = false;
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

  // Over-budget categories drive the in-app banner and native nudges.
  const overBudget = $derived<OverBudget[]>(overview?.over_budget ?? []);

  // Summarise the banner text: up to 2 categories, then "+N more".
  const overBudgetText = $derived.by(() => {
    const list = overBudget;
    if (!list.length) return "";
    const shown = list.slice(0, 2).map((o) => `${o.category} by ${money(o.by)}`);
    const rest = list.length - shown.length;
    let text = `Over budget in ${shown.join(", ")}`;
    if (rest > 0) text += ` +${rest} more`;
    return text;
  });

  // Pace warnings (budget categories on track to exceed) drive the amber chip.
  const paceWarnings = $derived<PaceWarning[]>(overview?.pace_warnings ?? []);

  // ONE coalesced budget nudge per month covering both over-budget and pace
  // warnings, rather than a separate native notification per category from two
  // effects (B4-4). The dedup key is persisted BEFORE notifying so a re-run or a
  // notify failure can't double-nudge.
  $effect(() => {
    if (!overview) return;
    const month = overview.month;
    const over = overview.over_budget ?? [];
    const pace = overview.pace_warnings ?? [];
    if (!month || (!over.length && !pace.length)) return;
    const key = `zanto.finance.nudge.${month}`;
    try {
      if (localStorage.getItem(key)) return;
      localStorage.setItem(key, "1"); // persist first — never re-nudge on failure
      const parts: string[] = [];
      if (over.length) parts.push(`over budget in ${over.map((o) => o.category).join(", ")}`);
      if (pace.length) parts.push(`on track to exceed ${pace.map((p) => p.category).join(", ")}`);
      ipc.notify("Budget check", `You're ${parts.join("; ")}.`);
    } catch {
      /* localStorage / notify unavailable — skip silently */
    }
  });

  const paceText = $derived.by(() => {
    const list = paceWarnings;
    if (!list.length) return "";
    const shown = list.slice(0, 2).map((p) => `${p.category} is on track to exceed its budget`);
    const rest = list.length - shown.length;
    let text = shown.join(", ");
    if (rest > 0) text += ` +${rest} more`;
    return text;
  });

  function money(v: number | undefined): string {
    return formatCurrency(v, currency);
  }

  // Resolve a widget's `source` against the overview data into a renderable shape.
  const KPI_ICON: Record<string, typeof Wallet> = {
    balance: Wallet,
    net_worth: Landmark,
    projected_net_worth: TrendingUp,
    month_total: TrendingDown,
    income: TrendingUp,
    net_cash_flow: Scale,
    transaction_count: Receipt,
  };

  // Only these overview fields are real KPI sources. An unknown source renders
  // an em dash instead of a confident, wrong "$0.00" (B4-5).
  const KPI_SOURCES = new Set([
    "balance",
    "net_worth",
    "projected_net_worth",
    "month_total",
    "income",
    "net_cash_flow",
    "transaction_count",
  ]);

  function kpiValue(source: string): string {
    if (!overview || !KPI_SOURCES.has(source)) return "—";
    if (source === "transaction_count") return `${overview.transaction_count ?? 0}`;
    return money((overview as any)[source] as number | undefined);
  }

  // Active tab gets the amber primary indicator; the rest are quiet.
  function tabClass(active: boolean): string {
    return [
      "inline-flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring",
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
          class="inline-flex items-center gap-1 overflow-x-auto rounded-lg border border-border bg-muted/40 p-0.5"
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
            aria-selected={tab === "transactions"}
            class={tabClass(tab === "transactions")}
            onclick={() => (tab = "transactions")}
          >
            <ListChecks class="size-4" /> Transactions
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "accounts"}
            class={tabClass(tab === "accounts")}
            onclick={() => (tab = "accounts")}
          >
            <Landmark class="size-4" /> Accounts
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "import"}
            class={tabClass(tab === "import")}
            onclick={() => (tab = "import")}
          >
            <Upload class="size-4" /> Import
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "subscriptions"}
            class={tabClass(tab === "subscriptions")}
            onclick={() => (tab = "subscriptions")}
          >
            <Repeat class="size-4" /> Subscriptions
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "trends"}
            class={tabClass(tab === "trends")}
            onclick={() => (tab = "trends")}
          >
            <LineChart class="size-4" /> Trends
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "goals"}
            class={tabClass(tab === "goals")}
            onclick={() => (tab = "goals")}
          >
            <Target class="size-4" /> Goals
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
      {:else if tab === "transactions"}
        {#key txFilter}
          <TransactionsView
            {currency}
            categories={profileCategories}
            accounts={(overview.accounts ?? []).map((a) => a.name)}
            initialFilter={txFilter}
          />
        {/key}
      {:else if tab === "accounts"}
        <Accounts
          accounts={overview.accounts}
          netWorth={overview.net_worth}
          {currency}
          onChanged={load}
        />
      {:else if tab === "import"}
        <Import onImported={load} />
      {:else if tab === "subscriptions"}
        <Subscriptions {currency} />
      {:else if tab === "trends"}
        <Trends {currency} />
      {:else if tab === "goals"}
        <Goals goalStatus={overview.goal_status} {currency} />
      {:else}
        {#if overBudget.length}
          <div
            class="flex items-center gap-2 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive"
          >
            <AlertTriangle class="size-4 shrink-0" />
            <span class="min-w-0 flex-1">{overBudgetText}</span>
          </div>
        {/if}

        {#if paceWarnings.length}
          <div
            class="flex items-center gap-2 rounded-md bg-warning/10 px-3 py-2 text-sm text-warning"
          >
            <AlertTriangle class="size-4 shrink-0" />
            <span class="min-w-0 flex-1">{paceText}</span>
          </div>
        {/if}

        {#if (overview.uncategorized_count ?? 0) > 0}
          <button
            type="button"
            class="flex w-full items-center gap-2 rounded-md bg-accent px-3 py-2 text-left text-sm text-accent-foreground outline-none transition-colors hover:bg-accent/80 focus-visible:ring-2 focus-visible:ring-ring"
            onclick={() => {
              txFilter = "uncategorized";
              tab = "transactions";
            }}
          >
            <AlertCircle class="size-4 shrink-0" />
            <span class="min-w-0 flex-1">
              {overview.uncategorized_count} uncategorized transaction{(overview.uncategorized_count ?? 0) === 1 ? "" : "s"} — review
            </span>
          </button>
        {/if}

        {#if editing}
          <WidgetBuilder bind:widgets={draftWidgets} onSaved={onWidgetsSaved} />
          <AccountsEditor onSaved={load} />
          <GoalsEditor accounts={(overview.accounts ?? []).map((a) => a.name)} onSaved={load} />
          <Budgets categories={profileCategories} onSaved={load} />
          <CategoryRules categories={profileCategories} />
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

        {#if overview.mom_delta !== undefined && (overview.series?.labels.length ?? 0) > 1}
          {@const up = overview.mom_delta >= 0}
          <div
            class={[
              "flex items-center gap-1.5 text-sm",
              up ? "text-destructive" : "text-success",
            ].join(" ")}
          >
            {#if up}
              <TrendingUp class="size-3.5 shrink-0" />
            {:else}
              <TrendingDown class="size-3.5 shrink-0" />
            {/if}
            <span>
              {up ? "↑" : "↓"} {money(Math.abs(overview.mom_delta))} ({Math.round((overview.mom_pct ?? 0) * 100)}%) vs last month
            </span>
          </div>
        {/if}

        {#if (overview.budget_status ?? []).length}
          <div class="rounded-lg border border-border bg-card p-3">
            <div class="mb-3 text-sm font-medium">Budget vs actual ({overview.month})</div>
            <BudgetBars status={overview.budget_status ?? []} {currency} />
          </div>
        {/if}

        <Forecast {currency} />

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
          {:else if w.kind === "budget"}
            <div class="rounded-lg border border-border bg-card p-3">
              <div class="mb-3 text-sm font-medium">{w.title}</div>
              <BudgetBars status={overview.budget_status ?? []} {currency} />
            </div>
          {:else if w.kind === "subscriptions"}
            <div class="rounded-lg border border-border bg-card p-3">
              <Subscriptions {currency} />
            </div>
          {:else if w.kind === "trends"}
            <div class="rounded-lg border border-border bg-card p-3">
              <Trends {currency} />
            </div>
          {:else if w.kind === "goals"}
            <div class="rounded-lg border border-border bg-card p-3">
              <div class="mb-3 text-sm font-medium">{w.title}</div>
              <Goals goalStatus={overview.goal_status} {currency} />
            </div>
          {:else if w.kind === "forecast"}
            <Forecast {currency} />
          {:else if w.kind === "accounts"}
            {@const accts = overview.accounts ?? []}
            <div class="rounded-lg border border-border bg-card p-3">
              <div class="mb-2 flex items-center justify-between text-sm font-medium">
                <span>{w.title}</span>
                <span class="tabular-nums">{money(overview.net_worth)}</span>
              </div>
              {#if accts.length}
                <ul class="space-y-1 text-sm">
                  {#each accts as a (a.name)}
                    <li class="flex items-center justify-between">
                      <span class="min-w-0 flex-1 break-words text-muted-foreground">{a.name}</span>
                      <span
                        class={[
                          "font-mono tabular-nums",
                          a.balance < 0 ? "text-destructive" : "",
                        ].join(" ")}
                      >
                        {money(a.balance)}
                      </span>
                    </li>
                  {/each}
                </ul>
              {:else}
                <div class="text-sm text-muted-foreground">No accounts yet.</div>
              {/if}
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  {/if}
</div>
