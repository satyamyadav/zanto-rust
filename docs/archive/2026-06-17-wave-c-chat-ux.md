# Wave C — Chat UX (C1–C8)

- **Date:** 2026-06-17
- **Depends on:** B2 segment model (`ChatSegment`/`ChatEntry` in `session.svelte.ts`,
  `segments/{Text,Reasoning,ToolCall}Segment.svelte`, `MessageList.svelte`,
  `Message.svelte` dispatching by `segment.kind`), B1 (`ipc/*`, `browse_dir`).

Each unit below is one work-item. Shared frontend files (`Message.svelte`,
`MessageList.svelte`, `Composer.svelte`, `session.svelte.ts`) are the contention points;
the coordinator batches units so no two in the same batch touch the same file.

General conventions: Svelte 5 runes, Tailwind v4, shadcn-svelte; reuse existing UI
primitives (`Button`, `details`/popover, `@lucide/svelte` icons already available via
deps). E2E is build-check only: `cargo build` + `pnpm check` (0 errors) + `pnpm build:web`.

## C1 — Stack-to-bottom + styling polish
- Files: `MessageList.svelte`, `Message.svelte`, `segments/TextSegment.svelte`.
- Chat content anchors to the **bottom** by default (few messages sit at the bottom, not
  the top) and autoscrolls on new content; add a "jump to latest" affordance that appears
  when scrolled up. Tighten message spacing/role styling (user vs assistant), code block
  styling. Do not change the segment data model.

## C2 — Copy + paste-expander
- Files: `Message.svelte` (or `segments/TextSegment.svelte`) + `Composer.svelte`.
- Add a copy-to-clipboard button on assistant messages and on fenced code blocks
  (hover-reveal; uses `navigator.clipboard`). In the composer, when pasted text exceeds a
  threshold (e.g. >2000 chars or >20 lines), insert it as a collapsed "📎 pasted N lines"
  chip/expander instead of dumping it inline; the full text is still sent.

## C3 — Error surface + retry
- Files: `session.svelte.ts`, `Message.svelte` (dispatch), new `segments/ErrorSegment.svelte`.
- `send()` already awaits `ipc.sendMessage`. On rejection, push an assistant entry with an
  `error` segment (message text) and keep the failed user text. The ErrorSegment renders the
  error + a **Retry** button that re-invokes `send(lastUserText)` (clear the error entry
  first). Add `{ kind: "error"; message: string; retryText: string }` to `ChatSegment`.
  No core/IPC change (the rejected promise carries the error string).

## C4 — Thinking UI block
- Files: `segments/ReasoningSegment.svelte` only.
- Style the reasoning segment as a collapsible "Thinking" panel: a spinner/pulse while
  streaming (`sessionStore.streaming` + this being the last segment), auto-collapsed once
  the turn completes, expandable to read the reasoning. Muted, secondary styling.

## C5 — Tool-call UI block
- Files: `segments/ToolCallSegment.svelte` only.
- Render the tool call as a compact card: tool `name`, collapsible `args` (JSON, pretty),
  a status pill (running → spinner; ok → check; error → red), and the `output` (collapsed
  by default, monospace). Driven entirely by the `tool_call` segment fields.

## C6 — Multi-loop workflow view
- Files: `Message.svelte` + new `segments/WorkflowGroup.svelte`. Depends on C5.
- When an assistant entry contains ≥2 consecutive `tool_call` segments, render them grouped
  under a "Workflow (N steps)" container with per-step status and a collapse-all toggle, so
  a multi-tool agent loop reads as one workflow rather than a stack of cards. Single tool
  calls render as today (C5). Pure presentation over existing segments.

## C7 — File @-tag / slash command in composer
- Files: `Composer.svelte` + `ipc.ts` (add `browseDir` wrapper over B1's `browse_dir`).
- Typing `@` opens an inline autocomplete listing files/dirs from `browseDir` (start at
  roots; descend on selection); choosing one inserts a `@path` token into the message.
  Typing `/` at line start opens a slash-command menu (seed: `/clear`, `/new` — wire to the
  session store where trivial; leave a registry others can extend). Keyboard nav + Esc.

## C8 — Session-persistent infinite scroll
- Files: core `session.rs` (paged loader), `ipc/session.rs`, `ipc.ts`, `MessageList.svelte`,
  `session.svelte.ts`.
- Add `Store::load_messages_page(session_id, offset, limit) -> Vec<(role,text[,meta])>`
  (newest-last; reuse `display_messages` filtering) and an IPC `load_session_page(id, offset,
  limit)`. `selectSession` loads the most recent page; scrolling to the top loads older pages
  and prepends (preserve scroll position). Keep the existing `load_session` working.

## Acceptance (every unit)
- `cargo build` clean; `pnpm check` 0 errors (pre-existing node-types warning OK);
  `pnpm build:web` clean. No regressions to streaming assembly or canvas routing.
