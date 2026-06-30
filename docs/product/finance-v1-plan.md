# Finance app — v1 product review, goal, and plan

- **Date:** 2026-06-30
- **Author:** PM review (audit + market research)
- **Status:** Proposal for review

This is a product review of the existing Finance micro-app, a gap analysis against
how personal finance is actually solved well, a single goal for the next version,
and a scoped plan to deliver it. Grounded in a first-hand code audit and cited
market research (sources at the end).

---

## 1. What we have today (honest summary)

The Finance app is **already feature-rich** — well beyond a basic expense tracker.
First-hand audit confirms all of this is built and working:

- **Data:** transactions (income/expense/transfer), accounts + balances, budgets,
  savings/debt goals, category rules, a user profile, and a customizable widget
  dashboard — all over a local SQLite JSON store.
- **Numbers (all deterministic Rust, never the LLM — correct call):** lifetime
  balance, net worth, monthly income/spend/net, 6-month spend series, MoM deltas,
  top categories, budget-vs-actual with run-rate pace warnings, **recurring/
  subscription detection**, **rest-of-month forecast**, goal progress, an
  uncategorized review queue.
- **Import:** CSV/TSV/XLSX/ODS with heuristic column mapping, robust money parsing,
  and deterministic account-scoped dedup (re-import is a no-op).
- **Surface:** an 8-tab dashboard (Dashboard/Transactions/Accounts/Import/
  Subscriptions/Trends/Goals/Resources), a first-run onboarding, and a chat agent
  with 6 transaction tools.

**The uncomfortable truth:** we have built *more breadth* than the products that
win (Copilot, Monarch, Lunch Money, Actual), but we're **missing the two things
that actually drive retention.** We over-invested in features and under-invested
in the friction-killers. See §3.

---

## 2. What the market says actually works (research)

The research is unusually consistent. Five findings that should drive every
decision:

1. **Retention is the whole game, and it's lost *after* setup, not during.**
   ~90% of users successfully connect an account; only ~20% are still active at 3
   months (Aite-Novarica). The drop-off is **ongoing-effort fatigue + weak
   actionable value**, not onboarding. [Inquirer/WSJ]

2. **The #1 retention killer is manual categorization / correction burden.**
   Fixing miscategorized transactions and entering missed items is what kills apps
   once novelty fades (~month 2–3). "Too complicated" (37%) and "forgot to track"
   (28%) are the top quit reasons. [econbrew, PMC scoping review]

3. **Tracking ≠ control — the value gap.** "Knowing you spent 22% on groceries
   doesn't help you decide whether you can afford a vacation." Pie charts give a
   *false* sense of control. Users abandon when the effort stops producing a
   decision they can act on. [Neculai]

4. **Budgeting guilt drives avoidance.** Strict, penalty-shaped budgets backfire:
   "overspending $50 feels worse than saving $100 feels good"; red "you failed"
   cues make people stop opening the app. A real Mint user: *"It didn't give me
   grace."* [econbrew, WSJ]

5. **AI helps at classification/language, never at arithmetic.** Auto-
   categorization (Copilot: ~90% after ~30 txns), subscription detection, NL
   queries over your own data, and import/column mapping are the genuine wins.
   But **LLMs hallucinate numbers that sound authoritative** — every figure must
   be computed deterministically; the LLM only classifies, narrates, and routes.
   [DEV.to/Valyu, moveo.ai, Monarch] — we already do this; keep it sacred.

**Our structural advantage:** we are local-first with no Plaid. The privacy-first
winners (Actual, Lunch Money) prove the low-friction local path is **CSV/OFX
file-drop + remembered per-bank mappings + visible dedup + rule/AI auto-categorize**.
We have the import spine; we're missing the memory and the AI categorize step.

---

## 3. Gap analysis (audit × research)

Ranked by impact on the things research says matter (retention, friction, value).

### Tier 1 — the retention-critical gaps

