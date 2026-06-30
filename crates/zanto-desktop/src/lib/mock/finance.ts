// Stateful mock finance backend for dev:mock — lets the new 4-tab finance UI be
// walked end-to-end without the Rust backend. Mirrors the SHAPES the real
// query_app("finance", …) / run_app_action("finance", …) return, with enough
// in-memory state to exercise add/edit/categorize/budget/goal/import flows.
//
// This is a MOCK: numbers are computed here in TS for the prototype. The real
// app computes them in deterministic Rust. Shapes are what matters.

export type MockTxn = {
  id: number;
  account: string;
  category: string; // "" or "uncategorized" when not set
  amount: number; // positive
  type: "income" | "expense";
  merchant: string;
  date: string; // YYYY-MM-DD
  source: "manual" | "import";
};
export type MockAccount = { name: string; type: string; opening_balance: number };
export type MockBudget = { category: string; limit: number };
export type MockGoal = { name: string; kind: "savings" | "debt"; account: string; target: number; target_date?: string };

const CURRENCY = "USD";
const CATEGORIES = ["groceries", "dining", "transport", "utilities", "rent", "subscriptions", "shopping", "income"];

// Merchant→category memory (the "correct once, rule forever" loop, mocked).
const rules: { merchant_contains: string; category: string }[] = [
  { merchant_contains: "whole foods", category: "groceries" },
  { merchant_contains: "trader joe", category: "groceries" },
  { merchant_contains: "starbucks", category: "dining" },
  { merchant_contains: "uber", category: "transport" },
  { merchant_contains: "shell", category: "transport" },
  { merchant_contains: "netflix", category: "subscriptions" },
  { merchant_contains: "spotify", category: "subscriptions" },
  { merchant_contains: "pg&e", category: "utilities" },
  { merchant_contains: "rent", category: "rent" },
  { merchant_contains: "salary", category: "income" },
  { merchant_contains: "amzn", category: "shopping" },
];

function categorize(merchant: string, requested?: string): string {
  if (requested && CATEGORIES.includes(requested.toLowerCase())) return requested.toLowerCase();
  const m = merchant.toLowerCase();
  const hit = rules.find((r) => m.includes(r.merchant_contains));
  return hit ? hit.category : "uncategorized";
}

let nextId = 1;
const accounts: MockAccount[] = [{ name: "Checking", type: "checking", opening_balance: 1200 }];
const txns: MockTxn[] = [];
let budgets: MockBudget[] = [
  { category: "groceries", limit: 400 },
  { category: "dining", limit: 150 },
];
let goals: MockGoal[] = [
  { name: "Emergency fund", kind: "savings", account: "Checking", target: 5000, target_date: "2026-12-31" },
];

// Seed a realistic month so the dashboard is alive on first open.
function seed(rows: [string, string, number, "income" | "expense"][]) {
  for (const [date, merchant, amount, type] of rows) {
    txns.push({
      id: nextId++, account: "Checking", category: categorize(merchant), amount, type, merchant,
      date, source: "import",
    });
  }
}
seed([
  ["2026-06-01", "SALARY ACME CORP", 3200, "income"],
  ["2026-06-02", "WHOLE FOODS MARKET #123", 84.2, "expense"],
  ["2026-06-03", "STARBUCKS STORE 4471", 6.75, "expense"],
  ["2026-06-04", "UBER TRIP", 18.4, "expense"],
  ["2026-06-05", "NETFLIX.COM", 15.49, "expense"],
  ["2026-06-06", "SHELL OIL 5567", 52.1, "expense"],
  ["2026-06-07", "AMZN MKTP US", 39.99, "expense"],
  ["2026-06-08", "STARBUCKS STORE 4471", 5.95, "expense"],
  ["2026-06-10", "PG&E UTILITY", 110.0, "expense"],
  ["2026-06-12", "TRADER JOES #455", 61.3, "expense"],
  ["2026-06-15", "SPOTIFY USA", 11.99, "expense"],
  ["2026-06-18", "UBER TRIP", 22.1, "expense"],
  ["2026-06-20", "WHOLE FOODS MARKET #123", 77.8, "expense"],
  ["2026-06-25", "MYSTERY MERCHANT LLC", 43.0, "expense"], // stays uncategorized → review nudge
]);

const MONTH = "2026-06";
const signed = (t: MockTxn) => (t.type === "income" ? t.amount : -t.amount);
const inMonth = (t: MockTxn) => t.date.startsWith(MONTH);

