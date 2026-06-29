# HTML page artifacts (sandboxed)

- **Date:** 2026-06-29
- **Status:** ✅ Shipped (2026-06-29). `html` catalogue artifact (file storage) +
  `ArtifactKind::Html` + `Html.svelte` rendering an `<iframe sandbox="allow-scripts">`
  (NO allow-same-origin) with an injected `default-src 'none'` CSP meta. Verified in
  mock: scripts run, `fetch` blocked by CSP, host fully isolated (parent unreadable,
  no Tauri bridge). Svelte-source compilation remains deferred to v2.

## Summary

Let the agent render an arbitrary HTML page as an artifact, displayed inside a
**locked-down sandboxed `<iframe srcdoc>` with `allow-scripts`** — JS runs, but the
iframe is null-origin and cannot reach the host app, the user's data, the Tauri
API, cookies, or the network. It opens in the canvas panel and is listed/openable
in the Artifact Hub.

## Motivation

Today the agent can only render a fixed set of catalogue components (chart, table,
metric, markdown, page, …) with schema-validated data — it cannot produce a custom
interactive page. The owner wants HTML/Svelte-page artifacts. This adds an `html`
artifact rendered in a hardened iframe. (Svelte-source compilation is explicitly a
later phase — see Out of scope.)

## Current state (grounded)

- **Block model** (`zanto-core/src/chat.rs:58-76`): `ChatBlock::Component {
  component_id, data, target }`, `Target { Inline, Canvas }`.
- **Catalogue** (`crates/zanto-desktop/src-tauri/catalogue.json`): 10 artifacts;
  each has `id`, `description`, `when_to_use`, `storage` ("view" | "file"),
  `data_schema`. The agent renders via the `render_artifact(id, data, target)`
  tool, which validates `data` against the catalogue's `data_schema`
  (`catalogue.rs:266-293`).
- **Frontend registry** (`src/lib/registry.ts`): `component_id` → Svelte
  component. `Block.svelte` looks up the registry, validates, renders `<Comp
  data={block.data} />`, else falls back to `<Json>`.
- **Canvas** (`Canvas.svelte`): renders a `target:"canvas"` block via `<Block>`;
  also hosts the ArtifactHub browser with a View|Artifacts tab.
- **Artifact Hub** (`ArtifactHub.svelte`): lists stored documents (file) + pinned
  views (db) in tabs; a viewer renders each.
- **HTML today**: markdown blocks go through `DOMPurify.sanitize(marked.parse(…))`
  then `{@html}`. **No iframe/srcdoc anywhere.** **CSP is `null`**
  (`tauri.conf.json` — fully permissive: inline + external scripts allowed).
- **Storage** (`zanto-core/src/artifacts/mod.rs`): `ArtifactKind { Text, Markdown,
  Image, Json }`, `ArtifactStore.save/list/read/delete`, files under
  `.zanto/artifacts/files/` (project) or `~/.local/share/zanto/artifacts/`
  (global), manifest `index.json`.

## Decisions (locked in review)

- **Sandbox:** `<iframe sandbox="allow-scripts" srcdoc=…>` — scripts run, host is
  isolated. **`allow-same-origin` is deliberately OMITTED** → the iframe is a null
  origin: no access to the parent DOM, `localStorage`, cookies, the Tauri
  `__TAURI__` bridge, or any same-origin resource.
- **Surface:** opens in the **canvas panel** AND is listed/openable in the
  **Artifact Hub** (file-storage artifact).

## The security model (the crux — read before implementing)

The app's global CSP is `null`, so the host page itself is permissive. The iframe
must therefore carry its OWN hardening; we do not rely on the host CSP. Layers:

1. **`sandbox="allow-scripts"`** and nothing else. Critically NOT
   `allow-same-origin` (combining the two would let the iframe remove its own
   sandbox), NOT `allow-popups`, `allow-top-navigation`, `allow-forms`,
   `allow-modals`. Null origin ⇒ no `window.parent` data access, no Tauri bridge.
2. **An injected CSP `<meta>` at the top of the srcdoc** that blocks network
   egress so a script can't exfiltrate or beacon out:
   `default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src
   'unsafe-inline'; font-src data:;` — i.e. inline script/style only, images only
   as `data:` URIs, NO `connect-src`/`fetch`/XHR/websocket, no remote `src`. This
   is injected by us, prepended to the agent HTML, so the agent can't weaken it
   (a second `<meta>` CSP can only *intensify*, never relax, the first).
3. **`referrerpolicy="no-referrer"`** and no `allow-downloads`.
4. The iframe is a leaf: it renders the agent string and nothing the agent sends
   can reach back into the app. Worst case is a misbehaving page that only affects
   its own iframe (CPU/visual) — contained, not a host compromise.

