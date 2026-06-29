# Chat bubble restyle + conversation spacing

- **Date:** 2026-06-29

## Summary

Restyle the user message bubble and rework the inter-turn spacing/rhythm for a
cleaner conversation, leaving the assistant turn as plain (bubble-less) text.

## Motivation

The chat thread's user bubble is a flat muted-gray rounded box; the assistant is
plain left-aligned text. The owner wants the user bubble restyled and the spacing
between turns reworked for better rhythm. (Assistant stays plain text — no bubble,
no avatars, per the owner.)

## Current state (grounded)

- **User bubble** (`Message.svelte:295`): right-aligned, `max-w-[85%]`,
  `bg-muted rounded-2xl rounded-br-sm px-4 py-2.5 text-sm shadow-sm`.
- **Assistant** (`Message.svelte:22-23`): left-aligned plain text, `max-w-[90%]`,
  no container styling.
- **Turn spacing** (`MessageList.svelte:81`): all turns in a `flex flex-col
  gap-4` — uniform 1rem gap regardless of speaker.

## Scope

**In scope**
- **User bubble restyle** — pick one direction (see Decision below) and apply it.
- **Spacing/rhythm** — replace the uniform `gap-4` with a rhythm that groups a
  user turn with its assistant reply and adds clearer separation between
  exchanges (e.g. smaller gap user→assistant, larger gap between exchanges), so
  the conversation reads as turns, not a flat list.

**Out of scope**
- Assistant bubble / container (stays plain text).
- Avatars or name labels.
- Any backend change. Pure `Message.svelte` + `MessageList.svelte` (+ `app.css`
  if a token/keyframe is needed).
- The attachment chips, copy/action rows, thinking block — unchanged except where
  spacing naturally shifts.

## Decision needed (pick in review)

The restyle direction is a visual choice — three concrete options, to choose ONE:

- **A — Accent-tinted bubble.** User bubble gets a subtle brand tint (e.g.
  `bg-primary/10` + `border border-primary/20`, keep the rounded-br-sm tail) so
  "you" is visually distinct from the muted UI without being loud. Quiet, on-brand.
- **B — Outlined / ghost bubble.** Drop the fill; user bubble becomes a bordered
  outline (`border border-border bg-transparent`) — lighter, more minimal, less
  weight on screen.
- **C — Keep fill, refine.** Keep `bg-muted` but refine: tighter padding, a softer
  radius, a hairline border, less shadow — a polish pass rather than a new look.

(If none fit, the owner names the target; the spec is updated before code.)

## Affected files

- `crates/zanto-desktop/src/lib/components/Message.svelte` — user-bubble classes.
- `crates/zanto-desktop/src/lib/components/MessageList.svelte` — turn spacing.
- `crates/zanto-desktop/src/app.css` — only if the chosen direction needs a new
  token (unlikely; existing tokens should suffice).

## Implementation steps

(Assumes a chosen direction; written for option A as the placeholder — swap the
exact classes to match the approved option.)

1. **Restyle the user bubble** (`Message.svelte` ~line 295)
   - Replace the bubble's class string with the approved direction's classes.
     E.g. for A: `... rounded-2xl rounded-br-sm bg-primary/10 border border-primary/20
     px-4 py-2.5 text-sm leading-relaxed text-foreground` (drop `shadow-sm` if the
     border carries the separation). Keep `data-role="user"` and the
     right-align/max-width wrapper. Preserve the in-bubble prose-color override
     behavior from `app.css` (the `[data-role="user"] .prose-zanto` rules) — verify
     legibility on the new background; adjust the override only if contrast needs it.

2. **Rework turn spacing** (`MessageList.svelte` ~line 81)
   - Change the single `gap-4` flex column into a rhythm that distinguishes
     intra-exchange from inter-exchange spacing. Simplest approach that needs no
     per-pair grouping: give each `Message` a top margin that depends on the
     speaker boundary — e.g. a larger top gap before a USER turn (start of a new
     exchange) and a smaller gap before an ASSISTANT turn (continuation of the same
     exchange). Implement by removing `gap-4` and adding a conditional margin on the
     Message wrapper keyed on `entry.role` and the previous entry's role, OR by
     wrapping each user+assistant pair — choose the lower-complexity option that
     reads cleanly; document which. Keep the summarized-divider, stopped-marker,
     loader, and queue-chip spacing consistent with the new rhythm.

3. **Verify legibility + the existing overrides**
   - The user bubble carries markdown via `.prose-zanto`, whose color is forced to
     a readable value inside the user bubble (app.css `[data-role="user"]` rules).
     Confirm text + inline code + links stay legible on the new background in BOTH
     themes; tweak only the user-bubble overrides if needed (don't touch the shared
     prose styles).

## Edge cases & risks

- **No new dependency.** UI/CSS only.
- **Both themes.** The chosen bubble background must be legible in light AND dark
  (we previously fixed a dark-on-violet contrast bug here — re-verify whichever
  direction is picked).
- **Inline code / links in the user bubble.** The `[data-role="user"]` prose
  overrides exist precisely because the bubble background differs from the page;
  changing the background may require re-tuning them (step 3).
- **Spacing regressions.** The loader, stopped marker, summarized divider, and
  queue chips all live in the same column; the new rhythm must not collide with
  them (e.g. the `-mt-2` on the stopped marker). Verify each still sits correctly.
- **Long content / code blocks.** The user bubble holds markdown incl. code; the
  restyle must not break overflow handling (the `min-w-0 break-words` wrapper in
  TextSegment stays).

## Acceptance criteria

Verifiable in the desktop app, both themes:

- [ ] The user bubble shows the approved restyle (the chosen direction's look),
      right-aligned, with legible text/code/links in light AND dark.
- [ ] Inter-turn spacing reads as grouped exchanges: a user turn and its assistant
      reply sit closer together than separate exchanges do.
- [ ] The stopped marker, summarized divider, loader, and queue chips still align
      correctly under the new spacing (no overlap/clipping).
- [ ] Assistant turns are unchanged (still plain text, no bubble).
- [ ] No regression to attachment chips, copy/action rows, or the thinking block.

## Manual test plan

1. `pnpm dev:mock`; send a few messages → user bubbles show the new style; the
   gap between an exchange's user+assistant is tighter than between exchanges.
2. Toggle Light/Dark (Settings → Theme) → bubble text/code/links legible in both.
3. Send a message with inline code and a link in the user text → both render
   legibly on the new bubble background.
4. Trigger a stop (`partial stop` + Stop) → the "Stopped" marker still sits
   correctly relative to the restyled turn.
5. Send a long message + a code block as the user → overflow/wrapping intact.
