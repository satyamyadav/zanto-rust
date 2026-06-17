# Link handling — intercept links, dismissable preview + open-external

- **Date:** 2026-06-17
- **Ask:** "handle links in a dismissable popup or canvas with button to open external."

## Problem
Rendered markdown (assistant text, `markdown` artifact, blocks) contains `<a href>` links.
In a Tauri webview a click would navigate the app away (or silently fail). Links must never
navigate the shell; instead show a controlled preview with an explicit "open in browser".

## Design
- **Global intercept:** capture clicks on any `<a href>` inside rendered content
  (`Block.svelte` markdown, `TextSegment`, `Markdown` block, prose). `preventDefault`, then
  route to the link handler. Implement once (a small action/util applied where sanitized
  HTML is injected) rather than per component.
- **Dismissable preview popup (default):** a popover/dialog showing the URL (host emphasized
  in `font-mono`, full path muted), and actions: **Open in browser** (primary) and
  **Dismiss**. "Open in browser" uses the already-bundled `tauri-plugin-opener`
  (`@tauri-apps/plugin-opener` `openUrl(url)`), opening in the system browser. Optional:
  **Copy link**.
- **Canvas option:** for richer cases (or a per-link choice), send the link to the right
  canvas as a "link card" (host, url, open-external button) instead of a transient popup.
  Default = popup; a "View in panel" button on the popup promotes it to the canvas.
- External-only: http/https open externally; in-app routes (none today) are not applicable.
  Refuse non-http(s) schemes.

## Affected files (frontend)
- `crates/zanto-desktop/src/lib/Block.svelte` + `lib/blocks/Markdown.svelte` +
  `lib/components/segments/TextSegment.svelte` (apply the link-intercept action), new
  `crates/zanto-desktop/src/lib/components/LinkPreview.svelte` (the popup/card), small util
  `lib/links.ts` (intercept + openUrl). `package.json` if `@tauri-apps/plugin-opener` JS
  binding isn't already present (the Rust plugin is initialized in `lib.rs`).
- No core change.

## Open questions
- Popup vs canvas as the **default** — spec assumes **popup**, with a one-click promote to
  canvas. Confirm.
- Allowlist/denylist of domains, or open anything? (Default: open any http(s) after the
  explicit user click — the popup *is* the confirmation.)

## Acceptance
- `pnpm check` 0 / `build:web` clean. Manual: clicking a link in an assistant message opens
  the preview (not navigation); "Open in browser" launches the system browser; Dismiss
  closes; the webview never navigates.
