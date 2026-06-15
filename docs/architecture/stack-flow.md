# Architecture — Stack Flow

One full turn, traced end-to-end through the real code. File references are
clickable. This is the spine that ties every other doc together.

## Entry: `zanto "update my chrome"`

### Phase 1 — CLI bootstrap (`zanto-cli`)

In [main.rs](../../crates/zanto-cli/src/main.rs), `main()`:

1. `Cli::parse()` — clap parses flags: positional `question`, `-m/--model`,
   `-e/--endpoint`, `-s/--session`, `-n/--new`, `-t/--title`, and the `sessions`
   subcommand.
2. `Settings::load()` — see [data-model.md](data-model.md). Ensures
   `.zanto/settings.json` exists, loads user + project JSON, merges, canonicalizes
   `allowed_paths` to absolute.
3. `workspace = canonicalize(".")` — the current dir, used to scope sessions.
4. If the `sessions` subcommand was given → `handle_sessions()` runs and returns;
   no LLM, no chat. (list / show / delete / clear)
5. Resolve runtime config:
   - `model` = `--model` › `settings.model` › `"qwen2.5:14b"`
   - `endpoint` = `--endpoint` › `settings.endpoint` › `"http://192.168.1.66:11434/"`,
     then `Box::leak`'d to `&'static str` (genai's resolver needs `'static`)
   - `policy` = `LastNTurns { max_turns }` from `settings.max_context_turns`,
     else `ContextPolicy::default()` (20 turns)
6. `permissions = Arc::new(PermissionGuard::new(&settings, StdinApprover))`
7. `store = Store::open()` — opens SQLite at `$ZANTO_DB` or the OS app-data path.
8. `resolve_session(...)` — `--new` forces a fresh `Session`; else resume by
   `--session` prefix; else resume the workspace's last session; else new.
9. `run_once(...)` (question given) or `run_interactive(...)` (REPL) → calls
   `chat()`.
10. `finalize_session(...)` — if title empty, `auto_title()` from first user
    message; bump `updated_at`; `store.save_session()`.

### Phase 2 — chat loop (`zanto-core`)

In [chat.rs](../../crates/zanto-core/src/chat.rs), `chat(config, store, session, question, policy)`:

1. `ToolService::new(permissions)` — builds the `fs` + `shell` tool categories,
   each holding a clone of the `Arc<PermissionGuard>`. See [tools.md](tools.md).
2. `session.save_session()` — ensures the session row exists before any message
   is appended (FK target).
3. Build the genai `Client` with a `ServiceTargetResolver`:
   - `override_endpoint = !model.starts_with("gemini")`
   - if overriding → force the configured Ollama endpoint
   - if not (Gemini) → pass the default target through, so genai uses Gemini's
     endpoint + `GEMINI_API_KEY`. See [llm.md](llm.md).
4. `push_msg(user question)` — append to `session.messages` **and**
   `store.append_message()`. Every `push_msg` does both.
5. `system_prompt` — a fixed `ChatMessage::system(...)`, never stored in the DB.
6. **Loop** (`turn = 1, 2, ...`):

```
send_messages = [system_prompt] + session.effective_messages(policy)
                                   └─ trims to last N turns [data-model.md]
req  = ChatRequest::new(send_messages).with_tools(ToolService::all_tools())
res  = client.exec_chat(model, req).await?

if res.tool_calls().is_empty():
    answer = res.first_text()
    fallback = extract_raw_tool_calls(answer)      # qwen malformed-JSON recovery
    if fallback non-empty:
        push_msg(assistant tool-calls); execute_tool_calls(fallback); turn++; continue
    else:
        push_msg(assistant answer); return answer   # ← exit
else:
    tool_calls = res.into_tool_calls()
    push_msg(assistant tool-calls)
    execute_tool_calls(tool_calls); turn++          # loop again
```

The loop ends only when the model returns text with no (parseable) tool calls.

### Phase 3 — tool execution (read/write ordering)

`execute_tool_calls(tools, store, session, tool_calls)`:

