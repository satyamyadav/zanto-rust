# Known Issues

_No open issues._

## Resolved

- **P2 — `ToolService::dispatch` Err-fallthrough routing.** Now routes by explicit
  `fs::owns(name)` / `shell::owns(name)` instead of relying on `Err` meaning
  "unknown tool". (`crates/zanto-core/src/tools/mod.rs`)
- **P2 — `run_command` read-only commands prompted for write.** `classify_op`
  gates read-only commands (`git status`, `pacman -Qi`, `ls`, …) as `Op::Read`;
  compound/redirected commands and anything off the allowlist stay `Op::Write`.
  Best-effort, not a security boundary. (`crates/zanto-core/src/tools/shell/run_command.rs`)
- **P3 — approver denied on non-exact input.** `StdinApprover::confirm` now loops:
  trims/normalizes input, matches the first char of `a/s/f/d`, re-prompts on
  unrecognized entries, denies only on EOF/error. (`crates/zanto-cli/src/main.rs`)
- **P3 — `flush_parallel` concurrent prompt race on stdin.** `PermissionGuard`
  serializes interactive prompts via a `tokio::sync::Mutex` and re-checks the grant
  cache after acquiring it, so a second waiter for the same path hits the cache
  instead of double-prompting. (`crates/zanto-core/src/permissions.rs`)
