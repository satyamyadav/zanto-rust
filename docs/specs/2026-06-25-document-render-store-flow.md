# Document flow: render + store, deliberate project save, no raw write_file drops

_Date: 2026-06-25_

## Problem

Asked to "write an article", the agent used `write_file` to drop
`agentic_ai_article.md` into the **project root**. Consequences:

1. The file landed in the project dir **ungated** — `set_project_dir`
   auto-adds the project to `allowed_paths`, so `write_file` there passes the
   permission check silently.
2. It did **not** appear in the Artifacts panel — `write_file` bypasses the
   artifact store entirely.
3. There was no render/preview-then-save flow — a raw file just appeared.

The codebase already has the right primitives: `render_artifact` (ephemeral
Canvas/inline view) and `store_artifact` (durable file in `.zanto/artifacts`,
browsable in the Artifacts panel). The model simply didn't use them for a
"write a document" request — it reached for `write_file`, which has a bare
description and no steering away from it.

## Desired flow (locked with user)

- **Default for "write a document/article/report/notes":** the agent
  **renders it in the Canvas AND persists it to the artifact store**
  (`.zanto/artifacts`), so it shows in the Artifacts panel. It does **not**
  write a raw file into the project root.
- **Deliberate permanent save:** a **static UI "Save to project…" button** on
  the rendered Canvas view writes the document into the project dir on demand.
  Persistence to the *project* is a user action, not something the model does
  arbitrarily.
- **Gating:** keep the project dir auto-allowed (no permission-model change);
  **steer tool choice** via tool descriptions + the artifact protocol so the
  model prefers `store_artifact`/`render_artifact` over `write_file` for
  documents. (We do not forbid `write_file` — it remains for genuine
  "edit this code file" tasks — we bias against it for generated documents.)

Net effect: generated documents flow into the browsable artifact store and the
Canvas, never littering the project root; the user promotes one to the project
dir with an explicit click.

## Layer 1 — prompt + tool-description steering (zanto-core + ipc/chat.rs)

### 1a. `write_file` description (`crates/zanto-core/src/tools/fs/write_file.rs`)

Current: `"Write content to a file, creating it and any missing parent
directories"`. Replace with a description that scopes it to *editing
existing/known files* and redirects generated documents to the artifact tools:

> "Write content to a specific file the user named or that you are editing
> (e.g. source code, a config file, an existing file at a known path). Do NOT
> use this to save a document, article, report, or notes you generated — those
> go to `store_artifact` (durable, browsable) and `render_artifact` (to show
> them). Creates missing parent directories."

This is description-only; the tool's behavior and permission check are unchanged.

### 1b. `ARTIFACT_PROTOCOL` (`crates/zanto-desktop/src-tauri/src/ipc/chat.rs`)

Append a clause that names the "write a document" case explicitly and pairs
render with store:

> "When the user asks you to WRITE a document, article, report, or notes
> (prose you generate, not an edit to a named code/config file): call
> `store_artifact` to persist it (it appears in the Artifacts browser) and
> `render_artifact` to show it in the Canvas. Do NOT use `write_file` to drop a
> generated document into the project folder — `write_file` is only for editing
> a specific file the user named."

Rationale: the existing protocol explains the tools but never maps the common
"write me a doc" intent onto render+store, so the model defaults to `write_file`.

## Layer 2 — auto-store on document render (decision: render + store together)

The user wants a rendered document to also be browsable in Artifacts without a
separate manual step. Two implementation options — pick during planning:

- **(A) Prompt-only:** rely on Layer 1's instruction telling the model to call
  BOTH `store_artifact` and `render_artifact`. Simplest; no Rust flow change;
  weaker guarantee (model may call only one).
- **(B) Auto-store on markdown render:** when `render_artifact` (or the chat
  app) renders a **markdown document** to the Canvas, also persist it to the
  artifact store in the same step, so it always shows in Artifacts. Stronger
  guarantee; a small change where the Canvas render is dispatched
  (`catalogue.rs` render path / `ipc/chat.rs`).

**Recommendation:** start with (A) (prompt-only) since the chosen gating
approach is "steer, don't enforce", and it keeps Layer 2 zero-code. Revisit (B)
only if the model still under-stores in manual testing. The plan should
implement (A) and note (B) as a follow-up if testing shows it's needed.

## Layer 3 — Canvas "Save to project" button (UI)

`crates/zanto-desktop/src/lib/components/Canvas.svelte` renders the agent view
via `<Block block={sessionStore.canvas} canPin={false} />`. When the canvas
block is a **markdown document**, show a static action button:

- **"Save to project…"** — writes the document to the project dir. On click:
  pick/confirm a filename (default from the artifact title), then call an IPC
  that writes into the project dir. Reuse `write_file`'s permission-checked
  path so it stays gated by the existing guard (project dir is allowed, so no
  prompt — but the write goes through the same `check(Op::Write)`).

Implementation notes:
- The button only appears for markdown-document canvas content, not for
  charts/tables/metrics (those are pinned via the existing pin flow, not saved
  as files).
- Need the raw document text + a title from `sessionStore.canvas`. Verify the
  canvas block carries the markdown text (it renders it, so the text is
  present on the block) and a title; if no title, default to a slug + `.md`.
- IPC: prefer an existing command if one writes text to a path
  (`write_file` is a *tool*, not necessarily an IPC). If no suitable IPC
  exists, add a thin `save_document_to_project(title, text)` command in
  `ipc/` that resolves `<project_dir>/<title>.md` and writes via the
  permission-checked path. The plan must confirm which exists before adding.

This is the "deliberate, static save action" the user asked for: deterministic,
not model-dependent.

## What is NOT changed

- The permission model: `set_project_dir` still auto-allows the project dir.
  No read-only-project mode, no removal of the auto-allow. (User chose "keep
  project allowed, steer tools only".)
- `write_file` behavior and its permission check — only its description.
- `render_artifact` / `store_artifact` / `pin_artifact` mechanics.
- The artifact store layout (`.zanto/artifacts`).

## Tests

- **zanto-core:** `write_file` description change is data-only; add/adjust a
  test asserting the description steers away from generated documents (mirror
  the style of existing tool-description tests if any; otherwise a simple
  `contains("store_artifact")` assertion on the description string).
- **ipc/chat.rs:** assert `ARTIFACT_PROTOCOL` contains the new "WRITE a
  document" clause (substring test, mirroring existing prompt-substring tests).
- **UI:** a Playwright test is optional and model-independent only for the
  button's presence/click → IPC call (can mock the canvas markdown state).
  Manual verification is the primary gate: ask "write an article", confirm it
  renders in Canvas + appears in Artifacts (Layer 1/2), and that "Save to
  project…" writes the file on click (Layer 3).
- `cargo build` + `cargo test` green; `pnpm check` 0/0.

## Risk / honesty notes

- Layers 1–2 are **steering, not enforcement** (per the chosen gating). A model
  can still call `write_file` for a document if it insists; the goal is to make
  the artifact path the obvious default, which fixes the reported common case.
- Layer 3 is the deterministic part — the one guaranteed, user-controlled path
  to the project dir.
- Splitting: Layer 1 (core+prompt) and Layer 3 (UI) are independent and could
  ship separately. Layer 1 is the highest-value, lowest-risk piece and should
  land first.

## Suggested implementation order

1. Layer 1 (write_file description + ARTIFACT_PROTOCOL clause) — core/prompt,
   small, high value. Manually verify the model now renders+stores instead of
   write_file.
2. Layer 3 (Canvas Save button + IPC) — the deliberate save.
3. Layer 2(B) only if manual testing shows the model still under-stores.
