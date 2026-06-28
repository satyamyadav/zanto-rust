# Tauri native polish — window state, single-instance, OS, notifications

- **Date:** 2026-06-17
- **Ask:** adopt Tauri built-ins for a more native, polished desktop experience.

## Summary
Five low-risk native upgrades. Current state: only `opener` + `dialog` plugins; an
800×600 window with default decorations; capabilities = core/dialog/opener defaults.

## 1. Window config (`tauri.conf.json`)
- `width: 1100, height: 740, minWidth: 900, minHeight: 600` (the 3-pane layout breaks
  below ~900px), `center: true`. Keep `decorations: true` (custom titlebar is a separate,
  later track). Optionally `"theme": null` (follow OS) — the app already has mode-watcher.

## 2. `window-state` plugin
- `tauri-plugin-window-state` (v2): persists window size/position/maximized across launches.
- `lib.rs`: `.plugin(tauri_plugin_window_state::Builder::default().build())`.
- Capability: add `window-state:default` (if it requires one) to `capabilities/default.json`.

## 3. `single-instance` plugin
- `tauri-plugin-single-instance` (v2): relaunch focuses the existing window.
- `lib.rs`: must be registered **first**, with a callback that shows + focuses the main
  window (`app.get_webview_window("main").map(|w| { let _ = w.unminimize(); let _ = w.set_focus(); })`).

## 4. `os` plugin
- `tauri-plugin-os` (v2): frontend platform detection so shortcut hints show **⌘ vs Ctrl**
  correctly. `ipc.ts`: a tiny `platform()` helper (or `@tauri-apps/plugin-os`). Capability
  `os:default`.

## 5. `notification` plugin — turn-done / approval-needed (agentic tie)
- `tauri-plugin-notification` (v2). Capability `notification:default` (+ permission).
- **Turn complete while unfocused:** in `ipc/chat.rs` `send_message`, after `chat()` returns,
  if the main window is not focused (`window.is_focused()? == false`), send a native
  notification: title "zanto", body "Reply ready" (or "Turn stopped"). Don't notify when
  focused.
- **HITL approval while unfocused:** in `interaction.rs` `TauriInteractor::request`, when the
  window is unfocused, fire a notification ("zanto needs your input") alongside the
  `interaction_request` event so a long-running gated turn doesn't silently wait.
- Frontend permission: request notification permission once on first launch
  (`isPermissionGranted`/`requestPermission`) — or do it Rust-side.

## Affected files
- `crates/zanto-desktop/src-tauri/tauri.conf.json` (window).
- `crates/zanto-desktop/src-tauri/Cargo.toml` (4 plugin deps).
- `crates/zanto-desktop/src-tauri/src/lib.rs` (plugin inits; single-instance first).
- `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` (turn-done notification).
- `crates/zanto-desktop/src-tauri/src/interaction.rs` (approval notification).
- `crates/zanto-desktop/src-tauri/capabilities/default.json` (new permissions).
- `crates/zanto-desktop/src/lib/ipc.ts` (os platform helper; notification permission).

## Out of scope (separate tracks)
- System tray, native menu, global shortcut (a "native shell" pass).
- Drag-and-drop onto the window → covered in the **document-io** spec (it's a doc input channel).
- Custom titlebar + vibrancy/mica (visual-polish track).
- Updater / autostart / deep-link / log (need release infra).

## Acceptance (build-check only)
- `cargo build` + `pnpm check` + `pnpm build:web` clean. Manual (`pnpm dev`): window remembers
  size/position; a second launch focuses the existing window; a notification fires when a
  turn finishes with the window in the background; ⌘/Ctrl labels match the OS.
