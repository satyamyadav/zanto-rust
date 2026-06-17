# Document I/O — read any document, attach from any channel, deliver downloadables

- **Date:** 2026-06-17
- **Decisions:** formats = **text-native + Office + PDF**; images = **vision when a
  multimodal provider is active**; downloadables = **artifact store + Save-a-copy/Reveal**.

## Goal
Let zanto (1) **read any document** the user points it at — from chat (attach/drag-drop),
the working/project dir, `@`-tags, or context sources; (2) **write/update** files on the
system (already gated); (3) **deliver downloadables** the user can save or reveal.

## Building blocks already present
Gated `read_file`/`write_file`/`edit_file`/`search_files`, `browse_dir` + `@`-tag, context
sources, the artifact store (A3, `.zanto/artifacts`), `dialog` + `opener` plugins. This spec
adds binary-document extraction, two new input channels, the vision path, and the
download UX.

---

## Phase 1 — `read_document` tool (text + Office + PDF)
New gated core tool so the agent can read non-plaintext documents.
- Files: new `crates/zanto-core/src/tools/fs/read_document.rs` (or `tools/docs/`), register in
  `tools/mod.rs`; `crates/zanto-core/Cargo.toml` deps.
- Tool `read_document { path, max_chars? }` — `permissions.check(path, Read)` first (uses the
  returned PathBuf). Detect by extension (+ magic bytes fallback) and extract:
  - **text/md/json/csv/code/log** → read as UTF-8 (reuse `read_file` logic). CSV → keep raw or
    a compact table.
  - **html** → strip to readable text (reuse the extractor from the web tool E3 `tools/web`).
  - **PDF** → text via `pdf-extract` (or `lopdf`) — best-effort, layout-lossy; note that.
  - **DOCX** → paragraphs via `docx-rs`/`docx-rust`.
  - **XLSX/XLS/ODS** → sheets→rows via `calamine` (rendered as a text table; cap rows).
  - **image** (png/jpg/webp/gif) → return `{ kind:"image", note:"attach as image to use a vision model" }`
    (the actual seeing happens via Phase 2, not this tool).
  - unknown/binary → a clear "unsupported binary format" message.
  - Returns JSON `{ path, kind, text }` (extraction capped at `max_chars`, default ~32k, with a
    truncation note). Read-only tool.
- Tests: pure extractor unit tests per format using tiny fixtures (a CSV, a minimal docx/xlsx if
  feasible, an HTML string) — no network.
- Deps to add (pick the lightest reliable): `calamine`, `pdf-extract` *or* `lopdf`, a docx crate,
  `mime_guess`. Document the choice; keep the binary size reasonable.

## Phase 2 — input channels: attach + drag-drop (desktop)
- **Composer attach button** (paperclip) → `dialog.open` (multi-file) → adds attachment chips.
- **Drag-and-drop onto the window** → Tauri core `onDragDropEvent` (frontend
  `getCurrentWebviewWindow().onDragDropEvent`) → adds the dropped paths as chips. (Disable the
  webview's default file-drop navigation.)
- An attachment = `{ path, name, kind }`. On send:
  - **document attachments** (text/office/pdf) → appended to the message as `@<path>` references
    (consistent with the `@`-tag), so the agent reads them with `read_document`. Lazy — no eager
    extraction, keeps context small. Chips reuse the C2/C7 chip styling.
  - **image attachments** → see Phase 3 (multimodal).
- Files: `Composer.svelte`, a small attachments helper in the session store, `lib/ipc.ts`
  (drag-drop listener), `tauri.conf.json`/capability for drag-drop if needed.

## Phase 3 — images via vision (when the provider supports it)
The only path where the model must *see* the file, not read extracted text. This needs the
image as an **image content part on the user message**, not a tool result.
- Core `chat.rs`/`ChatConfig`: accept optional **attachments** (image bytes + mime) for the turn;
  build the user `ChatMessage` with genai image content parts (verify genai 0.6.4 multimodal:
  `ContentPart`/`MessageContent` image-from-base64) when present.
- `send_message` (`ipc/chat.rs`): read image attachments → pass as turn attachments. Gate by
  provider capability: only attach images when `Settings::active()` provider is multimodal
  (Gemini/Claude/OpenAI); for a text-only provider (Ollama) **degrade gracefully** — drop the
  image with a one-line note in the message ("(image attached, but the current model can't read
  images — switch to a vision model)").
- Frontend: image chips render a thumbnail.

## Phase 4 — downloadables (output UX)
The agent saves a generated file via the existing `store_artifact` (already writes to
`.zanto/artifacts` or global). Make stored artifacts downloadable:
- Artifact Browser (extend 4d `ArtifactBrowser.svelte`): per-item **Save a copy…** (`dialog.save`
  → write the artifact bytes to the chosen path) and **Reveal in folder** (`opener` reveal/show).
- Backend: a `save_artifact_copy(id, dest)` command (read store bytes + write to dest) or do the
  write in the existing read path; a `reveal_path(path)` command via `opener`.
- Optional: a "downloadable" affordance inline in chat — when the agent saves a document, the
  saved-artifact card (from store_artifact's return) shows Save/Reveal directly.

## Cross-cutting
- **Permissions:** `read_document` and any new write go through `permissions.check` (HITL gated)
  exactly like the fs tools. Attaching a file auto-grants read on it (like `add_allowed_path`).
- **System prompt:** add a clause so the model knows: to read a PDF/Word/Excel/etc., call
  `read_document` (not `read_file`); attached files arrive as `@path` references.
- **Caps:** extraction + injection are size-capped; large spreadsheets/PDFs truncate with a note.

## Affected files (by phase)
1. core `tools/.../read_document.rs`, `tools/mod.rs`, `Cargo.toml`, system prompt clause.
2. `Composer.svelte`, `session.svelte.ts`, `ipc.ts`, `ipc/chat.rs` (attach plumbing), `tauri.conf.json`.
3. `chat.rs`/`ChatConfig` (multimodal), `ipc/chat.rs` (image attach + provider gate).
4. `ArtifactBrowser.svelte`, `ipc/artifacts.rs` (+ `lib.rs`), `ipc.ts`.

## Acceptance (build-check only; you verify in `pnpm dev`)
- `cargo build` + `cargo test -p zanto-core` (extractor tests) + `pnpm check` + `pnpm build:web`.
- Manual: read a CSV/PDF/DOCX/XLSX via `read_document`; drag a file onto the window → it attaches
  and the agent reads it; with a vision model, attach an image and ask about it; save a generated
  document and Save-a-copy/Reveal it.

## Phasing recommendation
Ship **Phase 1 (read_document)** first — it's the core capability and unblocks "read any
document" via the channels that already exist (@-tag, working dir). Then **Phase 2 (attach/drop)**,
**Phase 4 (downloadables)** (independent, can parallel Phase 2), then **Phase 3 (vision)** last
(heaviest; depends on multimodal wiring).
