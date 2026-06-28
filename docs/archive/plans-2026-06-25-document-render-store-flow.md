# Document Render+Store Flow — Implementation Plan

> Execute task-by-task. Steps use checkbox (`- [ ]`) syntax. After each task: build/test gate, then commit.

**Goal:** Stop the agent dropping generated documents into the project root via `write_file`; steer it to render+store as artifacts, and give the user a deterministic "Save to project…" button on the Canvas to persist a rendered document on demand.

**Architecture:** Two independent layers. Layer 1 (zanto-core + ipc/chat.rs): tool-description + system-prompt steering so the model uses `store_artifact`/`render_artifact` for generated documents instead of `write_file`. Layer 3 (UI + a thin IPC): a "Save to project…" button on markdown Canvas content that writes the text to a user-chosen path via a native save dialog, mirroring the existing `save_artifact_copy` pattern. (Layer 2 — auto-store-on-render — is prompt-only here, folded into Layer 1; the stronger Rust auto-store is a noted follow-up, not built.)

**Tech Stack:** Rust (zanto-core, Tauri commands, rmcp tools, genai), Svelte 5 + Tailwind (Canvas.svelte), tauri-plugin-dialog. Build: `cargo build`, `cargo test`; frontend: `pnpm check` in `crates/zanto-desktop`.

## Global Constraints

- Permission model unchanged: `set_project_dir` still auto-allows the project dir. No read-only mode, no removal of the auto-allow.
- `write_file` behavior + its `check(Op::Write)` gate unchanged — only its description string changes.
- `render_artifact` / `store_artifact` / `pin_artifact` mechanics unchanged.
- Layers 1 and 3 are independent and committed separately. Layer 1 ships first.
- `cargo build` + `cargo test` green; `pnpm check` 0/0.
- Steering is guidance, not enforcement (the model can still call write_file) — that is the chosen design, not a bug.

## File Structure

- `crates/zanto-core/src/tools/fs/write_file.rs` — description rescoped; new test. (Task 1)
- `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` — `ARTIFACT_PROTOCOL` clause; new substring test. (Task 2)
- `crates/zanto-desktop/src-tauri/src/ipc/files.rs` — new `save_document_to_project` IPC command. (Task 3)
- `crates/zanto-desktop/src-tauri/src/lib.rs` — register the new command in `generate_handler!`. (Task 3)
- `crates/zanto-desktop/src/lib/ipc.ts` — TS wrapper for the new command. (Task 4)
- `crates/zanto-desktop/src/lib/components/Canvas.svelte` — "Save to project…" button for markdown canvas. (Task 4)

---

### Task 1: Rescope the `write_file` description (Layer 1a)

**Files:**
- Modify: `crates/zanto-core/src/tools/fs/write_file.rs`

- [ ] **Step 1: Replace the description**

Find:

```rust
    fn description() -> Option<Cow<'static, str>> {
        Some("Write content to a file, creating it and any missing parent directories".into())
    }
```

Replace with:

```rust
    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Write content to a specific file the user named or that you are editing \
             (source code, a config file, an existing file at a known path). Do NOT use \
             this to save a document, article, report, or notes you generated — persist \
             those with store_artifact (durable, browsable in Artifacts) and show them \
             with render_artifact. Creates any missing parent directories."
                .into(),
        )
    }
```

- [ ] **Step 2: Add a test module asserting the steering**

`write_file.rs` has no test module. Append one at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::handler::server::router::tool::ToolBase;

    #[test]
    fn description_steers_generated_docs_to_artifacts() {
        let d = WriteFile::description().expect("description present");
        assert!(d.contains("store_artifact"), "should redirect docs to store_artifact: {d}");
        assert!(d.contains("Do NOT"), "should discourage doc drops: {d}");
    }
}
```

(If `ToolBase` is already in scope at module level, drop the duplicate `use`. Verify the trait path matches the one used at the top of the file — it imports `rmcp::handler::server::router::tool::{AsyncTool, ToolBase}` in the sibling `store_artifact.rs`; match whatever `write_file.rs` actually imports.)

- [ ] **Step 3: Build + test**

Run: `cargo test -p zanto-core write_file`
Expected: the new test passes; no other test breaks. If `WriteFile::description()` isn't callable in the test (trait not in scope), adjust the `use` to the exact trait path `write_file.rs` imports for `ToolBase`.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-core/src/tools/fs/write_file.rs
git commit -m "feat(core): steer write_file away from generated documents toward artifacts"
```

