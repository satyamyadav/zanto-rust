# Skills editor

- **Date:** 2026-06-29
- **Status:** ✅ Shipped (2026-06-29). Own `SkillsDialog` opened from the composer
  `/skill` menu ("Manage skills…"); Project|Global scope toggle; plain-textarea
  editor. Decisions taken: scope-pinned reads (yes), plain textarea v1 (no
  preview), and delete/rename of the active skill clears/follows the selection.

## Summary

A dedicated dialog to create, edit, rename, and delete the markdown skill files —
managing both project (`<project>/.zanto/skills/`) and global
(`<data_dir>/zanto/skills/`) skills via a scope toggle — opened from the
composer's `/skill` menu. The composer picker stays as-is for *selecting* a skill;
this is the authoring surface.

## Motivation

Skills are markdown files appended to the system prompt, but today they can only
be created/edited by hand in a text editor on disk — the app only lists and
selects them. The owner wants an in-app editor. It gets its own dialog (not a
Settings tab) to honor the existing decision that skills are managed from the
composer, not Settings (`SettingsDialog.svelte:307` comment, CLAUDE.md).

## Current state (grounded)

- **Disk:** `.md` files in two dirs, project shadows global by name. File stem =
  name, full body = content, no frontmatter
  (`zanto-core/src/context.rs:149-222`: `Skill{name, body}`, `list_skills`,
  `get_skill`, `skill_dirs`, `global_skills_dir`).
- **Core has NO write/delete** — only read. Must add.
- **IPC** (`ipc/skills.rs`): `list_skills() -> Vec<SkillDto>` (`SkillDto{name,
  preview:120ch}` — no body, no scope, no path), `set_active_skill(name)`.
  Registered `lib.rs:124-125`.
- **Frontend** (`ipc.ts:117,308-310`): `SkillDto{name,preview}`, `listSkills`,
  `setActiveSkill`. Composer `/skill` menu: `openSkillMenu`/`selectSkill`/
  `clearActiveSkill` + the active-skill chip (`Composer.svelte:194-486,616-626`).
- **Dialog pattern** to mirror: `settingsStore` (`stores/settings.svelte.ts`) +
  `SettingsDialog.svelte` (`Dialog.Root`/`Dialog.Content`), mounted in
  `routes/+page.svelte:41`.
- **Mock** (`mock/backend.ts:16,188-192`): seeds `reviewer`/`researcher`,
  `activeSkill` state. UI tests C-skill / C-skill-filter
  (`tests/ui/chat-behaviors.spec.ts:531,581`).

## Scope

**In scope**
- **Core CRUD API** in `context.rs`: `write_skill`, `delete_skill`,
  `rename_skill`, and a `read_skill_body` (or reuse `get_skill`), each scope-aware
  (project vs global), with name validation.
- **IPC commands**: `read_skill`, `save_skill`, `delete_skill`, `rename_skill`
  (scope param). Extend `SkillDto` with `scope` so the UI can label/group; add a
  `read_skill` that returns the full body for editing.
- **A `SkillsDialog`**: left = skill list grouped/filtered by a Project|Global
  scope toggle with New/delete/rename affordances; right = a markdown `<textarea>`
  editor for the selected skill's body, with Save. Opened from a new "Manage
  skills…" row in the composer `/skill` menu.
- **Permissions**: writes go through a dedicated IPC path that resolves the skill
  dir itself (NOT the model-facing `write_file` tool); the dir is app-owned, so no
  interactive approval — but the path is validated to stay inside the resolved
  skills dir (no traversal).
- **Mock**: extend the mock backend with the new commands over an in-memory skill
  map so the dialog works in `dev:mock` and is testable.

**Out of scope**
- The composer's *select/clear* flow and the active-skill chip — unchanged (the
  editor may refresh the picker's list, nothing more).
- Skill frontmatter / metadata / categories — skills stay plain markdown
  (stem = name, body = content). No schema change.
- Syntax highlighting / live markdown preview in the editor (plain textarea for
  v1; a preview pane is a possible follow-up).
- Settings dialog (no new tab).
- Versioning/history of skill files.

## Affected files

