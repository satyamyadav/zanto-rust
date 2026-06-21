# zanto v1 — Launch Plan

Date: 2026-06-21
Status: Plan (pending execution)

## Goal

Ship **zanto v1.0.0** as a downloadable desktop app (Orbit-style distribution):
unsigned installers for macOS, Windows, and Linux hosted on **GitHub Releases**,
fronted by a **static marketing site on GitHub Pages (Astro, dedicated repo)**,
announced via a **blog post** cross-posted to **LinkedIn**.

Not in v1: code signing / notarization, app-store presence, auto-update.

## Decisions (locked)

| Topic | Decision |
|---|---|
| Platforms | macOS + Windows + Linux, all **unsigned** |
| Distribution | GitHub Releases (artifacts), built in CI |
| Site host | **Dedicated repo** `zanto-site` → GitHub Pages |
| Site stack | **Astro** (marketing + blog) |
| Announcement | Blog post on the site → shared to LinkedIn |
| Product name | `zanto` (identifier `com.lazy.zanto`) |

## Open decisions (resolve during execution)

- **LICENSE** — none in the repo yet. Pick one before public release (MIT is the
  low-friction default for a BYO-key client; or a source-available license if you
  want to keep commercial options open). **Blocks public launch.**
- **Custom domain** — start on `*.github.io`; reserve `zanto.app`/similar as a
  fast follow (DNS + Pages CNAME). Plan keeps the domain swappable.
- **macOS build** — universal binary vs separate arm64/x64 dmgs (universal is
  simpler for users; larger artifact).
- **Telemetry** — recommend **none** for v1 (privacy is a selling point).

---

## Positioning

**One-liner:** *A local-first AI workspace for your desktop — bring your own
model and keys; your files never leave your machine.*

**Why it's different (the pitch):**
- **You own the stack.** Bring-your-own API key (stored in the OS keychain), or
  run fully offline against local **Ollama**. No middleman server.
- **10+ providers, one app.** Anthropic, OpenAI, Gemini, Groq, xAI, DeepSeek,
  Together, Fireworks, Cohere, Ollama — switch models live, tune generation
  params per provider.
- **Real tools, with consent.** Reads/writes/searches files, runs shell
  commands, fetches the web, parses PDFs/Office docs — every filesystem action is
  permission-gated (allow once / session / forever / deny).
- **Artifacts, not just chat.** Renders charts, tables, metrics, and documents
  inline or on a side canvas; pin the ones worth keeping.
- **Sessions that survive.** SQLite-backed, crash-safe, resumable, with automatic
  context summarization for long conversations.
- **Also a CLI.** `zanto` runs in the terminal for one-shot and interactive use.

**Audience:** developers and power users who want a private, model-agnostic AI
assistant with file/shell access they control.

---

## Workstream A — Release engineering (app repo)

**Deliverable:** `v1.0.0` tag produces a GitHub Release with installers for all 3 OSes.

### A1. Version + metadata
- Bump to `1.0.0`: workspace crates (`zanto-core`, `zanto-cli`) and
  `crates/zanto-desktop/src-tauri/tauri.conf.json` `version`.
- Fill Tauri bundle metadata: `bundle.category`, `shortDescription`,
  `longDescription`, `publisher`, `copyright`, `homepage`. Confirm `icons/` has
  the full set (256/512/icns/ico) — generate via `tauri icon` from a 1024px PNG
  if missing.
- Verify `bundle.targets`: per-OS → Linux `["appimage","deb"]`, macOS `["dmg"]`,
  Windows `["nsis","msi"]` (or keep `"all"` and let each runner emit its natives).

### A2. CI release workflow (`.github/workflows/release.yml`)
- Trigger: push of tag `v*` (and `workflow_dispatch` for dry runs).
- Matrix: `macos-latest` (or `macos-14` for arm), `ubuntu-22.04`, `windows-latest`.
- Steps per runner: checkout → setup Rust + Node → install frontend deps
  (`npm ci` in `crates/zanto-desktop`) → Linux deps (webkit2gtk etc.) →
  `tauri-apps/tauri-action` to build and **create/append the GitHub Release**.
- **Unsigned:** no signing secrets; macOS produces an ad-hoc/unsigned `.dmg`,
  Windows an unsigned NSIS/MSI. Document the consequences (below).
- Output assets named with version + OS + arch.
- Add a CI **build check** workflow (PR-triggered: `cargo build`, `cargo test`,
  `npm run check`) separate from release.

### A3. Release notes + install docs
- `CHANGELOG.md` and the GitHub Release body: highlights, known limitations
  ("unsigned — see install notes"), checksums.
