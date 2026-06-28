# Token counter

- **Date:** 2026-06-28

## Summary

Capture the LLM's actual token usage per turn (genai `captured_usage`), fall back
to a chars/4 estimate when the provider reports none, and surface it in the UI as
a per-message count, a cumulative session total, and a context-window gauge.

## Motivation

The app currently shows no token information — users can't see what a turn cost,
how much they've spent in a session, or how close they are to the model's context
window. The core already computes the model's context window per turn (for
auto-summarization) and genai 0.6 exposes per-response usage on the stream's
`End` event; both are unused by the UI. This wires them through.

## Scope

**In scope**
- **Core capture** (`chat.rs`): set `with_capture_usage(true)`; read
  `StreamEnd.captured_usage` (`prompt_tokens` / `completion_tokens` /
  `total_tokens`, all `Option<i32>`); aggregate across the turn's tool-loop
  iterations into one per-turn usage.
- **Estimate fallback**: when usage is absent (e.g. Ollama), estimate
  `total ≈ ceil(chars / 4)` over the sent prompt + answer; flag it as estimated
  so the UI can mark it `~`.
- **Thread to the UI**: add usage to the `chat_done` event payload (currently
  empty) and persist it on the assistant message so it survives reload.
- **Three UI surfaces** (Svelte):
  1. **Per-message** — token count on each assistant message (e.g. by the Copy
     button), `~`-prefixed when estimated.
  2. **Session total** — cumulative tokens for the conversation.
  3. **Context-window gauge** — session-or-last-turn tokens vs the model's window
     (the window is already computed per turn in `ipc/chat.rs`).

**Out of scope**
- **Live composer count** (dropped — actual usage doesn't exist before a
  response; a typing-time estimate was considered and rejected by the owner).
- Cost/$ estimation, per-provider pricing.
- Token breakdown details (reasoning tokens, cache tokens) — capture only
  prompt/completion/total; the `*_details` are ignored for now.
- Retroactive usage for already-persisted messages (only new turns get usage;
  old messages show nothing — backward compatible).

## Affected files

**Phase 1 — core capture + per-message**
- `crates/zanto-core/src/chat.rs` — capture usage on `End`; aggregate; estimate
  fallback; return it.
- `crates/zanto-core/src/chat.rs` (`ChatTurn`) — add a `usage` field.
- `crates/zanto-desktop/src-tauri/src/interaction.rs` — `chat_done` payload
  carries usage.
- `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` — pass the turn's usage into
  the `chat_done` emit + persist it on the assistant message meta.
- `crates/zanto-desktop/src-tauri/src/ipc/mod.rs` (`RenderMsg`) — surface
  persisted usage on reload.
- `crates/zanto-desktop/src/lib/ipc.ts` — `TokenUsage` type; `onChatDone` payload;
  `RenderMsg.usage`.
- `crates/zanto-desktop/src/lib/stores/session.svelte.ts` — store per-entry
  usage from `chat_done` + `toEntries`.
- `crates/zanto-desktop/src/lib/components/Message.svelte` — render the
  per-message count.

**Phase 2 — session total + context gauge**
- `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` — include the turn's `window`
  tokens in the `chat_done` payload (for the gauge denominator).
- `crates/zanto-desktop/src/lib/stores/session.svelte.ts` — derived session
  total + latest window.
- A small UI surface for the total + gauge (likely the chat footer/header — exact
  spot decided in phase 2, kept out of the message list).

## Implementation steps

### Phase 1 — core capture + per-message display

1. **Capture usage on the stream End** (`crates/zanto-core/src/chat.rs`,
   `stream_options` ~line 374 and the `ChatStreamEvent::End` arm ~line 441)
   - Add `.with_capture_usage(true)` to the `ChatOptions` chain (alongside the
     existing `.with_capture_content(true)`).
   - In the `End(end)` arm, after `captured_into_tool_calls`, read
     `end.captured_usage` and fold it into a per-turn accumulator declared before
     the tool loop: `let mut usage_acc = TurnUsage::default();`. Sum
     `prompt_tokens` / `completion_tokens` / `total_tokens` across iterations
     (each `Option<i32>` → add when `Some`). The loop runs once per tool-call
     iteration, so usage must accumulate, not overwrite.
   - NOTE: `captured_usage` is consumed by-value with `tool_calls` from the same
     `end`. `captured_into_tool_calls(self)` takes `self`, so read
     `end.captured_usage` (a field clone of the `Option<Usage>`) BEFORE calling
     `captured_into_tool_calls`, or restructure to read both. Verify the exact
     `StreamEnd` API (`captured_usage` is a public field per genai 0.6) and order
     the reads so neither move conflicts.