- `crates/zanto-core/src/context.rs` — add `write_skill` / `delete_skill` /
  `rename_skill` (+ a body reader if `get_skill` isn't reused), scope-aware, with
  name validation + a private `skill_path(scope, name)` helper and a
  `SkillScope { Project, Global }` enum. New unit tests.
- `crates/zanto-desktop/src-tauri/src/ipc/skills.rs` — add `read_skill`,
  `save_skill`, `delete_skill`, `rename_skill` commands; add `scope` to `SkillDto`;
  a `Scope` DTO mapping. Resolve dirs from `Settings` (project) /
  `context::global_skills_dir` (must be made `pub` or re-exposed).
- `crates/zanto-desktop/src-tauri/src/lib.rs` — register the 4 new commands
  (after line 125).
- `crates/zanto-desktop/src/lib/ipc.ts` — extend `SkillDto` with `scope`; add
  `readSkill` / `saveSkill` / `deleteSkill` / `renameSkill` bindings; a `SkillScope`
  type.
- `crates/zanto-desktop/src/lib/stores/skills.svelte.ts` (new) — a tiny
  `skillsStore { open }` + `openSkillsEditor()` mirroring `settingsStore`.
- `crates/zanto-desktop/src/lib/components/SkillsDialog.svelte` (new) — the editor.
- `crates/zanto-desktop/src/lib/components/Composer.svelte` — add a "Manage
  skills…" action to the `/skill` menu (or the slash registry) that calls
  `openSkillsEditor()`; refresh `skills` after the dialog closes.
- `crates/zanto-desktop/src/routes/+page.svelte` — mount `<SkillsDialog>`.
- `crates/zanto-desktop/src/lib/mock/backend.ts` — implement the new commands over
  an in-memory map seeded with the existing two skills (+ scope).

## Implementation steps

1. **Core: scope + name validation + path helper** (`context.rs`)
   - Add `pub enum SkillScope { Project, Global }`.
   - `fn skill_dir(scope, project_dir) -> Option<PathBuf>` (project →
     `<proj>/.zanto/skills`, global → `global_skills_dir()`). Make
     `global_skills_dir` reachable (pub(crate) → pub, or a thin pub wrapper).
   - `fn validate_skill_name(name) -> Result<(), String>`: non-empty, no path
     separators / `..` / leading dot, a conservative charset (alnum, `-`, `_`,
     space). Reject anything that wouldn't be a safe single filename.

2. **Core: write / delete / rename / read body** (`context.rs`)
   - `pub fn write_skill(scope, project_dir, name, body) -> Result<(), String>`:
     validate name, ensure dir exists (`create_dir_all`), write `<dir>/<name>.md`.
   - `pub fn delete_skill(scope, project_dir, name) -> Result<(), String>`:
     validate, remove the file (error if absent).
   - `pub fn rename_skill(scope, project_dir, old, new) -> Result<(), String>`:
     validate both, rename the `.md` (error if target exists).
   - Body for editing: reuse `get_skill(project_dir, name)` (already returns the
     body) OR add a scope-pinned `read_skill_body(scope, project_dir, name)` if the
     shadowing in `get_skill` would read the wrong dir — DECIDE in impl and note
     it (the editor must read the file IN the chosen scope, not the shadowed one).
   - Unit tests: write→list→read round-trip per scope; delete; rename;
     name-validation rejects traversal; project shadows global on list but the
     editor reads the right scope.

3. **IPC: DTO + commands** (`ipc/skills.rs`, `lib.rs`)
   - Add `scope: String` ("project" | "global") to `SkillDto`; `list_skills` tags
     each (it already iterates both dirs in core — surface which dir each came
     from; this needs core `list_skills` to report scope, OR list per-scope in the
     IPC layer by calling the dirs separately — pick the lower-churn option, note
     it).
   - `read_skill(name, scope) -> Result<String, String>` (full body).
   - `save_skill(name, scope, body) -> Result<SkillDto, String>` (write + return
     fresh DTO w/ preview for instant list refresh).
   - `delete_skill(name, scope) -> Result<(), String>`.
   - `rename_skill(old, new, scope) -> Result<(), String>`.
   - All resolve project dir from `Settings::load()` (like `list_skills` does) and
     map errors to `String`. Register all 4 in `lib.rs`.

4. **Frontend IPC bindings** (`ipc.ts`)
   - `export type SkillScope = "project" | "global";` extend `SkillDto` with
     `scope: SkillScope`.
   - `readSkill`, `saveSkill`, `deleteSkill`, `renameSkill` invoking the new
     commands.

5. **Skills store** (`stores/skills.svelte.ts`, new)
   - `export const skillsStore = $state<{ open: boolean }>({ open: false });` +
     `export function openSkillsEditor() { skillsStore.open = true; }` — mirrors
     `settingsStore`.

6. **SkillsDialog** (`components/SkillsDialog.svelte`, new)
   - `Dialog.Root bind:open`; two-pane `Dialog.Content` like SettingsDialog.
   - Left: a Project|Global scope toggle; the filtered skill list for that scope;
     a "+ New" affordance; per-row delete + rename. Selecting a row loads its body
     via `readSkill`.
   - Right: a `<textarea>` bound to the editing body + a Save button (calls
     `saveSkill`); New creates an empty unsaved draft (prompt for a name on first
     save, validated — surface core validation errors via toast).
   - On any mutation, refresh the list. Errors → `toast.error`.
   - Empty states: no skills in scope → a "Create your first skill" prompt;
     global scope always available, project scope shows a "set a project" hint when
     none is active (reuse the existing project-dir signal from `appStore.config`).

7. **Composer hook** (`Composer.svelte`)
   - Add a "Manage skills…" entry to the `/skill` menu footer (or the slash
     registry) → `openSkillsEditor()`. After the dialog closes, re-run
     `ipc.listSkills()` so the picker reflects edits. Do not touch select/clear.

8. **Mount + mock**
   - `routes/+page.svelte`: mount `<SkillsDialog bind:open={skillsStore.open} />`.
   - `mock/backend.ts`: back the new commands with an in-memory
     `Map<scope+name, body>` seeded from the current two skills (give them a
     scope); `list_skills` returns them with scope; read/save/delete/rename mutate
     the map so the dialog is fully exercisable in `dev:mock`.

## Edge cases & risks

- **No new dependency.** Core fs + existing dialog primitives.
- **Path traversal / unsafe names** — the ONLY real risk: a skill name must never
  escape the skills dir. Step 1's `validate_skill_name` is the gate; the IPC layer
  must not bypass it. Test traversal (`../foo`, `a/b`, `.hidden`) is rejected.
- **Project shadows global** — `get_skill`/`list_skills` dedupe by name with
  project winning. The editor edits a *specific scope's file*; reading must be
  scope-pinned so editing the global `reviewer` while a project `reviewer` exists
  opens the global file, not the shadowing project one (step 2 note).
- **No active project** — project scope is unusable without a project dir; the UI
  must disable/empty-state it, not error. Global scope always works.
- **Editing the active skill** — if the user edits/deletes the currently-selected
  skill, the next turn picks up the new body (skill is read fresh per turn from
  disk via `get_skill` in `chat.rs`); deleting the active skill should clear or
  warn — at minimum the next turn's `get_skill` returns `None` and the skill
  section is omitted (acceptable; optionally clear `selected_skill` on delete).
- **Concurrent on-disk edits** — out of scope; last write wins.
- **Dir creation** — global/project skills dir may not exist yet; `write_skill`
  must `create_dir_all`.
- **Permissions** — writes are app-owned (the skills dirs), NOT model-driven, so
  they bypass the `PermissionGuard` (which gates model file access). This is
  correct — but the IPC commands must resolve the path from the scope, never from
  a model/string, and validate the name.

## Acceptance criteria

Verifiable in the desktop app (mock reproduces CRUD) and `cargo test`:

- [ ] From the composer `/skill` menu, "Manage skills…" opens the SkillsDialog.
- [ ] The dialog lists skills for the selected scope; toggling Project|Global
      switches the list and the dir written to.
- [ ] Creating a skill (name + body) writes `<dir>/<name>.md`; it then appears in
      the composer `/skill` picker.
- [ ] Editing a skill's body and saving updates the file; reopening shows the new
      body; the next chat turn uses the updated skill.
- [ ] Renaming changes the filename; deleting removes it; both reflect in the
      picker.
- [ ] An unsafe name (`../x`, `a/b`, leading dot) is rejected with a clear error,
      no file written outside the skills dir.
- [ ] Editing global `reviewer` while a project `reviewer` exists edits the GLOBAL
      file (scope-pinned), not the shadowing project one.
- [ ] With no project set, project scope is empty-stated (not errored); global
      scope works.
- [ ] `cargo test -p zanto-core` green incl. new CRUD + validation tests.
- [ ] `pnpm check` 0/0; UI suite passes (existing C-skill / C-skill-filter still
      green — the select flow is untouched).

## Manual test plan

1. `cargo test -p zanto-core` → new write/delete/rename/validation tests pass.
2. `pnpm dev:mock`; composer `/skill` → "Manage skills…" → dialog opens with the
   two seeded skills.
3. New skill "tester" (global) with a body → Save → appears in the list and in the
   `/skill` picker.
4. Edit "tester" body → Save → reopen → new body shows.
5. Rename "tester"→"qa"; delete "qa" → both reflected.
6. Try name "../evil" → rejected with a toast, nothing written.
7. Toggle to Project scope with no project → empty-state hint, no error; set a
   project (existing flow) → project scope becomes usable.
8. Real app (`cargo run -p zanto-desktop` … actually `zanto-desktop`): create a
   skill, select it in the composer, send a turn → the skill steers the reply;
   confirm the `.md` exists on disk in the chosen dir.