- **Unsigned-install instructions** (also surfaced on the site /download page):
  - **macOS:** "right-click → Open" the first time, or
    `xattr -dr com.apple.quarantine /Applications/zanto.app`.
  - **Windows:** SmartScreen → "More info" → "Run anyway".
  - **Linux:** `chmod +x zanto_*.AppImage` and run; or `sudo dpkg -i` the `.deb`.

### A4. Repo hygiene for a public launch
- **README.md**: hero line, screenshot, feature bullets, download links/badges,
  quickstart (set a provider key or point at Ollama), build-from-source, CLI usage.
- **LICENSE** (see open decision) — required before flipping public.
- Scrub for anything not meant to be public (the repo is currently named
  `zanto-rust`; decide whether to rename to `zanto`).

---

## Workstream B — Marketing site (new repo `zanto-site`, Astro → GitHub Pages)

**Deliverable:** live static site with features + download + blog.

### B1. Scaffold
- New repo `zanto-site`; `npm create astro@latest` (+ Tailwind integration).
- Pages deploy via `withastro/action` + `actions/deploy-pages` on push to `main`.
- `astro.config` `site`/`base` set for the Pages URL (swappable for a custom
  domain later via `public/CNAME`).

### B2. Pages/content
- **Home:** hero (one-liner + primary **Download** CTA + screenshot/demo GIF),
  feature sections (providers, tools+permissions, artifacts, local-first/privacy,
  CLI), "how it works", footer (GitHub, blog).
- **Download:** per-OS buttons resolving to the **latest GitHub Release** assets
  (via the releases API at build time, or a "latest" redirect), plus the
  unsigned-install instructions per OS.
- **Blog:** Astro content collection; launch post is the first entry.
- (Optional v1.1) **Docs**: install + first-run + provider setup.

### B3. Assets (shared with the release)
- App icon / logo, 2–4 product screenshots (provider settings, a chart artifact,
  a permission prompt, the chat), one short demo GIF/MP4, and a 1200×630 **OG
  image** for link previews.

---

## Workstream C — Announcement

### C1. Blog post (lives at `zanto-site` /blog)
- Arc: the itch → what zanto is → the differentiators (own your stack, 10+
  providers, consented tools, artifacts) → a 60-second demo (GIF) → **download +
  install caveats (unsigned)** → roadmap (signing, auto-update, more apps) →
  call to try it / star the repo.
- Include OG image + screenshots; link prominently to Download.

### C2. LinkedIn post
- Short hook (the problem), 3–4 punchy feature bullets, demo GIF, link to the
  blog/site. Soft CTA ("kicking the tires welcome; it's unsigned/early").
- (Optional fast-follows: Show HN, r/LocalLLaMA, r/rust — note for later.)

---

## Sequencing

1. **A1–A2** version bump + release workflow → cut a **pre-release** `v1.0.0-rc.1`
   tag; verify all three installers download and run on a clean machine
   (esp. unsigned macOS/Windows flows).
2. **A3–A4** README, LICENSE, install docs, CHANGELOG.
3. **B1–B3** Astro site with Download wired to releases; deploy to Pages.
4. Capture screenshots/GIF/OG image (needs working builds from step 1).
5. **C1** write the blog post; publish site.
6. Cut final **`v1.0.0`** release (flip repo public if not already).
7. **C2** LinkedIn post linking to the live site/blog.

## Launch-day checklist

- [ ] `v1.0.0` Release public, all 3 installers attached + checksums
- [ ] Each installer installs & launches on a clean OS (unsigned flow documented)
- [ ] README renders with working download links
- [ ] LICENSE present; repo public
- [ ] Site live on Pages; Download buttons hit the right assets; OG preview looks right
- [ ] Blog post published
- [ ] LinkedIn post live with demo GIF + link

## Risks / notes

- **Unsigned friction.** macOS Gatekeeper and Windows SmartScreen will scare some
  users; mitigate with clear, screenshot-backed install steps and an honest
  "early/unsigned" framing. Signing + notarization is the top post-launch item.
- **No auto-update in v1.** Users re-download to upgrade; Tauri's updater is a
  fast-follow once a signing story exists.
- **Two repos.** App (`zanto-rust`) and site (`zanto-site`) — keep download links
  pointing at the app repo's releases so the site never goes stale.
- **First-run UX.** A new user with no key and no Ollama hits a wall — ensure the
  site/README quickstart makes "set a key or install Ollama" obvious, and the app
  shows a clear empty-state.
