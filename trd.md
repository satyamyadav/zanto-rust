# zanto — Technical Reference Document

## Overview

`zanto` is an AI assistant that orchestrates LLM chat with filesystem tools. It is split into two crates:

- **`zanto-core`** — pure library: chat orchestration, tool definitions, permission guard, session storage
- **`zanto-cli`** — binary frontend: CLI flags, interactive REPL, `StdinApprover`

The library is intentionally **Tauri-independent** so the same core can be used by:
- CLI (current)
- TUI (ratatui)
- HTTP API (axum)
- Tauri backend (IPC events)

---

## Key Dependencies

| Crate | Role |
|---|---|
| `genai 0.6` | LLM client — Ollama, sends chat + tool schemas, receives tool calls |
| `rmcp 1.7` | MCP framework — `ToolBase`/`AsyncTool`/`ToolRouter` for tool definitions and schema derivation |
| `schemars` | JSON schema generation from `#[derive(JsonSchema)]` on `Args` structs |
| `serde` / `serde_json` | Tool argument serialization; message content persistence |
| `tokio` (full) | Async runtime |
| `walkdir` + `globset` | Recursive directory search |
| `futures` | `join_all` for concurrent read tool execution |
| `rusqlite 0.40` (bundled) | SQLite — compiled in, no system dependency |
| `rusqlite_migration 2.5` | Versioned schema migrations over the bundled SQLite connection |
| `directories 6` | OS-conventional app-data path resolution via `ProjectDirs` |
| `uuid 1` (v4) | Random suffix for session IDs |
| `async-trait` | `Approver` trait with async methods |

Default LLM: `qwen2.5:14b` at `http://192.168.1.66:11434/` (configurable via settings or CLI flag).

---

## Source Structure

```
crates/
├── zanto-core/src/
│   ├── lib.rs           — module declarations
│   ├── chat.rs          — multi-turn orchestration loop
│   ├── config.rs        — Settings (dual-layer JSON), path resolution
│   ├── permissions.rs   — PermissionGuard, Approver trait, tilde expansion
│   ├── session.rs       — Store (SQLite), Session, ContextPolicy, SessionMeta
│   └── tools/
│       ├── mod.rs                 — ToolService: aggregates categories
│       └── fs/
│           ├── mod.rs             — FsTools, ToolRouter, dispatch(), is_readonly()
│           ├── list_directory.rs  — ListDirectory (readonly)
│           ├── read_file.rs       — ReadFile (readonly)
│           ├── write_file.rs      — WriteFile (mutating)
│           └── search_files.rs    — SearchFiles (readonly)
└── zanto-cli/src/
    └── main.rs          — Cli struct, StdinApprover, session lifecycle, subcommands
```

---

## Configuration

Settings are loaded from two JSON files, merged, then path-resolved:

1. **User config** (`$XDG_CONFIG_HOME/zanto/settings.json` or `~/.config/zanto/settings.json`)
2. **Project config** (`.zanto/settings.json` — auto-created on first run)

Project config takes precedence over user config. All fields are optional:

```json
{
  "allowed_paths": ["/home/user/projects/myproject"],
  "allow_read_outside": false,
  "allow_write_outside": false,
  "model": "qwen2.5:14b",
  "endpoint": "http://192.168.1.66:11434/",
  "max_context_turns": 20
}
```

`allowed_paths` entries are canonicalized to absolute paths at load time. `AllowForever` approval writes back to the project config via `Settings::persist_allowed_path()`.

---

## Permission System

### Approver trait

```rust
#[async_trait]
pub trait Approver: Send + Sync {
    async fn confirm(&self, path: &str, op: &str, resolved: &str) -> ApprovalResponse;
}
pub enum ApprovalResponse { AllowOnce, AllowSession, AllowForever, Deny }
```

`StdinApprover` is implemented in `zanto-cli`. Other frontends (Tauri, TUI, HTTP) inject their own `Approver` implementation.

### PermissionGuard

`PermissionGuard::check(path, op)` returns `Result<PathBuf, String>` — the resolved absolute path on success. Tools use this `PathBuf` for all filesystem operations (not the original string from the model).

Check sequence:
1. `allow_read_outside` / `allow_write_outside` bypass → `Ok(resolved)`
2. Path is a prefix of any entry in `allowed_paths` → `Ok(resolved)`
3. Path is in `session_grants` (in-memory set) → `Ok(resolved)`
4. Call `approver.confirm()` inline (blocks in the same model turn)
   - `AllowOnce` → `Ok(resolved)`
   - `AllowSession` → add to `session_grants`, `Ok(resolved)`
   - `AllowForever` → add to `session_grants` + persist to project config, `Ok(resolved)`
   - `Deny` → `Err("permission denied: ...")`

### Path resolution

`resolve(path)` in `permissions.rs`:
1. Expands a leading `~` to `$HOME` (or `$USERPROFILE` on Windows)
2. Calls `std::fs::canonicalize()` — follows symlinks, resolves `.`/`..`
3. For paths that don't exist yet (e.g. a file about to be written): canonicalizes the parent and appends the filename

The resolved absolute path is what gets compared against `allowed_paths`, shown in the approval prompt, and passed to FS calls.

