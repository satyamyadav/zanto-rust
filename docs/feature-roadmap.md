# Feature roadmap

The seven queued features, sequenced quick-wins-first: small UI polish, then
medium features, then the large new apps. Each is built through the native flow
(`/spec` → review → `/execute`); this doc is the order + scope sketch, not the
specs. Specs are written when a feature starts (the small ones —1, 2, 3— already
have specs).

## Order & status

| # | Feature | Size | Risk | Spec | Notes |
|---|---|---|---|---|---|
| 1 | Token counter | M | low | ✅ **shipped** | full-stack; per-message label + session gauge |
| 2 | Loader at end of message | S | low | ✅ **shipped** | tail loader persists whole turn |
| 3 | User chat-bubble restyle + spacing | S | low | ✅ **shipped** | refined fill + grouped spacing |
| 4 | Skills editor | M | med | — | UI over `.zanto/skills` markdown |
| 5 | Svelte/HTML-page artifacts | M-L | **high** | — | renders arbitrary HTML → security |
| 6 | File Manager app | L | med | — | new micro-app + agent tools |
| 7 | Video Editor app | XL | high | — | new app + media tooling (ffmpeg) |

Build top-to-bottom. Each ships and is verifiable before the next.

## Phase A — UI quick wins (1–3)

Small, low-risk, high-visibility. Do them back-to-back to build momentum.

- **1. Token counter** ✅ shipped — captures genai usage (chars/4 fallback), shows
  a per-message label + a session total/context gauge in the composer. Spec
  archived: `docs/archive/2026-06-28-token-counter.md`.
- **2. Loader at end of message** ✅ shipped — a "responding…" indicator at the
  thread tail that persists the whole busy turn, complementing the thinking-block
  spinner. Spec archived: `docs/archive/2026-06-29-message-loader.md`.
- **3. Chat-bubble restyle + spacing** ✅ shipped — refined the user bubble (fill
  kept, hairline border, softer radius, no shadow) and grouped inter-turn spacing
  so exchanges read as pairs. Spec archived:
  `docs/archive/2026-06-29-chat-bubble-restyle.md`.

## Phase B — medium features (4–5)

- **4. Skills editor.** A UI to create / edit / delete the markdown skills under
  `.zanto/skills` (and the global skills dir). Builds on the existing skill
  plumbing (`list_skills` / `get_skill`, the composer `/skill` picker, the
  `selected_skill` config). Scope sketch: a panel/dialog listing skills, a
  markdown editor, save/delete via new IPC over the existing `context.rs` skill
  loader. Medium; mostly UI + a couple of write IPCs. Needs its own clarification
  round (where it lives — Settings tab? own app? — and whether it edits global +
  project skills).

- **5. Svelte/HTML-page artifacts.** Render an agent-produced HTML (or Svelte)
  page as an artifact in the canvas/hub. **High risk — security.** Arbitrary
  HTML/JS in the app's webview is an XSS/exfiltration vector; must run sandboxed
  (an isolated `<iframe sandbox>` or a separate webview with no Tauri IPC access,
  CSP locked down). Scope sketch: a new artifact kind `html` (or `webpage`), a
  sandboxed renderer block, storage like other artifacts. The spec must lead with
  the threat model (what the page can/can't touch) before any rendering. Defer
  until the threat model is agreed.

## Phase C — large new apps (6–7)

Each is a full micro-app (Svelte panel + agent-operable tools over Tauri IPC),
per the micro-app architecture in `product.md`. These get their own brainstorm +
spec + likely multi-phase builds.

- **6. File Manager app.** A panel to browse/operate the filesystem (within the
  permission-gated allowed paths), agent-operable (the agent can navigate, move,
  rename, etc. via chat). Reuses the existing fs tools + permission guard. Scope
  sketch: a Svelte file-tree/list panel, file ops via new app tools, all gated by
  `PermissionGuard`. Medium-large; the permission model already exists, so the
  work is mostly the panel + app-tool wiring. Own spec.

- **7. Video Editor app.** The largest. A panel to trim/cut/arrange video, agent-
  operable. Almost certainly needs **ffmpeg** (a new system/bundled dependency —
  big packaging implication, especially after the WebKit-bundling lessons) and a
  media-preview surface. Scope sketch: ffmpeg-backed operations as app tools, a
  timeline/preview UI. XL; needs its own brainstorm — clarify the editing scope
  (trim-only vs multi-clip timeline vs effects), the ffmpeg dependency strategy
  (system vs bundled), and performance/preview approach before any spec. Last.

## Cross-cutting notes

- **No superpowers/context7** — native `/dev,/spec,/execute` only (see
  `working-flows.md`).
- **Each feature: spec → approve → execute → verify** (build + manual run). Big
  apps (6, 7) and the security-sensitive #5 get a dedicated clarification round
  before speccing.
- **Packaging awareness:** #7's ffmpeg dependency interacts with the Linux
  distribution work (system-libs `.tar.gz` / `.deb`/`.rpm`). Decide system vs
  bundled ffmpeg early.
- Order is a default, not a contract — reprioritize anytime.
