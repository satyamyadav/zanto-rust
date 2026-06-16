# Wave E — Artifacts & tools (E1–E4)

- **Date:** 2026-06-17
- **Depends on:** catalogue+registry+Block (B-era), A3 artifact store + tools
  (`store_artifact`/`list_stored_artifacts`/`read_stored_artifact`), B1 (`ipc/*`).

Chart library decision: **Chart.js** (thin Svelte wrapper).

## E1 — Chart artifact (Chart.js)
- Files: `crates/zanto-desktop/src-tauri/catalogue.json`, `crates/zanto-desktop/src/lib/registry.ts`,
  new `crates/zanto-desktop/src/lib/blocks/Chart.svelte`, `crates/zanto-desktop/package.json`.
- Add `chart.js` dep. New catalogue artifact `chart` with a `data_schema`:
  `{ type: "bar"|"line"|"pie"|"doughnut", title?, labels: string[], datasets: [{label?, data: number[]}] }`.
  Register `chart → Chart.svelte` in the registry. `Chart.svelte` mounts a Chart.js canvas
  (destroy on unmount, recreate on data change), themed to the app's CSS variables, sized to
  fit inline or canvas targets. Validate via the existing Rust+AJV path (no special-casing).
- Acceptance: `render_artifact("chart", …)` renders a chart; bad data → validation error.
  `pnpm check`/`build:web` clean; `cargo build` clean.

## E2 — Markdown-preview artifact
- Files: `catalogue.json`, `registry.ts`, new `crates/zanto-desktop/src/lib/blocks/Markdown.svelte`.
  (Sequence after E1 if a single owner does both — they share catalogue.json + registry.ts.)
- New artifact `markdown` with schema `{ title?, content: string }`. `Markdown.svelte`
  renders sanitized markdown (reuse `marked` + `DOMPurify` already used in `Block.svelte`;
  factor a shared helper if clean) with the app's prose styling. Useful for agent-produced
  documents and previewing stored markdown artifacts (E4).
- Acceptance: `render_artifact("markdown", {content})` shows formatted, sanitized markdown.

## E3 — Web browsing tool (core)
- Files: new `crates/zanto-core/src/tools/web/` (mod + `fetch_url`), `tools/mod.rs` (register),
  `crates/zanto-core/Cargo.toml` if a fetch/extract dep is needed (prefer reusing `reqwest`
  already pulled via genai; add `scraper`/`html2text` only if necessary).
- `fetch_url { url, mode?: "text"|"raw" }` → fetches over HTTPS, extracts readable text
  (strip scripts/styles; cap length), returns `{ url, title?, text }`. **Read-only** tool;
  gate via the permission system as a non-path "network" resource OR (simpler for v1) treat
  as always-allowed read but documented — match how the existing tools express read-only.
  Timeout + size cap; refuse non-http(s) schemes and obvious localhost SSRF (configurable).
- Acceptance: unit test parses a small HTML string → text extraction (no network in tests;
  factor the extractor as a pure fn and test that). `cargo build` + `cargo test -p zanto-core`.

## E4 — Global artifact browser (desktop)
- Files: new `crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs` (+ `lib.rs` register),
  `crates/zanto-desktop/src/lib/ipc.ts`, a new browser UI surface (e.g.
  `crates/zanto-desktop/src/lib/components/ArtifactBrowser.svelte`) reachable from the
  Sidebar or Canvas; reuse `Markdown.svelte` (E2) for md preview and an `<img>` for images.
- Commands wrapping A3's store: `list_stored_artifacts(scope?)`, `read_stored_artifact(id)`
  (call the core `ArtifactStore` held in state, or construct from `Settings`). `ipc.ts`
  wrappers. UI: list artifacts (title/kind/scope/date), click to preview (md rendered, image
  shown, json/text in a code view).
- Acceptance: browser lists artifacts saved via the store tools and previews markdown/image.
  Build-check clean. (Depends E2 for markdown preview — sequence after E2.)

## Acceptance (every unit)
`cargo build` + `cargo test -p zanto-core` + `pnpm check` (0 errors) + `pnpm build:web`.

## Batching note (coordinator)
E1→E2 share catalogue.json+registry.ts (one owner, sequential, or E2 after E1). E3 is
core-only (parallel-safe). E4 depends on E2 and adds its own `ipc/artifacts.rs`.
Suggested: `{E1, E3}` → `{E2}` → `{E4}`.