**G0. The app is overwhelming — a control panel, not a finance assistant.** A
normal user opens "Personal Finance" and meets **8 top-level tabs** (Dashboard,
Transactions, Accounts, Import, Subscriptions, Trends, Goals, Resources), an **Edit
mode** that reveals **5 more editor panels** (WidgetBuilder, AccountsEditor,
GoalsEditor, Budgets, CategoryRules), and **~6 separate forms** — across **18
components / ~2,800 lines** of UI. There is no single clear answer to "where did my
money go / am I okay this month?"; there's a configuration surface. This is the
*first* thing a user hits, and it reads as work, not help. Research is blunt that
**"too many features / decision fatigue"** is a top abandonment cause, and that the
aha must arrive fast and obvious. We buried it. (`Dashboard.svelte` tab strip +
`startEditing`; the 18 `apps/finance/*.svelte` files.)

**G1. The AI can't *do* the app.** The chat agent has tools only for
add/query/update/delete transaction + transfer. Budgets, accounts, goals, rules,
import, recurring/trends/forecast are **UI-only** — unreachable from chat. "Set a
budget" is a start-action chip the agent literally cannot fulfill. For an
*AI-native* product this is the defining seam: the agent can talk about your money
but can't change it. (`apps/finance/mod.rs` — 6 `agent_tools` vs the `query`/
`action` IPC surface.)

**G2. Categorization is rigid and never learns.** Closed taxonomy (the 7-item
profile list); no statistical/LLM auto-categorization; the only memory is
hand-written merchant-substring rules (create/delete only, first-match wins). A
freshly imported statement dumps most rows into "uncategorized," cleaned one row at
a time (no bulk recategorize; date/merchant not even inline-editable). This is
*exactly* the #1 documented abandonment cause, and it's our weakest area.
(`resolve_category_pure`, `TransactionsView.svelte`.)

**G3. No single "can I afford it?" answer (the value gap).** We surface lots of
numbers but never the one a normal user wants: **safe-to-spend** / how this month
is really tracking in plain language. We render data, not decisions.

### Tier 2 — credibility & correctness gaps

**G4. Import warnings are computed but hidden.** The backend reports
`truncated`/`malformed`/`total_rows`; `Import.svelte` ignores them. A statement
capped at 5,000 rows or with malformed rows imports **silently partial** — a trust
bug (the user thinks everything imported). (`Import.svelte` ParseResult/
ImportResult types.)

**G5. `monthly_income` is dead.** Collected at onboarding, stored, read *nowhere*.
The user enters it and nothing happens — a broken promise on the first screen.

**G6. Currency is inconsistent.** Dashboard threads `currency` everywhere, but the
`monthly_summary` and `transactions_table` chat artifacts hardcode `toLocaleString`
— the agent's rendered numbers ignore the user's currency.

### Tier 3 — depth gaps (real, but not what's killing us)

- **G7.** No per-bank **import mapping memory** (re-import re-maps every time) — the
  single highest-leverage low-friction feature per Lunch Money.
- **G8.** Recurring detection is monthly-only (misses weekly/annual, trials, price
  changes) and display-only (no upcoming-charge/cancel surfacing).
- **G9.** Goals ignore `target_date` (no "on pace?" / required monthly
  contribution); forecast is naive run-rate that ignores the recurring data we
  already detect.
- **G10.** Full-table-scan aggregation on every dashboard load — fine now, O(n) JSON
  parse that will bite with years of imported data.
- **G11.** No multi-currency, no reconciliation/cleared balances, no splits/tags/
  attachments, no transaction search, no export.

**The pattern:** Tier 3 is where we *added* features (forecast, goals, trends,
widgets). Tiers 1–2 — where retention is actually won or lost — is where we're
thin. **We should stop adding breadth and fix the friction-killers.**

---

## 4. The goal (v1)

> **A clear home dashboard that answers "where did my money go, and what's safe to
> spend?", backed by a few crisp, directly-editable tabs — with the AI as a
> parallel power-path wired into every view, not a replacement for it.**

