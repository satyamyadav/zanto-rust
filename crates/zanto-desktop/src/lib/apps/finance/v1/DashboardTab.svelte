<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { send } from "$lib/stores/session.svelte";
  import { formatCurrency } from "../format";
  import AiEditButton from "../AiEditButton.svelte";
  import EditSheet from "../EditSheet.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import MessageCircle from "@lucide/svelte/icons/message-circle";
  import Plus from "@lucide/svelte/icons/plus";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import TrendingUp from "@lucide/svelte/icons/trending-up";
  import TrendingDown from "@lucide/svelte/icons/trending-down";
  import Scale from "@lucide/svelte/icons/scale";
  import Landmark from "@lucide/svelte/icons/landmark";
  import Target from "@lucide/svelte/icons/target";
  import Repeat from "@lucide/svelte/icons/repeat";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  type Category = { category: string; total: number; trend?: number[] };
  type BudgetStatus = { category: string; limit: number; spent: number };
  type GoalStatus = {
    name: string;
    kind: "savings" | "debt";
    target: number;
    current: number;
    target_date?: string;
  };
  type Subscription = { merchant: string; amount: number; cadence: string };
  type Overview = {
    currency?: string;
    month?: string;
    income?: number;
    spent?: number;
    net?: number;
    safe_to_spend?: number;
    net_worth?: number;
    top_categories?: Category[];
    trend_months?: string[];
    uncategorized_count?: number;
    budget_status?: BudgetStatus[];
    goal_status?: GoalStatus[];
    series?: { labels: string[]; data: number[] };
    subscriptions?: Subscription[];
  };

  let { onReviewUncategorized }: { onReviewUncategorized?: () => void } = $props();

  let overview = $state<Overview | null>(null);
  let error = $state<string | null>(null);

  const currency = $derived(overview?.currency);
  function money(v: number | undefined): string {
    return formatCurrency(v, currency);
  }

  // Sensitive figures (Net, Net worth) are masked by default — reveal on click.
  let revealed = $state(false);
  function money_masked(v: number | undefined): string {
    return revealed ? money(v) : "••••";
  }

  // Category drill-down: which category's 3–6 month detail is open (null = none).
  let drillCategory = $state<string | null>(null);
  const drillData = $derived(
    (overview?.top_categories ?? []).find((c) => c.category === drillCategory) ?? null,
  );

  async function load() {
    error = null;
    try {
      overview = await ipc.queryApp("finance", "overview");
    } catch (e) {
      error = `${e}`;
    }
  }
  onMount(load);

  // ── Budget editor (overlay) ────────────────────────────────────────────────
  let budgetOpen = $state(false);
  let budgetCategory = $state("");
  let budgetLimit = $state("");
  let savingBudget = $state(false);
  let categories = $state<string[]>([]);

  async function openBudget() {
    if (!categories.length) {
      try {
        categories = (await ipc.queryApp("finance", "categories")) ?? [];
      } catch {
        categories = [];
      }
    }
    budgetCategory = categories[0] ?? "";
    budgetLimit = "";
    budgetOpen = true;
  }

  const canSaveBudget = $derived(!!budgetCategory && Number(budgetLimit) > 0);

  async function saveBudget() {
    if (!canSaveBudget) return;
    savingBudget = true;
    try {
      // Append to the existing budgets (mock save_budgets replaces wholesale).
      const existing = (overview?.budget_status ?? []).map((b) => ({
        category: b.category,
        limit: b.limit,
      }));
      const merged = existing.filter((b) => b.category !== budgetCategory);
      merged.push({ category: budgetCategory, limit: Number(budgetLimit) });
      await ipc.runAppAction("finance", "save_budgets", { budgets: merged });
      budgetOpen = false;
      await load();
    } finally {
      savingBudget = false;
    }
  }

  // Sparkline helper: scale series to a 0..1 height.
  const seriesMax = $derived(Math.max(1, ...(overview?.series?.data ?? [0])));
</script>

