# Artifact Delete + document action bar on messages — Plan

> Execute task-by-task. Build/test gate then commit after each.

**Goal:** Add Delete for stored document artifacts (core + IPC), surface Delete in the Artifact explorer, and give chat-message documents the explorer's action bar (Save a copy / Reveal / Delete) once saved.

**Architecture:** Layer 1 core `ArtifactStore::delete(id)` (locate across scopes, remove blob + index entry). Layer 2 `delete_stored_artifact` IPC + ipc.ts wrapper. Layer 3 explorer Delete button (inline confirm). Layer 4 Message.svelte tracks the saved artifact id from `storeDocumentArtifact` and shows Save-a-copy/Reveal/Delete for it.

**Tech Stack:** Rust (zanto-core artifacts, Tauri ipc), Svelte 5 + Tailwind. `cargo build`/`cargo test`; `pnpm check` in `crates/zanto-desktop`.

## Global Constraints

- Document (file) artifacts only — pinned views (DB) untouched.
- Delete uses inline confirm (swap button → Delete?/Cancel), mirroring the Settings API-key clear flow; no modal/toast-undo.
- `cargo test` green; `pnpm check` 0/0.
- Verification: core via unit test; UI via mock dev server.

## File Structure

- `crates/zanto-core/src/artifacts/mod.rs` — `delete` + test. (Task 1)
- `crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs` + `lib.rs` — `delete_stored_artifact`. (Task 2)
- `crates/zanto-desktop/src/lib/ipc.ts` — `deleteStoredArtifact` wrapper; type `storeDocumentArtifact` return. (Task 2/4)
- `crates/zanto-desktop/src/lib/components/ArtifactBrowser.svelte` — Delete button. (Task 3)
- `crates/zanto-desktop/src/lib/components/Message.svelte` — saved-id tracking + action bar. (Task 4)

---

### Task 1: Core `ArtifactStore::delete`

**Files:** Modify `crates/zanto-core/src/artifacts/mod.rs`

**Interfaces:**
- `delete(&self, id: &str) -> Result<()>` — locate across scopes, remove blob (ignore missing-file), rewrite that root's index without the entry; `NotFound` if no entry.

- [ ] **Step 1: Add `delete` (place after `locate`, inside `impl ArtifactStore`)**

VERIFIED: `locate(id) -> Result<(PathBuf, ArtifactRef)>` (NotFound if absent); `read_index(&Path)`, `write_index_atomic(&Path, &[ArtifactRef])`.

```rust
    /// Delete an artifact (blob + index entry) by id, searching both scopes.
    /// A missing blob file is tolerated as long as the index entry is removed.
    pub fn delete(&self, id: &str) -> Result<()> {
        let (root, art) = self.locate(id)?;

        // Remove the blob; a missing file is fine (index entry is the source of truth).
        let blob = root.join(&art.rel_path);
        match std::fs::remove_file(&blob) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        // Rewrite the index without this entry.
        let index: Vec<ArtifactRef> = read_index(&root)?
            .into_iter()
            .filter(|a| a.id != id)
            .collect();
        write_index_atomic(&root, &index)?;
        Ok(())
    }
```

(Confirm `std::io::Error` converts into `ArtifactError` via `?` — `read_index` already uses `?` on fs ops, so a `From<std::io::Error>` impl exists. If `e.into()` doesn't resolve, use the same error-wrap the file's other fs calls use.)

- [ ] **Step 2: Test (mirror `global_store()` style)**

Add to the `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn delete_removes_blob_and_index_entry() {
        let (store, _dir) = global_store();
        let art = store
            .save(ArtifactKind::Markdown, "Doc", b"body", Scope::Global)
            .unwrap();
        store.delete(&art.id).unwrap();
        assert!(store.list(Some(Scope::Global)).unwrap().is_empty());
        assert!(matches!(store.read(&art.id), Err(ArtifactError::NotFound(_))));
    }
```

- [ ] **Step 3: Build + test**

