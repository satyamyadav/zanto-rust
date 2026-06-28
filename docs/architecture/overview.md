# Architecture — Overview

Current-state map of zanto as it exists on `main`. Every claim traces to code.
No planned features. When code and this doc disagree, the code wins — fix the doc.

## Index

| Doc | Covers |
|---|---|
| [overview.md](overview.md) | Crate split, philosophy, component diagram (this file) |
| [stack-flow.md](stack-flow.md) | One full turn traced end-to-end + sequence diagram |
| [modules.md](modules.md) | Every module in both crates: role + key types |
| [data-model.md](data-model.md) | SQLite schema, settings layering, on-disk layout |
| [permissions.md](permissions.md) | Approver trait, path resolution, grant lifecycle |
| [tools.md](tools.md) | Tool contract, router, dispatch, readonly classification |
| [llm.md](llm.md) | genai adapter routing (Ollama vs Gemini), endpoint override |

## What zanto is

An AI assistant that orchestrates an LLM chat loop with filesystem and shell
tools, gated by a human-in-the-loop permission system, with session history
persisted to SQLite. The model picks tools and wires data; it never mutates the
system without passing the permission gate.

## Two crates

```
crates/
├── zanto-core/   — pure library: chat loop, tools, permissions, session, config
└── zanto-cli/    — binary `zanto`: CLI flags, interactive REPL, StdinApprover,
                    sessions subcommand
```

`zanto-core` is frontend-agnostic by design. It has no `main`, no stdin/stdout
assumptions, no Tauri dependency. The only coupling to a UI is the `Approver`
trait ([permissions.md](permissions.md)) — the core calls `approver.confirm(...)`
and the frontend decides how to ask the user. Today the only implementor is
`StdinApprover` in the CLI; a TUI / Tauri / HTTP frontend would inject its own.

## Component diagram

```
┌─────────────────────────── zanto-cli (binary) ───────────────────────────┐
│  Cli (clap)     StdinApprover : Approver     handle_sessions()            │
│      │                  │                          │                      │
└──────┼──────────────────┼──────────────────────────┼──────────────────────┘
       │ chat(config, store, session, q, policy)      │ Store ops
       ▼                  ▼                            ▼
┌─────────────────────────── zanto-core (library) ──────────────────────────┐
│                                                                            │
│  chat.rs ──── ToolService ──┬── fs::FsTools  (list/read/write/search/edit) │
│    │            (dispatch)  └── shell::ShellTools (run_command)            │
│    │                │                                                      │
│    │                └──► PermissionGuard.check(path, op) ──► Approver      │
│    │                          (Arc, shared by all tools)                   │
│    │                                                                       │
│    ├──► genai Client ──► Ollama (remote) | Gemini (cloud)   [llm.md]       │
│    │                                                                       │
│    └──► Store (SQLite/WAL) ──► sessions + messages           [data-model]  │
│                                                                            │
│  config.rs: Settings (user JSON + project JSON, merged, path-resolved)     │
└────────────────────────────────────────────────────────────────────────────┘
```

## The three invariants that define the system

1. **Tools never touch raw model paths.** Every tool calls
   `permissions.check(&path, op).await?` first and uses the returned `PathBuf`
   for the filesystem operation — never the string the model supplied. See
   [tools.md](tools.md) and [permissions.md](permissions.md).

2. **Reads batch, writes serialize.** Within a single LLM turn, read-only tool
   calls run concurrently (`join_all`); any mutating call flushes the pending
   read batch first, then runs alone. Model-returned order is preserved. See
   [stack-flow.md](stack-flow.md).

3. **History is append-only and crash-safe.** Each message is written to SQLite
   the moment it is produced, inside the loop — not batched at the end. A crash
   mid-turn leaves a consistent prefix. See [data-model.md](data-model.md).

## Source of truth pointers

- `docs/architecture/` — this set: current-state technical reference.
- `docs/test/` — testing reference + manual QA checklist.
- `docs/product.md` — product vision, micro-app architecture, directions.
- `docs/working-flows.md` — how work is done here.
- `docs/specs/` — active dated implementation specs.
- `docs/archive/` — shipped specs, plans, reviews, and proof artifacts (history).
- `docs/vision/` — FUTURE direction only (e.g. the GenUI-D web frontend). Not
  current state; kept separate on purpose.
