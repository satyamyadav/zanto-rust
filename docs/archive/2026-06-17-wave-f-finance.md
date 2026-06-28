# Wave F — Finance app (F1–F6)

- **Date:** 2026-06-17
- **Depends on:** existing Finance app (`apps/finance/mod.rs`, components
  `transactions_table`/`monthly_summary`), DataStore, E1 charts, A1 `project_dir`,
  B1 `browse_dir`, A4 skills/flows, C6 workflow view.

Scope: keep it **personal finance** for now. The Finance app is a micro-app mounted in
the shell; it has a chat + a right canvas + (new) a dashboard view.

## F1 — Overview dashboard + empty-state flows
- Files: `crates/zanto-desktop/src/lib/apps/finance/Dashboard.svelte` (new),
  wiring so mounting Finance shows the dashboard (e.g. as the canvas default or a panel),
  `apps/finance/mod.rs` (a `query` returning overview data), `registry.ts` if a new block.
- Dashboard shows KPIs (balance, this-month spend, top categories) + a chart (E1) built
  from DataStore transactions. When there is **no data**, show an empty state with primary
  actions ("Add a transaction", "Import", "Set up budget") that kick off chat flows
  (send the corresponding prompt / start action).
- Acceptance: with data → KPIs + chart; without data → empty-state CTAs. Build-check.

## F2 — Onboarding + personal-finance track setup
- Files: `apps/finance/mod.rs` (stores/setup actions), a `lib/apps/finance/Onboarding.svelte`,
  session/start wiring.
- First run: a short onboarding (income, currency, a couple of categories/accounts) that
  seeds DataStore (a `finance_profile` store + categories). Idempotent; skippable.
- Acceptance: completing onboarding persists a profile; re-mount detects it and skips.

## F3 — Project-dir resource files (read + optional upload to chat)
- Files: `lib/apps/finance/*` (a resources panel), uses `browseDir` (C7/B1) +
  `read_stored_artifact`/fs read; `ipc.ts` as needed.
- Let the user pick a project dir (A1 `project_dir`) and list resource files (statements,
  CSVs) from it; selecting one can read it into context or attach it to the chat composer
  as a reference for import flows.
- Acceptance: lists files from the chosen dir; "attach" includes the file in the next
  chat turn (as a context note / path). Build-check.

## F4 — Widgets & dashboards builder
- Files: `lib/apps/finance/*` (widget config UI), persists widget defs in DataStore.
- Let the user compose the dashboard from widgets (KPI, chart, table) bound to DataStore
  queries; persist the layout; render via existing block components + E1 charts.
- Acceptance: add/remove/reorder widgets; layout persists across reloads. (Depends F1, E1.)

## F5 — Chat quick starts
- Files: `apps/finance/mod.rs` (`start_actions`/NBA), small.
- Curate Finance `start_actions` (Add transaction, This month, Import statement, Budget
  status) surfaced as the chat-start NBA (existing mechanism).
- Acceptance: mounting Finance / new session shows the quick-start NBA. Build-check.

## F6 — Inbuilt finance workflows
- Files: `apps/finance/mod.rs` (multi-step flows), a skill/flow markdown under
  `.zanto/skills` or app skill text (A4), surfaced via the workflow view (C6).
- Provide canned multi-step flows (e.g. "Import & categorize a statement", "Monthly
  review") that drive a tool sequence; the multi-loop workflow view (C6) renders progress.
- Acceptance: invoking a workflow runs its steps; the thread shows the workflow grouping.
  Build-check. (Depends C6, A4.)

## Acceptance (every unit)
`cargo build` + `cargo test -p zanto-core` + `pnpm check` (0 errors) + `pnpm build:web`.

## Batching note (coordinator)
`apps/finance/mod.rs` is shared by F1/F2/F5/F6 → stagger those. F5 is tiny (do first or
fold in). Suggested: `{F5, F3}` → `{F1, F2}` → `{F4}` → `{F6}`, each syncing to updated
main. F4 depends F1; F6 depends C6.