function accountBalance(name: string): number {
  const acct = accounts.find((a) => a.name === name);
  const base = acct?.opening_balance ?? 0;
  return base + txns.filter((t) => t.account === name).reduce((s, t) => s + signed(t), 0);
}

function overview() {
  const monthTxns = txns.filter(inMonth);
  const income = monthTxns.filter((t) => t.type === "income").reduce((s, t) => s + t.amount, 0);
  const spent = monthTxns.filter((t) => t.type === "expense").reduce((s, t) => s + t.amount, 0);
  const byCat: Record<string, number> = {};
  for (const t of monthTxns.filter((t) => t.type === "expense"))
    byCat[t.category] = (byCat[t.category] ?? 0) + t.amount;
  const top_categories = Object.entries(byCat)
    .map(([category, total]) => ({ category, total }))
    .sort((a, b) => b.total - a.total);
  const uncategorized = monthTxns.filter((t) => t.type === "expense" && t.category === "uncategorized").length;

  // Safe-to-spend (W3): income − committed budgets remaining. Simplified for mock.
  const budgetRemaining = budgets.reduce((s, b) => s + Math.max(0, b.limit - (byCat[b.category] ?? 0)), 0);
  const committed = spent; // already spent this month
  const safe_to_spend = Math.max(0, income - committed - budgetRemaining);

  const budget_status = budgets.map((b) => ({
    category: b.category, limit: b.limit, spent: byCat[b.category] ?? 0,
  }));
  const goal_status = goals.map((g) => ({
    name: g.name, kind: g.kind, target: g.target, current: Math.max(0, accountBalance(g.account)),
    target_date: g.target_date,
  }));

  return {
    empty: txns.length === 0,
    currency: CURRENCY,
    month: MONTH,
    income, spent, net: income - spent,
    safe_to_spend,
    net_worth: accounts.reduce((s, a) => s + accountBalance(a.name), 0),
    top_categories,
    uncategorized_count: uncategorized,
    budget_status,
    goal_status,
    accounts: accounts.map((a) => ({ name: a.name, type: a.type, balance: accountBalance(a.name) })),
    // Insights (folded in): a 6-month spend series + detected subscriptions.
    series: { labels: ["Jan", "Feb", "Mar", "Apr", "May", "Jun"], data: [0, 0, 0, 0, 0, spent] },
    subscriptions: [
      { merchant: "Netflix", amount: 15.49, cadence: "monthly" },
      { merchant: "Spotify", amount: 11.99, cadence: "monthly" },
    ],
  };
}

// ── dispatch ────────────────────────────────────────────────────────────────
export async function financeQuery(query: string, _args: any): Promise<any> {
  switch (query) {
    case "overview": return overview();
    case "profile": return { setup: true, currency: CURRENCY, monthly_income: 3200, categories: CATEGORIES };
    case "categories": return CATEGORIES;
    case "list_transactions":
      return { rows: [...txns].sort((a, b) => b.date.localeCompare(a.date)) };
    case "accounts":
      return { accounts: accounts.map((a) => ({ ...a, balance: accountBalance(a.name) })) };
    case "budgets": return { budgets };
    case "goals": return { goals: overview().goal_status };
    default: return {};
  }
}

export async function financeAction(action: string, args: any): Promise<any> {
  switch (action) {
    case "add_transaction": {
      const t: MockTxn = {
        id: nextId++, account: args.account ?? "Checking",
        category: categorize(args.merchant ?? "", args.category),
        amount: Math.abs(args.amount ?? 0), type: args.type ?? "expense",
        merchant: args.merchant ?? "", date: args.date ?? "2026-06-28", source: "manual",
      };
      txns.push(t);
      return t;
    }
    case "update_transaction": {
      const t = txns.find((x) => x.id === args.id);
      if (!t) return { error: "not found" };
      if (args.category !== undefined) t.category = args.category;
      if (args.amount !== undefined) t.amount = Math.abs(args.amount);
      if (args.merchant !== undefined) t.merchant = args.merchant;
      if (args.account !== undefined) t.account = args.account;
      if (args.type !== undefined) t.type = args.type;
      return t;
    }
    case "categorize_transactions": {
      // bulk: { ids:[], category }
      for (const id of args.ids ?? []) {
        const t = txns.find((x) => x.id === id);
        if (t) t.category = args.category;
      }
      return { updated: (args.ids ?? []).length };
    }
    case "save_accounts": { accounts.length = 0; accounts.push(...(args.accounts ?? [])); return { accounts }; }
    case "save_budgets": { budgets = args.budgets ?? []; return { budgets }; }
    case "save_goals": { goals = args.goals ?? []; return { goals }; }
    default: return {};
  }
}
