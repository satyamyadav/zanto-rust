# Thinking block — an always-present "working" affordance per turn

- **Date:** 2026-06-17
- **Ask:** "add thinking block."

## Problem / interpretation
The reasoning segment (C4) only renders when the model streams reasoning tokens — most
local models (Ollama/qwen) never do, so the user sees no "thinking" while the agent works.
**Resolved (user): always-on working block.** Show a turn-level **Thinking/Working block**
that is always present during a running turn (independent of model reasoning), summarizing
what the agent is doing, collapsing to a summary when done. Real model reasoning (when
present) renders inside it.

## Design
- A per-assistant-turn header block, shown from the moment a turn starts streaming until
  `chat_done`:
  - **Live label** = the current activity, in priority order: latest reasoning delta (if
    any) → the active tool's name ("Running list_directory…") → "Thinking…". A small
    spinner/pulse (respects reduced-motion).
  - **Expandable** to reveal the full timeline: reasoning text (if any) + the tool-call
    steps (reuse the existing `ToolCallSegment`s / the agent-spine cluster).
  - On completion, collapse to a one-line summary: "Thought for N steps" (and elapsed time
    if cheap to compute), expandable on click.
- Data is already available in the store (the segment model: `reasoning` + `tool_call`
  segments, `sessionStore.streaming`, the live entry). This block is a *presentation* over
  those segments — it does not need new backend events.
- Relationship to the agent spine: the Thinking block is the **header/summary**; the spine
  remains the expanded timeline. Likely implementation: fold the spine's process cluster
  under this collapsible Thinking header so there is one coherent "working" affordance.

## Affected files (frontend only)
- `crates/zanto-desktop/src/lib/components/Message.svelte` (group process segments under the
  Thinking header) + new `crates/zanto-desktop/src/lib/components/segments/ThinkingBlock.svelte`.
- Possibly `MessageList.svelte` for the live/last-entry signal already threaded as `isLast`.
- No core/backend change.

## Open questions
- Is this the intent (a working/status block), or did you mean the existing reasoning block
  isn't appearing and just needs to be wired? If the latter, the fix is smaller (the
  reasoning capture flag already landed; the block shows only when a reasoning-capable model
  is used).
- Show elapsed time? (needs a turn start timestamp in the store.)

## Acceptance
- `pnpm check` 0 / `build:web` clean. Manual: a tool-only turn (no model reasoning) still
  shows a live "Working… running <tool>" header that collapses to "Thought for N steps".
