# Flatten agent blocks (de-card thinking / tool-call / workflow)

- **Date:** 2026-06-29
- **Status:** ✅ Shipped (2026-06-29). All three segment components lost their
  `rounded-md border` cards; nesting now reads from indentation + a faint
  `border-l border-border/50` guide. Status pills kept. Verified in light + dark.

## Summary

Remove the nested card borders from the thinking block, tool-call segment, and
workflow group so they render as quiet borderless rows (chevron + label + status
pill, expandable in place) instead of boxes-within-boxes.

## Motivation

An assistant turn that thinks + runs tools currently stacks three levels of
`rounded-md border border-border/60` cards: the ThinkingBlock card, the
WorkflowGroup card, and — inside it — a ToolCallSegment card per step, each with
bordered args/output rows. The result reads as cards-in-cards-in-cards (owner
feedback: "too much cards and borders"). Flatten to borderless rows.

## Current state (grounded)

- `ThinkingBlock.svelte:37` — outer `div.rounded-md border border-border/60`;
  expanded body `border-t border-border` (line 62).
- `WorkflowGroup.svelte:24` — outer `div.rounded-md border border-border/60`;
  steps container `border-t border-border` (line 44); renders N `ToolCallSegment`.
- `ToolCallSegment.svelte:48` — outer `div.rounded-md border border-border/60`;
  each args/output `section` is a `border-t border-border` block (line 32).
- Composition (`Message.svelte`): a single `tool_call` → standalone
  `ToolCallSegment`; ≥2 consecutive → `WorkflowGroup` wrapping `ToolCallSegment`s.
  So `ToolCallSegment` must look right both standalone AND nested.

## Scope

**In scope**
- Drop the card border/box from all three components; keep the header row
  (chevron + icon + label + status pill) and click-to-expand behavior.
- Replace the bordered inner sections (args/output, expanded thinking, workflow
  steps container) with indentation + a quiet left guide, no boxes.
- Keep the status pills (running/done/error) — they're the colored cue and are
  not "borders." Keep hover affordance on the clickable header (a subtle
  `hover:bg-muted/40` rounded row is fine — it's not a persistent border).

**Out of scope**
- The data model / segment coalescing logic in `Message.svelte` (unchanged).
- The user bubble, message spacing (just shipped), error segment, blocks.
- Light/dark tokens themselves.

## Affected files

- `crates/zanto-desktop/src/lib/components/segments/ThinkingBlock.svelte`
- `crates/zanto-desktop/src/lib/components/segments/ToolCallSegment.svelte`
- `crates/zanto-desktop/src/lib/components/segments/WorkflowGroup.svelte`

## Implementation steps

1. **ThinkingBlock** — remove `rounded-md border border-border/60` from the outer
   div (keep `text-xs`). Header stays a clickable row (keep `rounded-md
   hover:bg-muted/40` for the hover hit-area, no persistent border). Expanded body:
   drop `border-t border-border`; indent under the chevron (e.g. `pl-5`) with an
   optional faint left guide (`border-l border-border/50`), not a full box.

2. **ToolCallSegment** — remove the outer `rounded-md border border-border/60`.
   Header row stays clickable (keep the rounded hover). The `section` snippet: drop
   `border-t border-border`; render args/output indented under the header
   (`pl-5`), the `pre` unchanged otherwise. Works both standalone and nested
   (no border means nesting no longer doubles a box).

3. **WorkflowGroup** — remove the outer `rounded-md border border-border/60`.
   Header row stays clickable. Steps container (line 44): drop `border-t
   border-border`; indent the steps (`pl-5`, optional faint left guide) so they
   read as children of the workflow row, not a boxed list. Each child
   `ToolCallSegment` is now borderless (step 2), so no card-in-card.

4. **Visual pass** — confirm the three still read as distinct, tappable rows with
   clear hierarchy from indentation alone (workflow → its steps → each step's
   args/output), and that the status pills carry the color. Verify in both themes.

## Edge cases & risks

- **No new dependency.** UI/CSS only, three components.
- **Hierarchy legibility** — with borders gone, indentation + the left guide must
  carry the nesting. If steps look detached from the workflow header, the faint
  `border-l` guide on the indented container restores the visual link.
- **Hover hit-area** — keeping `rounded-md hover:bg-muted/40` on headers is fine
  (transient, not a card); it aids discoverability of the click target.
- **Both themes** — verify the faint guides/hover are visible but quiet in light
  AND dark.
- **No data/logic change** — coalescing into WorkflowGroup vs standalone is
  untouched; only presentation changes.

## Acceptance criteria

Verifiable in the desktop app (mock `think` and `workflow` triggers), both themes:

- [ ] A thinking turn shows a borderless "Thought…/Working…" row, expandable to
      indented text — no card box.
- [ ] A single tool call shows a borderless row (name + status pill), expandable
      to indented args/output — no card box.
- [ ] A multi-tool workflow shows a borderless "Workflow" row whose steps are
      indented rows under it — no card-in-card; at most a faint left guide.
- [ ] Status pills (running/done/error) still render with their colors.
- [ ] Hierarchy is readable from indentation alone; legible in light AND dark.
- [ ] No regression: expand/collapse still works at every level; hidden
      tool-calls (renders_as_block) still hidden.

## Manual test plan

1. `pnpm dev:mock`; send `think` → borderless thinking row, expand → indented
   text, no box.
2. Send `workflow` → borderless Workflow row; expand → two indented tool-call
   rows, each expandable to indented args/output; no nested boxes.
3. Toggle Light/Dark → guides/hover quiet but legible in both.
4. Send `chart with toolcall` → the block-producing tool call stays hidden
   (renders_as_block), the chart renders inline — unchanged.