```
read_batch = []
for call in tool_calls:
    if ToolService::is_readonly(call.fn_name):
        read_batch.push(call)                  # defer — batch it
    else:
        flush_parallel(read_batch); clear()    # drain reads BEFORE the write
        output = tools.dispatch(call)           # run the mutating call alone
        push_msg(ToolResponse)
flush_parallel(read_batch)                      # drain any trailing reads
```

`flush_parallel(batch)` runs the batched read-only calls concurrently with
`join_all`, then appends each `ToolResponse` in order. This is invariant #2 from
[overview.md](overview.md): reads concurrent, writes serialized, order preserved,
no read observes a same-turn write's partial state.

### Phase 4 — dispatch + permission gate

`ToolService::dispatch(name, args)` ([tools.md](tools.md)) tries
`fs::dispatch` then falls through to `shell::dispatch`. The matched tool's
`invoke()` runs:

```
resolved: PathBuf = svc.permissions.check(&args.path, Op::Read|Write).await?
# ↑ this is where the user is prompted, IN-LINE, same turn
std::fs::read_dir(&resolved)   # or read_to_string / write / WalkDir / Command
```

`PermissionGuard::check` ([permissions.md](permissions.md)):

```
resolved = resolve(path)             # expand ~, canonicalize (or parent+name)
if op-bypass flag set        → Ok(resolved)
if resolved under allowed    → Ok(resolved)
if resolved in session_grants→ Ok(resolved)
else approver.confirm(...)           # ← StdinApprover prints prompt, reads stdin
    AllowOnce    → Ok(resolved)
    AllowSession → cache in session_grants; Ok(resolved)
    AllowForever → cache + persist to .zanto/settings.json; Ok(resolved)
    Deny         → Err("permission denied …")   # surfaced to model as tool error
```

A `Deny` (or any tool error) is returned to the model as `Ok("error: ...")` text,
not a fatal `Err` — so the model can recover (try a different path/command)
rather than the session crashing.

## Full sequence diagram (one turn with one mutating tool call)

```
 CLI(main)      chat()         genai Client     ToolService    PermissionGuard   Approver(stdin)   SQLite
    │ chat(...)    │                │                │               │                 │             │
    ├─────────────►│ save_session ──┼────────────────┼───────────────┼─────────────────┼────────────►│
    │              │ push_msg(user)─┼────────────────┼───────────────┼─────────────────┼────────────►│
    │              │ exec_chat ────►│                │               │                 │             │
    │              │                │ POST /api/chat │               │                 │             │
    │              │◄── tool_calls ─│                │               │                 │             │
    │              │ push_msg(calls)┼────────────────┼───────────────┼─────────────────┼────────────►│
    │              │ execute_tool_calls ────────────►│ dispatch ────►│                 │             │
    │              │                │                │  invoke()     │ check(path,Write)│             │
    │              │                │                │               ├── confirm() ────►│  prompt     │
    │              │                │                │               │◄── AllowOnce ────┤  "> a"      │
    │              │                │                │  run sh -c    │◄─ Ok(PathBuf) ──│             │
    │              │ push_msg(resp) ┼────────────────┼───────────────┼─────────────────┼────────────►│
    │              │ exec_chat ────►│  (turn 2)      │               │                 │             │
    │              │◄── text ───────│                │               │                 │             │
    │              │ push_msg(asst) ┼────────────────┼───────────────┼─────────────────┼────────────►│
    │◄── answer ───┤                │                │               │                 │             │
    │ finalize_session (auto_title, save) ───────────┼───────────────┼─────────────────┼────────────►│
```

## Where each concern lives

| Concern | Code |
|---|---|
| Flag parsing, session lifecycle | [main.rs](../../crates/zanto-cli/src/main.rs) |
| Loop, ordering, fallback parser | [chat.rs](../../crates/zanto-core/src/chat.rs) |
| Tool routing + readonly tag | [tools/mod.rs](../../crates/zanto-core/src/tools/mod.rs) |
| Permission decision + path resolve | [permissions.rs](../../crates/zanto-core/src/permissions.rs) |
| Persistence, trimming | [session.rs](../../crates/zanto-core/src/session.rs) |
| Config merge + path resolve | [config.rs](../../crates/zanto-core/src/config.rs) |
