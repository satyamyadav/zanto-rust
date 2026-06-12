# Known Issues

## ToolService::dispatch — category fallthrough relies on Err for routing

**Priority:** P2

**Location:** `crates/zanto-core/src/tools/mod.rs` — `ToolService::dispatch()`

Dispatch tries `fs::dispatch` first and falls through to `shell::dispatch` only when fs returns `Err`. This works today only because category dispatchers return `Err` exclusively for *unknown tool name* — tool execution errors and invalid-argument errors are mapped to `Ok(format!("error: ..."))`. If any fs tool ever returns a real `Err` for a known tool, dispatch would wrongly retry it against shell and report a misleading "unknown tool" or mis-route.

**Fix:** Route by tool name explicitly instead of by `Err` fallthrough — e.g. each category exposes `fn owns(name: &str) -> bool`, and `dispatch` selects the owning category up front. Removes the dependency on error semantics for control flow.

## flush_parallel — concurrent permission prompts race on stdin

**Priority:** P3

**Location:** `crates/zanto-core/src/chat.rs` — `flush_parallel()`

When a batch of read-only tool calls runs concurrently via `join_all`, each call hits `PermissionGuard::check()` simultaneously. If two or more calls target the same un-granted path, multiple `approver.confirm()` calls race on stdin — the user sees duplicate prompts and one read consumes the other's input.

**Partial mitigation:** `AllowSession` / `AllowForever` grants are written to `session_grants` before the approver returns, so a second waiter that starts slightly later will hit the cache. The race window is narrow but real under concurrent access.

**Fix:** Acquire a per-path async lock (e.g. `tokio::sync::Mutex` keyed by canonical path in a `DashMap<PathBuf, Arc<Mutex<()>>>`) before entering the prompt. The second waiter blocks until the first resolves and then hits `session_grants` instead of prompting again.
