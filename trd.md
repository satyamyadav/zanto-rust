# zanto-core — Technical Reference Document

## Overview

`zanto-core` is a headless Rust binary (and future library) for an AI assistant that orchestrates LLM chat with filesystem and system tools. It is intentionally **Tauri-independent** so the same core can be used as:

- CLI binary (current)
- TUI (ratatui)
- HTTP API (axum)
- MCP server (exposed via rmcp)
- Tauri backend (IPC events)

---

## Crate Philosophy

Prefer small, focused crates over heavy frameworks. Examples:
- `walkdir` + `globset` for filesystem search (not a full fs abstraction)
- `axum` if an HTTP layer is needed (not a full web framework)
- `ratatui` for TUI (not a full application framework)
- Avoid re-implementing what a crate already does well

---

## Key Dependencies

| Crate | Role |
|---|---|
| `genai 0.6` | LLM client — talks to Ollama, sends chat + tool schemas, receives tool calls |
| `rmcp 1.7` | MCP framework — defines tools via `ToolBase`/`AsyncTool`, provides `ToolRouter` |
| `schemars` | JSON schema generation for tool parameters (driven by `#[derive(JsonSchema)]`) |
| `serde` / `serde_json` | Serialization for tool arguments |
| `tokio` (full) | Async runtime |
| `walkdir` | Recursive directory traversal |
| `globset` | Glob pattern matching |
| `futures` | `join_all` for concurrent tool execution |

LLM endpoint: `http://192.168.1.66:11434/` (Ollama, model `qwen2.5:14b`)

---

## Source Structure

```
crates/zanto-core/src/
├── main.rs          — entrypoint, calls chat::chat()
├── chat.rs          — multi-turn orchestration loop
└── tools/
    ├── mod.rs       — generic aggregator: all_tools(), dispatch(), is_readonly()
    └── fs/
        ├── mod.rs             — FsTools struct, ToolRouter, ServerHandler, schemas(), dispatch(), is_readonly()
        ├── list_directory.rs  — ListDirectory tool (readonly)
        ├── read_file.rs       — ReadFile tool (readonly)
        ├── write_file.rs      — WriteFile tool (mutating)
        └── search_files.rs    — SearchFiles tool (readonly)
```

---

## Tool Architecture

### Single source of truth

Tool schemas sent to the LLM are derived from `rmcp`'s `ToolRouter::list_all()`. No separate schema definitions exist — `ToolBase` (name, description, `#[derive(JsonSchema)]` on `Args`) is the only definition needed.

### Tool file contract

Each tool file is fully self-contained and contains exactly:

```rust
// Args struct — serde + JsonSchema, fields annotated with #[schemars(description = "...")]
pub struct Args { ... }

// Marker struct
pub struct MyTool;

// Static metadata
impl ToolBase for MyTool {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> { "my_tool".into() }
    fn description() -> Option<Cow<'static, str>> { Some("...".into()) }
    fn output_schema() -> Option<Arc<JsonObject>> { None }  // always None for String output
}

// Business logic
impl AsyncTool<super::FsTools> for MyTool {
    async fn invoke(_: &super::FsTools, args: Args) -> Result<String, ErrorData> { ... }
}
```

### Registering a new tool (3 lines in `fs/mod.rs`)

```rust
// 1. Declare module
pub mod my_tool;

// 2. Add to router
.with_async_tool::<my_tool::MyTool>()

// 3. Add to dispatch macro
try_invoke!(my_tool::MyTool);
```

If the tool is read-only, also add it to `is_readonly()` in `fs/mod.rs`.

### Tool categories

Each category (`fs`, future: `web`, `shell`) has its own subdirectory with:
- a service struct (e.g. `FsTools`) — stateless, implements `ServerHandler` for MCP use
- `tool_router()` — builds the `ToolRouter<FsTools>` from registered tools
- `schemas()` — converts rmcp tool definitions to genai `Tool` format
- `dispatch()` — routes a tool call by name using the `try_invoke!` macro
- `is_readonly()` — classifies tools for the execution scheduler

`tools/mod.rs` aggregates across all categories.

---

## Chat Orchestration (`chat.rs`)

Multi-turn loop:

1. Send system prompt + user message + all tool schemas to the LLM
2. If the LLM responds with tool calls, execute them (see below), then send results back
3. Receive final text answer

### Tool call execution — read/write ordering

Tool calls returned in a single LLM turn are executed with the following rules to preserve correctness when reads and writes are mixed:

- **Read-only tools** (`list_directory`, `read_file`, `search_files`): batched and run **concurrently** via `futures::join_all`
- **Mutating tools** (`write_file`): run **sequentially**, one at a time

Order is preserved: calls are processed in the order the model returned them. A batch of reads is flushed (and awaited) before any mutating call executes.

```
Model returns: [search_files, write_file, read_file]

→ flush [search_files] as concurrent batch   (completes)
→ execute write_file sequentially            (completes)
→ flush [read_file] as concurrent batch      (completes)
```

This prevents a read observing stale state from a write that was requested in the same turn.

### Marking a tool read-only

Override `is_readonly()` in the category's `mod.rs`. No changes needed in tool files themselves.

---

## Filesystem Access

Unrestricted — no path sandboxing. All paths accepted as-is from the LLM. This mirrors the Claude Code / agentic assistant model where the user is expected to trust the agent with full filesystem access.

Future: destructive operations (delete, overwrite) may get a confirmation step, not a path restriction.

---

## Future Roadmap

| Item | Notes |
|---|---|
| `edit_file` tool | Targeted line/block editing without full rewrite |
| `shell` tool category | Run bash commands, capture stdout/stderr |
| Confirmation flow | Human-in-the-loop for destructive mutations |
| Multi-turn loop | Extend chat.rs beyond 2 turns; loop until no tool calls |
| `lib` target | Expose `chat()` and tool dispatch as a library for Tauri / HTTP / TUI frontends |
| MCP server mode | Use `rmcp` to expose tools over stdio/SSE for external MCP clients |
| Additional LLM models | Swap model via config; support Claude via genai's Anthropic adapter |