**Why allow-scripts at all:** the owner chose interactive HTML/JS over static-only.
The null-origin + no-network CSP keeps that safe: scripts can manipulate their own
DOM but have nothing to touch and nowhere to send.

## Scope

**In scope**
- A new **`html`** catalogue artifact (`storage: "file"`), `data_schema`
  `{ content: string (the full HTML), title?: string }`.
- A **`Html.svelte`** block component rendering the hardened iframe (sandbox + the
  injected CSP meta + a resize/scroll container). Registered in `registry.ts`.
- **`ArtifactKind::Html`** added in core so HTML pages persist as file artifacts;
  the hub lists + opens them (viewer renders the same `Html.svelte`).
- The agent can `render_artifact("html", { content, title }, "canvas")`; the
  catalogue `description`/`when_to_use` tells the model what it's for and that
  scripts run sandboxed with no network.
- Mock: a scenario/trigger producing an `html` block so it's testable in
  `dev:mock`.

**Out of scope**
- **Svelte-source compilation** (compiling agent `.svelte` → JS). Deferred to a v2
  — it needs a compiler in the runtime and is much heavier. v1 ships HTML (the
  agent can still emit `<script>` for interactivity, sandboxed). Note this in the
  catalogue `description` so the model emits HTML, not Svelte SFCs.
- Relaxing the sandbox / allowing network / `allow-same-origin` — never.
- Hardening the global app CSP (separate concern; the iframe self-hardens).
- Inter-iframe messaging / the page calling app actions (no `postMessage` bridge
  in v1 — would re-introduce host attack surface).

## Affected files

- `crates/zanto-desktop/src-tauri/catalogue.json` — add the `html` artifact def
  (storage "file", schema, when_to_use mentioning sandboxed/no-network).
- `crates/zanto-core/src/artifacts/mod.rs` — add `ArtifactKind::Html` (+ its
  extension `.html`, MIME mapping, the `from`/`as_str` arms + any match on the
  enum). Unit test for the new kind round-tripping through save/list/read.
- `crates/zanto-desktop/src/lib/blocks/Html.svelte` (new) — the sandboxed iframe.
- `crates/zanto-desktop/src/lib/registry.ts` — `html: Html`.
- `crates/zanto-desktop/src/lib/components/ArtifactHub.svelte` — render an
  `html`-kind stored artifact via `Html.svelte` in the viewer (alongside the
  markdown/image/text arms).
- `crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs` — if `StoredArtifact`
  kind is an enum/string, ensure "html" flows through read/list (likely just the
  kind string; verify the viewer switch covers it).
- `crates/zanto-desktop/src/lib/mock/backend.ts` + `scenarios.ts` — an `html`
  block scenario + (optionally) a stored html artifact for the hub.
- Possibly `crates/zanto-core/src/tools/artifacts/store_artifact.rs` — if it
  validates/whitelists `kind`, allow "html".

## Implementation steps

1. **Core: `ArtifactKind::Html`** (`artifacts/mod.rs`)
   - Add the variant; wire its extension (`html`), any MIME/`as_str`/`from_str`/
     `Serialize` arms, and every `match` over `ArtifactKind` (compiler will flag
     them). Add a save→list→read round-trip test for an html blob.