The principle: **things are visible and directly editable** (not hidden behind
chat), the **flow is obvious**, and chat is an *extra* way to do the same things,
triggered from the UI. We fix the current tabs not by deleting them but by making
each one **clear and single-purpose**, and by removing the config-panel feel (the
Edit/WidgetBuilder mode, scattered always-on forms).

**The shape — 4 clear tabs:**

1. **Dashboard (home)** — the "am I okay?" answer: this month's spend,
   **safe-to-spend**, top categories, budget + goal progress, and an Insights
   section (trends + detected subscriptions). Read-clear by default.
2. **Transactions** — the full list, **directly editable** (category, amount,
   account, date), searchable/filterable, with bulk recategorize.
3. **Accounts** — accounts, balances, net worth; directly editable; transfers.
4. **Import** — drop a statement → map → preview → dedup → **auto-categorized**.

Budgets+Goals and Trends+Subscriptions **fold into the Dashboard as clear
sections** (not their own tabs), so 8 unclear tabs become 4 purposeful ones.

**The editing model (visible, direct, uncluttered):**
- Default state of every view is **clean and readable**. Views with multiple edit
  points get a single **"Edit" toggle at the top** that flips the whole view into
  editable state — so edit affordances don't clutter the read view.
- Editing happens in an **overlay side-panel / sheet**, not by cramming forms
  inline. Open panel → edit → save → back to the clean view.
- **Every edit point offers both paths:** the manual control, plus a small **"edit
  with AI" icon button** next to it that sends a prefilled, context-aware prompt to
  the chat panel (e.g. on a transaction: "recategorize this and similar"; on
  Accounts: "add an account"; on the Dashboard: "how's my month?"). Manual and AI
  are siblings, everywhere.

Three measurable outcomes:

- **One clear answer on open.** The Dashboard shows where money went + safe-to-spend
  + progress, legible at a glance — no config mode, no widget builder.
- **Time-to-value < 2 min.** Drop a statement → imported, **auto-categorized**,
  visible — **< 10% of rows uncategorized** on a typical statement.
- **Two paths to every action.** Each editable thing (transaction, account, budget,
  goal, category) is editable **manually in its view AND via an "edit with AI"
  trigger** into chat — every number stays deterministically computed.

This is *not* "add features." It's **make what we have clear, visible, and dual-path
(manual + AI)**, and cut the config cruft. Directly attacks G0, G1, G2, G3.

**Anti-goals (explicitly out):** no Plaid/bank-sync, no cloud/multi-device, no
investment tracking, no bill-negotiation, no zero-based-budgeting methodology
(YNAB's own churn proves it's too much), **no widget builder / user-built
dashboards**, no more than the 4 tabs. Keep it simple yet powerful.

---

## 5. The plan — v1 scope

Five workstreams, ordered by leverage. W0 sets the shape; the rest fill it.

### W0 — 4 clear tabs + the edit model (attacks G0, the first thing users hit)

The defining change. Restructure the dashboard from **8 unclear tabs + Edit mode**
to **4 clear, directly-editable tabs** (Dashboard / Transactions / Accounts /
Import), with budgets+goals and trends+subscriptions folded into the Dashboard as
sections.

- **Dashboard (home)** lays out, clearly: this-month spend + **safe-to-spend**
  (from W3), top categories, **budget bars + goal progress** (folded in, read-clear,
  edit via overlay), and an **Insights** section (trends + subscriptions, folded
  in). The "am I okay?" answer at a glance.
- **Each view's edit model:** clean read state by default; a top **"Edit" toggle**
  for multi-edit views; edits open in an **overlay side-panel** (a `Sheet`/side
  drawer), never inline form-cramming; **every edit control is paired with a small
  "edit with AI" icon** that fires a context prompt into chat (W2 provides the
  agent tools those prompts invoke).
- **Remove** the WidgetBuilder / user-built-dashboard Edit mode and the
  Subscriptions/Trends/Goals/Budgets/Resources **standalone tabs** as navigation —
  their compute (overview/trends/recurring/forecast) stays and now feeds the
  Dashboard sections + the agent.
