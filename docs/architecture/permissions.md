# Architecture — Permissions

The human-in-the-loop gate. Source: [permissions.rs](../../crates/zanto-core/src/permissions.rs).

## The contract

Every tool, before any filesystem or shell effect, calls:

```rust
let resolved: PathBuf = svc.permissions.check(&args.path, Op::Read|Write).await?;
// then operate on `resolved`, never on args.path
```

`check` returns the **resolved absolute path** on success. The tool uses that
`PathBuf` for the actual operation. This is the single rule that prevents a tool
from acting on an un-vetted, un-normalized model string.

## Approver — the UI seam

```rust
#[async_trait]
pub trait Approver: Send + Sync {
    async fn confirm(&self, path: &str, op: &str, resolved: &str) -> ApprovalResponse;
}
pub enum ApprovalResponse { AllowOnce, AllowSession, AllowForever, Deny }
```

`zanto-core` never reads stdin or prints a prompt. It calls `approver.confirm(...)`
and the frontend decides how to ask. The only implementor today is
`StdinApprover` in [main.rs](../../crates/zanto-cli/src/main.rs), which prints to
**stderr** (so stdout stays pipeable) and reads one line: `a`/`s`/`f`/`d`.

> Known issue (P3): any input that isn't exactly `f`/`s`/`a` is treated as `Deny`.
> A typo like `a\` silently denies. See `known_issues.md`.

## PermissionGuard.check — decision order

```
resolved = resolve(path)

1. bypass?     Op::Read  → allow_read_outside
               Op::Write → allow_write_outside
               if set → return Ok(resolved)          # no prompt at all

2. allowed?    resolved.starts_with(any allowed_path) → Ok(resolved)

3. granted?    resolved ∈ session_grants (in-memory) → Ok(resolved)

4. ask:        approver.confirm(path, op, resolved)
               AllowOnce    → Ok(resolved)                    # not remembered
               AllowSession → session_grants.insert; Ok       # this process only
               AllowForever → session_grants.insert
                              + Settings::persist_allowed_path # written to project config
                              → Ok(resolved)
               Deny         → Err("permission denied: <op> \"<path>\"")
```

`allowed` is a prefix match: granting `/home/lazy/dev` covers everything beneath
it. `session_grants` is a `Mutex<HashSet<PathBuf>>` keyed by the resolved path, so
a second access to the same path within the run skips the prompt.

### Grant lifetimes

| Response | This call | Rest of session | Future runs |
|---|---|---|---|
| AllowOnce | ✓ | re-prompts | re-prompts |
| AllowSession | ✓ | ✓ (in-memory) | re-prompts |
| AllowForever | ✓ | ✓ | ✓ (persisted to `.zanto/settings.json`) |
| Deny | ✗ → tool error | — | — |

## Path resolution — `resolve(path)`

1. `expand_tilde` — a leading `~`, `~/`, or `~\` expands to `$HOME` (or
   `$USERPROFILE`). Rust's stdlib does **not** expand `~`; without this,
   `~/Downloads` would fail to canonicalize and the tool would `ENOENT`. (This was
   a real bug — see the chrome story.)
2. `std::fs::canonicalize` — resolves symlinks, `.`/`..`, makes absolute.
3. If the path doesn't exist yet (e.g. a file about to be written): canonicalize
   the **parent** and append the filename, so writes to new files still produce a
   stable absolute path. If the parent is empty, use the current dir.

The resolved path is what's compared against `allowed_paths`, shown in the prompt,
cached in grants, and handed to the tool.

## Deny is not fatal

A `Deny` returns `Err(String)` from `check`, which the tool maps to an
`ErrorData`, which `dispatch`'s `try_invoke!` turns into `Ok("error: ...")`. So a
denied permission surfaces to the model as a tool-result error string — the model
can try a different path or command. The chat loop never crashes on a denial.

## Threading / concurrency note

Because read-only tools run concurrently ([stack-flow.md](stack-flow.md)),
multiple `check` calls can hit the approver at the same time. If two target the
same un-granted path they can race on stdin (duplicate prompts). Partially
mitigated by writing grants before the approver returns; full fix (per-path async
lock) is filed P3 in `known_issues.md`.