---

### Task 2: Add the "write a document" clause to ARTIFACT_PROTOCOL (Layer 1b)

**Files:**
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/chat.rs`

- [ ] **Step 1: Append the clause to `ARTIFACT_PROTOCOL`**

The `ARTIFACT_PROTOCOL` const string ends with the `read_document` guidance:

```rust
... Images are not OCR'd; attach them to a vision-capable model instead.";
```

Insert a new sentence into the string immediately before that final `read_document`/images guidance (or append just before the closing `";` — anywhere inside the const is fine, but keep it adjacent to the store/render explanation for coherence). Add:

```
When the user asks you to WRITE a document, article, report, or notes (prose you generate, \
not an edit to a file the user named): call `store_artifact` to persist it (it appears in the \
Artifacts browser) and `render_artifact` to show it in the Canvas. Do NOT use `write_file` to \
drop a generated document into the project folder — `write_file` is only for editing a specific \
file the user named. \
```

Concretely, change the tail of the const from:

```rust
... Use render_artifact to display, store_artifact to persist a document. \
`pin_artifact` KEEPS a view+data artifact ...
```

to insert the new clause right after the `Use render_artifact to display, store_artifact to persist a document.` sentence:

```rust
... Use render_artifact to display, store_artifact to persist a document. \
When the user asks you to WRITE a document, article, report, or notes (prose you generate, \
not an edit to a file the user named): call `store_artifact` to persist it (it appears in the \
Artifacts browser) and `render_artifact` to show it in the Canvas. Do NOT use `write_file` to \
drop a generated document into the project folder — `write_file` is only for editing a specific \
file the user named. \
`pin_artifact` KEEPS a view+data artifact ...
```

- [ ] **Step 2: Add a substring test**

In the `chat.rs` test module (or add one if none exists — check for `#[cfg(test)]` in the file first), add:

```rust
#[test]
fn artifact_protocol_steers_document_writes_to_store() {
    assert!(ARTIFACT_PROTOCOL.contains("WRITE a document"));
    assert!(ARTIFACT_PROTOCOL.contains("Do NOT use `write_file`"));
}
```

If `chat.rs` has no `#[cfg(test)] mod tests`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_protocol_steers_document_writes_to_store() {
        assert!(ARTIFACT_PROTOCOL.contains("WRITE a document"));
        assert!(ARTIFACT_PROTOCOL.contains("Do NOT use `write_file`"));
    }
}
```

(`ARTIFACT_PROTOCOL` is a private `const` in this module, so `use super::*;` brings it into scope.)

- [ ] **Step 3: Build + test**

Run: `cargo test -p zanto-desktop artifact_protocol` (or `cargo test` for the desktop crate's tauri lib — confirm the crate name; the src-tauri crate is `zanto-desktop` per its Cargo.toml `[package] name`). If the test target name differs, run `cargo test -p <src-tauri-crate>`.
Expected: the new test passes.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/ipc/chat.rs
git commit -m "feat(desktop): steer 'write a document' to store+render, not write_file"
```

- [ ] **Step 5: Manual verification of Layer 1**