2. **Catalogue: the `html` artifact** (`catalogue.json`)
   - `{ "id": "html", "storage": "file", "description": "Render a self-contained
     HTML page (sandboxed: scripts run with NO network and NO access to the app or
     your data). Emit a full HTML document as `content`.", "when_to_use": "When the
     user wants a custom interactive page/visualization that the fixed components
     can't express. Inline <style>/<script> only; no external/CDN resources (they
     are blocked).", "data_schema": { type object, properties: { content: {type
     string}, title: {type string} }, required: ["content"] } }`.
   - Verify `render_artifact` validates this and emits the block (no code change in
     `catalogue.rs` expected — it's schema-driven).

3. **`Html.svelte`** (new block component)
   - Props `{ data: { content: string; title?: string } }`.
   - Build the srcdoc = injected CSP `<meta>` (step from the security model) +
     a `<base target="_blank">`-less head + the agent's `content`. If `content`
     already has `<html>`, inject the meta right after `<head>` (or prepend a head
     if none); keep it robust to a bare fragment (wrap in a minimal document).
   - `<iframe sandbox="allow-scripts" referrerpolicy="no-referrer" srcdoc={…}
     class="h-full w-full border-0" title={data.title ?? "HTML artifact"}>`.
   - A surrounding container sizing it to the canvas/hub area (full height, scroll
     inside the iframe). No `allow-same-origin`, no other tokens.
   - Do NOT use `{@html}` for the content — it goes into the iframe `srcdoc`
     attribute (string), never into the host DOM.

4. **Register** (`registry.ts`) — `html: Html`. `Block.svelte` then renders it for
   `component_id:"html"`; the existing pin logic is view-only (`viewArtifacts`),
   so html (file storage) won't show a pin button — correct.

5. **Artifact Hub viewer** (`ArtifactHub.svelte`)
   - In the stored-artifact viewer switch, add an `html` arm rendering
     `<Html data={{ content: text, title }} />` so a saved html page reopens
     sandboxed (same component, same guarantees).

6. **Canvas** — already renders any `target:"canvas"` block via `<Block>`; once
   `html` is in the registry it works with no Canvas change. Verify the iframe
   fills the canvas pane and scrolls.

7. **Mock** — add an `html` trigger in `scenarios.ts` emitting a `render_artifact`/
   `chat_block` with a small interactive HTML doc (e.g. a button that mutates the
   iframe DOM + a fetch attempt that the CSP blocks) so the sandbox is observable
   in `dev:mock`. Optionally seed a stored html artifact for the hub.

## Edge cases & risks

- **No new dependency.** Iframe is native; no sanitizer needed (isolation, not
  sanitization, is the boundary). DOMPurify is NOT used here — we WANT the agent's
  HTML intact inside the sandbox, just contained.
- **`allow-same-origin` footgun** — combining it with `allow-scripts` lets the
  frame script remove the sandbox. We never set it. **The single most important
  invariant; assert it in a test/comment.**
- **CSP-meta bypass** — a second injected `<meta http-equiv="Content-Security-
  Policy">` cannot *loosen* the first; the agent prepending its own can only add
  restrictions. We inject ours FIRST in the head so it binds.
- **Network egress** — blocked by `default-src 'none'` + no `connect-src`. Verify
  a `fetch()` in the agent HTML fails (test step).
- **Tauri bridge** — null origin ⇒ `window.__TAURI__` is undefined inside the
  frame; the page can't invoke commands. Verify.
- **External resources / CDN** — blocked (no `script-src`/`img-src` remote). The
  catalogue text must tell the model to inline everything; a page referencing a
  CDN will render without it (acceptable, documented).
- **Large content** — `srcdoc` holds the whole doc as a string attribute; fine for
  reasonable pages. Multi-MB pages are an edge case (acceptable; not optimized).
- **Persisted html reopened** — must render through the SAME `Html.svelte` so the
  sandbox applies on reload too (step 5), never via `{@html}`.
- **Printing/links** — links inside open nowhere (no `allow-popups`/
  `allow-top-navigation`); acceptable for v1.

## Acceptance criteria

Verifiable in the desktop app (mock reproduces an html block) + `cargo test`:

- [ ] The agent rendering `render_artifact("html", { content }, "canvas")` shows
      the HTML page in the canvas panel, inside a sandboxed iframe.
- [ ] Inline `<script>` in the page RUNS (e.g. a button updates the iframe DOM),
      proving `allow-scripts` works.
- [ ] A `fetch()`/XHR/remote `<img>`/remote `<script>` in the page is BLOCKED (no
      network egress), proving the injected CSP holds.
- [ ] Inside the iframe, `window.parent`/`top` data access and `window.__TAURI__`
      are unavailable (null origin), proving host isolation.
- [ ] The iframe element has `sandbox="allow-scripts"` and NOT `allow-same-origin`
      (assert in the rendered DOM / a UI test).
- [ ] A saved html artifact opens from the Artifact Hub and re-renders sandboxed
      (same guarantees on reload).
- [ ] `cargo test -p zanto-core` green incl. the `ArtifactKind::Html` round-trip.
- [ ] `pnpm check` 0/0; UI suite green (existing artifact/canvas tests unaffected).

## Manual test plan

1. `cargo test -p zanto-core` → html artifact kind saves/lists/reads.
2. `pnpm dev:mock`; send the `html` trigger → canvas shows the page; click its
   button → DOM updates (scripts run); observe the blocked `fetch` (console CSP
   error), proving no network.
3. DevTools: inspect the iframe → `sandbox="allow-scripts"` only; evaluate
   `frames[0].window.__TAURI__` → undefined; `frames[0].document` access throws
   (cross-origin) — host isolated.
4. (If wired) save the page → it appears in the Artifact Hub → reopen → renders
   sandboxed again.
5. Real app: ask the agent for "an interactive HTML page with a counter button" →
   it renders and the button works, with no network and no app access.