2. **Define `TurnUsage` and add it to `ChatTurn`** (`crates/zanto-core/src/chat.rs`)
   - Add a serializable struct:
     ```rust
     #[derive(Debug, Clone, Default, Serialize, Deserialize)]
     pub struct TurnUsage {
         pub prompt_tokens: Option<u32>,
         pub completion_tokens: Option<u32>,
         pub total_tokens: Option<u32>,
         /// True when these are a chars/4 estimate (provider reported no usage).
         #[serde(default)]
         pub estimated: bool,
     }
     ```
   - Add `pub usage: TurnUsage` to `ChatTurn` (with `#[serde(default)]` for
     backward-compatible deserialization of old persisted turns).

3. **Estimate fallback** (`crates/zanto-core/src/chat.rs`, after the tool loop,
   before returning the `ChatTurn`)
   - If `usage_acc.total_tokens` is `None` (provider gave nothing), compute an
     estimate: `prompt ≈ ceil(sent_chars / 4)`, `completion ≈ ceil(answer_chars
     / 4)`, `total = prompt + completion`, set `estimated = true`. `sent_chars`
     is the char length of the messages sent this turn (sum over
     `send_messages`' text content); `answer_chars` is the accumulated `answer`
     length. Use the LAST iteration's prompt for the estimate (consistent with
     what the model actually saw last), or the sum — pick the sum for a
     conversation-level feel; decide in implementation and note it. Mark
     `estimated = true` so the UI shows `~`.
   - Populate the returned `ChatTurn.usage` from `usage_acc` (real) or the
     estimate.

4. **`chat_done` payload carries usage**
   (`crates/zanto-desktop/src-tauri/src/interaction.rs` ~line 146)
   - The sink emits `chat_done` with `()`. Change the emit to carry the turn's
     usage. Since the sink doesn't have the final `ChatTurn` (the IPC layer does),
     the cleanest path: have `ipc/chat.rs` emit `chat_done` with the usage AFTER
     `chat()` returns, and remove/guard the sink's bare `chat_done` so it isn't
     double-emitted. Verify which currently fires (`interaction.rs:146` sink vs
     any ipc emit) and consolidate to one `chat_done` carrying
     `{ usage, window_tokens }`. (window_tokens is phase 2; phase 1 sends
     `{ usage }`.)
   - If the sink's `on_done` must stay the single emitter for ordering reasons,
     pass the usage into the sink before the final emit (e.g. a
     `sink.set_usage(turn.usage)` call from `ipc/chat.rs` before the turn
     completes) — choose the approach with the smaller, clearer diff and note it.

5. **Persist usage on the assistant message**
   (`crates/zanto-desktop/src-tauri/src/ipc/chat.rs` + `ipc/mod.rs`)
   - Where the assistant turn is persisted (the per-message meta JSON used by
     `RenderMsg::from_meta`), include `usage` in the meta object so it round-trips
     on `load_session`. Add `pub usage: Option<TurnUsageDto>` (or reuse the core
     `TurnUsage` shape) to `RenderMsg` and parse it in `from_meta` alongside
     `segments`/`stopped`/`attachments`.

6. **Frontend types + event** (`crates/zanto-desktop/src/lib/ipc.ts`)
   - Add `export type TokenUsage = { prompt_tokens?: number; completion_tokens?:
     number; total_tokens?: number; estimated?: boolean }`.
   - Change `onChatDone` to deliver the payload:
     `onChatDone: (cb: (p: { usage?: TokenUsage }) => void) => listen<{ usage?:
     TokenUsage }>("chat_done", (e) => cb(e.payload ?? {}))`.
   - Add `usage?: TokenUsage` to `RenderMsg`.

7. **Store the per-entry usage**
   (`crates/zanto-desktop/src/lib/stores/session.svelte.ts`)
   - Add `usage?: TokenUsage` to `ChatEntry`.
   - In the `onChatDone` handler (~line 193): attach `payload.usage` to the live
     assistant entry (the last entry / `streamIdx`) before clearing stream state.
   - In `toEntries` (~line 302): map `RenderMsg.usage` → `ChatEntry.usage` so
     reloaded sessions show counts.

8. **Render the per-message count**
   (`crates/zanto-desktop/src/lib/components/Message.svelte`)
   - For assistant entries with `entry.usage?.total_tokens`, show a small muted
     label near the Copy button: `{estimated ? "~" : ""}{total} tokens`
     (e.g. `1,234 tokens` or `~1,234 tokens`). Tooltip with the prompt/completion
     split if present. No label when usage is absent (old messages).

### Phase 2 — session total + context gauge

9. **Add `window_tokens` to the `chat_done` payload**
   (`crates/zanto-desktop/src-tauri/src/ipc/chat.rs`)
   - The per-turn `window` is already computed (~line 266). Include it in the
     `chat_done` payload: `{ usage, window_tokens }`. Update the TS payload type +
     `onChatDone`.

10. **Derived session total + latest window**
    (`crates/zanto-desktop/src/lib/stores/session.svelte.ts`)
    - A `$derived` sum of `total_tokens` across all assistant entries' usage
      (session total). Track the latest `window_tokens` from the most recent
      `chat_done`.

