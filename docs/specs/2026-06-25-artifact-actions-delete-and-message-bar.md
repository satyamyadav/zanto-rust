# Artifact actions: Delete + document action bar on chat messages

_Date: 2026-06-25_

## Problem / request

The Artifact **explorer** document preview shows "Save a copy…" and "Reveal in
folder". The user wants:
1. Those same actions available on the **document as it appears in the chat**
   (the rendered/authored document — what the user calls the "artifact view"),
   not only in the explorer.
2. A **Delete** action added in both places.

## Grounding (verified in code)

- Documents render **inline in the chat message**, never on the canvas (only
  `component` blocks reach `sessionStore.canvas`). So the "artifact view" for a
  document is the chat message (which already has Copy + a Save button), and the
  explorer preview. The canvas View hosts charts/tables (pin flow), not file
  docs — out of scope here.
- Save a copy (`save_artifact_copy`) and Reveal (`reveal_artifact`) are existing
  IPCs keyed by a **stored artifact id**.
- There is **no delete** anywhere — neither `ArtifactStore` nor an IPC.
- A chat message's document is **not stored** until the user clicks Save
  (render-only). Until then it has no id, so file actions (copy/reveal/delete)
  can't apply.

## Decisions (locked with user)

- **Message action bar = "save first, then file actions".** Before the document
  is saved: the message shows **Copy** + **Save** (current behavior). After Save
  stores it (returning an id) — or if the message corresponds to an
  already-stored doc — the message shows **Save a copy… / Reveal in folder /
  Delete** for that id (the same bar as the explorer), in place of the bare Save.
- **Delete uses inline confirm** (swap the Delete button to "Delete? ✓ / ✕",
  mirroring the existing API-key clear flow in Settings) — in BOTH the explorer
  and the message. No modal, no toast-undo.

## Layer 1 — core delete (zanto-core)

`crates/zanto-core/src/artifacts/mod.rs`: add

```rust
/// Delete an artifact (blob + index entry) by id, searching both scopes.
/// Returns Ok(()) even if the blob file was already missing, as long as the
/// index entry is removed. NotFound if no entry matches.
pub fn delete(&self, id: &str) -> Result<()>
```

Implementation: reuse the `locate(id)` pattern (finds root + ArtifactRef across
scopes). Remove the blob file (ignore a missing-file error), then rewrite that
root's index without the entry (`write_index_atomic`). If no entry in any scope,
return `ArtifactError::NotFound`.

Test (mirror existing `global_store()` style): save → delete → `list` is empty
and `read` returns NotFound.

## Layer 2 — delete IPC (desktop)

`crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs`: add

```rust
#[tauri::command]
pub fn delete_stored_artifact(id: String) -> Result<(), String>
```

calling `store().delete(&id)`. Register in `lib.rs`. Add `ipc.ts` wrapper
`deleteStoredArtifact(id)`. (Save-copy + reveal wrappers already exist:
`saveArtifactCopy`, `revealArtifact`.)

## Layer 3 — explorer: add Delete (ArtifactBrowser.svelte)

In the document-actions header (currently Save a copy / Reveal), add a **Delete**
button with inline confirm:

- Local state `confirmingDelete: boolean` (reset when `selectedDoc` changes).
- Default: a Delete button (destructive-tinted text/icon, `Trash2`).
- On click → `confirmingDelete = true`, swapping to "Delete? [Delete] [Cancel]".
- Confirm → `ipc.deleteStoredArtifact(docId)`, then clear `selectedDoc`, bump
  `sessionStore.artifactsTick` (refreshes the list), toast "Deleted".
- Cancel → `confirmingDelete = false`.

The list reload already tracks `artifactsTick`, so the deleted item disappears.

## Layer 4 — message action bar (Message.svelte)

The message already has Copy + (for documents) a Save button. Change Save's
handler to remember the resulting id, and render the file-action bar once known:

- Add `let savedArtifactId = $state<string | null>(null)`.
- `saveMessageDocument`: after `storeDocumentArtifact(...)` (which returns the
  ArtifactRef JSON), parse `.id` and set `savedArtifactId`. Bump `artifactsTick`,
  toast "Saved to Artifacts". (Type the IPC return as `{ id: string }` or parse
  defensively.)
- Render logic in the message footer (only when `isDocument`):
  - If `savedArtifactId === null`: show **Save** (current).
  - Else: show **Save a copy… / Reveal in folder / Delete** for
    `savedArtifactId`, reusing `ipc.saveArtifactCopy` / `ipc.revealArtifact` /
    `ipc.deleteStoredArtifact`. Delete uses the same inline-confirm pattern;
    after a successful delete, reset `savedArtifactId = null` (the message
    reverts to showing Save) and bump `artifactsTick`.
- Copy stays always-present.

Note on re-save/upsert: because `Store::save` upserts by (title, scope), if the
user edits nothing and saves again the id is stable; `savedArtifactId` stays
valid. If the document title changes between saves, a new id results — acceptable;
`savedArtifactId` reflects the latest save.

Edge: `savedArtifactId` is per-message-component local state, lost on reload of
the thread. That's acceptable — after a reload the message reverts to "Save"
(the doc may already be stored, but re-saving upserts harmlessly). Not worth
threading saved-id persistence through the session model for this.

## Tests

- core: `delete` test (save→delete→empty + NotFound). `cargo test -p zanto-core`.
- desktop: build green; no new prompt/string test needed.
- UI: `pnpm check` 0/0. Manual: explorer — select a doc, Delete (inline confirm),
  it disappears from the list. Message — write a doc, Save (bar becomes Save a
  copy/Reveal/Delete), Delete (reverts to Save, removed from Artifacts).

## Out of scope

- Canvas View (component artifacts) — unchanged; documents don't render there.
- Pinned views delete — this is document (file) artifacts only.
- Bulk delete / multi-select.

## Suggested order

1. Layer 1 (core delete + test).
2. Layer 2 (IPC + wrapper).
3. Layer 3 (explorer Delete) — smallest UI, verifiable alone.
4. Layer 4 (message action bar) — depends on 1–2 + the saved-id tracking.