- **First run = value, not a form.** Onboarding shrinks to the minimum (currency +
  optional income) or defers behind "import your first statement," so the aha
  (your own money, categorized) comes before setup.

Mostly *re-laying-out compute we already have* + a consistent edit pattern (toggle →
overlay panel → manual control + AI icon). The app stops looking like a control
panel and starts reading as a clear money view you can edit two ways.

### W1 — AI-native categorization (attacks G2, the #1 killer)

Add **LLM-assisted categorization** on import and on demand, learning the user's
taxonomy — so the one screen is never a wall of "uncategorized" (the lived result
of the end-to-end test: 14/14 uncategorized on a clean statement).

- On import, batch-classify rows with the LLM **constrained to the user's category
  list** (classification from a fixed set — not free generation; numbers untouched).
- **"Correct once, rule forever":** when a merchant is categorized, auto-offer a
  merchant rule (Copilot's loop — what gets to ~90%).
- Keep the deterministic cascade as the floor; the LLM only fills "uncategorized."

### W2 — The agent as a parallel edit path (attacks G1, powers the "edit with AI" icons)

Add agent tools (permissioned, deterministic handlers reusing the existing `action`
path — no new persistence/math): `set_budget`, `add_account`, `set_goal`,
`add_category_rule`, `categorize_transactions` (bulk), `import_statement`. These are
what the **"edit with AI" icons** (W0) actually invoke — so "set a grocery budget to
$400," "add an account," "recategorize this and similar" work from chat *in parallel
with* the manual controls in each view. The agent doesn't replace the UI; it's the
second path wired into it. (Also: bulk-recategorize + inline edit live in the
Transactions view itself, manually.)

### W3 — The "safe-to-spend" answer (attacks G3, fills W0's headline)

- A single **safe-to-spend** number: (income or `monthly_income` — finally using
  G5) − committed recurring − budgeted-remaining, stated plainly. Deterministic
  math, LLM narration. This is the one screen's headline.
- A one-line **"how's my month"** the agent generates from the computed overview,
  with **grace, not guilt** (research #4) — informational, never "you failed."

### W4 — Import trust + memory (attacks G4, G7)

- **Surface the warnings** the backend already computes (truncated/malformed/
  total-rows) — closes the silent-partial trust bug.
- **Remember the column mapping per account** (Lunch Money's top low-friction
  feature): re-import to the same account skips mapping.
- **Live date-parse preview** in mapping (Actual's green date) — kills the #1 CSV
  bug. Fix the two **currency artifacts** (G6).

### W5 — Typed relational storage (foundation; attacks G2/G9 structurally)

Migrate finance off the schemaless JSON `DataStore` onto **typed SQLite tables**
with foreign keys and `CHECK` constraints (full DDL in §6). This is the foundation
the other workstreams sit on:

- **`categories(parent_id)`** as a real table replaces the closed
  profile-string taxonomy + freeform category strings — the structural fix for G2
  (W1's auto-categorization writes a real `category_id`, subcategories become
  possible, renames are FK-safe).
- **`budget_items`** (budget→category) and **`goal_contributions`**
  (goal←account funding history) replace the JSON blobs — the structural fix for G9.
- **FK + CHECK integrity** the JSON store can't enforce (e.g. `transaction_type IN
  ('income','expense')`, valid `account_id`).
- **Coexists** with `DataStore` (sessions/skills/artifacts keep using it); finance
  gets its own `rusqlite_migration` table set + a **one-time migration** of existing
  JSON finance documents into the typed tables.
- Every finance handler (`do_add_transaction`, import, overview, aggregate, …) is
  rewritten to query SQL instead of `json_extract` over blobs — *and this also fixes
  G10* (real indexed columns + date-range push-down replace full-table JSON scans).

**Decisions taken (see §6):** adopt typed tables; **direct balances** (no
double-entry ledger in v1); **drop the `users` table / `user_id`** — the app is
single-user and the engine already scopes by `workspace`, so a user concept would
be dead weight (like today's unused `monthly_income`).

**Sequencing:** **W5 lands first** — it's the foundation W1/W2/W3 all write through
(category_id, budget_items, goal_contributions). Then **W2** (agent tools over the
new tables) as the prerequisite that makes W0's tab cut safe, **W0 + W1** together
(clear UI that isn't a wall of uncategorized), then **W3** (safe-to-spend) and
**W4** (import polish). W0 without W2 strands features; W2 without W0 just adds tools
to an overwhelming UI — they ship as a pair, both on top of W5.

---

## 6. The v1 schema (typed SQLite, direct balances)

Adapted from the proposed relational design, with two v1 simplifications: **no
`users` table** (single-user, local-first — the engine scopes by `workspace`), and
**direct balances** (no double-entry ledger yet, but shaped so one could be layered
later without a rewrite). FKs + `CHECK`s stay — the integrity the JSON store lacks.

```sql
-- Categories: a real tree (parent_id), replacing the closed profile-string list.
-- Seeded with sensible defaults; the user/agent can add children.
CREATE TABLE fin_categories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_id  INTEGER REFERENCES fin_categories(id),
    name       TEXT NOT NULL,
    type       TEXT NOT NULL CHECK(type IN ('income','expense'))
);

CREATE TABLE fin_accounts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    type            TEXT NOT NULL CHECK(type IN ('checking','savings','card','cash')),
    institution     TEXT,
    opening_balance REAL NOT NULL DEFAULT 0,   -- current balance = opening + Σ movement
    currency        TEXT NOT NULL DEFAULT 'USD'
);

CREATE TABLE fin_transactions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id       INTEGER NOT NULL REFERENCES fin_accounts(id),
    category_id      INTEGER REFERENCES fin_categories(id),   -- NULL = uncategorized
    amount           REAL NOT NULL,                            -- stored positive; sign by type
    transaction_type TEXT NOT NULL CHECK(transaction_type IN ('income','expense')),
    merchant         TEXT,
    notes            TEXT,
    transaction_date DATE NOT NULL,
    source           TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual','import')),
    import_hash      TEXT,                                     -- dedup key for imports
    created_at       DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_txn_date     ON fin_transactions(transaction_date);
CREATE INDEX idx_txn_account  ON fin_transactions(account_id);
CREATE INDEX idx_txn_category ON fin_transactions(category_id);
CREATE INDEX idx_txn_hash     ON fin_transactions(import_hash);

CREATE TABLE fin_transfers (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    from_account_id INTEGER NOT NULL REFERENCES fin_accounts(id),
    to_account_id   INTEGER NOT NULL REFERENCES fin_accounts(id),
    amount          REAL NOT NULL,
    transfer_date   DATE NOT NULL
);

CREATE TABLE fin_budgets (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    start_date DATE,
    end_date   DATE
);

CREATE TABLE fin_budget_items (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    budget_id      INTEGER NOT NULL REFERENCES fin_budgets(id),
    category_id    INTEGER NOT NULL REFERENCES fin_categories(id),
    planned_amount REAL NOT NULL
);

CREATE TABLE fin_goals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK(kind IN ('savings','debt')),
    target_amount REAL NOT NULL,
    target_date   DATE
);

