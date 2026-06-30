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

  type Category = { category: string; total: number };
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
            ? ""
            : "s"} need a category — review
        </span>
      </button>
    {/if}

    <!-- KPI cards -->
    <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
      {#each [{ label: "Spent", value: overview.spent, Icon: TrendingDown }, { label: "Income", value: overview.income, Icon: TrendingUp }, { label: "Net", value: overview.net, Icon: Scale }, { label: "Net worth", value: overview.net_worth, Icon: Landmark }] as k (k.label)}
        <div class="rounded-lg border border-border bg-card p-3">
          <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
            <k.Icon class="size-3.5" />
            {k.label}
          </div>
          <div class="mt-1 font-display text-2xl font-semibold tabular-nums">
            {money(k.value)}
          </div>
        </div>
      {/each}
    </div>

    <!-- Top categories -->
    {@const cats = overview.top_categories ?? []}
    {#if cats.length}
      {@const catMax = Math.max(1, ...cats.map((c) => c.total))}
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="mb-3 text-sm font-medium">Top categories</div>
        <div class="space-y-2.5">
          {#each cats as c (c.category)}
            <div class="space-y-1">
              <div class="flex items-center justify-between text-sm">
                <span class="capitalize">{c.category}</span>
                <span class="font-mono tabular-nums text-muted-foreground">{money(c.total)}</span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full rounded-full bg-primary/70"
                  style={`width: ${(c.total / catMax) * 100}%`}
                ></div>
              </div>
            </div>
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
