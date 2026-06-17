# Inputs & Outputs — rework context sources + project dir (better UX)

- **Date:** 2026-06-17
- **Ask:** "Input output: context source and project dir" + "feature is copy of old app,
  find a better approach and UX."

## Problem
The G3 wiring exposed context sources as a flat folder list bolted into Settings —
a copy of the existing allowed-paths/"Folder access" pattern. Project dir isn't surfaced
at all. There's no coherent model of **what the agent reads (inputs)** vs **where it
writes (outputs)**. This spec replaces the copied approach with a purpose-built one.

## Concept: one Workspace model
- **Inputs (read context):** files/dirs fed to every turn as context (`context_sources`).
- **Project (output + read):** the project directory. `.zanto/` (configs, saved
  documents/artifacts) is written here; it's also readable as a source. One per workspace.
- Permissions/allowed-paths stay as the *security* layer (what the agent may touch);
  Inputs/Project are the *intent* layer (what it should use). Selecting a Project or Input
  auto-grants read on it (calls `add_allowed_path`) so the two layers stay consistent.

## UX (the "better approach")
A dedicated **Workspace** surface (not buried in the Settings folder list) — e.g. a panel
opened from the Sidebar, or a left-rail section:
- **Project** row: shows the current project dir (or "No project — outputs go to the global
  store"), a **Set project** button (folder picker), and a note of where outputs land
  (`<project>/.zanto/artifacts`). Clear, single value.
- **Context sources** list: each entry shows name, file/dir icon, and an **enable toggle**
  (use without deleting) — add `enabled: bool` per source. Add via the folder/file picker
  **or** the file browser (`browse_dir`) so you can pick from the tree. Remove per row.
- **Active-context summary near the composer:** a small chip — "◇ 3 sources · ~/work" —
  that opens the Workspace surface; makes it obvious what's feeding the agent. Optional:
  show approximate size / a warning when sources exceed the context cap (A4 has caps).
- Drop the cramped Settings "Context sources" section in favor of this surface (or keep a
  link to it from Settings).

## Backend changes
- `Settings.context_sources` becomes a list of `{ path, enabled }` (migrate old `Vec<String>`
  via serde — accept both shapes). `load_context` filters to `enabled`.
- IPC: `set_project_dir(path)` (+ persist + auto allow-path), `list_context_sources` /
  `add` / `remove` / `toggle_context_source(path, enabled)`. Expose project_dir + sources in
  `ConfigDto`.
- `send_message` already injects `load_context(enabled sources)`; keep.

## Affected files
- `crates/zanto-core/src/config.rs` (context_sources shape + migration), `context.rs`
  (honor `enabled`), `crates/zanto-desktop/src-tauri/src/ipc/config.rs` (commands),
  `lib.rs` (register), `ipc.ts` (wrappers/types), new
  `crates/zanto-desktop/src/lib/components/Workspace.svelte` (the surface) + Sidebar entry +
  a composer context chip; remove the old Settings context list.

## Resolved (user)
- The Workspace surface is a **Sidebar dialog** (a "Workspace" button, consistent with
  Settings/Artifacts) holding Project dir + context sources; a context chip sits by the composer.

## Open question
- Single project per app/workspace (recommended) vs per-session?

## Acceptance
- Build-check clean; old `Vec<String>` settings still load (migration). Manual: set a
  project, add/toggle/remove sources, see the composer chip reflect active context, and
  confirm a turn actually uses an enabled source (and ignores a disabled one).