CREATE TABLE fin_goal_contributions (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_id           INTEGER NOT NULL REFERENCES fin_goals(id),
    account_id        INTEGER NOT NULL REFERENCES fin_accounts(id),
    amount            REAL NOT NULL,
    contribution_date DATE NOT NULL
);

-- Per-account import mapping memory (W4): remember column mapping per account.
CREATE TABLE fin_import_profiles (
    account_id INTEGER PRIMARY KEY REFERENCES fin_accounts(id),
    mapping    TEXT NOT NULL    -- JSON: {date, merchant, category, amount/debit/credit}
);
```

Notes:
- Tables are `fin_`-prefixed and live in the same `zanto.db`, under their own
  `rusqlite_migration` set — they don't touch the `data_stores`/session schema.
- `goal.kind` and `import_hash`/`source` carry forward today's behavior so the
  migration is faithful.
- `fin_import_profiles` realizes W4's mapping memory as a real table.
- Direct-balance shape: current balance = `opening_balance` + Σ(signed txns) ±
  transfers ± contributions, computed in SQL. A future ledger could replace the
  derivation without changing the UI/agent contract.

---

## 7. Explicitly parked (not v1)

Per "simple yet powerful — not too much load": multi-currency/FX (G11),
reconciliation/cleared balances (G11), goal pacing + contribution math (G9),
smarter recurring (weekly/annual/trials, upcoming-charge surfacing) (G8),
aggregation indexing/date push-down (G10 — revisit when a real user has years of
data), splits/tags/attachments, export. These are good — they're just not what's
between us and retention. Park them with a note, don't delete them.

---

## 8. Why this is the right bet

Two things kill these apps and we have both: the app **looks like work** (8 tabs, a
config mode, forms — "too many features / decision fatigue"), and the effort
**doesn't turn into a decision** (a clean statement imported 14/14 *uncategorized*
in the real end-to-end test, headline insight = "uncategorized $2,006").

The fix isn't more features — it's **clarity + dual paths**. We cut 8 unclear
tabs and a config mode down to **4 crisp, directly-editable tabs** with a clear
home dashboard, a consistent edit model (toggle → overlay panel), and a small
**"edit with AI" icon beside every manual control** so the agent is a parallel
power-path, not a wall to hide behind. We uniquely already have the hard, correct
part (deterministic engine, import spine, recurring/forecast math) *and* an LLM in
the loop. v1 turns that into: open it → one clear answer → drop a file → it's
categorized for you → edit anything two ways (by hand or by asking). Clearer, more
visible, more powerful — mostly re-laying-out parts we've already built.

**Validation note:** §1–§3 and this bet are grounded in a first-hand run of the
real backend (onboarding → account → CSV import → categorize → dashboard → agent),
not theory. The "14/14 uncategorized" and "no safe-to-spend / `monthly_income`
unused" findings are observed, not inferred.

---

## Sources

Market/abandonment: [Inquirer/WSJ (Aite-Novarica 20%/90%)](https://www.inquirer.com/business/mint-budgeting-app-shutting-down-tips-20231112.html),
[PMC scoping review](https://pmc.ncbi.nlm.nih.gov/articles/PMC11694054/),
[econbrew — why budgeting apps fail](https://www.econbrew.com/post/why-budgeting-apps-fail-the-hidden-behavioral-aspects),
[Neculai — why budgeting apps fail](https://medium.com/@stefanneculai/why-most-budgeting-apps-fail-and-what-actually-works-88904191967e),
[Monarch — Mint shutdown (ex-PM)](https://www.monarch.com/blog/mint-shutting-down).
Local-first/import: [Actual Budget](https://actualbudget.org/docs/transactions/importing/),
[Lunch Money import](https://support.lunchmoney.app/guides/import-via-csv),
[Tiller](https://tiller.com/resources/bank-transaction-automation/).
AI role: [DEV.to/Valyu — AI hallucinating financial data](https://dev.to/valyuai/why-your-ai-agent-keeps-hallucinating-financial-data-and-how-to-fix-it-180d),
[moveo.ai — why LLMs struggle at math](https://moveo.ai/blog-new/why-llm-struggle),
[Monarch AI features](https://help.monarch.com/hc/en-us/articles/16116906962452-About-Monarch-s-AI-Features),
[Copilot intelligence](https://help.copilot.money/en/articles/8182433-copilot-intelligence-for-spending).
Onboarding: [Userpilot fintech onboarding](https://userpilot.com/blog/fintech-onboarding/),
[Chameleon — aha moment](https://www.chameleon.io/blog/successful-user-onboarding).
</content>
