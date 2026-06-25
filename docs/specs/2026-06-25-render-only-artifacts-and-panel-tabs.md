# Render-only documents, deliberate save, panel tabs, artifact upsert + sort

_Date: 2026-06-25_

## Context

Layer 1 steering (prior change) worked: the model now renders generated
documents to the Canvas and calls `store_artifact` instead of `write_file`.
But testing surfaced four issues, and the desired behavior shifted.

## Issues + decisions (locked with user)

1. **Auto-store is unwanted → render only, deliberate save.** The model should
   RENDER a generated document to the Canvas but NOT auto-call `store_artifact`.
   Persisting is a deliberate user action (the "Save" button). Nothing
   accumulates in Artifacts automatically.
2. **Opening the Artifacts panel destroys the rendered Canvas view.**
   `openArtifacts()` sets `sessionStore.canvas = null`; closing the browser then
   leaves a blank panel. → The right panel should use **tabs** (rendered View +
   Artifacts browser) that coexist; opening Artifacts must not clear the canvas.
3. **Re-saving the same document duplicates it** in Artifacts. `Store::save`
   always `new_id()` + `index.push`. → Upsert by (title, scope): replace the
   existing entry's content in place instead of appending a second.
4. **Artifacts list isn't sorted by creation time.** `list_root` returns
   index insertion order. → Sort by `created_at` descending (newest first).

### Derived decision (flagged for review)

With "render only + deliberate save", the **Save button should create the
artifact-store entry** (so the saved document appears in the Artifacts panel),
not write a loose file via the native dialog. Rationale: the panel is the home
for saved documents; the existing native-file export already exists separately
as `save_artifact_copy` ("Save a copy"). So:

- The message **"Save to project…"** button changes from `saveDocumentToProject`
  (native file dialog) to **storing an artifact** (`store_artifact`, project
  scope) via a new IPC. This makes Save populate the Artifacts panel, and makes
  issue #3's upsert the dedup guard for repeated saves of the same title.
- The previously-added `save_document_to_project` IPC + its native-dialog flow
  is **removed** (it was built for the now-reverted "write to project dir"
  model). If a user wants a loose file copy, that's the Artifacts panel's
  existing "Save a copy" action.

(If the user prefers Save to keep writing a loose project file instead of an
artifact, that is a one-line swap — call out in review.)

## Layer A — revert auto-store steering (prompt)

`crates/zanto-desktop/src-tauri/src/ipc/chat.rs` `ARTIFACT_PROTOCOL`: the clause
added last change tells the model to call BOTH `store_artifact` and
`render_artifact` for a generated document. Change it to **render only**:

> "When the user asks you to WRITE a document, article, report, or notes (prose
> you generate): call `render_artifact` to show it in the Canvas. Do NOT call
> `store_artifact` for it and do NOT use `write_file` — the user saves it
> deliberately with the Save button if they want to keep it. `store_artifact`
> is only for when the user explicitly asks to save/persist a document."

`crates/zanto-core/src/tools/fs/write_file.rs`: the description's redirect to
`store_artifact`/`render_artifact` stays accurate (still steers away from
write_file for docs) — adjust only if it now over-promises auto-store; minimal
edit, keep the "don't drop generated docs here" intent.

Update the two prompt-substring tests to match the new wording.

## Layer B — artifact store: upsert + sort (zanto-core)

`crates/zanto-core/src/artifacts/mod.rs`:

### B1. Upsert by (title, scope) in `save`

Currently `save` always pushes a new entry. Change to: if an entry with the
same `title` already exists in this scope's index, **overwrite its blob and
update its `created_at`** (or keep created_at and add `updated_at` — simpler to
reuse the existing id + rewrite content, refreshing `created_at` so it sorts to
the top as "most recent"). Concretely:

- Read the index. Find an existing `ArtifactRef` with the same `title` and
  `scope`.
- If found: reuse its `id` + `rel_path`, overwrite the blob file, update its
  `created_at` to now, write the index back (replacing that entry — not pushing).
- If not found: current behavior (new id, push).

This makes "save the same document again" update in place. Add a unit test:
save title X twice → `list` returns exactly one entry for X, with the second
content.

