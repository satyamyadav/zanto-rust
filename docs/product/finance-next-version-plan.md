# Personal Finance — Next-Version Product Plan

**Author:** PM review (fintech)  ·  **Date:** 2026-06-18  ·  **Status:** Draft for review

---

## 1. Where it stands today (honest read)

**What's built (and good):**
- Local-first, AI-native: add/query transactions by chat; deterministic Rust aggregation (never the LLM doing math) — correct by construction.
- Dashboard: balance / this-month / count KPIs, 6-month spend bar, top categories. Customizable widgets (kpi/chart/table, add/remove/reorder).
- Onboarding profile (currency, monthly income, categories), canned multi-step workflows (statement import, monthly review), a resources panel.
- This session: charts render reliably, string amounts coerced to numbers, view-tools render inline.

**The uncomfortable truth — it's a transaction logger, not a finance manager.** Five gaps block the core promise ("where is my money going, am I on track?"):

| # | Gap | Why it's serious |
|---|---|---|
| **1** | **No money model.** `amount` is a single unsigned number; `balance` = naive sum of everything. No income vs expense, no transfers, no sign convention. `monthly_income` is collected and **never used**. | The headline numbers are wrong the moment a user logs a paycheck *and* a coffee. This undermines trust — fatal in fintech. |
| **2** | **Ledger is immutable.** DataStore is insert-only; no edit/delete. A mis-categorized or duplicate transaction is permanent. | "Import & categorize a statement" double-counts on re-run and can't be corrected. The flagship workflow is unsafe in practice. |
| **3** | **No budgets.** "Set a budget" is a button that just chats — no budget store, no budget-vs-actual, no alerts. | Budgeting is table-stakes for PFM. Its absence is the difference between "a log" and "a coach." |
| **4** | **Categorization is unenforced.** Profile categories aren't validated; the LLM invents new ones → fragmented reporting. No rules. | Reports drift and stop being comparable month to month. |
| **5** | **No insight layer.** No recurring/subscription detection, no month-over-month deltas, no trends or anomalies. | Nothing here a spreadsheet can't do. No reason to come back weekly. |

**Constraint to embrace, not fight:** no backend, no bank aggregation (Plaid/Teller need a server + credentials). That's fine — **lean into local-first privacy as the wedge**: "your financial data never leaves your machine." Import-driven, not sync-driven.

---

## 2. North Star & goals

**North Star Metric:** *Trusted Monthly Reviews* — the count of months in which a user views a **complete, correctly-categorized** summary (data completeness × engagement). It only goes up when the numbers are right *and* the user comes back.

**Supporting metrics:** transactions corrected/recategorized per active month (data quality), % transactions auto-categorized correctly, budgets set per active user, recurring charges surfaced.

**Product goals for the next version (v0.2 → v0.5):**
- **G1 — Correct money model:** income / expense / transfer; real net cash flow and balance.
- **G2 — Editable, trustworthy ledger:** edit / delete / recategorize; dedupe on import.
- **G3 — Budgets & nudges:** category budgets, budget-vs-actual, overspend alerts.
- **G4 — Reliable import:** CSV/statement → parsed → user-reviewed batch insert → dedup.
- **G5 — Insight that earns a weekly open:** recurring detection, MoM deltas, category trends.

**Explicitly NOT now (say no, loudly):** live bank sync, investment/portfolio tracking, multi-user/cloud, tax filing, crypto. They need a backend or a different product; they'd dilute the local-first wedge.

---

## 3. Roadmap (phased, each phase shippable)

### Phase 0 — Foundation (enabler, no user-facing feature)
The data layer must support correction and a real money model before anything else is worth building.
- DataStore: add **update + delete** (currently insert-only) — or a typed `transactions` table with stable IDs.
- Transaction schema v2: `id`, `type` (`income`|`expense`|`transfer`), signed/abs `amount`, `account?`, `note?`, `source` (manual|import), `import_hash` (for dedupe).
- Migrate aggregation: `balance` = income − expense; `month_total` = expenses only.
- **Exit criteria:** logging income + an expense yields a correct balance and cash-flow figure.

### Phase 1 — "Trustworthy Ledger" (v0.2) — *highest priority*
Targets G1, G2.
- Edit/delete/recategorize a transaction (UI affordance on the transactions table + an `update_transaction` / `delete_transaction` tool).
- Income/expense/transfer in add flow + a **Cash Flow** KPI (in − out) replacing the naive balance.
- Import dedupe via `import_hash` (date+amount+merchant) — re-running a statement no longer double-counts.
- Category enforcement: validate against profile categories + a tiny rules map (`merchant → category`), with an "uncategorized" review queue.
- **Success:** users can fix any wrong row; a re-imported statement adds zero duplicates; ≥80% of imported rows land categorized.

### Phase 2 — "Budgets & Insight" (v0.3)
Targets G3, G5.
- Budget store + per-category monthly budgets; **budget-vs-actual** widget with over/under coloring; overspend nudge (in-app + the existing native notification).
- Recurring/subscription detection (same merchant+~amount monthly) → a "Subscriptions" view.
- Month-over-month deltas and a category-trend line chart; upgrade "Monthly review" to narrate these.
- **Success:** budget set rate among active users; subscriptions surfaced ≥1/user; weekly open rate up.

### Phase 3 — "Import & Accounts" (v0.4)
Targets G4 + breadth.
- Structured statement import pipeline: CSV/XLSX → column-map → **preview & confirm batch** → dedup insert (reuses `read_document`, but parses to rows instead of LLM-one-by-one).
- Multiple accounts (checking/savings/card) + per-account balances; transfers between accounts net to zero.
- **Success:** a 100-row statement imports in one reviewed step with correct categories and zero dupes.

### Phase 4 — "Goals & Forecast" (v0.5)
- Savings goals + debt payoff tracking; net worth (across accounts).
- Simple cash-flow forecast (recurring + average discretionary) → "you'll have ~X at month end."
- Proactive nudges ("dining is 80% of budget on the 18th").
- **Success:** goal set rate; forecast viewed; proactive nudge → action.

---

## 4. Sequencing logic & risks
- **Why Phase 0/1 first:** every later feature (budgets, forecast, insights) is wrong if the money model and dedupe aren't fixed. Building budgets on a naive balance ships a lie. Fix trust before reach.
- **Biggest risk:** categorization quality with small local models. Mitigate with deterministic rules + a review queue, not model-only inference (same lesson as the chart tool — give the model a narrow, lenient, one-shot path and do the math in Rust).
- **Effort shape:** Phase 0 is mostly backend (DataStore mutability + migration); Phase 1 is balanced; Phases 2–4 are increasingly frontend/insight. Each phase is independently shippable and demoable.

---

## 5. One-line pitch for v0.2
*"A private, on-device money manager you talk to — that finally gets the numbers right and lets you fix them."*
