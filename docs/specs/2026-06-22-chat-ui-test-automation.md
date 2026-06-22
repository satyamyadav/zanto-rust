# Spec: automate the Chat UI checklist (C-1..C-12)

**Date:** 2026-06-22
**Scope:** Automate the 12 Chat rows of `docs/zanto-test-checklist.csv` as Playwright specs over the existing mock-bridge harness, extending the scenario router and mock handlers as needed. Annotate the CSV. Desktop client only; no app/runtime code changes.

## Background

The split-bridge harness (mock Tauri alias + scenario router + Playwright) already covers R-1..R-9. This batch covers the Chat behaviors. The streaming model (`src/lib/stores/session.svelte.ts`) uses display segments — `text`, `reasoning`, `tool_call`, `block`, `error{message,retryText}` — driven by mock events (`chat_chunk`, `chat_reasoning`, `chat_tool_call`, `chat_tool_result`, `chat_block`, `chat_done`, `chat_stopped`, `chat_summarized`). `send()` queues messages typed while `busy` (FIFO) and, when `ipc.sendMessage` rejects, pushes an `error` segment carrying the retry text.

## Rows

| Row | Behavior | Mock support needed |
|---|---|---|
| C-1 | Streaming — tokens appear incrementally | default scenario (multi-chunk) |
| C-2 | Stop mid-turn — partial kept + 'Stopped' marker | new `partial stop` scenario (stream text, then block until interrupt) |
| C-3 | Queue while busy — pending bubble, FIFO dispatch | blocking scenario + queue UI |
| C-4 | Thinking block — reasoning + tool, collapses to 'Thought for N steps' | new `thinking` scenario (reasoning + tool_call + tool_result) |
| C-5 | Workflow grouping — multiple tools → 'Workflow (N steps)' | new `workflow` scenario (≥2 tool calls) |
| C-6 | Copy — copy a reply / code block | clipboard read (Playwright permission) |
| C-7 | Paste expander — large paste collapses to a chip; full text still sent | composer behavior only |
| C-8 | @-tag — file-picker autocomplete inserts @path | `browse_dir` mock (already exists) |
| C-9 | Slash menu — `/new`, `/clear` | composer behavior; `new_session` handler (exists) |
| C-10 | Error + retry — inline error card + Retry re-runs | new `error` scenario (`send_message` rejects) |
| C-11 | Infinite scroll — older messages load on scroll-to-top, position preserved | paginated `load_session_page` mock |
| C-12 | Link handling — reply URL → popup (Open in browser / Copy / View in panel); app never navigates | new `link` scenario (markdown with a URL) |

## Mock infrastructure (Task 1)

Extend `src/lib/mock/scenarios.ts` with these scenarios (triggers are case-insensitive substrings; keep more-specific triggers earlier):

- **`error`** (trigger `"trigger error"`): not a normal event script — the mock `send_message` must REJECT for this trigger so `send()`'s catch pushes the error segment. Implement via a scenario flag `throws: true`; in `backend.ts`, `if (sc.throws) throw new Error("mock: simulated turn failure")` before emitting.
- **`partial stop`** (trigger `"partial stop"`): emit one `chat_chunk {text:"Partial answer so far"}`, then set `blocking:true` (park until `interrupt_turn`, like the existing silent-stop). On interrupt the existing path emits `chat_stopped`+`chat_done`; the partial text segment remains.
- **`thinking`** (trigger `"think"`): emit `chat_reasoning {text:"Considering options"}`, `chat_tool_call {id:"t1",name:"read_file",args:{path:"/x"}}`, `chat_tool_result {id:"t1",output:"ok",ok:true}`, `chat_chunk {text:"Done."}`, `chat_done`.
- **`workflow`** (trigger `"workflow"`): emit two `chat_tool_call`+`chat_tool_result` pairs (e.g. `list_directory`, `read_file`) then `chat_chunk`+`chat_done` — so the UI groups them as 'Workflow (2 steps)'.
- **`link`** (trigger `"link"`): emit a `chat_chunk` whose markdown contains an http URL, e.g. `"See https://example.com for details."`, then `chat_done`. (The link interception in `links.svelte.ts`/`Message` renders it and intercepts clicks.)

Add to `backend.ts`:
- The `throws` handling in `send_message` (before the event loop).
- **Paginated `load_session_page`** for C-11: build a deterministic list of N (e.g. 60) `RenderMsg` entries in-module (alternating user/assistant, e.g. `text: "msg #<i>"`), and have `load_session_page(args:{offset,limit})` return the slice. Keep `load_session` returning the existing stopped-turn fixture (R-3 depends on it) OR returning the first page — choose so R-3 still passes; if `load_session` must change, re-verify R-3. `list_sessions` already returns a session to open.