### B2. Sort `list` by `created_at` descending

In `list` (or `list_root`), sort the returned `Vec<ArtifactRef>` by
`created_at` descending so the newest is first. When `list(None)` merges project
+ global, sort the merged result by `created_at` desc (the current "project
first" grouping is replaced by recency; if the user wants scope grouping
preserved, sort within each group — decide in plan, default to global recency
sort). Add a unit test: save A then B → `list` returns [B, A].

## Layer C — panel tabs (UI)

The right Canvas panel currently switches exclusively between: promoted link,
artifact browser (`panelMode === "browser"`), agent canvas block, finance
dashboard, empty. Issues #1/#2 want the **rendered view** and the **Artifacts
browser** to coexist as tabs.

`crates/zanto-desktop/src/lib/components/Canvas.svelte` +
`crates/zanto-desktop/src/lib/components/Sidebar.svelte` +
`crates/zanto-desktop/src/lib/stores/session.svelte.ts`:

- **Stop nulling the canvas when opening Artifacts.** In `openArtifacts()`
  (Sidebar.svelte:31-34), remove `sessionStore.canvas = null`. Opening Artifacts
  sets `panelMode = "browser"` but leaves `canvas` intact.
- **Add tabs to the panel** when a rendered canvas view exists AND/OR the
  browser is open. A small tab bar at the top of the Canvas panel:
  - "View" tab → the rendered `sessionStore.canvas` block.
  - "Artifacts" tab → the `ArtifactBrowser`.
  Switching tabs is local panel state; it does not destroy either side.
  Closing the browser (its X) returns to the View tab if a canvas exists, else
  the empty state.
- Precedence/edge cases: promoted link still takes the panel (it's a transient
  open-card). Finance dashboard is the app's default view when no canvas/browser.
  The tab bar appears only when there is a rendered view to tab between (don't
  show a lone "Artifacts" tab when there's no canvas — the browser can fill the
  panel as today in that case).

Exact tab UI is a plan-level detail; the invariant is: **opening/closing the
Artifacts browser never destroys the rendered canvas view.**

## Layer D — Save button → store artifact (UI + IPC)

`crates/zanto-desktop/src/lib/components/Message.svelte`: the "Save to project…"
button currently calls `ipc.saveDocumentToProject`. Change it to call a new
`store_document_artifact(title, text)` IPC that calls the core `store_artifact`
path (project scope), then refreshes the catalogue so the Artifacts panel
updates. Relabel the button **"Save"** (it saves to Artifacts, not a raw project
file). On success: toast "Saved to Artifacts".

- New IPC `store_document_artifact(title: String, text: String) -> Result<...>`
  in `ipc/artifacts.rs`, wrapping the existing `Store::save` (now upserting) with
  `kind = markdown, scope = project`. Title derived from the message's first
  heading (reuse the existing `suggestedName`-style derivation, minus the `.md`).
- Remove `save_document_to_project` (files.rs) + its registration + its ipc.ts
  wrapper (no longer used).
- After save, call the catalogue/artifacts refresh so the panel reflects it
  without reopening.

## Tests

- core: B1 upsert test (save twice → one entry, new content); B2 sort test
  (save A,B → [B,A]). `cargo test -p zanto-core`.
- desktop: updated `ARTIFACT_PROTOCOL` substring test (render-only wording);
  build green.
- UI: `pnpm check` 0/0; manual — write a doc (renders, NOT auto-stored, absent
  from Artifacts); click Save (appears in Artifacts, sorted newest-first); save
  again (updates in place, no dup); open Artifacts then close (rendered view
  survives via tabs).

## Constraints / out of scope

- No permission-model change.
- Pinned views (DB) untouched — this is about the document (file) artifact store.
- The `save_artifact_copy` "Save a copy" native-export action stays as-is.

## Suggested order

1. Layer B (core upsert + sort) — self-contained, unit-tested.
2. Layer A (prompt revert to render-only) — small.
3. Layer D (Save → store artifact IPC, remove old IPC).
4. Layer C (panel tabs) — largest UI piece.
