# Known Issues

## flush_parallel — concurrent permission prompts race on stdin

**Location:** `crates/zanto-core/src/chat.rs` — `flush_parallel()`

When a batch of read-only tool calls runs concurrently via `join_all`, each call hits `PermissionGuard::check()` at the same time. If two or more calls target the same un-granted path, multiple prompts for the same path will race on stdin simultaneously — the user sees duplicate prompts and one stdin read will get the other's input.

**Fix:** acquire a per-path async lock (e.g. `tokio::sync::Mutex` keyed by canonical path, or a `DashMap<PathBuf, Arc<Mutex<()>>>`) before entering the prompt, so the second waiter blocks until the first resolves and then hits the session cache instead of prompting again.
