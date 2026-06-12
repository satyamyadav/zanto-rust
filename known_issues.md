# Known Issues

## ToolService::dispatch — category fallthrough relies on Err for routing

**Priority:** P2

**Location:** `crates/zanto-core/src/tools/mod.rs` — `ToolService::dispatch()`

Dispatch tries `fs::dispatch` first and falls through to `shell::dispatch` only when fs returns `Err`. This works today only because category dispatchers return `Err` exclusively for *unknown tool name* — tool execution errors and invalid-argument errors are mapped to `Ok(format!("error: ..."))`. If any fs tool ever returns a real `Err` for a known tool, dispatch would wrongly retry it against shell and report a misleading "unknown tool" or mis-route.

**Fix:** Route by tool name explicitly instead of by `Err` fallthrough — e.g. each category exposes `fn owns(name: &str) -> bool`, and `dispatch` selects the owning category up front. Removes the dependency on error semantics for control flow.

## run_command — read-only commands prompt for write permission

**Priority:** P2

**Location:** `crates/zanto-core/src/tools/shell/` — `run_command` + `ShellTools::is_readonly`

`run_command` is classified as always-mutating, so every shell invocation calls `permissions.check(.., Op::Write)`. Read-only commands (`git status`, `git log`, `pacman -Qi`, `whoami`, `git remote -v`, `ls`, `cat`) therefore trigger a *write*-permission prompt. Observed in the 13 June 2026 Gemini run: ~10 of the approval prompts were for inspect-only commands. Heavy, misleading friction — the user is asked to grant write access to merely read state.

The tool cannot statically know whether an arbitrary `sh -c` string mutates, so blanket-mutating is the safe default — but it over-prompts badly.

**Fix:** Add a read-only command-prefix allowlist (e.g. `git status|log|diff|show|ls-remote`, `pacman -Q*`, `ls`, `cat`, `whoami`, `pwd`, `echo`). When the command's leading token(s) match, route through `Op::Read` instead of `Op::Write`. Heuristic and conservative — unmatched commands stay mutating. Note in `known_issues` that this is best-effort, not a security boundary (a read-classified command could still mutate if crafted; acceptable since the user already controls the prompt content). A stricter alternative: separate `run_command` (mutating) from a `run_query` read-only tool and let the model pick — but that pushes classification onto the model.

## approver — non-exact input falls through to deny

**Priority:** P3

**Location:** `crates/zanto-cli/src/main.rs` — `StdinApprover::confirm`

Any response that isn't exactly `f`/`s`/`a` is treated as `Deny` (the `_ =>` arm). A stray character (observed: `a\` from a typo) silently denies and costs a turn. Consider trimming/normalising input and re-prompting on unrecognised entries instead of denying.

## flush_parallel — concurrent permission prompts race on stdin

**Priority:** P3

**Location:** `crates/zanto-core/src/chat.rs` — `flush_parallel()`

When a batch of read-only tool calls runs concurrently via `join_all`, each call hits `PermissionGuard::check()` simultaneously. If two or more calls target the same un-granted path, multiple `approver.confirm()` calls race on stdin — the user sees duplicate prompts and one read consumes the other's input.

**Partial mitigation:** `AllowSession` / `AllowForever` grants are written to `session_grants` before the approver returns, so a second waiter that starts slightly later will hit the cache. The race window is narrow but real under concurrent access.

**Fix:** Acquire a per-path async lock (e.g. `tokio::sync::Mutex` keyed by canonical path in a `DashMap<PathBuf, Arc<Mutex<()>>>`) before entering the prompt. The second waiter blocks until the first resolves and then hits `session_grants` instead of prompting again.
