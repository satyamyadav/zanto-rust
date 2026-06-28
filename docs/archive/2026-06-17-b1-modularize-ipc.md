# B1 — Modularize IPC + file-browse command

- **Date:** 2026-06-17
- **Wave:** B (shell scaffolding), batch 1 (∥ B2)
- **Owner of:** `crates/zanto-desktop/src-tauri/src/ipc.rs` → `ipc/`, `lib.rs` handler list

## Summary
Split the single `ipc.rs` into domain submodules so later units own disjoint files
(kills the merge hotspot). Pure refactor — every command keeps its exact name,
signature, and behavior — plus **one new command** (`browse_dir`) that the file
explorer (Wave C/E) and @-tagging (C7) need.

## Affected files
- Delete `crates/zanto-desktop/src-tauri/src/ipc.rs`; create:
  - `ipc/mod.rs` — `DesktopState`, `RenderMsg`, `ConfigDto`, `ConfigPatch`, shared
    `active_app_id`; `pub use` re-exports of every command so `lib.rs` paths still work
    (or update `lib.rs` to `ipc::chat::send_message` etc.).
  - `ipc/chat.rs` — `send_message`.
  - `ipc/apps.rs` — `list_apps`, `get_catalogue`, `mount_app`, `unmount_app`,
    `query_app`, `run_app_action`.
  - `ipc/session.rs` — `list_sessions`, `load_session`, `new_session`,
    `delete_session`, `rename_session`.
  - `ipc/config.rs` — `get_config`, `set_config`, `pick_folder`, `add_allowed_path`.
  - `ipc/files.rs` — **new** `browse_dir`.
- `crates/zanto-desktop/src-tauri/src/lib.rs` — `mod ipc;` stays; update
  `tauri::generate_handler!` to the (possibly re-pathed) command set; register `browse_dir`.

## Design
- Keep `DesktopState` in `ipc/mod.rs` unchanged (other modules reference
  `super::DesktopState` / `crate::ipc::DesktopState`). `interaction::respond` stays in
  `interaction.rs` (unchanged) but its `State<DesktopState>` path still resolves.
- `browse_dir`:
  ```rust
  #[derive(Serialize)] pub struct FileEntry { pub name: String, pub path: String, pub is_dir: bool }
  #[tauri::command] pub async fn browse_dir(state: State<DesktopState>, path: Option<String>) -> Result<Vec<FileEntry>, String>
  ```
  - `path = None` → list the allowed roots (from `Settings::load().allowed_paths`) +
    `project_dir` as top-level entries. Otherwise list that dir's immediate children.
  - **Gate reads** via `state.permissions.check(&path, Op::Read)` (reuse the existing
    guard) so it can't escape granted roots; map errors to `String`. Dirs first, sorted.

## Acceptance checks
- `cargo build` clean; `cargo test -p zanto-core` unaffected.
- `pnpm check` 0 errors; `pnpm build:web` clean.
- All pre-existing IPC commands behave identically (no signature/return changes).
- `browse_dir(None)` returns the allowed roots; `browse_dir(Some(root))` lists children;
  a path outside grants is rejected.

## Notes / handoff
- B3 extends `ipc/config.rs` (providers/keys). C7/C8/E4 add commands in their own
  `ipc/*` submodule or the relevant existing one. Frontend `ipc.ts` gains `browseDir`.
- This is a mechanical refactor — lighter model is fine.
