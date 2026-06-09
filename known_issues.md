# Known Issues

## flush_parallel — concurrent permission prompts race on stdin

**Location:** `crates/zanto-core/src/chat.rs` — `flush_parallel()`

When a batch of read-only tool calls runs concurrently via `join_all`, each call hits `PermissionGuard::check()` simultaneously. If two or more calls target the same un-granted path, multiple `approver.confirm()` calls race on stdin — the user sees duplicate prompts and one read consumes the other's input.

**Partial mitigation:** `AllowSession` / `AllowForever` grants are written to `session_grants` before the approver returns, so a second waiter that starts slightly later will hit the cache. The race window is narrow but real under concurrent access.

**Fix:** Acquire a per-path async lock (e.g. `tokio::sync::Mutex` keyed by canonical path in a `DashMap<PathBuf, Arc<Mutex<()>>>`) before entering the prompt. The second waiter blocks until the first resolves and then hits `session_grants` instead of prompting again.