Scenario flags (`throws?`, `blocking?`) are mock-internal — not contract fixtures. No new fixture is required unless a paginated `load_session_page` fixture is preferred for the contract test (optional; if added, contract-test it).

Verify after Task 1: `pnpm check` clean, `pnpm test:ui` — all EXISTING specs still pass (default/chart/finance/silent-stop scenarios unchanged; the new scenarios are additive).

## Specs (one file: `tests/ui/chat.spec.ts` already exists for the basic send; ADD a `tests/ui/regression-chat.spec.ts` companion OR a new `tests/ui/chat-behaviors.spec.ts`)

Create `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`. Each test triggers its scenario and asserts on the real rendered DOM (discover selectors from `src/lib/components/` — `MessageList`, `Message`, `Composer`, `segments/*`, `ThinkingBlock`, `WorkflowGroup`, `ErrorSegment` — do NOT change them):

- **C-1 streaming:** send a default message; assert the assistant text becomes visible (streamed). A strict incremental assertion is optional; minimally assert the final streamed text renders.
- **C-2 stop mid-turn:** send `"partial stop"`; once the partial text appears, click Stop; assert the partial text REMAINS and the 'Stopped' marker shows.
- **C-3 queue while busy:** send a `"partial stop"` (blocking) message; while busy, type a second message and submit; assert a pending/queued indicator appears; click Stop; assert the queued message is then dispatched (its user bubble appears / it runs).
- **C-4 thinking block:** send `"think"`; assert a thinking/working indicator appears and collapses to a 'Thought for N steps' control that is expandable (assert the collapsed label, then expand and assert reasoning content).
- **C-5 workflow grouping:** send `"workflow"`; assert the tool calls are grouped under a 'Workflow (2 steps)' (or N-steps) label.
- **C-6 copy:** send a default message; hover the reply; click the copy control; assert clipboard contains the reply text (grant clipboard permission in the test). If clipboard read is unavailable in the runner, assert the 'Copied' feedback instead.
- **C-7 paste expander:** focus the composer; paste a large multi-line string (e.g. 60 lines); assert it collapses to a 'pasted N lines' chip in the composer; send; assert the user message still carries the full text (or the chip expands to it).
- **C-8 @-tag:** type `@` in the composer; assert a file autocomplete appears (backed by `browse_dir`); pick an entry; assert an `@<path>` token is inserted.
- **C-9 slash menu:** type `/` at line start; assert the slash menu lists `/new` and `/clear`; selecting `/new` starts a new session (assert empty thread / new session); (`/clear` is already covered in R-8 — a light re-assert is fine but not required).
- **C-10 error + retry:** send `"trigger error"`; assert an inline error card with a Retry control appears; click Retry; assert the turn re-runs (the error clears / a normal reply streams). To make Retry succeed, the retry resend should hit a non-throwing scenario — Retry resends the SAME text (`"trigger error"`), which throws again; so assert Retry RE-ATTEMPTS (a second error card or a loading state), OR adjust the `error` trigger so only the first attempt throws (e.g. a one-shot flag in the mock that throws once then succeeds) — document the choice. Prefer the one-shot approach so Retry visibly recovers.
- **C-11 infinite scroll:** open a session (mock paginated `load_session_page`); scroll the message list to the top; assert older messages load (a higher message count / an older message id becomes visible) and the scroll position is preserved (the previously-top message stays in view). Use Playwright auto-waiting, no fixed sleeps.
- **C-12 link handling:** send `"link"`; the reply contains an http URL; click the rendered link; assert a preview popup/card appears with the expected actions (Open in browser / Copy / View in panel) and that the app did NOT navigate (URL/root unchanged). 'Open in browser' routes through `ipc.openExternal` (mock no-op) — assert it's invokable without error.

For any scenario whose component `data`/markdown shape doesn't render, adjust the scenario in `scenarios.ts` (allowed) — never the component.

## CSV (final task)

Append to each C-row's `Automation` column `auto: tests/ui/chat-behaviors.spec.ts` (or the file the test lives in). Use the programmatic csv-module approach (preserve multi-line quoted cells; one column already exists). Verify row count unchanged and integrity (no cell altered).

## Out of scope
- Layer-B finance logic (separate batch).
- Rows requiring a live model for their ASSERTION (none of C-1..C-12 do — all assert UI plumbing the mock can drive).

## Success criteria
- `pnpm test:ui` passes including all new C-1..C-12 specs and the pre-existing specs.
- `pnpm check` clean; `cargo test` unaffected (no Rust change unless a paginated fixture + contract test is added).
- CSV C-rows annotated; integrity preserved.
- No app/runtime code changed.

## Notes on honesty
These specs assert UI plumbing driven by canned mock events — they verify the chat UI renders/handles each event correctly, NOT real model/tool behavior. That is the correct scope for UI tests; the CSV annotation should read `auto:` (UI), with the live-model end-to-end versions remaining manual (FLOW-* rows).
