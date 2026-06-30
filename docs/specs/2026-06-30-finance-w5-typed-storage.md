# Finance W5 — typed SQLite storage + migration

- **Date:** 2026-06-30
- **Status:** Proposal — awaiting approval before implementation
- **Plan ref:** `docs/product/finance-v1-plan.md` §5 (W5) + §6 (schema)

## Summary

Migrate the finance app off the schemaless JSON `DataStore` onto typed SQLite
tables (`fin_*`) with foreign keys and `CHECK` constraints, a one-time migration of
existing JSON finance documents, and a rewrite of every finance handler to query
SQL instead of `json_extract`. This is the foundation W1/W2/W3 build on.

## Motivation

Today the finance app stores everything as `serde_json::Value` blobs in generic
`(id, data TEXT, created_at)` tables, queried via `json_extract`. Consequences the
audit + end-to-end test found:
- **No category integrity** — categories are freeform strings enforced only by a
  closed profile list; the rigid-taxonomy + 14/14-uncategorized problem is rooted
  here (G2).
- **No FK/CHECK** — nothing stops an orphaned `account`, an invalid `type`, a budget
  referencing a deleted category.
- **Thin relational models** — budgets/goals are JSON blobs; goals have no funding
  history (G9).
- **Full-table JSON scans** on every overview/trends/forecast (G10).

Typed tables fix all four structurally and give W1 a real `category_id` to write.

## Scope

**In scope**
- New typed tables (`fin_*`, §6 of the plan) created in the shared `zanto.db`.
- A raw-SQL accessor on `DataStore` so the finance app can run typed SQL over the
  same connection (decision A below).
- A one-time **migration** that reads the existing JSON finance stores
  (`transactions`, `finance_profile`, `finance_accounts`, `finance_budgets`,
  `finance_goals`, `finance_category_rules`) and writes them into the typed tables,
  idempotently (safe to run once per workspace; a marker row prevents re-running).