11. **Session total + gauge UI**
    - Add a compact indicator showing `session total` and a gauge of
      `latest-turn-or-session tokens / window_tokens` (e.g. a thin bar or
      `4.2k / 8k`). Placement: the chat footer or header — NOT in the scrolling
      message list. Exact component + spot finalized when implementing phase 2;
      keep it small and out of the way. Estimated totals carry the `~` marker.

## Edge cases & risks

- **No new dependency.** genai 0.6 already provides `captured_usage`; the rest is
  threading + UI.
- **Usage consumed-by-value on `StreamEnd`.** The `End` arm already calls
  `captured_into_tool_calls(self)`. Reading `captured_usage` must not conflict
  with that move — read the field first. **Flagged**; resolved in step 1.
- **Ollama / local models report no usage** → estimate fallback (chars/4) with
  `estimated = true` → UI shows `~N tokens`. Accuracy is rough but non-zero, which
  the owner chose over showing nothing.
- **Tool-loop multi-iteration turns**: usage must SUM across iterations, not take
  the last. Step 1 accumulates. A turn with 3 tool calls reports the combined
  token cost.
- **Double `chat_done`**: today the sink emits it. Adding an ipc-level emit risks
  emitting twice (UI would run `onChatDone` twice → wrong totals). Step 4
  consolidates to exactly one emitter. **Must verify only one fires** (test plan
  asserts a single increment).
- **Backward compatibility**: `ChatTurn.usage` and `RenderMsg.usage` are
  `#[serde(default)]` / optional — old persisted sessions deserialize fine and
  simply show no token label.
- **chars/4 is English-biased**; for code/CJK it under/over-counts. Acceptable for
  an estimate marked `~`. Not used when real usage is present.
- **Session total across summarization**: when older turns are summarized out of
  the window, their tokens still count toward the *session* total (cumulative
  cost) but not the *window gauge* (which is current-window occupancy). Keep the
  two numbers distinct; document in the UI tooltip.

## Acceptance criteria

Verifiable by running the CLI (which uses the same `chat()` core path) and the
desktop app:

- [ ] `cargo run -p zanto-cli -- "say hi in 3 words"` against a usage-reporting
      provider (e.g. an OpenAI/Anthropic/Gemini key) prints/logs a non-zero
      `total_tokens` for the turn (the `ChatTurn.usage` is populated). [Phase 1]
- [ ] The same against local Ollama yields `usage.estimated == true` with a
      non-zero chars/4 estimate (no provider usage, fallback engaged). [Phase 1]
- [ ] In the desktop app (mock or real), an assistant message shows a
      `N tokens` label (or `~N tokens` when estimated); reloading the session
      keeps the label (persisted). [Phase 1]
- [ ] `chat_done` fires exactly once per turn (the session total increments by
      one turn's usage, not double). [Phase 1]
- [ ] A multi-tool-call turn reports SUMMED usage across iterations, not just the
      last call. [Phase 1]
- [ ] The chat UI shows a cumulative session total that equals the sum of
      per-message counts. [Phase 2]
- [ ] The context-window gauge shows `used / window` where `window` matches the
      active model's context window (e.g. ~8k for a default Ollama model). [Phase 2]
- [ ] Old sessions (persisted before this change) load without error and show no
      token labels (backward compatible). [Phase 1]

## Manual test plan

Phase 1 (core path is shared with the CLI, so the CLI proves capture):

1. With an OpenAI/Anthropic/Gemini key configured:
   `cargo run -p zanto-cli -- "reply with the single word: ok"`
   → the turn completes; a debug log / the returned `ChatTurn.usage.total_tokens`
   is `Some(n)` with `n > 0` and `estimated == false`.
   (If the CLI doesn't surface usage in output, add a one-line `eprintln!` of
   `turn.usage` under a debug flag for this verification, or assert via a unit
   test on the aggregation helper.)
2. With Ollama active (`qwen2.5` etc.):
   `cargo run -p zanto-cli -- "reply with the single word: ok"`
   → `ChatTurn.usage.estimated == true`, `total_tokens == Some(m)`, `m > 0`
   (chars/4 of prompt+answer).
3. `cargo test -p zanto-core` → a unit test for the usage aggregator (sums two
   `Some` iterations; falls back to estimate when both `None`) passes.
4. Desktop, mock mode: `pnpm dev:mock`, send a message →
   the assistant message renders a `… tokens` label; refresh / reopen the session
   → the label persists. (Mock `send_message` fixture extended with a `usage`
   field for this.)
5. Desktop, real provider: send a message → label shows the real count; send
   another → the session total (phase 2) equals the sum of the two message
   counts; the gauge denominator matches the model window.

Phase 2 adds: send N messages, confirm the footer/header total == sum of
per-message labels, and the gauge bar fills proportionally to `used / window`.
