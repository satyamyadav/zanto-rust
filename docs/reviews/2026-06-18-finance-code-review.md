# Code Review — Finance v0.2–v0.5 + chart/artifact changes (brutal)

**Date:** 2026-06-18 · **Reviewer:** 3 parallel deep-review agents + verification of top claims · **Verdict below is unsparing by request.**

## TL;DR verdict

The **pure aggregation logic is genuinely good** — well-factored free functions with real unit tests (balance, normalize, budgets, recurring, trends, forecast, goals, account balances, import mapping). That layer is the strong part and I stand behind it.

**Everything at a boundary is unverified and that is exactly where the serious bugs are.** Four finance versions shipped in one session with **zero `pnpm dev` runs**. The result: two confirmed P0s (one security, one "the default UI can't be saved"), several silent financial-correctness bugs, a god-file that's outgrown its own validators, and ~0% behavioral test coverage on IPC and Svelte. A finance app that silently reports the wrong net worth is worse than one that crashes. Several findings here are that class of bug.

The single most damning fact: **I added widget kinds to the default layout and the builder across three versions but never updated the save validator, so the first user who customizes their dashboard loses half of it.** Pure velocity-over-verification, and no test or type-check could catch it because it's a runtime data mismatch.

---

## P0 — Confirmed shipped bugs (verified against the code)

### P0-1 · The default dashboard cannot round-trip through save
`finance/mod.rs:529` allows only `kpi|chart|table`; `default_widgets()` ships `budget, subscriptions, accounts, goals, forecast` too.
**Effect:** enter Edit mode → Save → those 5 widget kinds are silently filtered out. The user's customized dashboard loses budgets/accounts/goals/forecast/subscriptions on first save.
**Cause:** I extended `default_widgets()` + `WidgetBuilder` SOURCES in v0.3–v0.5 but never updated the `do_save_widgets` whitelist. Self-inflicted.
**Fix:** widen the match to all real kinds (`kpi|chart|table|budget|subscriptions|accounts|goals|forecast`), or derive the allowed set from one shared list so it can't drift again.