<div class="space-y-5">
  {#if error}
    <div class="text-sm text-destructive">Couldn't load your dashboard: {error}.</div>
  {:else if !overview}
    <div class="space-y-4">
      <div class="h-24 animate-pulse rounded-lg border border-border bg-muted/40"></div>
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {#each Array(4) as _, i (i)}
          <div class="h-20 animate-pulse rounded-lg border border-border bg-muted/40"></div>
        {/each}
      </div>
    </div>
  {:else}
    <!-- Header: month + safe to spend -->
    <div class="rounded-xl border border-border bg-card p-4">
      <div class="flex items-start justify-between gap-3">
        <div>
          <div class="text-xs uppercase tracking-wide text-muted-foreground">{overview.month}</div>
          <div class="mt-1 text-sm text-muted-foreground">Safe to spend</div>
          <div class="font-display text-4xl font-semibold tabular-nums text-foreground">
            {money(overview.safe_to_spend)}
          </div>
        </div>
        <Button
          variant="outline"
          size="sm"
          onclick={() => send("How's my month going?")}
        >
          <MessageCircle class="size-4" /> How's my month?
        </Button>
      </div>
    </div>

    <!-- Uncategorized nudge -->
    {#if (overview.uncategorized_count ?? 0) > 0}
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded-md bg-accent px-3 py-2 text-left text-sm text-accent-foreground outline-none transition-colors hover:bg-accent/80 focus-visible:ring-2 focus-visible:ring-ring"
        onclick={() => onReviewUncategorized?.()}
      >
        <AlertCircle class="size-4 shrink-0" />
        <span class="min-w-0 flex-1">
          {overview.uncategorized_count} transaction{(overview.uncategorized_count ?? 0) === 1
            ? " needs"
            : "s need"} a category — review
        </span>
      </button>
    {/if}

    <!-- KPI cards: semantic colors (income green, spend rose, net by sign,
         net worth neutral). Net + Net worth are masked behind a reveal toggle. -->
    {@const netPositive = (overview.net ?? 0) >= 0}
    <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
      <!-- Spent (rose/red) -->
      <div class="rounded-lg border border-rose-200 bg-rose-50 p-3 dark:border-rose-900/50 dark:bg-rose-950/30">
        <div class="flex items-center gap-1.5 text-xs text-rose-700 dark:text-rose-300">
          <TrendingDown class="size-3.5" /> Spent
        </div>
        <div class="mt-1 font-display text-2xl font-semibold tabular-nums text-rose-700 dark:text-rose-300">
          {money(overview.spent)}
        </div>
      </div>
      <!-- Income (green) -->
      <div class="rounded-lg border border-emerald-200 bg-emerald-50 p-3 dark:border-emerald-900/50 dark:bg-emerald-950/30">
        <div class="flex items-center gap-1.5 text-xs text-emerald-700 dark:text-emerald-300">
          <TrendingUp class="size-3.5" /> Income
        </div>
        <div class="mt-1 font-display text-2xl font-semibold tabular-nums text-emerald-700 dark:text-emerald-300">
          {money(overview.income)}
        </div>
      </div>
      <!-- Net (green/red by sign; masked) -->
      <div
        class={[
          "rounded-lg border p-3",
          netPositive
            ? "border-emerald-200 bg-emerald-50 dark:border-emerald-900/50 dark:bg-emerald-950/30"
            : "border-rose-200 bg-rose-50 dark:border-rose-900/50 dark:bg-rose-950/30",
        ].join(" ")}
      >
        <div
          class={[
            "flex items-center gap-1.5 text-xs",
            netPositive ? "text-emerald-700 dark:text-emerald-300" : "text-rose-700 dark:text-rose-300",
          ].join(" ")}
        >
          <Scale class="size-3.5" /> Net
        </div>
        <div
          class={[
            "mt-1 font-display text-2xl font-semibold tabular-nums",
            netPositive ? "text-emerald-700 dark:text-emerald-300" : "text-rose-700 dark:text-rose-300",
          ].join(" ")}
        >
          {money_masked(overview.net)}
        </div>
      </div>
      <!-- Net worth (neutral; masked) -->
      <div class="rounded-lg border border-border bg-card p-3">
        <div class="flex items-center justify-between text-xs text-muted-foreground">
          <span class="flex items-center gap-1.5"><Landmark class="size-3.5" /> Net worth</span>
          <button
            type="button"
            onclick={() => (revealed = !revealed)}
            title={revealed ? "Hide Net & Net worth" : "Show Net & Net worth"}
            aria-label={revealed ? "Hide sensitive figures" : "Show sensitive figures"}
            class="rounded p-0.5 text-muted-foreground transition-colors hover:text-foreground"
          >
            {#if revealed}<EyeOff class="size-3.5" />{:else}<Eye class="size-3.5" />{/if}
          </button>
        </div>
        <div class="mt-1 font-display text-2xl font-semibold tabular-nums text-foreground">
          {money_masked(overview.net_worth)}
        </div>
      </div>
    </div>

    <!-- Top categories — each shows its 3–6 month trend as mini vertical bars
         (how it's trending, not just this month). Click a row to drill in. -->
    {@const cats = overview.top_categories ?? []}
    {@const months = overview.trend_months ?? []}
    {#if cats.length}
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="mb-3 text-sm font-medium">Categories — 6-month trend</div>
        <div class="divide-y divide-border">
          {#each cats as c (c.category)}
            {@const tmax = Math.max(1, ...(c.trend ?? [c.total]))}
            <button
              type="button"
              onclick={() => (drillCategory = c.category)}
              class="flex w-full items-center gap-3 py-2.5 text-left outline-none transition-colors hover:bg-muted/40 focus-visible:bg-muted/40"
            >
              <span class="w-24 shrink-0 truncate text-sm capitalize">{c.category}</span>
              <!-- mini vertical bars -->
              <span class="flex h-8 flex-1 items-end gap-0.5">
                {#each c.trend ?? [c.total] as v, i (i)}
                  <span
                    class={[
                      "min-h-[2px] w-full rounded-sm",
                      i === (c.trend?.length ?? 1) - 1 ? "bg-primary" : "bg-primary/35",
                    ].join(" ")}
                    style={`height: ${(v / tmax) * 100}%`}
                    title={months[i] ? `${months[i]}: ${money(v)}` : money(v)}
                  ></span>
                {/each}
              </span>
              <span class="w-24 shrink-0 text-right font-mono text-sm tabular-nums text-muted-foreground">
                {money(c.total)}
              </span>
              <ChevronRight class="size-4 shrink-0 text-muted-foreground" />
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Budgets -->
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="mb-3 flex items-center justify-between">
        <div class="text-sm font-medium">Budgets</div>
        <Button variant="outline" size="sm" onclick={openBudget}>
          <Plus class="size-3.5" /> Budget
        </Button>
      </div>
      {#if (overview.budget_status ?? []).length}
        <div class="space-y-3">
          {#each overview.budget_status ?? [] as b (b.category)}
            {@const pct = b.limit > 0 ? b.spent / b.limit : 0}
            {@const over = b.spent > b.limit}
            <div class="space-y-1">
              <div class="flex items-center justify-between text-sm">
                <span class="capitalize">{b.category}</span>
                <div class="flex items-center gap-2">
                  <span
                    class={[
                      "font-mono tabular-nums",
                      over ? "text-destructive" : pct >= 0.8 ? "text-warning" : "text-muted-foreground",
                    ].join(" ")}
                  >
                    {money(b.spent)} / {money(b.limit)}
                  </span>
                  <AiEditButton prompt={`Adjust my ${b.category} budget`} />
                </div>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full rounded-full bg-success"
                  style={`width: ${Math.min(1, pct) * 100}%`}
                ></div>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <div class="text-sm text-muted-foreground">No budgets yet. Add one to track a category.</div>
      {/if}
    </div>

    <!-- Goals -->
    {#if (overview.goal_status ?? []).length}
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="mb-3 flex items-center gap-1.5 text-sm font-medium">
          <Target class="size-4 text-muted-foreground" /> Goals
        </div>
        <div class="space-y-3">
          {#each overview.goal_status ?? [] as g (g.name)}
            {@const pct = g.target > 0 ? g.current / g.target : 0}
            <div class="space-y-1">
              <div class="flex items-center justify-between text-sm">
                <span>{g.name}</span>
                <span class="font-mono tabular-nums text-muted-foreground">
                  {money(g.current)} / {money(g.target)}
                </span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full rounded-full bg-primary"
                  style={`width: ${Math.min(1, pct) * 100}%`}
                ></div>
              </div>
              {#if g.target_date}
                <div class="text-xs text-muted-foreground">Target by {g.target_date}</div>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Insights: 6-month sparkline + subscriptions -->
    <div class="grid gap-3 sm:grid-cols-2">
      {#if overview.series && overview.series.labels.length}
        <div class="rounded-lg border border-border bg-card p-4">
          <div class="mb-3 text-sm font-medium">Spend, last 6 months</div>
          <div class="flex h-24 items-end gap-2">
            {#each overview.series.data as d, i (i)}
              <div class="flex flex-1 flex-col items-center gap-1">
                <div class="flex w-full flex-1 items-end">
                  <div
                    class="w-full rounded-t bg-primary/60"
                    style={`height: ${(d / seriesMax) * 100}%`}
                  ></div>
                </div>
                <div class="text-[10px] text-muted-foreground">{overview.series.labels[i]}</div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if (overview.subscriptions ?? []).length}
        <div class="rounded-lg border border-border bg-card p-4">
          <div class="mb-3 flex items-center gap-1.5 text-sm font-medium">
            <Repeat class="size-4 text-muted-foreground" /> Subscriptions
          </div>
          <ul class="space-y-2 text-sm">
            {#each overview.subscriptions ?? [] as s (s.merchant)}
              <li class="flex items-center justify-between">
                <span>{s.merchant}</span>
                <span class="text-muted-foreground">
                  <span class="font-mono tabular-nums">{money(s.amount)}</span>
                  <span class="text-xs">/ {s.cadence}</span>
                </span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
    </div>
  {/if}
</div>

<!-- Category drill-down (overlay): the selected category's 6-month trend + an
     "ask AI" to see its transactions. Open when drillCategory is set. -->
<EditSheet
  open={drillCategory !== null}
  title={drillCategory ? `${drillCategory} — 6-month trend` : "Category"}
  footer={false}
  onClose={() => (drillCategory = null)}
>
  {#if drillData}
    {@const months = overview?.trend_months ?? []}
    {@const tmax = Math.max(1, ...(drillData.trend ?? [drillData.total]))}
    <div class="space-y-4">
      <div>
        <div class="text-xs text-muted-foreground">This month</div>
        <div class="font-display text-3xl font-semibold tabular-nums">{money(drillData.total)}</div>
      </div>
      <div class="flex h-44 items-end gap-1.5">
        {#each drillData.trend ?? [drillData.total] as v, i (i)}
          {@const isCurrent = i === (drillData.trend?.length ?? 1) - 1}
          <div class="flex min-w-0 flex-1 flex-col items-center gap-1.5">
            <!-- value above the bar, compact (no currency code to avoid overflow) -->
            <div class="text-[10px] tabular-nums text-muted-foreground">
              {v.toLocaleString(undefined, { maximumFractionDigits: 0 })}
            </div>
            <div class="flex w-full flex-1 items-end">
              <div
                class={["w-full rounded-t", isCurrent ? "bg-primary" : "bg-primary/40"].join(" ")}
                style={`height: ${Math.max(4, (v / tmax) * 100)}%`}
                title={`${months[i] ?? ""}: ${money(v)}`}
              ></div>
            </div>
            <div class={["text-[10px]", isCurrent ? "font-medium text-foreground" : "text-muted-foreground"].join(" ")}>
              {months[i] ?? ""}
            </div>
          </div>
        {/each}
      </div>
      <Button
        variant="outline"
        size="sm"
        class="w-full"
        onclick={() => { send(`Show my ${drillData?.category} transactions and how it's trending`); drillCategory = null; }}
      >
        <MessageCircle class="size-4" /> Ask about {drillData.category}
      </Button>
    </div>
  {/if}
</EditSheet>

<!-- Budget overlay editor -->
<EditSheet
  bind:open={budgetOpen}
  title="Add budget"
  onSave={saveBudget}
  canSave={canSaveBudget}
  saving={savingBudget}
>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div class="text-xs text-muted-foreground">Set a monthly limit for a category.</div>
      <AiEditButton prompt="Add a budget" size="md" />
    </div>
    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Category</span>
      <select
        class="h-9 w-full rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
        bind:value={budgetCategory}
      >
        {#each categories as c (c)}
          <option value={c}>{c}</option>
        {/each}
      </select>
    </label>
    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Monthly limit</span>
      <Input type="number" min="0" step="1" bind:value={budgetLimit} placeholder="400" />
    </label>
  </div>
</EditSheet>
