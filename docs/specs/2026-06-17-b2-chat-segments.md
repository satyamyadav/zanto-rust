# B2 — Chat decomposition + segment model

- **Date:** 2026-06-17
- **Wave:** B (shell scaffolding), batch 1 (∥ B1); **depends on A5**
- **Owner of:** `interaction.rs` (TauriSink), frontend chat thread + session store

## Summary
Turn the chat thread from "role + single block" into a **segment model** so the
thinking block (C4), tool-call block (C5), and workflow view (C6) are just segment
renderers — not edits to one mega-component. Wire A5's new `ChatSink` methods to Tauri
events and assemble segments in the store. This is the pivotal scaffolding for Wave C.

## Affected files
- `crates/zanto-desktop/src-tauri/src/interaction.rs` — extend `TauriSink` to implement
  A5's `on_reasoning`/`on_tool_call`/`on_tool_result`, emitting Tauri events.
- `crates/zanto-desktop/src/lib/ipc.ts` — new event listeners + types.
- `crates/zanto-desktop/src/lib/stores/session.svelte.ts` — `ChatSegment` model + assembly.
- `crates/zanto-desktop/src/lib/components/Chat.svelte` — slim to layout; extract
  `MessageList.svelte` (new).
- `crates/zanto-desktop/src/lib/components/Message.svelte` — render an entry's segments.
- New `crates/zanto-desktop/src/lib/components/segments/` — `TextSegment.svelte`,
  `ReasoningSegment.svelte`, `ToolCallSegment.svelte` (minimal; C4/C5 polish later).

## Design

### Backend events (TauriSink)
Implement the three A5 methods to emit:
- `chat_reasoning { text }`
- `chat_tool_call { id, name, args }`
- `chat_tool_result { id, output, ok }`
Keep `chat_chunk`/`chat_block`/`chat_done` exactly as they are.

### Frontend model (session store)
```ts
type ChatSegment =
  | { kind: "text"; text: string }
  | { kind: "reasoning"; text: string }
  | { kind: "tool_call"; id: string; name: string; args: any; output?: string; ok?: boolean }
  | { kind: "block"; block: ChatBlock };
type ChatEntry = { role: "user" | "assistant"; segments: ChatSegment[] };
```
- Replace the current single-`block` `ChatEntry` with `segments`. A user message = one
  `text` segment. **Migration:** `selectSession`/`newSession` wrap existing markdown into
  `[{kind:"text",...}]`; NBA seed becomes a `block` segment.
- Streaming assembly (extend `initStreaming`):
  - `chat_chunk` → append to the trailing `text` segment of the live assistant entry
    (create entry/segment as needed).
  - `chat_reasoning` → append to a trailing `reasoning` segment.
  - `chat_tool_call` → push a `tool_call` segment (close any open text/reasoning).
  - `chat_tool_result` → find the `tool_call` segment by `id`, set `output`/`ok`.
  - `chat_block` → push a `block` segment (canvas blocks still go to `sessionStore.canvas`).
  - `chat_done` → finalize.

### Rendering
- `Message.svelte` iterates `entry.segments` and dispatches to the segment component by
  `kind`. Text/block reuse the current `Block.svelte` markdown/component rendering.
  Reasoning + tool_call get minimal collapsible placeholders (C4/C5 make them nice).

## Acceptance checks
- `cargo build` clean; `pnpm check` 0 errors; `pnpm build:web` clean.
- Existing flows still render: a plain text turn shows one text bubble; a tool turn shows
  a tool_call segment that fills in its result; canvas blocks still land in the panel;
  reopening a session shows persisted text (segments rebuilt from markdown).

## Notes / handoff
- C4 styles `ReasoningSegment` (collapsible "Thinking"); C5 styles `ToolCallSegment`
  (name + args + result, status pill); C6 groups consecutive tool_call segments into a
  workflow view. D1 persists segments into message `metadata` for durable replay.
- Keep `ChatBlock`/`Target` types from `ipc.ts` unchanged.
