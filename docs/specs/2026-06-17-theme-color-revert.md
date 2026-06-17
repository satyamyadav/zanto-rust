# Restore the previous theme colors (keep the rest of Workbench)

- **Date:** 2026-06-17
- **Ask:** "the old color of theme was better."

## Summary
Revert **only the color tokens + radius** in `app.css` to the pre-Workbench values
(violet primary on neutral grays, `--radius: 0.6rem`). Keep every structural Workbench
gain: bundled fonts, `font-display`/`font-mono` usage, `success`/`warning` tokens, the
consolidated `.prose-zanto`, global reduced-motion, and the agent-spine keyframe.

## Design
- In `crates/zanto-desktop/src/app.css`, restore the `:root` and `.dark` color values to
  the originals (violet `--primary: oklch(0.52 0.19 278)` light / `oklch(0.68 0.2 278)`
  dark; neutral `--background`/`--foreground`/`--muted`/`--border`/`--accent`/`--sidebar*`
  as before) and `--radius: 0.6rem`.
- **Keep** the added `--success`/`--warning` (+ `-foreground`) tokens — they are new, not
  part of the old palette, and tool/status pills now depend on them. Use neutral-friendly
  green/amber values consistent with the violet theme.
- **Do not** touch the `@theme inline` font vars, the `.prose-zanto` block, the
  `spine-pulse` keyframe, or the reduced-motion media query.
- Net effect: because components use tokens (`bg-primary`, the spine node uses `primary`,
  active bars use `primary`), the accent reverts to violet everywhere automatically — the
  agent spine and primary buttons become violet again with **no component edits**.

## Open question
- The signature spine "pulses amber" was the one bold mark. Reverting `--primary` to violet
  makes the spine **violet**. If you want to keep an amber activity signal *while* the rest
  of the theme is violet, add a dedicated `--activity` token (amber) and point the spine at
  it. **Default for this spec: spine follows `--primary` (violet), no extra token.**

## Affected files
- `crates/zanto-desktop/src/app.css` (only).

## Acceptance
- `pnpm check` 0 errors, `pnpm build:web` clean. Visual: violet accent + old neutrals
  restored in light/dark; fonts, spine, prose, tool pills unchanged in structure.
