# A5 — Structured streaming segments

- **Date:** 2026-06-17
- **Wave:** A (core foundations), batch 3 (after A4 — shares `chat.rs`)
- **Owner of:** `crates/zanto-core/src/chat.rs` (this batch)

## Summary
Extend the `ChatSink` so a turn streams **typed segments**, not just text: reasoning
("thinking") deltas, tool-call start, and tool-result. This powers the C4 thinking
block, C5 tool-call block, and C6 workflow view without those UI units touching the
core loop. Default no-op methods keep existing sinks (and the CLI) unchanged.

## Affected files
- `crates/zanto-core/src/chat.rs` — `ChatSink` trait, loop, `route_tool_calls`.

## Design

### Trait (additive; default no-op bodies via `#[async_trait]`)
```rust
pub struct ToolCallView { pub id: String, pub name: String, pub args: Value }

#[async_trait]
pub trait ChatSink: Send + Sync {
    async fn on_text(&self, delta: &str);
    async fn on_reasoning(&self, _delta: &str) {}                 // NEW — thinking
    async fn on_tool_call(&self, _call: &ToolCallView) {}         // NEW — about to run
    async fn on_tool_result(&self, _id: &str, _output: &str, _ok: bool) {} // NEW — finished
    async fn on_block(&self, block: &ChatBlock);
}
```
Existing `TauriSink` keeps compiling (gains the new behavior in B2). CLI sink (none)
unaffected.

### Loop
- Handle `ChatStreamEvent::ReasoningChunk(c)` → `sink.on_reasoning(&c.content)`.
  (Enable `capture_reasoning_content` only if needed; live deltas are enough.)
- In `route_tool_calls`, for every call (base, flushed-read, and app):
  emit `on_tool_call(&ToolCallView{ id: call.call_id, name: call.fn_name, args })`
  **before** dispatch, and `on_tool_result(&id, &output_or_err, ok)` **after**.
  - For the concurrent read batch (`flush_parallel`), emit `on_tool_call` for each
    before `join_all`, and `on_tool_result` per result after.
- No change to persistence or the returned `ChatTurn` (segments are live-only here;
  durable replay is D1).

## Acceptance checks
- `cargo build` clean; existing 28 tests pass (default methods mean no caller breaks).
- New test: a fake `ChatSink` recording calls, driven through `route_tool_calls` with a
  stub tool list, asserts `on_tool_call` precedes `on_tool_result` and ids match.
  (Reasoning path needs a live model — assert only that the event arm compiles + a
  unit test of the ordering invariant in `route_tool_calls`.)

## Notes / handoff
- B2 extends `TauriSink` to emit `chat_reasoning` / `chat_tool_call` /
  `chat_tool_result` Tauri events and models them as `ChatSegment`s in the store.
- Keep `chat_chunk`/`chat_block`/`chat_done` exactly as-is (streaming already shipped).