Run the app (`pnpm tauri dev` or the user's normal launch). Ask: "write an article on X". Confirm the agent now calls `store_artifact` + `render_artifact` (article renders in Canvas AND appears in the Artifacts panel) rather than `write_file` into the project root. Note: this is model-dependent steering; if a weak model still uses write_file, record it — Task 5 (Layer 3) still gives the deterministic save, and the spec's Layer 2(B) auto-store is the escalation.

---

### Task 3: `save_document_to_project` IPC command (Layer 3 backend)

**Files:**
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/files.rs`
- Modify: `crates/zanto-desktop/src-tauri/src/lib.rs` (register command)

**Interfaces:**
- Produces: `save_document_to_project(text: String, suggested_name: String) -> Result<bool, String>` — opens a native save dialog defaulted into the project dir with `suggested_name`, writes `text` to the chosen path, returns `false` if cancelled. Consumed by Task 4.

**Verified facts (read before writing):**
- `project_dir` is NOT held on `DesktopState`. The codebase reads it fresh via `Settings::load()` (see `get_config` at `config.rs:30` → `let settings = Settings::load();`) or the in-crate helper `load_project_settings()` (`config.rs:165`). Use `Settings::load().project_dir` (an `Option<String>`) — no `State` needed for the dir. `Settings` is from `zanto_core::config` (already imported in `config.rs`; import it in `files.rs`).
- `save_artifact_copy` (`artifacts.rs:197`) is the exact working reference for the dialog API: `app.dialog().file().set_file_name(..).set_title(..).blocking_save_file()`, then `chosen.into_path()`, then `std::fs::write`.

- [ ] **Step 1: Add the command, mirroring `save_artifact_copy`**

In `files.rs`, add (import `tauri_plugin_dialog::DialogExt` locally as `save_artifact_copy` does, and `zanto_core::config::Settings`):

```rust
/// Save a rendered document (e.g. the markdown shown in the Canvas) to a file
/// the user picks. Defaults the dialog into the project dir when one is set, so
/// the common "save to project" path is one click. Writes `text` verbatim.
/// Returns Ok(false) if the user cancels the dialog.
#[tauri::command]
pub async fn save_document_to_project(
    app: tauri::AppHandle,
    text: String,
    suggested_name: String,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;
    use zanto_core::config::Settings;

    let project_dir = Settings::load().project_dir;

    let mut dialog = app
        .dialog()
        .file()
        .set_file_name(&suggested_name)
        .set_title("Save document");
    if let Some(dir) = project_dir.as_deref() {
        dialog = dialog.set_directory(dir);
    }

    let Some(chosen) = dialog.blocking_save_file() else {
        return Ok(false);
    };
    let dest = chosen.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&dest, text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(true)
}
```

If `zanto_core::config::Settings` is already imported at the top of `files.rs`, drop the local `use`. Confirm the dialog builder methods (`set_directory`, `blocking_save_file`, `into_path`) compile — they are copied verbatim from `save_artifact_copy`, which builds against the installed `tauri-plugin-dialog`.

- [ ] **Step 2: Register the command**

In `lib.rs`, find the `tauri::generate_handler![ ... ]` list and add `ipc::files::save_document_to_project` (match the module path used for the other `files` commands like `browse_dir`/`open_path`).

- [ ] **Step 3: Build**

Run: `cargo build -p zanto-desktop` (or the src-tauri crate name).
Expected: compiles. Fix the `project_dir` accessor / dialog-builder API to match the installed `tauri-plugin-dialog` version (the `set_directory`/`blocking_save_file`/`into_path` calls mirror `save_artifact_copy`, which already compiles — copy its exact API usage).

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/ipc/files.rs crates/zanto-desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): add save_document_to_project IPC (native save dialog into project dir)"
```

---

### Task 4: Canvas "Save to project…" button (Layer 3 UI)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/ipc.ts` (wrapper)
- Modify: `crates/zanto-desktop/src/lib/components/Canvas.svelte`

**Interfaces:**
- Consumes: `save_document_to_project` from Task 3.

- [ ] **Step 1: Add the TS IPC wrapper**

In `ipc.ts`, alongside the other `invoke` wrappers (match the existing style — e.g. how `open_path`/`save_artifact_copy` are wrapped if present), add:

```ts
  saveDocumentToProject: (text: string, suggestedName: string): Promise<boolean> =>
    invoke("save_document_to_project", { text, suggestedName }),
```

(Confirm the arg-casing convention: Tauri maps snake_case command args; check whether existing wrappers pass camelCase keys that Tauri converts, or snake_case. Match a neighboring wrapper exactly — e.g. `save_artifact_copy`'s wrapper if it exists, else `read_image_data_url`.)

- [ ] **Step 2: Add the button to the markdown Canvas branch**

In `Canvas.svelte`, the agent-canvas branch is:

```svelte
  {:else if sessionStore.canvas}
    <div class="h-full overflow-auto p-4">
      <Block block={sessionStore.canvas} canPin={false} />
    </div>
```

Replace with a version that shows a "Save to project…" action when the canvas block is a markdown document:

```svelte
  {:else if sessionStore.canvas}
    <div class="flex h-full flex-col">
      {#if sessionStore.canvas.kind === "markdown"}
        <div class="flex shrink-0 items-center justify-end border-b border-border px-4 py-2">
          <Button size="sm" variant="outline" onclick={saveCanvasDocument}>
            <SaveIcon class="size-3.5" />
            Save to project…
          </Button>
        </div>
      {/if}
      <div class="min-h-0 flex-1 overflow-auto p-4">
        <Block block={sessionStore.canvas} canPin={false} />
      </div>
    </div>
```

- [ ] **Step 3: Add the handler + imports**

In `Canvas.svelte`'s `<script>`, add the import and handler (match existing toast import style — Canvas already imports `Button`; add `SaveIcon` and `toast`):

```ts
  import SaveIcon from "@lucide/svelte/icons/save";
  import { toast } from "svelte-sonner";
  import { ipc } from "$lib/ipc";

  // Derive a default filename from the document's first heading, else a slug.
  function suggestedName(text: string): string {
    const firstHeading = text.split("\n").find((l) => l.startsWith("#"));
    const base = (firstHeading ?? "document")
      .replace(/^#+\s*/, "")
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 60) || "document";
    return `${base}.md`;
  }

  async function saveCanvasDocument() {
    const block = sessionStore.canvas;
    if (!block || block.kind !== "markdown") return;
    try {
      const saved = await ipc.saveDocumentToProject(block.text, suggestedName(block.text));
      if (saved) toast.success("Document saved");
    } catch (e) {
      toast.error("Could not save the document", { description: `${e}` });
    }
  }
```

(Verify `ipc` isn't already imported in Canvas.svelte; if it is, don't duplicate. Verify `sessionStore.canvas` is the `ChatBlock | null` with `kind: "markdown"; text: string` — confirmed in the spec.)

- [ ] **Step 4: Build gate**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Manual + visual verification**

Run the app. Ask "write an article on X" so a markdown document renders in the Canvas. Confirm:
- A "Save to project…" button shows above the rendered markdown (and NOT for chart/table/metric canvas views).
- Clicking it opens a native save dialog defaulted into the project dir with a sensible `.md` filename.
- Saving writes the file; "Document saved" toast appears.
- Cancelling the dialog shows no error toast.

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-desktop/src/lib/ipc.ts crates/zanto-desktop/src/lib/components/Canvas.svelte
git commit -m "feat(desktop): Save to project button on markdown Canvas documents"
```

---

### Task 5: Full verification gate

**Files:** none.

- [ ] **Step 1: Workspace build + tests**

Run: `cargo build` then `cargo test`
Expected: green (the two new Rust tests pass; nothing else regresses).

- [ ] **Step 2: Frontend gate**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: `0 ERRORS 0 WARNINGS`.

- [ ] **Step 3: End-to-end manual flow**

Ask "write an article on X". Confirm the full intended flow:
1. Renders in the Canvas + appears in the Artifacts panel (Layer 1 steering worked), and did NOT silently drop a file in the project root.
2. "Save to project…" persists it to the project dir on explicit click (Layer 3).

Record the result. If step 1's render+store didn't happen (model still used write_file), note it as the trigger for the spec's Layer 2(B) follow-up (auto-store on markdown render) — out of scope for this plan.

---

## Self-Review

**Spec coverage:**
- Layer 1a (write_file description) → Task 1. ✓
- Layer 1b (ARTIFACT_PROTOCOL clause) → Task 2. ✓
- Layer 2 (render+store) → prompt-only via Task 1/2 instruction; Layer 2(B) explicitly deferred. ✓
- Layer 3 (Canvas Save button + IPC) → Tasks 3, 4. ✓
- Gating unchanged → Global Constraints; no task touches the permission model. ✓
- Tests → Task 1 Step 2, Task 2 Step 2; verification → Task 5. ✓

**Placeholder scan:** No TBD/TODO. The two "confirm the exact accessor/API" notes (project_dir read in Task 3, arg-casing in Task 4) are real verification steps against existing code (`save_artifact_copy`, neighboring wrappers), not placeholders — each names the concrete reference to copy.

**Type/name consistency:** `save_document_to_project(text, suggested_name)` defined in Task 3, wrapped as `saveDocumentToProject(text, suggestedName)` in Task 4 Step 1, called in Task 4 Step 3. `sessionStore.canvas` is `ChatBlock | null` with the `markdown` variant `{ kind: "markdown"; text: string }` (confirmed). `SaveIcon` from `@lucide/svelte/icons/save`. ✓

**Risk:** Task 3's `project_dir` accessor and dialog API are the one place needing a read of existing code before writing — flagged in-step, with `save_artifact_copy` named as the exact working reference to copy.
