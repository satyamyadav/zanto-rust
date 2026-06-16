# Architecture — Module Map

Every module in both crates, its role, and key public types. Current state.

## zanto-core

Declared in [lib.rs](../../crates/zanto-core/src/lib.rs):

```rust
pub mod chat;
pub mod config;
pub mod permissions;
pub mod session;
pub mod tools;
```

### `chat` — [chat.rs](../../crates/zanto-core/src/chat.rs)

The orchestration loop. Frontend-agnostic.

| Item | Role |
|---|---|
| `ChatConfig { model, endpoint, permissions }` | Inputs the caller supplies per run |
| `chat(config, store, session, question, policy) -> Result<String, _>` | The multi-turn loop; returns final assistant text |
| `push_msg(store, session, msg)` | Append to `session.messages` **and** persist — the only way messages enter history |
| `execute_tool_calls(...)` | Read/write ordering: batch reads, serialize writes |
| `flush_parallel(...)` | Run a read-only batch concurrently via `join_all` |
| `extract_raw_tool_calls(text) -> Vec<ToolCall>` | Fallback parser: scans assistant text for `{"name":..,"arguments":..}` when genai didn't parse a tool call (qwen via Ollama) |

### `config` — [config.rs](../../crates/zanto-core/src/config.rs)

Dual-layer JSON settings.

| Item | Role |
|---|---|
| `Settings { allowed_paths, allow_read_outside, allow_write_outside, model, endpoint, max_context_turns }` | Merged config |
| `Settings::load()` | ensure project config → load user + project → `merge` → `resolve_paths` |
| `Settings::persist_allowed_path(abs)` | Append an absolute path to project config (used by `AllowForever`) |
| `PROJECT_CONFIG = ".zanto/settings.json"` | Project config path constant |

Private: `ensure_project_config`, `user_path` (XDG), `load_file`, `resolve_paths`
(canonicalize each allowed path), `merge`.

### `permissions` — [permissions.rs](../../crates/zanto-core/src/permissions.rs)

The human-in-the-loop gate. UI-agnostic via the `Approver` trait.

| Item | Role |
|---|---|
| `trait Approver { async fn confirm(path, op, resolved) -> ApprovalResponse }` | Injected by the frontend |
| `enum ApprovalResponse { AllowOnce, AllowSession, AllowForever, Deny }` | User's answer |
| `enum Op { Read, Write }` | Operation class |
| `PermissionGuard` | Holds `allowed: Vec<PathBuf>`, bypass flags, `Arc<dyn Approver>`, `Mutex<HashSet<PathBuf>>` session grants |
| `PermissionGuard::new<A: Approver>(settings, approver)` | Constructor |
| `PermissionGuard::check(path, op) -> Result<PathBuf, String>` | The decision; returns the resolved absolute path |

Private: `is_allowed` (prefix match), `expand_tilde`, `resolve` (tilde + canonicalize).

### `session` — [session.rs](../../crates/zanto-core/src/session.rs)

Persistence + context window.

| Item | Role |
|---|---|
| `Session { id, title, workspace, created_at, updated_at, messages }` | A conversation |
| `SessionMeta { …, message_count }` | List-view row (no messages loaded) |
| `enum ContextPolicy { All, LastNTurns { max_turns } }` | Trimming policy; `default()` = 20 turns |
| `Session::new(title, workspace)` | Fresh session with generated id |
| `Session::effective_messages(policy)` | Messages to send (trimmed, no system msg) |
| `auto_title(messages)` | First 60 chars of first user message |
| `Store` | `Mutex<Connection>`; `unsafe impl Sync` |
| `Store::open()` / `open_at(path)` | `$ZANTO_DB` or app-data; runs migrations |
| `Store::{save_session, append_message, load_session, delete_session, list_sessions, last_session_id, find_by_prefix, clear}` | CRUD |
| `db_path()`, `unix_now_pub()`, `format_ts_display(secs)` | Helpers |

Private: `migrations()` (rusqlite_migration), `trim_to_turns`, `role_str`,
`new_id` (timestamp + uuid suffix), `format_ts`, `is_leap`.

### `tools` — [tools/mod.rs](../../crates/zanto-core/src/tools/mod.rs)

Aggregates tool categories. See [tools.md](tools.md) for the contract.

| Item | Role |
|---|---|
| `ToolService { fs, shell }` | Holds both categories |
| `ToolService::new(permissions)` | Clones the `Arc` into each category |
| `all_tools() -> Vec<GenaiTool>` | fs schemas ++ shell schemas |
| `dispatch(name, args)` | fs::dispatch, else shell::dispatch |
| `is_readonly(name)` | fs OR shell readonly classification |

#### `tools::fs` — [tools/fs/mod.rs](../../crates/zanto-core/src/tools/fs/mod.rs)

`FsTools { permissions }`. Tools: `list_directory`, `read_file`, `search_files`
(readonly); `write_file`, `edit_file` (mutating). Each tool is its own file
implementing `ToolBase` + `AsyncTool<FsTools>`. Dispatch via the `try_invoke!`
macro. `is_readonly` lists the three read tools.

#### `tools::shell` — [tools/shell/mod.rs](../../crates/zanto-core/src/tools/shell/mod.rs)

`ShellTools { permissions }`. Tool: `run_command` (always mutating —
`is_readonly` returns `false` for all). Runs `sh -c <command>` in
`tokio::task::spawn_blocking`, returns `exit N\n<stdout>\n[stderr]\n<stderr>`.

## zanto-cli

Single file: [main.rs](../../crates/zanto-cli/src/main.rs).

| Item | Role |
|---|---|
| `Cli` (clap derive) | Flags + optional `sessions` subcommand |
| `Commands::Sessions { action }` | `List{all}`, `Show{id}`, `Delete{id}`, `Clear{all}` |
| `StdinApprover : Approver` | Prompts on stderr, reads stdin: `a`/`s`/`f`/`d` |
| `main()` | Bootstrap → resolve session → chat → finalize |
| `resolve_session(...)` | new / by-prefix / last / fresh |
| `finalize_session(...)` | auto-title + save |
| `run_once`, `run_interactive` | One-shot vs REPL |
| `handle_sessions(action, workspace)` | Subcommand dispatch |
| `resolve_id`, `truncate` | Helpers |

## Test modules

| Location | Coverage |
|---|---|
| `session.rs` `#[cfg(test)]` | Store CRUD, prefix lookup, context trimming, auto-title (12) |
| `permissions.rs` `#[cfg(test)]` | tilde expand, allow/deny, grant caching (5) |
| [zanto-cli/tests/e2e.rs](../../crates/zanto-cli/tests/e2e.rs) | 7 `#[ignore]` e2e tests, real Ollama, `ZANTO_DB` isolation |

Totals: 17 unit (always run) + 7 e2e (`--include-ignored`, live Ollama).
