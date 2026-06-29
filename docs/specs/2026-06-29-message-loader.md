# Persistent message loader

- **Date:** 2026-06-29

## Summary

Show a "responding" loader at the end of the chat thread for the *entire*
duration a turn is busy — not only the pre-first-token gap — so there's always a
visible signal the assistant is working.

## Motivation

Today `MessageList.svelte` shows a bouncing-dots "thinking" indicator ONLY before
the live turn has any segment (lines ~106-118); the moment text or a thinking
block appears, the dots vanish and the only "working" cue is the small spinner
inside the (collapsed-by-default) ThinkingBlock. During long streaming answers or
tool loops there's no obvious thread-level "still going" signal. The owner wants a
loader that persists for the whole busy turn.

## Scope

**In scope**
- Replace the pre-content-only dots with an indicator at the thread tail that is
  visible whenever `sessionStore.busy` is true, for the whole turn (before, during
  streaming, and across tool-call gaps), and disappears when the turn ends
  (`busy` false) or is stopped.
- Make it complement, not duplicate, the ThinkingBlock spinner: a single,
  unobtrusive thread-level cue (e.g. a small animated row below the last message)
  — not a second prominent spinner competing with the thinking block.
- Respect reduced-motion (the app already gates animations globally in `app.css`).

**Out of scope**
- Token counts / progress percentages (separate feature).
- Changing the ThinkingBlock's own live spinner.
- The queued-message chips (lines ~119+) — unchanged.
- Any backend change — this is purely `MessageList.svelte` (+ maybe a tiny CSS).

## Affected files

- `crates/zanto-desktop/src/lib/components/MessageList.svelte` — the loader
  condition + markup. **Only file changed** (plus possibly an `app.css` keyframe
  if a custom animation is used; prefer existing Tailwind `animate-*`).

## Implementation steps

1. **Generalize the loader condition** (`MessageList.svelte` ~line 109)
   - Current guard shows dots only when busy AND the live turn has no segments:
     `sessionStore.busy && !(last is assistant with segments > 0)`.
   - Change to show the indicator whenever `sessionStore.busy` is true. So it
     stays through streaming and tool gaps. Keep it at the thread tail (after the
     `{#each convo}`, before the queue chips), so it always trails the latest
     content.

2. **Differentiate pre-content vs streaming label** (optional, same block)
   - Before any content: label "thinking" (as today).
   - Once the live turn has content (streaming/tools underway): label
     "responding…" (or keep a label-less animated row). Derive from the same
     "last entry is assistant with segments > 0" check used today — now used to
     pick the LABEL, not to hide the loader.
   - Keep it ONE row; do not also render when the turn isn't busy.

3. **Avoid double-cue with the ThinkingBlock**
   - The ThinkingBlock shows a spinner on the live turn. To not look like two
     spinners, make THIS loader a low-key animated row (the existing bouncing
     dots are fine — they read as "thread is active," distinct from the block's
     inline spinner). Visually verify the two don't fight (dots at the tail, block
     spinner inside the message — acceptable; if they clash, drop the dots'
     "thinking" text while the block is visible).

4. **Reduced motion**
   - The global `@media (prefers-reduced-motion: reduce)` rule in `app.css`
     already neutralizes `animate-*`. Confirm the dots degrade to a static row
     (no extra work expected; verify).

## Edge cases & risks

- **No new dependency.** UI-only.
- **Double indicator**: the main risk is the tail loader + the ThinkingBlock
  spinner reading as redundant. Mitigation in step 3; verified visually.
- **Stop / interrupt**: when a turn is stopped, `busy` goes false and the loader
  must vanish (the existing `busy` gate handles this — confirm the "Stopped"
  marker shows and the loader is gone).
- **Queued messages**: the loader sits before the queue chips; a busy turn with a
  queued next message shows the loader + the dashed queue chip — correct.
- **Empty/instant turns**: a turn that errors or returns instantly flips `busy`
  fast; the loader may flash briefly — acceptable.

## Acceptance criteria

Verifiable in the desktop app (mock mode reproduces busy/streaming):

- [ ] After sending, the loader appears immediately and STAYS visible through the
      whole turn — before the first token, during streaming text, and across tool
      calls — disappearing only when the turn completes.
- [ ] When the turn is stopped (Stop), the loader disappears and the "Stopped"
      marker shows.
- [ ] The loader and the ThinkingBlock spinner do not read as two competing
      spinners (one low-key tail row + the block's inline spinner).
- [ ] With `prefers-reduced-motion`, the loader shows as a static (non-animated)
      row, not removed.
- [ ] No regression: queued-message chips still render; idle (not busy) shows no
      loader.

## Manual test plan

1. `pnpm dev:mock`; send a message using a streaming scenario (default reply) →
   the loader shows on send and remains until the reply finishes.
2. Send the `workflow` mock trigger (two tool calls) → the loader stays visible
   across the tool-call gap, not just at the start.
3. Send a `silent stop` / `partial stop` trigger, click Stop → loader vanishes,
   "Stopped" marker appears.
4. Toggle the OS reduced-motion setting (or emulate in devtools) → the loader is
   a static row.
5. Idle (no turn running) → no loader at the tail.
