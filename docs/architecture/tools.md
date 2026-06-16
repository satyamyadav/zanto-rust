# Architecture — Tools

The tool system: contract, registration, dispatch, and the read/write
classification. Sources: [tools/mod.rs](../../crates/zanto-core/src/tools/mod.rs),
[tools/fs/mod.rs](../../crates/zanto-core/src/tools/fs/mod.rs),
[tools/shell/mod.rs](../../crates/zanto-core/src/tools/shell/mod.rs).

## Current tools

| Tool | Category | Class | Effect |
|---|---|---|---|
| `list_directory` | fs | read | `read_dir(resolved)` |
| `read_file` | fs | read | `read_to_string(resolved)` |
| `search_files` | fs | read | `WalkDir` + `globset` glob match |
| `write_file` | fs | **write** | create parents + `write(resolved)` |
| `edit_file` | fs | **write** | unique-match string replace |
| `run_command` | shell | **write** | `sh -c` in `spawn_blocking` |

`run_command` is classed write because the tool can't know statically whether an
arbitrary `sh -c` string mutates. This over-prompts on read-only commands
(`git status`, `pacman -Qi`) — filed P2 in `known_issues.md`.

## The tool contract (one file per tool)

Each tool is a self-contained file. Example shape (from `edit_file.rs`):

```rust
#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args { /* #[schemars(description=...)] fields */ }

pub struct EditFile;

impl ToolBase for EditFile {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> { "edit_file".into() }
    fn description() -> Option<Cow<'static, str>> { Some("…".into()) }
    fn output_schema() -> Option<Arc<JsonObject>> { None }  // always None for String
}

impl AsyncTool<super::FsTools> for EditFile {
    async fn invoke(svc: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        let resolved = svc.permissions.check(&args.path, Op::Write).await
            .map_err(|e| ErrorData::internal_error(e, None))?;
        // … operate on `resolved` …
    }
}
```

`ToolBase` (name, description, JSON-schema'd `Args`) is the **single source of
truth** for the schema sent to the LLM — there's no separate schema declaration.

## Schema generation — `rmcp` → genai

`schemas()` in each category converts `rmcp`'s `ToolRouter::list_all()` into genai
`Tool` values:

```rust
ToolRouter::list_all()  →  for each t:
    GenaiTool::new(t.name)
        .with_description(t.description)
        .with_schema(t.schema_as_json_value())
```

`ToolService::all_tools()` concatenates `fs::schemas()` and `shell::schemas()` and
hands them to `ChatRequest::with_tools(...)`.

## Dispatch — the `try_invoke!` macro

Each category's `dispatch(svc, name, args)` matches the tool name with a macro that
deserializes `Args`, calls `invoke`, and maps errors to strings:

```rust
macro_rules! try_invoke {
    ($T:ty) => {
        if name == <$T>::name() {
            let param = serde_json::from_value(args)
                .unwrap_or_else(...) ; // → "invalid arguments: …"
            return Ok(<$T>::invoke(svc, param).await
                .unwrap_or_else(|e| format!("error: {}", e.message)));
        }
    };
}
```

Key behaviour: **tool execution errors become `Ok("error: …")`**, not `Err`. Only
an *unknown tool name* returns `Err("unknown tool: …")`. That's what makes denials
and failures recoverable by the model instead of fatal.

`ToolService::dispatch` tries `fs` then `shell`:

```rust
if let Ok(result) = fs::dispatch(&self.fs, name, args.clone()).await {
    return Ok(result);
}
shell::dispatch(&self.shell, name, args).await
```

> Known issue (P2): this fallthrough relies on fs returning `Err` only for unknown
> names. If an fs tool ever returned a real `Err`, it would be wrongly retried
> against shell. Fix: explicit name-based routing per category. See
> `known_issues.md`.

## Read/write classification

`ToolService::is_readonly(name)` = `fs::is_readonly(name) || shell::is_readonly(name)`.

- `fs::is_readonly` → true for `list_directory`, `read_file`, `search_files`.
- `shell::is_readonly` → always `false`.

The chat loop uses this to decide batching ([stack-flow.md](stack-flow.md)):
read-only calls run concurrently, mutating calls run alone after the read batch
drains.

## Registering a new tool

Within a category's `mod.rs`, three additions:

```rust
pub mod my_tool;                                  // 1. declare
// in tool_router():
    .with_async_tool::<my_tool::MyTool>()          // 2. router
// in dispatch():
    try_invoke!(my_tool::MyTool);                   // 3. dispatch
```

If read-only, also add its name to `is_readonly`. A whole new category mirrors
`fs/` or `shell/`: a `*Tools { permissions }` struct, `tool_router`, `schemas`,
`dispatch`, `is_readonly`, then wire it into `ToolService` (field, `all_tools`,
`dispatch`, `is_readonly`).

## Why categories hold `Arc<PermissionGuard>`

`ToolService::new` clones the one `Arc<PermissionGuard>` into both `FsTools` and
`ShellTools`. All tools share the same guard, so `session_grants` and the allowlist
are consistent across every tool in a run.