Run: `cargo test -p zanto-core artifacts`
Expected: new test passes; existing artifact tests still pass.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-core/src/artifacts/mod.rs
git commit -m "feat(core): ArtifactStore::delete (blob + index entry, across scopes)"
```

---

### Task 2: Delete IPC + wrapper

**Files:** `ipc/artifacts.rs`, `lib.rs`, `ipc.ts`.

**Interfaces:**
- `delete_stored_artifact(id: String) -> Result<(), String>` → `store().delete(&id)`.
- ipc.ts `deleteStoredArtifact(id)`.

- [ ] **Step 1: Add the command in `ipc/artifacts.rs`** (near `store_document_artifact`)

```rust
/// Delete a stored document artifact (blob + index entry) by id.
#[tauri::command]
pub fn delete_stored_artifact(id: String) -> Result<(), String> {
    store().delete(&id).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register in `lib.rs`** (next to `store_document_artifact`)

Add `ipc::artifacts::delete_stored_artifact,` to the `generate_handler!` list.

- [ ] **Step 3: ipc.ts wrapper** (next to `saveArtifactCopy`)

```ts
  // Delete a stored document artifact (blob + index entry).
  deleteStoredArtifact: (id: string) => invoke<void>("delete_stored_artifact", { id }),
```

- [ ] **Step 4: Build**

Run: `cargo build -p zanto-desktop` then (from `crates/zanto-desktop`) `pnpm check`.
Expected: compiles; 0/0.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs crates/zanto-desktop/src-tauri/src/lib.rs crates/zanto-desktop/src/lib/ipc.ts
git commit -m "feat(desktop): delete_stored_artifact IPC + wrapper"
```

---

### Task 3: Explorer Delete button (inline confirm)

**Files:** Modify `crates/zanto-desktop/src/lib/components/ArtifactBrowser.svelte`

- [ ] **Step 1: Add Trash icon import + confirm state + handler**

In the import (line 4: `import { FolderOpen, Download, X as XIcon } from "@lucide/svelte";`), add `Trash2`:

```svelte
  import { FolderOpen, Download, Trash2, X as XIcon } from "@lucide/svelte";
```

Add state near `selectedDoc` (line ~29): `let confirmingDelete = $state(false);`. Reset it when the selection changes — in the effect that nulls `selectedDoc` on backend/scope change (the reload `$effect`, line ~92), add `confirmingDelete = false;` alongside `selectedDoc = null;`. Also reset on selecting a new doc (find where `selectedDoc = await ipc.readStoredArtifact(id)` is set, line ~55, and set `confirmingDelete = false;` there too).

Add the handler near `saveCopy`/`revealDoc`:

```svelte
  async function deleteDoc(id: string) {
    try {
      await ipc.deleteStoredArtifact(id);
      selectedDoc = null;
      confirmingDelete = false;
      sessionStore.artifactsTick++; // reload the list
      toast.success("Deleted");
    } catch (e) {
      toast.error("Could not delete the document", { description: `${e}` });
    }
  }
```

(`sessionStore` is already imported — added in the prior change for `artifactsTick`. Confirm; if not, import it.)

- [ ] **Step 2: Add the Delete button to the document-actions header**

In the actions header (lines ~252-261, the `<div>` with Save a copy / Reveal), append a Delete control with inline confirm. Replace the closing of that `<div>` to include:

```svelte
          <div class="flex items-center justify-end gap-2 border-b border-border px-3 py-2">
            <Button size="sm" variant="outline" onclick={() => saveCopy(docId)}>
              <Download class="size-4" />
              Save a copy…
            </Button>
            <Button size="sm" variant="outline" onclick={() => revealDoc(docId)}>
              <FolderOpen class="size-4" />
              Reveal in folder
            </Button>
            {#if confirmingDelete}
              <Button size="sm" variant="destructive" onclick={() => deleteDoc(docId)}>Delete</Button>
              <Button size="sm" variant="ghost" onclick={() => (confirmingDelete = false)}>Cancel</Button>
            {:else}
              <Button size="sm" variant="outline" onclick={() => (confirmingDelete = true)}>
                <Trash2 class="size-4" />
                Delete
              </Button>
            {/if}
          </div>
```

- [ ] **Step 3: Build**

Run (from `crates/zanto-desktop`): `pnpm check` — 0/0.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/ArtifactBrowser.svelte
git commit -m "feat(desktop): Delete (inline confirm) in the artifact explorer"
```

---

### Task 4: Message action bar — saved-id tracking + Save a copy/Reveal/Delete

**Files:** Modify `crates/zanto-desktop/src/lib/components/Message.svelte`; type the IPC in `ipc.ts`.

- [ ] **Step 1: Type `storeDocumentArtifact`'s return**

In `ipc.ts`, change the wrapper to return the id-bearing ref:

```ts
  // Persist a generated markdown document to the project artifact store
  // (deliberate Save). Upserts by title. Returns the stored artifact ref.
  storeDocumentArtifact: (title: string, text: string) =>
    invoke<{ id: string; title: string }>("store_document_artifact", { title, text }),
```

- [ ] **Step 2: Track the saved id + confirm state in Message.svelte**

Near the other `$state` (after `copied`), add:

```ts
  let savedArtifactId = $state<string | null>(null);
  let confirmingDelete = $state(false);
```

Add Trash/Download/Folder icons to the imports (Message already imports `SaveIcon`, `CopyIcon`, etc.):

```ts
  import Trash2Icon from "@lucide/svelte/icons/trash-2";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import FolderOpenIcon from "@lucide/svelte/icons/folder-open";
```

- [ ] **Step 3: Update `saveMessageDocument` to capture the id**

```ts
  async function saveMessageDocument() {
    try {
      const ref = await ipc.storeDocumentArtifact(documentTitle(copyText), copyText);
      savedArtifactId = ref?.id ?? null;
      sessionStore.artifactsTick++;
      toast.success("Saved to Artifacts");
    } catch (e) {
      toast.error("Could not save the document", { description: `${e}` });
    }
  }

  async function saveCopyOfDocument() {
    if (!savedArtifactId) return;
    try {
      const saved = await ipc.saveArtifactCopy(savedArtifactId);
      if (saved) toast.success("Saved a copy");
    } catch (e) {
      toast.error("Could not save a copy", { description: `${e}` });
    }
  }

  async function revealDocument() {
    if (!savedArtifactId) return;
    try {
      await ipc.revealArtifact(savedArtifactId);
    } catch (e) {
      toast.error("Could not reveal the file", { description: `${e}` });
    }
  }

  async function deleteSavedDocument() {
    if (!savedArtifactId) return;
    try {
      await ipc.deleteStoredArtifact(savedArtifactId);
      savedArtifactId = null;
      confirmingDelete = false;
      sessionStore.artifactsTick++;
      toast.success("Deleted");
    } catch (e) {
      toast.error("Could not delete the document", { description: `${e}` });
    }
  }
```

- [ ] **Step 4: Render the bar conditionally in the footer**

In the message footer (the `{#if isDocument}` block currently holding the Save button), replace the lone Save button with: when `savedArtifactId === null` show Save; else show Save a copy / Reveal / Delete (with inline confirm). Keep Copy always present (it's the sibling button outside this `{#if}`):

```svelte
          {#if isDocument}
            {#if savedArtifactId === null}
              <button
                type="button"
                onclick={saveMessageDocument}
                aria-label="Save document to Artifacts"
                class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <SaveIcon class="size-3.5" />
                Save
              </button>
            {:else}
              <button type="button" onclick={saveCopyOfDocument} aria-label="Save a copy"
                class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <DownloadIcon class="size-3.5" /> Save a copy…
              </button>
              <button type="button" onclick={revealDocument} aria-label="Reveal in folder"
                class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <FolderOpenIcon class="size-3.5" /> Reveal in folder
              </button>
              {#if confirmingDelete}
                <button type="button" onclick={deleteSavedDocument} aria-label="Confirm delete"
                  class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-destructive hover:bg-destructive/10 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  <Trash2Icon class="size-3.5" /> Delete?
                </button>
                <button type="button" onclick={() => (confirmingDelete = false)} aria-label="Cancel delete"
                  class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  Cancel
                </button>
              {:else}
                <button type="button" onclick={() => (confirmingDelete = true)} aria-label="Delete document"
                  class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  <Trash2Icon class="size-3.5" /> Delete
                </button>
              {/if}
            {/if}
          {/if}
```

(The existing Copy button stays as the first child of the footer flex row, unchanged. This block replaces ONLY the previous `{#if isDocument}` Save-button block.)

- [ ] **Step 5: Build**

Run (from `crates/zanto-desktop`): `pnpm check` — 0/0.

- [ ] **Step 6: Visual verification (mock)**

Add the throwaway heading-doc scenario (the `write doc` trigger used before: a chat reply with `# Title` markdown). Then:
- Send it → message shows **Copy + Save** (savedArtifactId null).
- Click Save → toast "Saved to Artifacts"; the bar becomes **Save a copy / Reveal / Delete**.
- Click Delete → swaps to **Delete? / Cancel**; confirm → toast "Deleted", bar reverts to **Save**.
- Open Artifacts → after Save the doc is listed; after Delete it's gone (artifactsTick reload).
- Explorer: select a doc, Delete (inline confirm) → removed from list.
Revert the throwaway scenario.

- [ ] **Step 7: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Message.svelte crates/zanto-desktop/src/lib/ipc.ts
git commit -m "feat(desktop): document action bar on chat messages (save a copy / reveal / delete)"
```

---

### Task 5: Full gate

- [ ] `cargo build && cargo test` green (new delete test passes). `pnpm check` 0/0.
- [ ] End-to-end manual: explorer Delete; message Save→bar→Delete→revert.

---

## Self-Review

**Spec coverage:** Delete core → Task 1. Delete IPC → Task 2. Explorer Delete → Task 3. Message action bar (save-first-then-actions) + Delete → Task 4. Inline-confirm in both → Tasks 3 & 4. ✓

**Placeholder scan:** none. The one "confirm `e.into()` resolves" note (Task 1) names the existing fs-error pattern to match — real check.

**Type/name consistency:** `delete(id)` (core) → `delete_stored_artifact(id)` (IPC) → `deleteStoredArtifact(id)` (ipc.ts) → used in ArtifactBrowser (Task 3) and Message (Task 4). `storeDocumentArtifact` retyped to `{id,title}` (Task 4 S1), `.id` consumed in `saveMessageDocument`. `savedArtifactId`/`confirmingDelete` local to Message. `artifactsTick` bumped on delete in both surfaces. ✓
