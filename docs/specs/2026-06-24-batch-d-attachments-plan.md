# Batch D — Attachments end-to-end Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use `- [ ]`.

**Goal:** #7 persist a message's attachments so they survive reopen; #6 show attachments on the user bubble (live + reopened); #5 a click-to-view image viewer; #3 render file paths as openable anchor links (relative shown, absolute opened).

**Architecture:** Core stores attachment metadata (paths only, not bytes) in the user message's metadata JSON; the desktop `RenderMsg`/frontend carry it through. Two small permission-checked IPC commands added (`read_image_data_url` for the viewer, `open_path` for file links). Verified via cargo tests (store round-trip, contract) + the Playwright mock harness (bubble chips, reopen, viewer-opens, link-click). Real image rendering / native file dialog / OS open remain the user's manual check.

**Tech Stack:** Rust (genai/serde/rusqlite), Tauri commands, Svelte 5, `@playwright/test`.

## Global Constraints

- Persist attachment **metadata only** (`{ path, name, isImage }`), never binary blobs, in the user message's metadata JSON. Backward-compatible: absent metadata = no attachments (existing sessions unaffected).
- New IPC commands go through the existing seam: add to `src/lib/ipc.ts` (`ipc.*` wrappers), register the Rust `#[tauri::command]`, implement a mock handler, and add a `contract/fixtures/<cmd>.json` with a `#[test]`. Both new commands are **permission-checked** (`state.permissions.check(path, Op::Read)`).
- **#3 trigger (decided, conservative):** linkify only **backticked absolute or `~`-prefixed paths** in rendered assistant markdown (low false-positive); display the path shortened relative to the project dir when under it, else as-is; clicking opens via `open_path`. Do NOT auto-detect bare slash-strings in prose.
- Verify after each task: `cargo test` (158 baseline) green; `cd crates/zanto-desktop && pnpm check` 0; `pnpm test:ui` all pass; clippy 0 on touched crates. The seam guard (only `ipc.ts` + `mock/` import `@tauri-apps`) must stay green.
- Discover exact current code by reading files. Real-runtime aspects (actual image bytes, native pickFiles dialog, OS file open) are the user's manual verification — note them.

---

### Task 1: #7 — persist attachment metadata on the user message

**Files:**
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` (persist the user message's attachments)
- Modify: `crates/zanto-core/src/session.rs` (`display_messages_meta` / metadata read passes attachments through; or `RenderMsg::from_meta`)
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/mod.rs` (`RenderMsg` gains `attachments`)
- Modify: `crates/zanto-desktop/src/lib/ipc.ts` (`RenderMsg` type gains `attachments?`)
- Modify: `crates/zanto-desktop/src/lib/mock/backend.ts` + `contract/fixtures/load_session.json` (a message with attachments)
- Modify: `crates/zanto-desktop/src-tauri/tests/contract.rs`

**Interfaces:**
- Produces: a per-message metadata field `attachments: [{ path, name, isImage }]`; `RenderMsg.attachments?: { path, name, isImage }[]`.

- [ ] **Step 1: Core — store + round-trip test (cargo)**

Decide the storage: the user message's metadata JSON gets an `"attachments"` array. Write a core test (in `session.rs` or the store tests) that appends a user message with attachment metadata via the existing `append_message_meta` path and reads it back via `load_message_meta`/`display_messages_meta`, asserting the attachments survive. Use the existing temp-store test helpers. Implement whatever read-side change is needed so `display_messages_meta` (or the desktop `RenderMsg::from_meta`) exposes `attachments`.

- [ ] **Step 2: Desktop — write attachments in send_message**

