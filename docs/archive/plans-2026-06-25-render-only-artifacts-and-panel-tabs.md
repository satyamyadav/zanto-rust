# Render-only docs, deliberate save, panel tabs, artifact upsert+sort — Plan

> Execute task-by-task. Steps use checkbox (`- [ ]`). Build/test gate then commit after each.

**Goal:** Make generated documents render-only (not auto-stored); add a deliberate Save that stores an artifact (upserting, no dup); sort the Artifacts list newest-first; and tab the right panel so opening Artifacts no longer destroys the rendered view.

**Architecture:** Four layers. B (core `ArtifactStore`: upsert-by-(title,scope) + recency sort). A (prompt: render-only steering). D (Save button → new `store_document_artifact` IPC over `Store::save`; remove the old native-file IPC). C (panel tabs in Canvas; `openArtifacts` no longer nulls the canvas; a refresh signal so the browser reflects a save).

**Tech Stack:** Rust (zanto-core artifacts, Tauri ipc), Svelte 5 + Tailwind. Build: `cargo build`/`cargo test`; `pnpm check` in `crates/zanto-desktop`.

## Global Constraints

- No permission-model change. Pinned views (DB) untouched. `save_artifact_copy` (native "Save a copy") stays.
- `Store::save` callers: only `store_artifact` tool + the new IPC. Upsert must not break the tool's existing return shape (returns the `ArtifactRef` JSON).
- `cargo test` green; `pnpm check` 0/0.
- Verification: core via unit tests; UI via mock dev server + the existing throwaway-scenario pattern (add, verify, revert).

## File Structure

- `crates/zanto-core/src/artifacts/mod.rs` — `save` upsert; `list` recency sort; 2 tests. (Task 1)
- `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` — `ARTIFACT_PROTOCOL` render-only; update test. (Task 2)
- `crates/zanto-core/src/tools/fs/write_file.rs` — keep description steer; adjust test only if wording drifts. (Task 2, minor)
- `crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs` — new `store_document_artifact` cmd. (Task 3)
- `crates/zanto-desktop/src-tauri/src/ipc/files.rs` + `lib.rs` — remove `save_document_to_project`. (Task 3)
- `crates/zanto-desktop/src/lib/ipc.ts` — swap wrapper. (Task 3)
- `crates/zanto-desktop/src/lib/components/Message.svelte` — Save → store artifact; relabel. (Task 3)
- `crates/zanto-desktop/src/lib/stores/session.svelte.ts` — `artifactsTick` refresh signal + don't-null-canvas. (Task 4)
- `crates/zanto-desktop/src/lib/components/Sidebar.svelte` — `openArtifacts` keeps canvas. (Task 4)
- `crates/zanto-desktop/src/lib/components/Canvas.svelte` — panel tabs. (Task 4)
- `crates/zanto-desktop/src/lib/components/ArtifactBrowser.svelte` — track refresh signal. (Task 4)

---

### Task 1: Core — upsert by (title, scope) + recency sort

**Files:** Modify `crates/zanto-core/src/artifacts/mod.rs`

**Interfaces:**
- `Store::save(kind, title, bytes, scope) -> Result<ArtifactRef>` — unchanged signature; behavior becomes upsert (reuse id/rel_path of an existing same-(title,scope) entry, overwrite blob, refresh `created_at`).
- `Store::list(scope) -> Result<Vec<ArtifactRef>>` — now sorted `created_at` desc.

- [ ] **Step 1: Upsert in `save`**

Replace the body of `save` (currently: `let id = new_id();` … `index.push(art.clone());`) so it looks up an existing entry first. Find:

```rust
        let root = self.root(scope)?;
        let id = new_id();
        let ext = ext_for(kind, title);
        let rel_path = format!("files/{id}.{ext}");

        let file_path = root.join(&rel_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, bytes)?;

        let art = ArtifactRef {
            id,
            kind,
            title: title.to_string(),
            rel_path,
            scope,
            created_at: unix_now_pub(),
        };

        let mut index = read_index(&root)?;
        index.push(art.clone());
        write_index_atomic(&root, &index)?;
        Ok(art)
```

Replace with:

