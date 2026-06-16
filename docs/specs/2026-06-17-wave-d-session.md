# Wave D — Session features (D1–D3)

- **Date:** 2026-06-17
- **Depends on:** A2 (session schema: `archived`, `summary`, message `metadata`;
  `set_archived`/`set_summary`/`list_sessions_archived`/`append_message_meta`/
  `load_message_meta`/`system_info`), B2 (segment model), B1 (`ipc/*`).

## D1 — Persist + restore artifacts/decisions/Q&A
So reopening a thread shows it exactly as it was (component blocks, tool calls,
reasoning — not just plain text).
- Files: `crates/zanto-core/src/chat.rs` (persist), `crates/zanto-desktop/src-tauri/src/ipc/session.rs`
  (`load_session` returns rich segments), `crates/zanto-desktop/src/lib/ipc.ts`,
  `crates/zanto-desktop/src/lib/stores/session.svelte.ts` (rebuild segments).
- Core: when appending the assistant turn, store a compact JSON of the turn's
  non-text segments (component blocks, tool calls+results, reasoning) into the message
  `metadata` column via `append_message_meta`. Define a stable JSON schema
  (`{segments:[…]}`) mirroring the frontend `ChatSegment` (text|reasoning|tool_call|block).
  Reuse `ChatBlock`/`ToolCallView`. Keep plain assistant text in `content` as today.
- Desktop: extend the load path so a past session returns, per assistant message, the
  metadata segments when present (else a single text segment). Add a richer return type
  (e.g. `RenderMsg { role, text, segments?: Value }`) or a new `load_session_rich`; keep
  `load_session` working. `selectSession` rebuilds `convo` entries from those segments
  instead of flattening to text.
- Acceptance: a turn with a tool call + component block, reopened, shows the tool-call
  segment and the block (not just text). Build-check + a core test for metadata round-trip.

## D2 — Session archive (+ keep delete)
- Files: `crates/zanto-desktop/src-tauri/src/ipc/session.rs` (+ `lib.rs` register),
  `crates/zanto-desktop/src/lib/ipc.ts`, `crates/zanto-desktop/src/lib/components/Sidebar.svelte`,
  `crates/zanto-desktop/src/lib/stores/session.svelte.ts`.
- Commands: `archive_session(id)`/`unarchive_session(id)` (→ `Store::set_archived`),
  `list_archived_sessions()` (→ `Store::list_sessions_archived`). `ipc.ts` wrappers.
- Sidebar: each session row gets an Archive action (alongside the existing rename/delete);
  an "Archived" collapsible/section lists archived sessions with Unarchive. Active list
  already excludes archived (A2). Keep delete.
- Acceptance: archive moves a session out of the active list into Archived; unarchive
  restores it; delete still works. Build-check.

## D3 — Summarization to save context
- Files: new `crates/zanto-core/src/summarize.rs` (or in `chat.rs`), `session.rs` wiring,
  `lib.rs` (core mod). Desktop optional trigger later.
- Core: `summarize_session(client/config, session) -> String` that asks the model for a
  concise running summary of older turns; store via `Store::set_summary`. Integrate with
  `ContextPolicy`: when history exceeds the turn budget, prepend the stored `summary` as a
  system note and trim older turns (instead of hard-dropping them). Add a
  `ContextPolicy::Summarize { keep_last }` variant or fold into existing trimming — keep
  `effective_messages` behavior backward-compatible by default.
- Acceptance: a unit test that `set_summary` is used by the context assembly when present;
  summarization function compiles and is callable. (Live summary quality verified manually.)
  Build-check + `cargo test -p zanto-core`.

## Acceptance (every unit)
`cargo build` + `cargo test -p zanto-core` + `pnpm check` (0 errors) + `pnpm build:web`,
all clean. No regression to streaming/segment assembly.

## Batching note (coordinator)
`ipc/session.rs` is shared by D1+D2 → different batches. D3 is core-only (parallel-safe
with a desktop unit). Suggested: `{D3, D2}` then `{D1}` (D1 also edits `session.svelte.ts`
which D2 touches).
