# zanto — Claude Code Guide

## Response style

Answer in as few words as possible. No greetings, filler, or unsolicited explanation.
No affirmations ("Great!", "Sure!", "Of course!"). Be direct and honest; critique when warranted.
Short sentences. Plain language.

## Process

The canonical flow is in [docs/working-flows.md](docs/working-flows.md). Read it.

**Never write implementation code before the spec is approved.**

For non-trivial changes: use `/dev <request>` to spec → review → implement. Present the spec and wait for explicit go-ahead before touching source files.

For small, self-contained edits (typos, one-liner fixes, doc updates): proceed directly.

**Do not use the superpowers or context7 plugins.** One flow only: the native `/dev`, `/spec`, `/execute`. See working-flows.md.

**Cross-project:** this repo stores no context for other repos (e.g. zanto-site). If a change is needed there, output the requirements as a copy-pasteable block — do not create files for it here.

## Build & verify

```bash
cargo build                         # compile both crates
cargo build -p zanto-core           # core lib only
cargo build -p zanto-cli            # cli binary only
cargo test                          # run tests
cargo run -p zanto-cli              # interactive session
cargo run -p zanto-cli -- "question" # one-shot
cargo run -p zanto-cli -- sessions list
```

`cargo build` is the compile gate. A green build does not prove behaviour — always run the CLI manually to confirm the changed flow works end-to-end.

## Slash commands

| Command | What it does | Model |
|---|---|---|
| `/spec <request>` | Write a spec for the change — no code | opus |
| `/execute <spec-path>` | Implement a spec'd change | sonnet |
| `/dev <request>` | Full loop: spec → review → execute | opus (spec), sonnet (impl) |

Specs live in `docs/specs/YYYY-MM-DD-<slug>.md`. Shipped specs move to `docs/archive/`. Do not use superpowers/context7 (see [docs/working-flows.md](docs/working-flows.md)).

## Architecture

Two crates:

```
crates/
├── zanto-core/    — pure library (chat, tools, permissions, session, config)
└── zanto-cli/     — binary frontend (StdinApprover, CLI flags, sessions subcommand)
```

**Key modules in `zanto-core`:**

| Module | Role |
|---|---|
| `chat.rs` | Multi-turn LLM orchestration loop; `Store` + `Session` aware |
| `config.rs` | `Settings` — dual-layer JSON (user + project), path resolution |
| `permissions.rs` | `PermissionGuard`, `Approver` trait, tilde expansion, `check()` → `PathBuf` |
| `session.rs` | `Store` (SQLite/WAL), `Session`, `ContextPolicy`, session IDs |
| `tools/mod.rs` | `ToolService` — aggregates tool categories |
| `tools/fs/` | Filesystem tools: `list_directory`, `read_file`, `write_file`, `search_files` |

**Tool contract:** every tool calls `svc.permissions.check(&path, op).await` first, then uses the returned `PathBuf` for FS operations — never the raw string from the model.

**Read/write ordering:** reads batch concurrently (`join_all`); reads flush before any mutation; model-returned order preserved.

**Session storage:** single SQLite at `~/.local/share/zanto/zanto.db` (Linux). Schema versioned via `rusqlite_migration`. Messages appended incrementally — crash-safe.

## Key files

| File | Purpose |
|---|---|
| `docs/working-flows.md` | How work is done here (the canonical flow) |
| `docs/architecture/` | Technical reference (overview, modules, data-model, permissions, tools, llm, stack-flow) |
| `docs/test/` | Testing reference (`testing.md`) + manual QA checklist (`qa-checklist.csv`) |
| `docs/product.md` | Product vision, micro-app architecture, directions |
| `docs/specs/` | Active dated spec files (`YYYY-MM-DD-<slug>.md`) |
| `docs/archive/` | Completed/shipped specs, plans, reviews (history only) |
| `.zanto/settings.json` | Project-level config (auto-created) |
| `~/.config/zanto/settings.json` | User-level config |