```rust
        let root = self.root(scope)?;
        let mut index = read_index(&root)?;

        // Upsert by (title, scope): if a same-title entry exists in this scope,
        // reuse its id + rel_path, overwrite the blob, and refresh created_at so
        // it sorts as most-recent. Otherwise create a fresh entry.
        let existing = index.iter().position(|a| a.title == title && a.scope == scope);

        let (id, rel_path) = match existing {
            Some(i) => (index[i].id.clone(), index[i].rel_path.clone()),
            None => {
                let id = new_id();
                let ext = ext_for(kind, title);
                (id.clone(), format!("files/{id}.{ext}"))
            }
        };

        let file_path = root.join(&rel_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, bytes)?;

        let art = ArtifactRef {
            id,
            kind,
            title: title.to_string(),
            rel_path,
            scope,
            created_at: unix_now_pub(),
        };

        match existing {
            Some(i) => index[i] = art.clone(),
            None => index.push(art.clone()),
        }
        write_index_atomic(&root, &index)?;
        Ok(art)
```

Note: on upsert, `ext` is taken from the existing `rel_path` (kept), so a title that changes kind keeps the original extension — acceptable; titles are stable per document. (If `ext_for` matters for a changed kind, that's an edge case out of scope.)

- [ ] **Step 2: Sort `list` by recency**

In `list`, after building `out`, sort descending by `created_at` before returning. Change the tail of `list` from:

```rust
        }
        Ok(out)
    }
```

to:

```rust
        }
        // Newest first across whatever scopes were collected.
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }
```

- [ ] **Step 3: Tests**

Add to the existing `#[cfg(test)] mod tests`. VERIFIED: tests use the
`global_store()` helper returning `(store, _dir)` (an `$ZANTO_ARTIFACTS`-isolated
tempdir) and operate on `Scope::Global`; `read` returns `(ArtifactRef, Vec<u8>)`.
Mirror that exactly — do NOT use `ArtifactStore::new(Some(path))`/`Scope::Project`
(parallel env-var isolation is built into `global_store()`):

```rust
    #[test]
    fn save_upserts_same_title_in_scope() {
        let (store, _dir) = global_store();
        store.save(ArtifactKind::Markdown, "Doc", b"v1", Scope::Global).unwrap();
        store.save(ArtifactKind::Markdown, "Doc", b"v2", Scope::Global).unwrap();
        let listed = store.list(Some(Scope::Global)).unwrap();
        let docs: Vec<_> = listed.iter().filter(|a| a.title == "Doc").collect();
        assert_eq!(docs.len(), 1, "same-title save should upsert, not duplicate");
        let (_, bytes) = store.read(&docs[0].id).unwrap();
        assert_eq!(bytes, b"v2", "content should be the latest save");
    }

    #[test]
    fn list_is_sorted_newest_first() {
        let (store, _dir) = global_store();
        let a = store.save(ArtifactKind::Markdown, "A", b"a", Scope::Global).unwrap();
        let b = store.save(ArtifactKind::Markdown, "B", b"b", Scope::Global).unwrap();
        let listed = store.list(Some(Scope::Global)).unwrap();
        // Newest first: B must not sort after A. (created_at is 1s-resolution, so
        // a tie is possible; the assertion tolerates a tie via `<=`.)
        let ia = listed.iter().position(|x| x.id == a.id).unwrap();
        let ib = listed.iter().position(|x| x.id == b.id).unwrap();
        assert!(ib <= ia, "newest (B) should not sort after older (A)");
    }
```

If `global_store()`'s exact name/return differs at implementation time, match
whatever `save_list_read_markdown` uses verbatim.

- [ ] **Step 4: Build + test**

Run: `cargo test -p zanto-core artifacts`
Expected: new tests pass; existing artifact tests still pass (esp. `save_list_read_markdown`).

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-core/src/artifacts/mod.rs
git commit -m "feat(core): artifact store upserts by title+scope and lists newest-first"
```

---

### Task 2: Prompt — render-only steering

**Files:** Modify `crates/zanto-desktop/src-tauri/src/ipc/chat.rs`; check `write_file.rs`.

- [ ] **Step 1: Replace the document clause in `ARTIFACT_PROTOCOL`**

Find the clause added last change:

```rust
When the user asks you to WRITE a document, article, report, or notes (prose you generate, \
not an edit to a file the user named): call `store_artifact` to persist it (it appears in the \
Artifacts browser) and `render_artifact` to show it in the Canvas. Do NOT use `write_file` to \
drop a generated document into the project folder — `write_file` is only for editing a specific \
file the user named. \
```

Replace with:

```rust
When the user asks you to WRITE a document, article, report, or notes (prose you generate): \
call `render_artifact` to show it in the Canvas. Do NOT call `store_artifact` for it and do NOT \
use `write_file` — the user saves it deliberately with the Save button if they want to keep it. \
Only call `store_artifact` when the user EXPLICITLY asks to save or persist a document. \
```

- [ ] **Step 2: Update the substring test**

The test `artifact_protocol_steers_document_writes_to_store` asserts the old wording. Update it to the render-only intent:

```rust
    #[test]
    fn artifact_protocol_steers_documents_to_render_only() {
        assert!(ARTIFACT_PROTOCOL.contains("call `render_artifact` to show it"));
        assert!(ARTIFACT_PROTOCOL.contains("Do NOT call `store_artifact`"));
        assert!(ARTIFACT_PROTOCOL.contains("Do NOT") && ARTIFACT_PROTOCOL.contains("write_file"));
    }
```

(Rename from the old test name; remove the old assertions for "WRITE a document"/"Do NOT use `write_file`" since the wording changed.)

- [ ] **Step 3: Check `write_file.rs` description**

Its description says: don't use write_file for generated docs, use store_artifact/render_artifact. Under render-only that's still directionally right (don't drop docs via write_file). Leave it unchanged — its test (`description_steers_generated_docs_to_artifacts`) still passes (it checks for `store_artifact` + `Do NOT`). No edit needed. Confirm by re-running its test.

- [ ] **Step 4: Build + test**

Run: `cargo test -p zanto-desktop artifact_protocol` and `cargo test -p zanto-core write_file`
Expected: both pass with the new/unchanged assertions.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/ipc/chat.rs
git commit -m "feat(desktop): documents render-only; store only on explicit save"
```

---

### Task 3: Save button → store artifact (IPC swap)

**Files:** `ipc/artifacts.rs` (new cmd), `ipc/files.rs` + `lib.rs` (remove old), `ipc.ts`, `Message.svelte`.

**Interfaces:**
- New: `store_document_artifact(title: String, text: String) -> Result<Value, String>` — calls `store().save(Markdown, &title, text.as_bytes(), Scope::Project)`, returns the `ArtifactRef` JSON (like `store_artifact`). Consumed by Message.svelte as `ipc.storeDocumentArtifact`.
- Removed: `save_document_to_project` (files.rs), its registration, its `ipc.ts` wrapper.

- [ ] **Step 1: Add `store_document_artifact` in `ipc/artifacts.rs`**

Use the existing `store()` helper (artifacts.rs:138) and `ArtifactKind`/`Scope` (already imported). Add:

VERIFIED: `root(Scope::Project)` returns `Err(NoProjectRoot)` when there is no
project dir — so the command MUST fall back to `Scope::Global` in that case, or
Save fails whenever no project is set. The store is built via the `store()`
helper (artifacts.rs:138) from `Settings`. Pick scope from whether a project dir
exists:

```rust
/// Persist a generated markdown document to the artifact store (the deliberate
/// "Save" action from a chat message). Upserts by title (see ArtifactStore::save),
/// so re-saving the same document updates it in place. Saves to the project scope
/// when a project dir is set, else the global store. Returns the ref as JSON.
#[tauri::command]
pub fn store_document_artifact(title: String, text: String) -> Result<Value, String> {
    let scope = if Settings::load().project_dir.is_some() {
        Scope::Project
    } else {
        Scope::Global
    };
    let art = store()
        .save(ArtifactKind::Markdown, &title, text.as_bytes(), scope)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&art).map_err(|e| e.to_string())
}
```

(`Settings`, `Scope`, `ArtifactKind`, `Value`, `store()` are all already in
scope in artifacts.rs — confirmed at its imports.)

- [ ] **Step 2: Register it; remove the old command**

In `lib.rs` `generate_handler!`: add `ipc::artifacts::store_document_artifact`; remove `ipc::files::save_document_to_project`.
In `ipc/files.rs`: delete the `save_document_to_project` fn (added earlier).

- [ ] **Step 3: Swap the ipc.ts wrapper**

In `ipc.ts`: remove `saveDocumentToProject`; add:

```ts
  // Persist a generated markdown document to the project artifact store
  // (deliberate Save). Upserts by title. Returns the stored artifact ref.
  storeDocumentArtifact: (title: string, text: string) =>
    invoke<unknown>("store_document_artifact", { title, text }),
```

- [ ] **Step 4: Update Message.svelte Save handler + label**

Replace `saveMessageDocument` to call the new IPC and trigger a panel refresh (the `artifactsTick` signal from Task 4 — if Task 4 not done yet, this still works; the tick just won't exist until then. To keep Task 3 self-contained, import and bump the tick only if present. Simplest: do the bump in Task 4 when the signal exists. For now, the toast confirms the save.). Also derive a clean title (strip leading `#`, no `.md`):

```ts
  function documentTitle(text: string): string {
    const heading = text.split("\n").find((l) => /^#{1,6}\s/.test(l));
    return (heading ?? "Untitled document").replace(/^#+\s*/, "").trim().slice(0, 80) || "Untitled document";
  }

  async function saveMessageDocument() {
    try {
      await ipc.storeDocumentArtifact(documentTitle(copyText), copyText);
      toast.success("Saved to Artifacts");
    } catch (e) {
      toast.error("Could not save the document", { description: `${e}` });
    }
  }
```

Remove the old `suggestedName` (`.md` filename) helper. Relabel the button text from "Save to project…" to **"Save"** (and keep `aria-label="Save document to project"` → change to `aria-label="Save document to Artifacts"`).

- [ ] **Step 5: Build**

Run: `cargo build -p zanto-desktop` then (from `crates/zanto-desktop`) `pnpm check`.
Expected: compiles; 0/0. Fix the `Scope::Project`-with-no-project-dir case if `root()` errors (per Step 1 note).

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs crates/zanto-desktop/src-tauri/src/ipc/files.rs crates/zanto-desktop/src-tauri/src/lib.rs crates/zanto-desktop/src/lib/ipc.ts crates/zanto-desktop/src/lib/components/Message.svelte
git commit -m "feat(desktop): Save stores a document artifact (upsert) instead of a loose file"
```

---

### Task 4: Panel tabs + don't-destroy-canvas + refresh signal

**Files:** `session.svelte.ts`, `Sidebar.svelte`, `Canvas.svelte`, `ArtifactBrowser.svelte`, `Message.svelte` (tick bump).

- [ ] **Step 1: Add an artifacts refresh signal to the session store**

In `session.svelte.ts`, add to `sessionStore`:

```ts
  artifactsTick: 0, // bump to make an open ArtifactBrowser reload its lists
```

- [ ] **Step 2: `openArtifacts` keeps the canvas**

In `Sidebar.svelte` `openArtifacts()`, remove the `sessionStore.canvas = null;` line so opening the browser no longer destroys the rendered view:

```ts
  function openArtifacts() {
    sessionStore.promotedLink = null;
    sessionStore.panelMode = "browser";
  }
```

- [ ] **Step 3: Tab the Canvas panel**

In `Canvas.svelte`, replace the mutually-exclusive `panelMode === "browser"` / `sessionStore.canvas` branches with a tabbed layout when BOTH a rendered canvas exists and the browser is open; otherwise behave as today. Add local tab state and render:

```svelte
<script lang="ts">
  // ...existing imports...
  // Which panel tab is active when both a rendered view and the browser exist.
  let panelTab = $state<"view" | "artifacts">("view");
  // When the browser opens, focus its tab; when it closes, fall back to view.
  $effect(() => {
    if (sessionStore.panelMode === "browser") panelTab = "artifacts";
    else panelTab = "view";
  });
</script>
```

Then in the template, replace the `{:else if sessionStore.panelMode === "browser"}` and `{:else if sessionStore.canvas}` branches with a combined block:

```svelte
  {:else if sessionStore.canvas && sessionStore.panelMode === "browser"}
    <!-- Both exist: tab between the rendered view and the Artifacts browser so
         opening Artifacts never destroys the rendered canvas. -->
    <div class="flex h-full flex-col">
      <div class="flex shrink-0 items-center gap-1 border-b border-border px-2 py-1.5">
        <button
          type="button"
          aria-pressed={panelTab === "view"}
          onclick={() => (panelTab = "view")}
          class="rounded-md px-2.5 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {panelTab === 'view' ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground'}"
        >View</button>
        <button
          type="button"
          aria-pressed={panelTab === "artifacts"}
          onclick={() => (panelTab = "artifacts")}
          class="rounded-md px-2.5 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {panelTab === 'artifacts' ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground'}"
        >Artifacts</button>
      </div>
      <div class="min-h-0 flex-1 overflow-auto">
        {#if panelTab === "view"}
          <div class="h-full overflow-auto p-4"><Block block={sessionStore.canvas} canPin={false} /></div>
        {:else}
          <ArtifactBrowser onClose={() => (sessionStore.panelMode = null)} />
        {/if}
      </div>
    </div>
  {:else if sessionStore.panelMode === "browser"}
    <ArtifactBrowser onClose={() => (sessionStore.panelMode = null)} />
  {:else if sessionStore.canvas}
    <div class="h-full overflow-auto p-4">
      <Block block={sessionStore.canvas} canPin={false} />
    </div>
```

Order matters: the combined `canvas && browser` branch must come BEFORE the lone `browser` and lone `canvas` branches. Keep the promoted-link branch first (unchanged) and the finance/empty branches last (unchanged).

Closing the browser sets `panelMode = null`; the `$effect` flips `panelTab` back to "view", and the lone-`canvas` branch then renders the rendered view (survived). 

- [ ] **Step 4: ArtifactBrowser reloads on the tick**

In `ArtifactBrowser.svelte`, make the reload effect also track `sessionStore.artifactsTick` so a save while it's open refreshes the list. In the `$effect` that reads `backend`/`scope` (around line 55), add `void sessionStore.artifactsTick;` alongside the existing `void backend; void scope;`. Import `sessionStore` if not already imported.

- [ ] **Step 5: Bump the tick after a save (Message.svelte)**

In `saveMessageDocument` (Task 3 Step 4), after a successful `storeDocumentArtifact`, bump the signal so an open browser refreshes:

```ts
  import { sessionStore } from "$lib/stores/session.svelte"; // if not already imported
  // ...
      await ipc.storeDocumentArtifact(documentTitle(copyText), copyText);
      sessionStore.artifactsTick++;
      toast.success("Saved to Artifacts");
```

(Message.svelte already imports `sessionStore` — confirm; if so, don't duplicate.)

- [ ] **Step 6: Build gate**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: 0/0.

- [ ] **Step 7: Visual verification (mock)**

Start `pnpm dev:mock`. Temporarily add a mock scenario that renders a heading markdown doc inline (a chat reply with `# Title` content) — the `write doc` pattern used before. Then:
- Send it → renders in chat; the message shows a **Save** button; nothing in Artifacts yet (render-only).
- Click Save → toast "Saved to Artifacts".
- Open Artifacts (sidebar) → the doc is listed; a rendered canvas view (if any was sent to canvas) survives behind a **View/Artifacts tab**; switching tabs keeps both; closing Artifacts returns to View (not blank).
- Save the same doc again → no duplicate in the list (upsert); list newest-first.
Revert the throwaway mock scenario afterward.

Note: a chart/table canvas view is the easy way to get `sessionStore.canvas` populated for the tab test (markdown renders inline, not to canvas — confirmed). Use the existing `chart` trigger to populate the canvas, then open Artifacts to verify the tabs coexist.

- [ ] **Step 8: Commit**

```bash
git add crates/zanto-desktop/src/lib/stores/session.svelte.ts crates/zanto-desktop/src/lib/components/Sidebar.svelte crates/zanto-desktop/src/lib/components/Canvas.svelte crates/zanto-desktop/src/lib/components/ArtifactBrowser.svelte crates/zanto-desktop/src/lib/components/Message.svelte
git commit -m "feat(desktop): panel tabs for rendered view + Artifacts; keep canvas on browse"
```

---

### Task 5: Full gate

- [ ] **Step 1:** `cargo build && cargo test` — green; the 2 new core tests + updated prompt test pass.
- [ ] **Step 2:** (from `crates/zanto-desktop`) `pnpm check` — 0/0.
- [ ] **Step 3:** End-to-end manual: write a doc (render-only, not in Artifacts) → Save (appears, newest-first) → save again (no dup) → open/close Artifacts with a chart on canvas (tabs coexist, view survives).

---

## Self-Review

**Spec coverage:** #1 render-only → Task 2 (+ Task 3 makes Save the deliberate persist). #2 panel tabs + keep-canvas → Task 4. #3 upsert → Task 1 Step 1. #4 sort → Task 1 Step 2. Derived "Save stores artifact" → Task 3. ✓

**Placeholder scan:** none. The two "read before finalizing" notes (Task 1 Step 3 test-helper style; Task 3 Step 1 `Scope::Project` no-project-dir behavior) name the exact code to check — real verification, not TBD.

**Type/name consistency:** `store_document_artifact(title, text)` (Rust) ↔ `storeDocumentArtifact(title, text)` (ipc.ts) ↔ called in Message.svelte. `artifactsTick` defined in session store (Task 4 S1), tracked in ArtifactBrowser (S4), bumped in Message (S5). Old `saveDocumentToProject`/`save_document_to_project` fully removed (Task 3 S2/S3). Tab branch ordering called out explicitly. ✓
