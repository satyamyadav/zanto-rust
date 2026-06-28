# macOS ad-hoc signing + zanto-site visuals

- **Date:** 2026-06-28

## Summary
Make macOS installs need no Terminal command (ad-hoc sign the bundle); upgrade the marketing site visuals — provider names as styled nominative-use text chips, per-feature line icons, and a GitHub **star** button in the nav — using the existing app palette.

## Motivation
- **Install friction (macOS):** zanto ships fully unsigned, so Gatekeeper quarantines the `.app` and the user must run `xattr -dr com.apple.quarantine …` in Terminal. Ad-hoc signing (`signingIdentity: "-"`, as orbit-cowork uses) removes the quarantine-command step — the user just right-clicks → **Open** once. No paid cert, no secrets, no CI change.
- **Site is text-only:** the provider strip is a row of plain words, feature cards have no visuals, and the nav only links to GitHub. Styling the provider names as chips, adding small accent-tinted line icons per feature, and turning the nav GitHub link into a **star** call-to-action raises perceived quality (ref: tailawesome 11ty landing page) while keeping the dark theme and the existing palette (`--color-accent` violet `#7c6cff`, hue 278 — matches the app's `oklch(… 278)` primary; `--color-accent-2` `#4ad6c4`).
- **Provider logos — deliberately NOT used.** Research: simple-icons SVGs are CC0 for the artwork, but the project's DISCLAIMER and trademark law are explicit that CC0 does **not** grant trademark/logo rights, and providers (OpenAI, Anthropic, Google, etc.) require permission to use their marks and prohibit implying endorsement. A 10-logo hero row implies partnership none of them have granted. Stating compatibility with the providers' **names as text** is nominative fair use and needs no permission — so the strip uses text chips, not logos.

## Scope

**In scope**
1. `tauri.conf.json`: add `bundle.macOS.signingIdentity: "-"` (ad-hoc). Keep `productName: "zanto"` (lowercase, intentional brand) and `bundle.targets: "all"` unchanged.
2. Update the macOS install note copy in three places to reflect "right-click → Open, no command": `release.yml` `releaseBody`, README install table + note, `zanto-site/src/pages/download.astro`.
3. `zanto-site`:
   - Provider strip on `index.astro`: style the existing provider **names** as bordered chips (nominative use). No logos, no `public/logos/`.
   - Feature cards on `index.astro`: add per-feature inline line-SVG icons.
   - Nav (`Header.astro`): replace the plain "GitHub" text link with a **star button** ("★ Star" + label) linking to the repo.
   - `download.astro`: macOS card copy (no command) + swap blank platform icons for inline SVGs.

**Out of scope**
- Real Developer-ID signing + notarization (paid Apple account) — stays Gate 1 in the validation framework.
- Windows code signing / SmartScreen removal.
- Renaming the product to title-case "Zanto".
- Changing `bundle.targets`, identifier, icons, or any Rust code.
- **Provider brand logos** — excluded for trademark reasons (see Motivation); names rendered as text only.
- A live star **count** (would need the GitHub API / a fetch at build time). The button is a static "Star on GitHub" CTA, not a counter.
- Committing/pushing either repo (left to the user; zanto-site is a separate repo).

## Affected files

**Code repo (`local-work`)**
- `crates/zanto-desktop/src-tauri/tauri.conf.json` — add `bundle.macOS.signingIdentity: "-"`.
- `.github/workflows/release.yml` — reword the macOS line in `releaseBody`.
- `README.md` — reword the macOS install row + the "early, unsigned release" note.

**Site repo (`/home/lazy/dev/github/zanto-site`)**
- `src/pages/index.astro` — provider strip renders styled text chips; feature cards gain inline line-SVG icons.
- `src/components/Header.astro` — nav "GitHub" link becomes a star button.
- `src/pages/download.astro` — macOS card: drop the `xattr` command, keep right-click→Open; replace blank macOS icon.

## Implementation steps

1. **Ad-hoc sign the macOS bundle** (`crates/zanto-desktop/src-tauri/tauri.conf.json`)
   - In the `bundle` object add:
     ```json
     "macOS": { "signingIdentity": "-" }
     ```
   - Leave `productName`, `targets: "all"`, `identifier`, `icon` untouched. Ad-hoc signing makes Gatekeeper accept right-click→Open without the quarantine xattr command.

2. **Reword macOS release note** (`.github/workflows/release.yml`)
   - In `releaseBody`, replace the macOS bullet:
     - From: `- macOS: right-click the app → Open the first time, or run \`xattr -dr com.apple.quarantine /Applications/zanto.app\`.`
     - To: `- macOS: right-click zanto.app → **Open** → **Open** the first time (ad-hoc signed; no Terminal command needed).`
   - Leave Windows/Linux bullets unchanged.

3. **Reword README install row + note** (`README.md`)
   - macOS table row (line ~43): change the note cell to: `Right-click the app → **Open** → **Open** (ad-hoc signed — no Terminal command).` Drop the `xattr` command.
   - The "early, unsigned release" blockquote (lines ~14-15): keep, but clarify it's "ad-hoc signed, not yet notarized" so the macOS prompt is a one-time right-click→Open (not a Terminal command). Windows note unchanged.
   - Roadmap line (~99) unchanged (real signing is still future).

4. **Update download page macOS card** (`zanto-site/src/pages/download.astro`)
   - In the `platforms` array, macOS entry: set `note` to `"Right-click the app → Open → Open the first time (ad-hoc signed — no Terminal needed)."` and `cmd: null`.
   - Replace the empty `icon: ""` for macOS with an inline SVG (Apple-style mark or a generic monitor) — to avoid licensing concerns use a simple monochrome `monitor`/`apple` line glyph rendered the same way as the new feature icons (currentColor). Windows/Linux `icon` may stay as the existing unicode for this change, OR be swapped to matching inline SVGs for consistency (do all three for visual parity).
   - Update the "Why the security prompts?" paragraph: macOS is now ad-hoc signed (one-time right-click→Open); Windows SmartScreen still warns. Notarization remains the post-launch item.

5. **Style the provider strip as text chips** (`zanto-site/src/pages/index.astro`)
   - Keep the existing `providers: string[]` array (names only — no logos). Add a small "Works with" eyebrow label above the strip.
   - Render each name as a bordered chip: `class="rounded-full border border-line bg-ink-2/60 px-3 py-1 text-sm text-white/55 transition-colors hover:border-accent/40 hover:text-white/80"`.
   - This is nominative use (stating compatibility) — no trademark/logo issue. No new assets, no `href()` needed (text only).

6. **Add the GitHub star button to nav** (`zanto-site/src/components/Header.astro`)
   - Replace the plain `<a … href={SITE.repo}>GitHub</a>` (line 14) with a star CTA: a bordered button containing an inline star SVG + the text "Star", linking to `SITE.repo` (`target="_blank" rel="noopener"`).
     - Markup: `<a class="flex items-center gap-1.5 rounded-lg border border-line px-3 py-1.5 text-sm text-white/80 hover:border-accent/40 hover:text-white" href={SITE.repo} target="_blank" rel="noopener"><svg …star…/>Star</a>`.
     - Star icon: inline 16×16 SVG, `fill="currentColor"` (filled star) so it reads as a GitHub-star affordance.
   - Keep the Features, Blog, and Download nav items unchanged. The Download button stays the primary accent CTA; the Star button is secondary (bordered).
   - This is a static CTA — no live star count (see Out of scope).

7. **Add per-feature line icons** (`zanto-site/src/pages/index.astro`)
   - Add an `icon` field (inline SVG string or a small component) to each of the 7 `features` entries, mapped by theme:
     - "You own the stack" → key
     - "10+ providers, one app" → shuffle/swap
     - "Real tools, with consent" → shield-check
     - "Artifacts, not just chat" → bar-chart
     - "Sessions that survive" → database
     - "Apps on the same engine" → grid/layers
     - "Also a CLI" → terminal
   - Render the icon above the card title: a ~40px rounded tile `bg-accent/10 border border-accent/20` containing the `stroke="currentColor"` line SVG in `text-accent`. Keep card markup otherwise as-is.
   - Implement icons as inline SVG (1.5 stroke, 24×24 viewBox, `fill=none stroke=currentColor`) — no icon-library dependency.

8. **Build-verify the site** (no file change)
   - `cd /home/lazy/dev/github/zanto-site && pnpm build` must pass; confirm `dist/index.html` contains the feature-card `<svg>`s, the chip-styled provider strip, and the nav star button.

## Edge cases & risks
- **No new crate/npm dependency.** Feature/star icons are inline SVG; provider chips are text. No icon library, no asset files.
- **Trademark — handled by design:** provider brand **logos are deliberately excluded** (research confirmed CC0 ≠ trademark grant; providers require permission and prohibit implied endorsement). Provider **names as text** are nominative fair use and need no permission. A "Works with" eyebrow (not "Partners"/"Powered by") avoids implying a partnership. This is the conservative, ship-safe choice.
- **Ad-hoc signing is not notarization.** macOS will still show a one-time "unidentified developer" prompt; right-click→Open bypasses it without Terminal. On some macOS versions a quarantined download may still need the right-click path — copy says "first time", which is accurate. It does NOT eliminate the prompt entirely (only a paid Developer ID + notarization does).
- **`signingIdentity: "-"` on CI:** ad-hoc signing needs no secrets and works on the `macos-latest` runner; `tauri-action` honors `bundle.macOS.signingIdentity`. No workflow logic change required — only the release-note text.
- **Inline SVG verbosity:** 7 feature icons + 1 star + 3 platform icons add inline markup to `index.astro`/`download.astro`/`Header.astro`. Acceptable; keeps zero runtime deps and full theme control. If it bloats the file, icons may be factored into a tiny `Icon.astro` — optional, not required.
- **No live star count:** the nav button is a static CTA; it does not show the current star number (would require a build-time GitHub API call). Out of scope by decision.

## Acceptance criteria
- [x] `cat crates/zanto-desktop/src-tauri/tauri.conf.json` shows `bundle.macOS.signingIdentity: "-"`; `productName` still `"zanto"`; `targets` still `"all"`.
- [x] `grep -i xattr .github/workflows/release.yml README.md` (code repo) and `zanto-site/src/pages/download.astro` return **no matches** — the Terminal command is gone from all three.
- [x] macOS install copy in all three files reads "right-click → Open" with no command block.
- [x] No `public/logos/` directory is created; no provider brand logo SVGs are added.
- [x] `cd /home/lazy/dev/github/zanto-site && pnpm build` exits 0.
- [x] `dist/index.html` provider strip renders the names as bordered chips (with a "Works with" eyebrow), and each of the 7 feature cards has an inline accent-tinted `<svg>` icon.
- [x] `dist/` nav (any built page, e.g. `dist/index.html`) shows a **Star** button (inline star `<svg>` + "Star") linking to the repo, in place of the plain "GitHub" text link.
- [x] Visual check via sampled render: provider chips, feature-icon tiles, and the nav star button are visible; dark theme + violet accent intact.

## Manual test plan
This change is desktop-packaging + static-site; the CLI is unaffected. Verification is build + asset + visual, not `cargo run`.

1. **Tauri config valid:**
   `cargo build -p zanto-desktop` (code repo) → compiles; confirms the JSON edit didn't break the bundle config parse.
   _(Full `.dmg`/ad-hoc-sign output is only produced by `pnpm tauri build` on macOS — note in PR that the runtime check happens on the next tagged release via `release.yml`; cannot be exercised on Linux dev box.)_
2. **No stray command in notes:**
   `grep -rn "xattr" README.md .github/workflows/release.yml /home/lazy/dev/github/zanto-site/src/pages/download.astro` → no output.
3. **Site builds:**
   `cd /home/lazy/dev/github/zanto-site && pnpm build` → "Complete!"; confirm no `dist/logos/` was produced (`ls dist/logos 2>/dev/null` → not found).
4. **Markup present:**
   `grep -c "<svg" dist/index.html` ≥ `8` (7 feature icons + nav star, plus the existing wordmark); `grep -ci "works with" dist/index.html` → ≥ `1`; `grep -o ">Star<" dist/index.html` present in nav.
5. **Visual spot-check:**
   Render `dist/index.html` (or `pnpm preview`) and screenshot the hero/provider strip + feature grid + nav; confirm provider **chips** (not logos), accent-violet feature-icon tiles, and the nav Star button on the dark theme.
