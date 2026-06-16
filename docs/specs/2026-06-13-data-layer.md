# Data Layer — workspace stores, schemaless records, structured queries

- **Date:** 2026-06-13

## Summary
A workspace-scoped data layer in `zanto-core`: a **data engine** (ungated library
API) that sub-apps/backend flows call directly, and **gated LLM tools** layered on
top for the model. Records are schemaless JSON; queries are a structured filter
API. Storage is logical "stores" (friendly-named) in the existing single
`zanto.db`.

## Motivation
The finance vertical — and every future sub-app — needs to persist and query
structured data (transactions, categories, budgets) without exposing SQLite to the
user. Two distinct callers need it:
1. **Internal features / static flows / backend APIs** — must call the store
   directly, as a backend, **ungated** (no HITL prompt per row; bulk import of 247
   transactions can't mean 247 prompts).
2. **LLM agents** — when the model is given generic data tools, those calls **are**
   gated through the existing `Approver`.

This is the core design decision from review: **the data engine is ungated; the
LLM-facing tools are a gated wrapper over it.** Same engine, two access paths.

## Scope

**In scope (Phase 1 — data engine, this spec's primary deliverable):**
- `zanto-core::data` module: `DataStore` engine over `zanto.db`.
- Schemaless JSON record storage, workspace-scoped logical stores.
- Structured filter query API (no raw SQL exposed).
- Store registry so `list_stores` works and friendly names map to physical tables.
- Unit tests (no LLM).

**In scope (Phase 2 — gated LLM tools, specced here, built after Phase 1 lands):**
- `tools/data/` category: `create_store`, `insert_record`, `query_store`,
  `list_stores` — thin wrappers over the engine, **gated**.
- A gating mechanism for non-filesystem resources (data ops aren't paths).

**Out of scope:**
- The finance skill, import parsers (CSV/PDF/Excel), artifacts/canvas. Separate specs.
- Update/delete records, schema migration of existing records, indexes/perf tuning.
  (Add when a vertical needs them.)
- Cross-workspace queries.

## Affected files
- `crates/zanto-core/src/lib.rs` — add `pub mod data;`
- `crates/zanto-core/src/data/mod.rs` — new: `DataStore`, `Query`, `Filter`, errors
- `crates/zanto-core/src/data/query.rs` — new: filter → SQL translation (or inline in mod)
- `crates/zanto-core/src/session.rs` — reuse `Store`'s DB path / connection (see step 1)
- *(Phase 2)* `crates/zanto-core/src/tools/data/{mod,create_store,insert_record,query_store,list_stores}.rs`
- *(Phase 2)* `crates/zanto-core/src/tools/mod.rs` — register the `data` category
- *(Phase 2)* `crates/zanto-core/src/permissions.rs` — resource gating for data ops

## Design

### Storage model
Single physical DB (`zanto.db`, the same file sessions use — honoring "single db
file, different logical stores"). Each logical store is one table:

```sql
CREATE TABLE <physical_table> (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    data        TEXT NOT NULL,          -- a JSON object (schemaless record)
    created_at  INTEGER NOT NULL
);
```

Records are arbitrary JSON objects (`serde_json::Value`). Custom fields and
user-controlled rearrangement come free — no migration to add a field.

### Workspace scoping + registry
Stores are workspace-scoped. A registry **meta-table** in `zanto.db` maps
`(workspace, friendly_name) → physical_table`:

```sql
CREATE TABLE IF NOT EXISTS data_stores (
    workspace   TEXT NOT NULL,
    name        TEXT NOT NULL,         -- friendly: "transactions"
    table_name  TEXT NOT NULL,         -- physical: "ds_<hash>_transactions"
    created_at  INTEGER NOT NULL,
    UNIQUE(workspace, name)
);
```

`physical_table = "ds_" + short_hash(workspace) + "_" + sanitized(name)`.
Users/model only ever see the friendly `name`; physical tables stay hidden.

> **Open decision for approval:** the earlier note said "keep the map in local
> config (`.zanto/settings.json`)." This spec instead keeps the registry **in the
> DB** (`data_stores` meta-table) so the mapping can't drift from the data and stays
> atomic with it (the same "don't keep state in two places" principle as the
> sessions design). If you'd rather have it in `.zanto/`, say so and I'll switch.

### Query model (structured filter API)
No raw SQL is exposed. A query is filters (AND-combined) + optional sort + limit:

```rust
pub struct Query {
    pub filters: Vec<Filter>,
    pub sort: Option<Sort>,         // field + Asc/Desc
    pub limit: Option<usize>,
}
pub struct Filter { pub field: String, pub op: FilterOp, pub value: Value }
pub enum FilterOp { Eq, Ne, Gt, Gte, Lt, Lte, Contains }  // Contains = substring on text
pub struct Sort { pub field: String, pub dir: Dir }
```

`field` is a top-level JSON key (e.g. `"category"`, `"amount"`). The engine
translates to SQL via `json_extract(data, '$.<field>')`, binding `value` as a
parameter (no string interpolation → no injection). `Gt/Gte/Lt/Lte` compare
numerically when the value is a number, lexically otherwise. Unknown fields match
nothing (json_extract returns NULL), not an error.

### Engine API (ungated — the backend the sub-apps use)
```rust
impl DataStore {
    pub fn open(workspace: &str) -> Result<Self, DataError>;     // reuses zanto.db path
    pub fn create_store(&self, name: &str) -> Result<(), DataError>;
    pub fn list_stores(&self) -> Result<Vec<String>, DataError>; // friendly names
    pub fn insert(&self, store: &str, record: &Value) -> Result<i64, DataError>; // returns id
    pub fn query(&self, store: &str, q: &Query) -> Result<Vec<Record>, DataError>;
}
pub struct Record { pub id: i64, pub data: Value, pub created_at: u64 }
```
Internal features call these directly. **No `Approver`, no permission check** —
this is the "works as backend" path.

### LLM tools (Phase 2 — gated wrappers)
`tools/data/` mirrors `tools/fs/`. Each tool calls the gate, then the engine:

```rust
async fn invoke(svc, args) -> Result<String, ErrorData> {
    svc.permissions.check_data(&args.store, DataOp::Write|Read).await?;  // see below
    svc.data.insert(&args.store, &args.record) ...
}
```

Gating mechanism for non-path resources — minimal extension of the existing gate:
- Add `Op::Data` (or a parallel `check_resource(resource: &str, op)`). The
  `Approver::confirm(path, op, resolved)` signature already takes strings, so the
  store name is passed as the "resource"; a separate `HashSet<String>` holds data
  grants (mirrors `session_grants`). `AllowSession`/`AllowForever` behave as today.
- Internal features bypass entirely by calling the engine, not the tools.

> The gate detail is deliberately light here — Phase 2 will spec it precisely once
> Phase 1 (the engine) is in and the finance internal flow proves the API shape.

## Edge cases & risks
- **Store name → table sanitization:** names must be sanitized (alnum + `_`) before
  forming a table name; reject/normalize others to prevent SQL identifier issues.
  Table names come only from `create_store`, never directly from a query.
- **Same DB as sessions:** data tables share `zanto.db` with `sessions`/`messages`.
  Acceptable per the single-file decision; the `ds_` / `data_stores` prefixes avoid
  collision. WAL already on.
- **`json_extract` numeric vs text compare:** documented above; numeric compares
  rely on the stored JSON being a real number, not a string. The finance import
  must insert `amount` as a number.
- **No update/delete yet:** the finance "recategorize" flow will need update —
  flagged as the first likely follow-on, intentionally out of this spec.
- **Concurrency:** `DataStore` uses the same `Mutex<Connection>` pattern as `Store`.

## Acceptance criteria
- [ ] `cargo test -p zanto-core` — new engine unit tests green, existing 17 still green
- [ ] Create a store, insert 3 JSON records, query with an `Eq` filter → returns the
      matching record(s) only
- [ ] Query with `Gt` on a numeric field returns numerically-correct results
- [ ] `list_stores` returns friendly names for the current workspace only
- [ ] Two workspaces with the same store name don't collide (different physical tables)
- [ ] No raw SQL string is ever built from user/model input (all values bound as params)
- [ ] Engine API requires no `Approver` (compiles and runs with no permission plumbing)

## Manual test plan
Phase 1 is library-level; exercised via unit tests (no CLI surface yet). A throwaway
example or test asserts:
1. `DataStore::open("/ws/a")` → `create_store("transactions")`
2. `insert("transactions", {"merchant":"DMart","amount":4200,"category":"groceries"})` ×3 with varied values
3. `query("transactions", filters=[{field:"category",op:Eq,value:"groceries"}])` → correct subset
4. `query` with `{field:"amount",op:Gt,value:1000}` → numeric filter correct
5. `list_stores()` → `["transactions"]`; a second workspace sees none of these
```
