# Artifact persistence model — route by what the artifact IS

- **Date:** 2026-06-17
- **Ask:** "all artifacts are not saved — only some (complete page, md file) saved; others
  are chat block / canvas views."
- **Resolved (user):** persistent **view+data** → **DB store**; **file** (md/image) →
  **filesystem store**; choose **working (project) dir vs global** by what's present, and
  **ask the user to set a working dir** when context calls for it.

## Three classes
1. **Ephemeral view** — pure presentation over transient data (`table`, `metric`, `list`,
   `kv`, `json`, `chart`, `nba`, finance views). Shown via `render_artifact` as a chat block
   or canvas view. **Not persisted** to any durable store; survives only inside the session
   (D1 message metadata for thread replay).
2. **Persistent view+data** — a view the user wants to keep and re-open (a pinned chart, a
   saved report/page that composes components + their data). Persisted as a **record in the
   DataStore (DB)**: `{ component_id, data, target, title, created_at }`, so it can be
   **re-rendered** later and listed in the browser. Structured/queryable, not a file.
3. **File document** — a real file: **markdown document** or **image** (a "complete page"
   exported as markdown counts here). Persisted to the **filesystem artifact store (A3)**.

## Storage routing
- **render_artifact(id, data, target)** → ephemeral view (class 1). Never writes a store.
- **save_artifact(...)** → persist, routing by class:
  - **file kind** (markdown / image / page-as-markdown) → **filesystem** store, at
    `<project>/.zanto/artifacts` when a working/project dir is set, else the **global** store.
  - **view kind** (a view+data the user pins) → **DataStore** record (component_id + data).
- **Browser (E4)** lists **both** backends: filesystem documents + DB-pinned views; opening a
  DB view re-renders it as a block, a file opens its markdown/image.

## Working-dir vs global + the prompt
- Resolution order for file saves: `project_dir` if set → else global.
- **Ask when context calls for it:** when the agent saves a file-class artifact and
  `project_dir` is unset, surface a HITL prompt (reuse the `ask`/confirm channel): "Save to a
  project folder or the global store?" with a folder picker → on pick, set `project_dir`
  (and allow-path) so future outputs land there. Don't silently choose project when none
  exists; don't nag when global is fine. (Ties into the Workspace spec, which owns setting
  `project_dir`.)

## Tooling + catalogue
- Catalogue `ArtifactDef` gains a class hint so the model knows save behavior, e.g.
  `storage: "ephemeral" | "view" | "file"` (table/chart… = ephemeral-or-view; markdown/page/
  image = file). Render vs save is the *action*; storage backend follows the class above.
- **One save tool** `save_artifact` (route internally) is simpler for the model than two;
  keep `render_artifact` strictly for display. Update the system-prompt artifact protocol +
  tool descriptions: "render a table/chart to **show** it (not saved); **save** a markdown
  doc/page (file) or pin a view (kept in your data) with save_artifact."
- "Complete page": a `page` = ordered sections (heading + markdown), rendered by a
  `Page.svelte`; saved as a **markdown file** (class 3) by default, or as a **DB view** if it
  embeds live view+data components.

## Affected files
- `crates/zanto-core/src/artifacts/mod.rs` (fs store; project-vs-global already supported) +
  a **DataStore-backed** view-pin path (new small module or reuse `data`), `tools/artifacts/`
  (`save_artifact` routing + descriptions), `crates/zanto-desktop/src-tauri/catalogue.json`
  (`storage` field + `page`), `catalogue.rs`, `registry.ts` + new `lib/blocks/Page.svelte`,
  `ipc/artifacts.rs` + `ArtifactBrowser.svelte` (list both backends), `ipc/chat.rs`
  ARTIFACT_PROTOCOL.

## Open questions
- One `save_artifact` (recommended) vs separate `store_document` + `pin_view`?
- DB-pinned views: a new `artifacts` logical DataStore, or fold into an existing store?

## Acceptance
- Build-check clean; catalogue parses. Manual: a table/chart renders but is **not** in the
  browser; saving a markdown doc writes a file (project if set, else global, with the prompt
  when unset); pinning a chart stores it in the DB and it re-renders from the browser.