In `ipc/chat.rs send_message`, when persisting the user message, write its `attachments` into the message metadata: build `[{ path, name, isImage }]` from the incoming `image_paths` (mark `isImage: true`) and any document attachment paths the turn carried. (Doc attachments arrive as `@path` tokens in the text — if only `image_paths` are separately available, persist those; note the limitation if doc-attachment paths aren't separately threaded.) Extend `RenderMsg` (ipc/mod.rs) with `pub attachments: Vec<AttachmentMeta>` (serde) and populate it in `RenderMsg::from_meta`.

- [ ] **Step 3: TS type + mock + contract**

Add `attachments?: { path: string; name: string; isImage: boolean }[]` to `RenderMsg` in `ipc.ts`. Update `contract/fixtures/load_session.json` so one message carries an `attachments` array; add the Rust contract assertion (the fixture deserializes into `Vec<RenderMsg>` with attachments). Keep the mock `load_session` returning it.

- [ ] **Step 4: Verify + commit**

`cargo test` (incl. contract), `pnpm check`. (Frontend rendering of these is Task 2.)
```bash
git add crates/zanto-core/src/session.rs crates/zanto-desktop/src-tauri/src/ipc/chat.rs crates/zanto-desktop/src-tauri/src/ipc/mod.rs crates/zanto-desktop/src/lib/ipc.ts crates/zanto-desktop/src/lib/mock/backend.ts crates/zanto-desktop/contract/fixtures/load_session.json crates/zanto-desktop/src-tauri/tests/contract.rs
git commit -m "feat(desktop): persist message attachment metadata across reopen (#7)"
```

---

### Task 2: #6 — show attachments on the user bubble (live + reopened)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/stores/session.svelte.ts` (ChatEntry carries attachments; `send` + reopen map them)
- Modify: `crates/zanto-desktop/src/lib/components/Composer.svelte` (`submit` passes attachment metadata to `send`)
- Modify: `crates/zanto-desktop/src/lib/components/Message.svelte` (render attachment chips on the user bubble)
- Test: `crates/zanto-desktop/tests/ui/`

**Interfaces:** Consumes `RenderMsg.attachments` (Task 1).

- [ ] **Step 1: Thread attachments into the user entry**

Extend `ChatEntry` with `attachments?: { path, name, isImage }[]`. Change `send(text, imagePaths, attachments?)` (or pass the full attachment list) so the pushed user `entry("user", …)` carries the attachment metadata; update Composer's `submit` to pass the composer's `attachments` (it already has `{path,name,isImage}`). On reopen, `toEntries`/`from_meta` maps `RenderMsg.attachments` onto the entry.

- [ ] **Step 2: Render chips on the user bubble**

In `Message.svelte`, when a user entry has attachments, render small chips below the text: a doc icon + name for non-images, an image thumbnail for images (thumbnail source loaded in Task 4 via the viewer's data-url path; for now an icon placeholder is acceptable and upgraded in Task 4). Keep it minimal and consistent with the composer's chip style.

- [ ] **Step 3: Test (Playwright)**

Add a test: mock `pick_files`/`pickFiles` to return an attachment path, attach it in the composer, send → assert an attachment chip appears on the user bubble. Then a reopen test: mock `load_session` returns a message with attachments → assert the chips render on reopen. (Discover the real attach affordance + chip selectors.)

- [ ] **Step 4: Verify + commit**

`pnpm check` + `pnpm test:ui`.
```bash
git add crates/zanto-desktop/src/lib/stores/session.svelte.ts crates/zanto-desktop/src/lib/components/Composer.svelte crates/zanto-desktop/src/lib/components/Message.svelte crates/zanto-desktop/tests/ui/
git commit -m "feat(desktop/ui): user message bubble shows attachments, live + on reopen (#6)"
```

---

### Task 3: backend IPC for the viewer + file-open (`read_image_data_url`, `open_path`)

**Files:**
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/files.rs` (or a suitable ipc module) — two new commands
- Modify: `crates/zanto-desktop/src-tauri/src/lib.rs` (register them)
- Modify: `crates/zanto-desktop/src/lib/ipc.ts` (`ipc.readImageDataUrl`, `ipc.openPath`)
- Modify: `crates/zanto-desktop/src/lib/mock/backend.ts` (mock handlers) + `contract/fixtures/{read_image_data_url,open_path}.json` + `contract.rs`

**Interfaces:**
- Produces: `read_image_data_url(path) -> String` (a `data:<mime>;base64,…` URL; permission-checked Read; size-capped) and `open_path(path) -> ()` (open with the OS default app via the opener plugin / `opener::open`; permission-checked Read).

- [ ] **Step 1: Implement the two commands (permission-checked)**

`read_image_data_url`: `state.permissions.check(&path, Op::Read)` → read bytes (cap, e.g. 10 MB) → infer mime from extension → return `format!("data:{mime};base64,{b64}")`. `open_path`: `permissions.check(&path, Op::Read)` → open via the bundled opener (`@tauri-apps/plugin-opener` backend / `opener` crate) or `tauri_plugin_opener`. Register both in `lib.rs invoke_handler`.

- [ ] **Step 2: ipc.ts wrappers + mock + contract**

Add `readImageDataUrl(path) => invoke("read_image_data_url",{path})` and `openPath(path) => invoke("open_path",{path})` to `ipc.ts`. Mock: `read_image_data_url` returns a tiny valid data-url (1×1 png); `open_path` no-op. Add fixtures + `#[test]`s (response shapes: a String / null).

- [ ] **Step 3: Verify + commit**

`cargo test` (contract), `pnpm check`, `pnpm test:ui` (seam guard still green — the new `@tauri-apps` usage stays in ipc.ts/mock).
```bash
git add crates/zanto-desktop/src-tauri/src/ipc/files.rs crates/zanto-desktop/src-tauri/src/lib.rs crates/zanto-desktop/src/lib/ipc.ts crates/zanto-desktop/src/lib/mock/backend.ts crates/zanto-desktop/contract/fixtures crates/zanto-desktop/src-tauri/tests/contract.rs
git commit -m "feat(desktop): read_image_data_url + open_path IPC commands (permission-checked)"
```

---

### Task 4: #5 — image viewer

**Files:**
- Create: `crates/zanto-desktop/src/lib/components/ImageViewer.svelte`
- Modify: `crates/zanto-desktop/src/lib/components/Message.svelte` (image chip → open viewer; thumbnail src via `readImageDataUrl`)
- Modify: `crates/zanto-desktop/src/lib/components/Composer.svelte` (image attachment chip → open viewer too, optional)
- Test: `crates/zanto-desktop/tests/ui/`

**Interfaces:** Consumes `ipc.readImageDataUrl` (Task 3) + the bubble attachment chips (Task 2).

- [ ] **Step 1: ImageViewer component**

A modal/lightbox: shows the image at full size (src = `readImageDataUrl(path)`), Esc/click-outside to close, a title (file name), and next/prev if a message has multiple images. Keyboard-accessible.

- [ ] **Step 2: Wire image chips to open it**

In `Message.svelte`, an image attachment chip renders a thumbnail (via `readImageDataUrl`) and on click opens `ImageViewer` for that image. 

- [ ] **Step 3: Test**

Send/reopen a message with an image attachment (mock `read_image_data_url` returns a 1×1 png data-url) → click the image chip → assert the viewer opens showing the image → Esc closes it. No fixed sleeps.

- [ ] **Step 4: Verify + commit**

`pnpm check` + `pnpm test:ui`.
```bash
git add crates/zanto-desktop/src/lib/components/ImageViewer.svelte crates/zanto-desktop/src/lib/components/Message.svelte crates/zanto-desktop/src/lib/components/Composer.svelte crates/zanto-desktop/tests/ui/
git commit -m "feat(desktop/ui): click-to-view image viewer for attachments (#5)"
```

---

### Task 5: #3 — file paths as openable anchor links

**Files:**
- Modify: `crates/zanto-desktop/src/lib/links.svelte.ts` (detect + open file paths)
- Modify: the rendered-markdown path (`Block.svelte`/`Markdown.svelte`/`Message.svelte`) to linkify backticked absolute paths
- Test: `crates/zanto-desktop/tests/ui/`

**Interfaces:** Consumes `ipc.openPath` (Task 3) + the existing `interceptLinks`/http-link handling.

- [ ] **Step 1: Linkify backticked absolute paths**

In the rendered markdown, turn a backticked **absolute or `~`-prefixed** path (`/…`, `~/…`, `C:\…`) into an `<a>` whose visible text is the path shortened relative to the project dir (when under it) and whose data carries the absolute path. Extend `links.svelte.ts`'s click interception to recognize these file-path anchors and call `ipc.openPath(absolutePath)` (instead of the http branch). Do NOT linkify bare (non-backticked) slash-strings — too many false positives.

- [ ] **Step 2: Test**

Use a `link`-style scenario whose assistant message contains a backticked absolute path (e.g. `` `/home/user/project/src/main.rs` ``). Assert it renders as a clickable link showing the (relative) path; click it → assert `open_path` is invoked (mock no-op, no error/navigation). Confirm an http link still works (C-12 unchanged) and a backticked *relative* path or prose slash-string is NOT linkified.

- [ ] **Step 3: Verify + commit**

`pnpm check` + `pnpm test:ui`.
```bash
git add crates/zanto-desktop/src/lib/links.svelte.ts crates/zanto-desktop/src/lib/blocks crates/zanto-desktop/src/lib/components/Message.svelte crates/zanto-desktop/tests/ui/
git commit -m "feat(desktop/ui): backticked absolute file paths render as openable links (#3)"
```

---

## Self-Review

**Spec coverage:** #7 → Task 1; #6 → Task 2; IPC for #5/#3 → Task 3; #5 → Task 4; #3 → Task 5. Covered.

**Placeholder scan:** Storage/read sites and component selectors are delegated to implementers (read the files) — consistent with prior batches; each task states the data shape, the new IPC contracts, and a concrete test. The one acknowledged uncertainty (whether doc-attachment paths are separately threaded vs only image_paths) is called out in Task 1 Step 2 with instruction to persist what's available and note the limitation.

**Type consistency:** `AttachmentMeta`/`{path,name,isImage}` consistent across core metadata, `RenderMsg.attachments`, ipc.ts type, ChatEntry, and the chip rendering. New IPC names (`read_image_data_url`/`readImageDataUrl`, `open_path`/`openPath`) consistent between Rust command, ipc.ts wrapper, mock, and contract.

**Risk:** Task 1 is the keystone (schema/metadata + backward-compat) — gate on the cargo round-trip + contract. #3 (Task 5) carries false-positive risk — the backticked-absolute-only rule is the mitigation; the test must assert a prose slash-string is NOT linkified. Real-runtime parts (image bytes via `read_image_data_url` on a real file, native `pickFiles`, OS `open_path`) are the user's manual verification — the mock proves the UI wiring only.