---

## Tool Architecture

### Single source of truth

Tool schemas sent to the LLM come from `ToolRouter::list_all()` → `rmcp_to_genai()` converter. No separate schema definitions exist.

### Tool file contract

Each tool is a self-contained file:

```rust
pub struct Args { ... }  // serde + JsonSchema, fields with #[schemars(description = "...")]
pub struct MyTool;

impl ToolBase for MyTool {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> { "my_tool".into() }
    fn description() -> Option<Cow<'static, str>> { Some("...".into()) }
    fn output_schema() -> Option<Arc<JsonObject>> { None }  // always None for String output
}

impl AsyncTool<super::FsTools> for MyTool {
    async fn invoke(svc: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        let resolved = svc.permissions.check(&args.path, Op::Read).await
            .map_err(|e| ErrorData::internal_error(e, None))?;
        // use `resolved` (PathBuf) for all FS calls, not args.path
    }
}
```

### Registering a new tool

Three lines in `fs/mod.rs`:
1. `pub mod my_tool;`
2. `.with_async_tool::<my_tool::MyTool>()` in `tool_router()`
3. `try_invoke!(my_tool::MyTool);` in `dispatch()`

If read-only, also add the name to `is_readonly()`.

---

## Chat Orchestration (`chat.rs`)

Signature:
```rust
pub async fn chat(
    config: ChatConfig,
    store: &Store,
    session: &mut Session,
    question: &str,
    policy: &ContextPolicy,
) -> Result<String, Box<dyn Error>>
```

Multi-turn loop:
1. Ensure session row exists in DB (`store.save_session`)
2. Append user message to `session.messages` and persist
3. Build context: `[system_prompt] + session.effective_messages(policy)`
4. Send to LLM with all tool schemas
5. If tool calls returned: execute them (see below), persist results, loop
6. If no tool calls: append assistant text, return it

System prompt is never stored in the DB — injected fresh each loop iteration.

### Tool call execution — read/write ordering

Tool calls from a single LLM turn are executed with these rules:

- **Read-only** (`list_directory`, `read_file`, `search_files`): batched and run **concurrently** via `join_all`
- **Mutating** (`write_file`): run **sequentially**

Batched reads are flushed before any mutation executes. Order from the model is preserved.

```
Model returns: [search_files, write_file, read_file]
→ flush [search_files] concurrently    (done)
→ execute write_file sequentially      (done)
→ flush [read_file] concurrently       (done)
```

---

## Session and History (`session.rs`)

### Storage

Single SQLite file at the OS-conventional app-data path:

| OS | Path |
|---|---|
| Linux | `$XDG_DATA_HOME/zanto/zanto.db` → `~/.local/share/zanto/zanto.db` |
| macOS | `~/Library/Application Support/zanto/zanto.db` |
| Windows | `%APPDATA%\zanto\zanto.db` |

WAL mode enabled (`PRAGMA journal_mode=WAL`) — concurrent readers, crash-safe.

Schema is managed via `rusqlite_migration` — versioned migrations applied with `Migrations::to_latest()`.

### Schema

```sql
sessions (id TEXT PK, title TEXT, workspace TEXT, created_at INTEGER, updated_at INTEGER)
messages (id INTEGER PK, session_id TEXT FK, position INTEGER, role TEXT, content TEXT)
  UNIQUE(session_id, position)
```

Messages are appended incrementally during the chat loop — history up to a crash is preserved.

### Session struct

```rust
pub struct Session {
    pub id: String,        // "20260610T143000-a3f2b8c1" — sortable, no chrono dep
    pub title: String,     // auto-generated from first user message
    pub workspace: String, // canonicalized CWD at creation — project-scoped filtering
    pub messages: Vec<ChatMessage>,
    ...
}
```

### Context trimming (ContextPolicy)

```rust
pub enum ContextPolicy {
    All,
    LastNTurns { max_turns: usize },  // default: 20
}
```

`LastNTurns` splits the message list at User-role boundaries into turns, takes the last N, and flattens. Tool call/response pairs stay intact within the same turn. System messages are excluded (injected fresh by caller).

---

## CLI (`zanto-cli`)

```
zanto [QUESTION]           # one-shot or interactive if no question
  -m, --model <MODEL>
  -e, --endpoint <URL>
  -s, --session <ID>       # resume by ID or prefix
  -n, --new                # force new session
  -t, --title <TITLE>      # title for new session

zanto sessions list [--all]
zanto sessions show <ID>
zanto sessions delete <ID>
zanto sessions clear [--all]
```

Default behavior: resumes the last session for the current workspace. `--all` spans all workspaces.

`StdinApprover` prompts on stderr (so stdout can be piped) and reads from stdin.

---

## Future Roadmap

| Item | Notes |
|---|---|
| `edit_file` tool | Targeted line/block editing without full rewrite |
| `shell` tool category | Run commands, capture stdout/stderr |
| Per-path async lock | Fix `flush_parallel` stdin race (see known_issues.md) |
| MCP server mode | Expose tools over stdio/SSE for external MCP clients |
| Additional LLM models | Claude via genai's Anthropic adapter |
| Tauri / TUI frontend | Inject custom `Approver` for native UI dialogs |
