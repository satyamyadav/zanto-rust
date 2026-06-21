# Known Issues

## Open

_No open issues._

## Backlog (non-bug)

Deferred features and code-quality cleanups live in [docs/backlog.md](docs/backlog.md).

## Resolved

- **P1 — UTF-8 byte-slice panic on non-ASCII tool output.** `log_preview()`
  truncates by char boundary; replaced `&output[..min(120)]` at both log sites.
  (`crates/zanto-core/src/chat.rs`)
- **P2 — tool-call brace parser not string-quoting aware.** `extract_raw_tool_calls`
  now tracks string/escape state, so a `}` inside a JSON string no longer drops the
  call. (`crates/zanto-core/src/chat.rs`)
- **P2 — `Box::leak(endpoint)` per `chat()` call.** The Ollama endpoint override is
  built once as an `Arc<str>`-backed `Endpoint` and cloned into the resolver — no
  per-turn leak. (`crates/zanto-core/src/chat.rs`)
- **P3 — `send_message` loaded settings three times.** Reuses the single per-turn
  `settings` snapshot for the context policy. (`crates/zanto-desktop/src-tauri/src/ipc/chat.rs`)
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
