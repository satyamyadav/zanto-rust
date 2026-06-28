# A2 — Session schema & lifecycle foundation

- **Date:** 2026-06-17
- **Wave:** A (core foundations), batch 1 (∥ A1)
- **Owner of:** `crates/zanto-core/src/session.rs`

## Summary
Extend the session store for the roadmap's session features: `archived` flag,
`summary` text, and a per-message `metadata` JSON column (for D1 artifact/decision
persistence). Add Store lifecycle methods and a **system-info builder** (consumed by
A4). Filesystem-only changes are append-only migrations — no collision with A3.

## Affected files
- `crates/zanto-core/src/session.rs` — migrations, struct fields, Store methods, builder.

## Design

### Migrations (append to the existing `vec!`, after the `app_id` migration)
```sql
ALTER TABLE sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN summary  TEXT;
ALTER TABLE messages ADD COLUMN metadata TEXT;   -- JSON blob, nullable
```
Keep each as a separate `M::up(...)` (migration versions 3, 4, 5).

### Struct changes
- `Session`: add `pub archived: bool`, `pub summary: Option<String>`.
  `Session::new` sets `archived: false`, `summary: None`.
- `SessionMeta`: add `pub archived: bool`.
- `save_session` upsert: include `archived`, `summary` columns.
- `load_session`: hydrate `archived`/`summary`.

### Per-message metadata
- Add `pub fn append_message_meta(&self, session_id, pos, msg: &ChatMessage, metadata: Option<&Value>) -> Result<()>`.
  Existing `append_message` delegates with `metadata = None` (unchanged callers).
- Add `pub fn load_message_meta(&self, session_id) -> Result<Vec<Option<Value>>>`
  (positional, parallel to messages) for D1 restore. Do **not** change
  `Session.messages` type.

### Lifecycle methods (Store)
```rust
pub fn set_archived(&self, id: &str, archived: bool) -> Result<()>;
pub fn set_summary(&self, id: &str, summary: Option<&str>) -> Result<()>;
```
- `list_sessions(workspace, app)` — **exclude archived by default** (`WHERE archived = 0`).
- Add `list_sessions_archived(workspace, app)` (`WHERE archived = 1`) so the desktop
  archive view (D2) has a source. Signature of the existing `list_sessions` is otherwise
  unchanged (avoid rippling into desktop mid-wave).

### System-info builder
```rust
pub fn system_info() -> String   // OS, arch, cwd, shell ($SHELL), today's date
```
Returns a short block (≤ ~6 lines) like:
`System: linux x86_64 · cwd: /… · shell: /bin/zsh · date: 2026-06-17`.
A4 prepends this to the system prompt at session start.

## Acceptance checks
- `cargo build` clean; existing 28 tests pass (migrations apply over an existing DB).
- New tests: archive/unarchive round-trip; `set_summary` persists + loads;
  `append_message_meta` + `load_message_meta` round-trip a JSON value;
  `list_sessions` hides archived, `list_sessions_archived` shows only archived;
  `system_info()` is non-empty and contains the date.

## Notes / handoff
- A4 calls `session::system_info()`. D1 uses `append_message_meta`/`load_message_meta`.
  D2 uses `set_archived`/`list_sessions_archived`. D3 uses `set_summary`.