- Rewrite of every finance `query`/`action`/`aggregate` handler to use SQL.
- Tests: schema creation, migration fidelity (JSON → typed round-trips the
  end-to-end journey's data), FK/CHECK enforcement, and the existing finance unit
  tests adapted to the new storage.

**Out of scope (later workstreams / parked)**
- Agent tools (W2), auto-categorization (W1), safe-to-spend (W3), import polish
  (W4) — W5 only changes *storage*, keeping behavior identical.
- Double-entry ledger, multi-currency, the `users` table (all parked per the plan).
- The prototype UI wiring (happens after W5/W2/W1 land).

## Key architectural decision

`DataStore` opens the `zanto.db` connection with `foreign_keys ON` + WAL and holds
it in a private `Mutex<Connection>`; it exposes no raw SQL. Finance needs typed SQL.
Two options:

- **(A) Add a scoped raw-SQL accessor to `DataStore`** — e.g.
  `DataStore::with_conn(|conn| …)` (or `execute_batch` / `query_typed` helpers)
  that lends the existing locked connection. **CHOSEN.** One connection, one WAL
  writer, no second-connection lock contention; finance tables live in the same
  file alongside `data_stores` and `sessions` (all already coexist via
  `CREATE TABLE IF NOT EXISTS`).
- (B) Finance opens its own `Connection` to the same file. Rejected: two writers to
  one SQLite file invites `SQLITE_BUSY` under WAL, and duplicates pragma/setup.

The `fin_*` tables are **not** workspace-prefixed like the JSON stores
(`ds_<hash>_<name>`). Finance is single-app, single-user; a `workspace` column on
each table (or simply one workspace per db, as today) keeps it scoped. **Decision:**
add a `workspace TEXT` column to top-level finance tables (accounts, transactions,
budgets, goals, categories) defaulting to the active workspace, matching how
`DataStore` already isolates workspaces — so multi-workspace stays correct.

## Affected files

- `crates/zanto-core/src/data/mod.rs` — add the raw-SQL accessor (`with_conn` or a
  small typed-exec API) + the `fin_*` schema (idempotent `CREATE TABLE IF NOT
  EXISTS`, FK/CHECK, indexes). Possibly a `fin_meta` marker table for the one-time
  migration guard.
- `crates/zanto-desktop/src-tauri/src/apps/finance/mod.rs` — rewrite
  `do_add_transaction`, `do_update_transaction`, `do_delete_transaction`,
  `do_save_*` (profile/accounts/budgets/goals/category_rule), `do_import_transactions`,
  `do_add_transfer`, and all `query` handlers to SQL.
- `crates/zanto-desktop/src-tauri/src/apps/finance/aggregate.rs` — rewrite
  `overview`/`trends`/`recurring`/`forecast` aggregations as SQL (indexed,
  date-range push-down — fixes G10).
- `crates/zanto-desktop/src-tauri/src/apps/finance/import.rs` — `import_hash`/
  parsing stay; dedup now queries `fin_transactions.import_hash` (indexed).
- `crates/zanto-desktop/src-tauri/src/apps/finance/migrate_json.rs` (new) — the
  one-time JSON → typed migration.
- Tests in each.

## Implementation steps

1. **Raw-SQL accessor on `DataStore`** (`data/mod.rs`)
   - Add `pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, DataError>`
     that locks the mutex and lends `&Connection`. (Or a pair of `exec_batch` +
     `query_rows` helpers if a closure API is undesirable — decide in impl; the
     closure form is the most flexible and keeps locking internal.)
   - Document that callers must not re-enter `DataStore` methods inside the closure
     (would deadlock the mutex).

2. **`fin_*` schema** (`data/mod.rs`, run from finance `ensure_store`)
   - Add the DDL from plan §6, with a `workspace TEXT NOT NULL` column on
     accounts/transactions/categories/budgets/goals (FKs stay intra-workspace by
     construction). `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS`.
   - Seed default categories (the 7 today) per workspace on first creation if the
     category table is empty for that workspace.

3. **One-time JSON → typed migration** (`migrate_json.rs`)
   - Guard: a `fin_meta(workspace, migrated_at)` row; skip if present.
   - Read each JSON store via the existing `DataStore::query` and insert into the
     typed tables: categories from the profile list; accounts; transactions
     (mapping `type`/`amount`/`merchant`/`category` string → `category_id` by name,
     NULL if unknown; carry `import_hash`/`source`/`date`); budgets → budget +
     budget_items; goals; category rules (kept as a `fin_category_rules` table —
     add it to §6: `{id, merchant_contains, category_id}`).
   - Idempotent + non-destructive: leave the JSON stores intact (rollback safety);
     a later cleanup can drop them once typed storage is proven.

4. **Rewrite handlers to SQL** (`mod.rs`, `aggregate.rs`, `import.rs`)
   - Each `do_*`/`query`/aggregate becomes parameterized SQL over `fin_*`.
     Preserve EXACT current behavior (amounts positive + sign by type; category
     enforcement → now an FK lookup + the rules table; dedup via `import_hash`).
   - `resolve_category_pure` becomes: requested name → category_id if it exists in
     this workspace; else first matching `fin_category_rules`; else NULL
     (uncategorized). Same cascade, typed.

5. **Tests**
   - Schema: tables/indexes/constraints exist; FK rejects an orphan txn; CHECK
     rejects bad `type`.
   - Migration: seed the JSON stores with the end-to-end journey's data, run
     migrate, assert the typed tables match (counts, a sampled row, category
     mapping, budgets→items, dedup hashes preserved).
   - Behavior parity: the existing finance unit tests (balance, normalize,
     resolve_category, trends, recurring) pass against SQL, adapted.

## Edge cases & risks

- **Mutex re-entry deadlock** — the `with_conn` closure must not call back into
  `DataStore`. Documented + reviewed at each callsite.
- **Migration correctness is the main risk** — it's a one-shot data move. Mitigated
  by: non-destructive (JSON kept), idempotent guard, and a fidelity test over real
  journey data. If migration finds malformed legacy rows, it logs + skips (never
  aborts the app), matching today's lossy-default tolerance.
- **Behavior drift** — the rewrite must preserve exact semantics (positive amounts,
  sign-by-type, category cascade, dedup). The adapted unit tests are the guard.
- **`category_id` NULL vs "uncategorized"** — today "uncategorized" is a magic
  string; in typed storage it's `category_id IS NULL`. Aggregations that group by
  category must coalesce NULL → "uncategorized" for display. Noted.
- **No behavior change is visible to the user** — W5 is pure refactor of storage.
  The UI/agent contracts (`query`/`action` shapes) stay identical, so nothing
  downstream breaks. This is deliberate: ship the foundation invisibly, then W1/W2
  add value on top.
- **Single DB file shared with sessions** — `CREATE TABLE IF NOT EXISTS` + the same
  connection means no migration-version collision (the established rule).

## Acceptance criteria

- [ ] `cargo build -p zanto-desktop` green; `cargo test -p zanto-desktop` green.
- [ ] The `fin_*` tables exist with FK + CHECK + indexes; a FK-violating insert and
      a CHECK-violating insert both error.
- [ ] Migration: a workspace with legacy JSON finance data, after first finance
      access, has equivalent typed rows (transactions count + categories +
      accounts + budgets→items + goals + rules), JSON left intact, and re-running
      is a no-op.
- [ ] Behavior parity: the same `query("overview")` / `query("list_transactions")`
      / `action("add_transaction")` etc. return the SAME shapes/values as before
      (the prototype's mock shapes are the target the real backend will later match
      in W2/W3 — but W5 keeps TODAY's shapes).
- [ ] The end-to-end journey (onboarding → account → import → overview) produces
      correct categorized data over typed storage (the categorization itself still
      needs W1 — W5 just stores a real `category_id`).
- [ ] Existing finance unit tests pass (adapted to SQL).

## Manual test plan

1. `cargo test -p zanto-desktop finance` → schema, migration-fidelity, parity tests
   pass.
2. Build + run the real app (`zanto-desktop`), open Finance with a pre-existing
   JSON db → data still shows (migrated), add/edit a transaction → persists in
   `fin_transactions`; inspect `zanto.db` to confirm typed rows + the JSON stores
   untouched.
3. Re-open → no re-migration (marker honored).
