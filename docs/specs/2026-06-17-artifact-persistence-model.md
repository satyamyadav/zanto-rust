# Artifact persistence model — save documents, not view blocks

- **Date:** 2026-06-17
- **Ask:** "all artifacts are not saved — only some artifacts like complete page, md file
  will be saved; others will be chat block or canvas views."

## Problem
Today two overlapping concepts exist: `render_artifact` (catalogue → ephemeral chat/canvas
block) and `store_artifact` (A3 → writes a file to disk, any kind). Nothing in the model
says *which* artifacts are durable. The intent: **documents are saved; views are ephemeral.**

## Model
Split artifacts into two classes:

- **Documents (persistable → A3 store, browseable):** a **markdown document**, a **complete
  page** (a composed multi-section report/page), and **images**. These are saved to
  `<project>/.zanto/artifacts` (or global), listed in the Artifact Browser, and can be
  re-opened later.
- **Views (ephemeral → chat block / canvas only):** `table`, `metric`, `list`, `kv`,
  `json`, `chart`, `nba`, and the finance views. These render via `render_artifact` and are
  **not** written to disk. They persist only inside the session (D1 stores them in message
  metadata for thread replay) — not in the global artifact store.

## Design
- **Catalogue:** add a `persist: "document" | "view"` (or `savable: bool`) field to each
  `ArtifactDef` in `catalogue.json`. Views = `view`; markdown + a new `page` = `document`.
- **Tool roles, made explicit:**
  - `render_artifact(id, data, target)` — display a **view** inline/canvas. If called on a
    `document`-class id, it still renders, but does not by itself persist.
  - `store_artifact(kind, title, content)` — persist a **document** (markdown/page/image)
    to the store; restrict/guide it to document kinds (drop `text`/`json` as storable, or
    keep but de-emphasize). Returns a ref the agent can mention; the doc opens in the
    Artifact Browser / a markdown view.
  - Update tool descriptions + the system-prompt artifact protocol to state: "render a
    table/chart to show it (not saved); save a markdown document or page with store_artifact
    (durable, appears in Artifacts)."
- **New `page` document artifact:** a "complete page" = an ordered list of sections (heading
  + markdown/blocks) rendered as a full document and savable. Schema:
  `{ title, sections: [{ heading?, markdown }] }` (v1: markdown sections; embedding view
  blocks inside a page is a later extension). Component: `Page.svelte` (reuses `Markdown`).
- **Artifact Browser (E4):** already lists only the store → automatically shows only
  documents/images. No change beyond the new `page` kind rendering.

## Affected files
- `crates/zanto-desktop/src-tauri/catalogue.json` (add `persist`/`page`),
  `crates/zanto-desktop/src/lib/registry.ts` + new `lib/blocks/Page.svelte`,
  `crates/zanto-desktop/src-tauri/src/catalogue.rs` (carry the field; render_artifact
  unaffected), `crates/zanto-core/src/tools/artifacts/store_artifact.rs` (guide kinds +
  description), `ipc/chat.rs` ARTIFACT_PROTOCOL (clarify save-vs-show), `artifacts/mod.rs`
  `ArtifactKind` add `Page` if stored as its own kind (or store pages as markdown).

## Open questions
- Is "complete page" a distinct savable type (recommended: yes, a `page` document), or just
  "a long markdown doc"? Spec assumes a `page` document type with sections.
- Should `store_artifact` auto-render the saved document inline too (save **and** show), or
  just save + return a link/ref? (Recommend: save and show a compact "Saved: <title>" card
  with an open action.)

## Acceptance
- Build-check clean; `catalogue.json` still parses (Rust `Catalogue::load`). Manual: asking
  for a table/chart renders a view that does **not** appear in the Artifact Browser; saving a
  markdown doc / page **does** appear and re-opens.