### P0-2 · Confused-deputy: import write path bypasses the read permission check
`ipc/finance.rs:19` permission-checks the **parse** (`permissions.check(path, Op::Read)`), but `ipc/apps.rs:run_app_action` does **no checks at all** and `import_transactions` (`finance/mod.rs:548`) trusts client-supplied `rows`.
**Effect:** the permission prompt on parse is security theater — any webview code can call `run_app_action("finance","import_transactions",{rows:[…fabricated…]})` with rows that never touched a real file or a permission gate. Same for `add_transaction`/`delete_transaction`/`save_*`/`add_transfer`.
**Caveat (fair):** for a local app the webview runs our own code, so the practical threat is lower than a web context — but the *architecture* is wrong: gate the read, then hand data to an ungated mutation.
**Fix:** make `import_transactions` take a permission-checked path and parse server-side (don't trust client rows), or move the permission boundary onto the write. Either way the parse-check + client-rows pattern must go.

---

## Critical — financial correctness (backend)

### C1 · Money silently vanishes when an account is renamed/deleted
`finance/mod.rs:compute_account_balances`. Income/expense on a missing account (`bal.get_mut(acct)` → `None`) is a silent no-op; a transfer debits `from` but drops the credit if `to` is gone. Accounts are an **editable** list while transactions are immutable, so a rename orphans every historical row pointing at the old name. **Net worth silently shrinks**, and disagrees with `lifetime_balance` (which still counts those rows). This is the most dangerous bug in the codebase.
**Fix:** accumulate balances for *all* account names seen in transactions (materialize an "Unlinked" bucket and include it in net worth), or forbid rename/delete of an account that has transactions without an explicit merge.

### C2 · Legacy/missing-`type` rows count as expenses; missing-`account` rows pin to "Cash"
`normalize_txn` defaults absent `type` → **expense** and absent `account` → **Cash**. A pre-v0.2 income row is now an expense forever; a pre-v0.4 row whose Cash account was later renamed vanishes from net worth (C1). The test at `finance/mod.rs` *asserts the expense-default as correct* — that's data corruption dressed as back-compat. With **no schema migrations anywhere** for the per-app stores (only read-time defaulting), this is the entire upgrade story and it's lossy.

### C3 · Import dedupe key omits the account
`import_hash = date|amount|merchant` only. Importing the same statement into two accounts, or two genuinely distinct same-day/same-merchant/same-amount charges across cards, collapse to one (`duplicate_skipped`). **Real rows silently dropped.**
**Fix:** include `account` in the hash.

---

## High

- **H1 · XLSX dates import as Excel serial numbers.** `parse_table` stringifies calamine cells with `to_string()`; a date cell comes through as `"45810"`, not a date. Every XLSX-imported transaction gets a garbage date → breaks month bucketing, recurring, trends, forecast. CSV is fine; XLSX is likely broken end-to-end. Needs calamine's typed `Data::DateTime` handling. (Untested — no real .xlsx was ever imported.)
- **H2 · `coerce_amount` is a fragile hand-rolled parser.** Strips everything but digit/`.`/`-`. Trailing-minus debits (`"5-"`, a common bank/Excel convention) → parse fails → `0.0` → row dropped. European `"1.234,56"` → `1.23456` (100× wrong). The doc comment even *claims* `"12,50"`→12.5 but it yields 1250. Imports are the bulk path; silent wrong/zero amounts are unnoticed correctness failures. Use a real money parser; never silently drop a zero-coerced row.
- **H3 · Silent row loss in import.** `parse_table` `take(MAX_ROWS=5000)` truncates with no `truncated` flag; malformed CSV rows are `if let Ok` skipped with no count. A 7000-row statement imports 5000 and says nothing. Data loss without notice.
- **H4 · f64 money everywhere with exact comparisons.** `is_over = spent > limit`, `complete = current >= target` are exact float compares over summed cents; net worth displays `1449.9999998`. Latent, accumulating. Use integer minor units, or at minimum round every money output to 2dp and compare with an epsilon.
- **H5 · Pace projection counts future-dated spend.** `spent` includes any row whose date `starts_with(this_month)`, including future dates within the month; `projected = spent/f` with `f=day/days` then over-projects wildly (a row dated the 30th entered on the 5th projects ~6×). Also day-1 `f≈0.03` makes one early expense a false pace alarm. Bound to `date ≤ today`; add a min-elapsed-days guard.
- **H6 · Unbounded insert-only singleton stores.** profile/widgets/budgets/accounts/goals each `insert` a new full row on every save and read back `max_by_key(saved_at)`. No compaction. `compute_overview` scans 3 of these every load; `get_profile` is scanned on **every transaction add** (via category resolution). O(saves) growth on the hot path — and `DataStore::update` already exists to fix it. Same-second saves tie-break nondeterministically (no `ORDER BY id`).
- **H7 · Goal linked to a renamed account silently reports "complete."** `compute_goal_status` reads `balance_of(account)`; a missing account → 0.0 → a debt goal shows `owed=0, complete=true`. Same fragility class as C1: can't distinguish "not found" from "zero."

---

## Medium

- **M1 · ~5 full table scans + full deserialize of the entire transactions store per dashboard load** (`compute_overview`, `compute_recurring`, `compute_trends`, `compute_forecast`, `compute_account_balances`, plus `compute_monthly_summary` ignores its `category` param and full-scans). Performance cliff that only shows with real data — which zero manual testing guarantees you won't see.
- **M2 · `update_transaction` re-resolves category and can clobber a deliberate edit.** Editing only the category to an off-profile value silently downgrades to "uncategorized"; editing the merchant can overwrite a correct category because a rule now matches. Surprising on the edit path.
- **M3 · `do_import_transactions` is 1000+ lock cycles with no surrounding transaction.** Per-row `do_add_transaction` (each: lock, dedupe query, insert, unlock). Partial failure leaves half a statement imported with no rollback.
- **M4 · `detect_recurring` "median" is the upper-middle element** (not a true median for even gap counts) and amount-buckets by `round()` so `$9.49`/`$9.99` split a stable subscription near the `.5` boundary. Heuristic, but mislabeled and lossy at boundaries.
- **M5 · `last_n_months` fails open** to a single-element vec on malformed input → 6-month series collapses to 1, MoM=0, forecast `prev3` empties. Only safe because input is always `today()[..7]`; a one-char format change silently zeroes the dashboard.

---

## Frontend (Svelte)

- **F-C1 · `load()` nulls `overview` on every refetch** (`Dashboard.svelte:124`), so saving any editor (`onSaved={load}`) tears the editing panel down to a skeleton mid-edit, and re-triggers the notify `$effect`s on every refresh. Root cause of multiple bugs. **Fix:** keep stale `overview`, swap on success, use a separate `loading` flag.
- **F-C2 · Import round-trips up to 5000 rows through JSON IPC twice** (parse returns all rows to JS `$state`, JS sends them all back to `import_transactions`). The file was already server-side at parse. Wrap as `$state.raw` at minimum; better, keep rows server-side keyed by a token. Pairs with **F-H1: column mapping sends header *names*, not indices** → breaks on duplicate/empty headers (common in bank CSVs); the `{#each columns as c (c)}` key throws on duplicate names.
- **F-H2 · Native-notify effects can double-fire and burst.** A category both over-budget and pace-warned fires two OS notifications (separate effects/keys); N over-budget categories fire N notifications at once. Notify happens *before* the localStorage `setItem`, so a notify throw → re-notify next load. Coalesce; persist-then-notify.
- **F-H3 · Index-keyed editable lists** in `AccountsEditor`/`GoalsEditor` (`{#each rows as r, i (i)}`) with remove/reorder → bound inputs re-associate to the wrong row after a delete. Classic Svelte each-by-index corruption. Use stable ids.
- **F-H4 · Import allows `account=""`** (button stays enabled with no accounts configured) → rows imported under an empty account. Disable + CTA.
- **F-M · `kpiValue((overview as any)[source])` returns a confident `$0.00` for unknown/removed sources** — indistinguishable from a real zero. Every finance IPC call is `any`-typed (`ipc.ts`), so every `as T` is an unchecked assertion. MoM block duplicated between `Dashboard` and `Trends`; accounts list duplicated between the widget and `Accounts.svelte`; `format.ts` fallback drops `minimumFractionDigits`. `window.confirm` for delete vs toasts elsewhere.
- **F-Chart · `Chart.svelte` `$effect` fires before the async `await import` resolves** (`chart` null), so data that changes during the import window is dropped; `buildOptions` runs twice on mount; `updateOptions(...,true,true)` redraws paths every tick. Streaming pre-mount updates silently lost.

---

## Architecture / cross-cutting

- **A1 · Tool-call hiding at render time, not data time** (`Message.svelte` HIDDEN_TOOL_CALLS). Consequences: `stepCount` shows **"0 steps"** for a turn that ran two hidden tools; WorkflowGroup coalescing runs post-filter so mixed hidden/visible runs mis-group or vanish — actively fighting the F6 "inbuilt workflows" feature; the set is keyed on **bare tool name with no app scoping**, so any future app's `query_transactions` is silently hidden, and lumping domain tools (`monthly_summary`) into a set commented "artifact-system internal" is a category error. **Fix:** flag the segment as "renders as block" when the result is `AppResult::Block`, filter on that.
- **A2 · Persisted-but-hidden transcript bloat.** `assistant_turn_meta` persists the full tool `output` for hidden tools (a `monthly_summary` or 500-row `import_transactions` result) into the message metadata column — data the UI throws away at render and re-parses every reload. Persist the block reference, not the raw output.
- **A3 · `notify(title,body)` is an unauthenticated native-notification primitive** — any caller, any text, no rate limit, no length cap, no source attribution (phishing surface). Cap + rate-limit + scope it.
- **A4 · No bounds on `chart` series length or `import_transactions` row count** — a weak model echoing 10k points renders synchronously in the webview.
- **A5 · finance/mod.rs is a 1700-line god-file** mixing manifest, App-trait impl, deterministic flows, pure aggregation, parsing, and tests; `compute_overview` is a 100-line function that bolts on a new block per version and hard-wires v0.2→v0.5 order. The insert-only get/save scaffolding is copy-pasted 5× (profile/widgets/budgets/accounts/goals). Split into `aggregate.rs` / `import.rs` / `stores.rs`; keep `mod.rs` for manifest + dispatch.

---

## Corrected / disputed findings (accuracy matters in a brutal review)

- **"Stopped marker has no UI consumer / renders nothing on reload"** — **wrong.** `MessageList.svelte:89` renders `{#if entry.stopped}`. The v0.2 C-2 persistence fix + that render path are plausibly correct. Still **never run in `pnpm dev`**, so "plausibly" is doing work — but the specific claim of a missing consumer is false.
- **calamine float stringification producing `"9.9900001"`** — **wrong**; Rust's shortest-round-trip Display keeps `"9.99"`. (The real XLSX problem is date serials, H1.)
- **`import_hash` non-deterministic across runs** — **wrong**; `DefaultHasher::new()` has fixed seeds, so it's stable. (The real problem is C3, the missing account in the key.)

---

## What I'd fix first (ordered by damage / effort)

1. **P0-1** — widen `do_save_widgets` whitelist (one-line; stops silent dashboard data loss). Trivial.
2. **C1 + H7** — orphaned-account money loss + goal false-complete. Materialize an "Unlinked" bucket; distinguish not-found from zero. Highest correctness payoff.
3. **P0-2** — move the import permission boundary onto the write (parse server-side from a checked path; stop trusting client rows). Closes the confused deputy and kills the 5000-row round-trip (F-C2) at the same time.
4. **F-C1** — stop nulling `overview` on refetch. Fixes the mid-edit teardown and the notify re-fire.
5. **H2 + H3** — money parsing + silent import truncation/skip reporting. Imports are the bulk path; silent loss is unacceptable here.
6. **H1** — verify/fix XLSX date handling (or restrict import to CSV until fixed and say so).
7. **H6 / M1 / M3** — store growth + per-load full scans + non-transactional import. Performance + integrity.
8. **A1/A2** — move tool-hiding and block persistence to the data layer.
9. **A5** — split the god-file before v0.6.

## Honest self-assessment of the process

- **The good:** disciplined spec→decide→implement→test loop per version; every pure function is unit-tested; the decomposition into deterministic Rust + thin Svelte is sound; commits are atomic and revertible.
- **The bad:** I optimized for "tests + `pnpm check` green" and treated that as done. It isn't — it never caught the two P0s, because both live exactly where there are no tests (IPC wiring, runtime UI). I shipped a feature (widget kinds) across three versions while leaving its validator behind, and a brutal read found it in minutes. Four versions without a single `pnpm dev` run is the root cause of every P0 here.
- **The fix to the process:** behavioral coverage at the boundaries (one integration test per IPC command; smoke-test the import + dashboard-save flows in `pnpm dev`) before any further feature work. Velocity bought breadth at the cost of trust, and for a *finance* app trust is the product.
