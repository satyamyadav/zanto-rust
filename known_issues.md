# Known Issues

## Open

All pre-existing (not introduced by the genai-provider-settings work); surfaced
during that branch's reviews and deferred as separate work.

- **P1 — UTF-8 byte-slice panic on non-ASCII tool output.** `chat.rs:512` and
  `chat.rs:750` do `&output[..output.len().min(120)]`, slicing by **byte**
  index. Tool output whose UTF-8 encoding straddles byte 120 (emoji, CJK,
  accented path) panics with "byte index 120 is not a char boundary". Fix: use a
  char-boundary-safe truncation (e.g. `output.chars().take(120).collect()`).
  Most worth scheduling — a hard panic on real input.
- **P2 — `extract_raw_tool_calls` brace parser is not string-quoting aware.**
  `chat.rs:760-781` matches `{`/`}` with a depth counter that ignores braces
  inside JSON string values, so a tool call whose arguments contain a `}` in a
  string (e.g. `{"cmd":"echo }"}`) terminates early, fails to parse, and is
  silently dropped — the turn ends as if no tool was called. Fix: track string/
  escape state, or prefer the structured tool-call path over raw text extraction.
- **P2 — `Box::leak(endpoint)` per `chat()` call.** `chat.rs:240` leaks one heap
  allocation for the endpoint string every turn (needed as `'static` for the
  resolver closure). Unbounded growth across a long interactive/desktop session.
  Fix: own the endpoint via the resolver's captured state instead of leaking, or
  cache the leaked `&'static str` per distinct endpoint.
- **P3 — `send_message` loads settings three times.** `ipc/chat.rs:139, 221,
  224` each call `Settings::load()` (disk read + `ensure_project_config`), opening
  an inconsistency window if the file changes mid-turn. Fix: reuse the binding
  from line 139.

## Backlog (non-bug)

Deferred features and code-quality cleanups live in [docs/backlog.md](docs/backlog.md).

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
