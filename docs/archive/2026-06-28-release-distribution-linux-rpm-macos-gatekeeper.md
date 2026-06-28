# Release distribution: newer-WebKit Linux builds (.deb/.rpm/AppImage) + macOS Gatekeeper notes

- **Date:** 2026-06-28

## Summary

Build Linux on ubuntu-24.04 so the bundled WebKit is recent enough that the
AppImage actually renders (instead of a blank screen on modern distros), ship all
three Linux artifacts (.deb, .rpm, AppImage — currently the .rpm is silently
dropped), and correct the stale macOS Gatekeeper instructions — release-workflow +
notes only, no app code.

## Motivation

Investigation of two reported install failures:

**Linux — blank screen (AppImage).** The AppImage bundles WebKitGTK 4.1 built on
the **ubuntu-22.04** runner (~2.36-era WebKit; 89 MB, verified byte-identical to a
sibling project's AppImage). On a rolling distro (Arch, Mesa 26, WebKit 2.52) that
frozen old WebKit can't initialize EGL against the modern GPU stack →
`EGL_BAD_PARAMETER` → blank window. Two independent fixes apply:
- **Update the bundled WebKit** by building on **ubuntu-24.04** (ships
  webkit2gtk-4.1 ~2.44+), recent enough to init EGL on current Mesa/GPU stacks →
  the AppImage renders. This is the real AppImage fix.
- **Ship the system-WebKit packages too** (.deb/.rpm): their binary links the
  host's `/usr/lib/libwebkit2gtk-4.1.so.0` (verified via `ldd` on the sibling
  RPM's `tauri-app`; RPM is 10 MB vs 85 MB AppImage — the delta is bundled
  WebKit), so they work regardless of bundled-WebKit age. zanto already builds the
  `.rpm` (`bundle.targets: "all"`) but the workflow drops it in two places, so
  users never see it.

Goal: working installs across **Arch (AppImage), Ubuntu/Debian (.deb), Fedora
(.rpm)** + the portable AppImage for everything else.

**macOS — Gatekeeper "Not Opened" dialog.** Builds are ad-hoc signed
(`macOS.signingIdentity: "-"`), not notarized. The release notes tell users to
"right-click → Open" — but macOS 15 (Sequoia) **removed** that bypass for
ad-hoc/unsigned apps. The current path is System Settings → Privacy & Security →
"Open Anyway", or `xattr -dr com.apple.quarantine`. The notes are stale and leave
users stuck. (The sibling macOS build is configured identically — same dialog — so
this is purely an instructions fix; real notarization needs an Apple Developer
account we don't have → deferred.)

## Scope

**In scope** (`.github/workflows/release.yml` only)
- Bump the Linux build leg from `ubuntu-22.04` to `ubuntu-24.04` (newer bundled
  WebKit). Update both the matrix `platform` and the `if:` guard on the Linux
  deps step.
- Plumb the already-built Linux `.rpm` through: the artifact-upload glob, the
  final `find` filter, and a rename case (`zanto-<version>-Linux-x86_64.rpm`).
- Rewrite the GitHub Release `--notes` block:
  - Linux: list all three — `.deb` (Ubuntu/Debian), `.rpm` (Fedora), AppImage
    (Arch + portable). Keep a one-line AppImage fallback hint
    (`WEBKIT_DISABLE_DMABUF_RENDERER=1`) for edge cases.
  - macOS: replace the wrong "right-click → Open" line with the macOS-15-correct
    path (System Settings → Privacy & Security → Open Anyway) + the Terminal
    `xattr -dr com.apple.quarantine` alternative.
  - Windows: unchanged.

**Out of scope**
- macOS notarization. Owner has an Apple Developer account but **no paid
  membership** ($99/yr not set up). Notarization + a **Developer ID Application**
  certificate require the *active paid* membership — a free/unpaid account cannot
  get a Developer ID cert or submit to the notary service. So notarization is
  **deferred** until payment is active; this change only corrects the bypass
  instructions. (Follow-up when paid: Developer ID cert + app-specific password as
  GitHub secrets, `notarytool submit` + `stapler` staple steps in CI — its own
  spec.)
- Stripping/unbundling WebKit from the AppImage (rejected: reintroduces a host
  dependency without a package manager to satisfy it; the base-bump is the clean
  fix instead).
- Any source-code, `tauri.conf.json`, or app behavior change.
- Linux ARM, Windows MSI/store signing.

## Affected files

- `.github/workflows/release.yml` — base bump (2 lines), `.rpm` plumbing
  (3 spots), `--notes` rewrite. **Only file changed.**

## Implementation steps

1. **Bump the Linux runner to ubuntu-24.04** (`release.yml`, build matrix,
   ~line 42, and the deps-step guard, ~line 56)
   - In the matrix, change `- platform: ubuntu-22.04` → `ubuntu-24.04`
     (the Linux leg; keep `name: linux`, `args: ""`).
   - Change the deps step guard `if: matrix.platform == 'ubuntu-22.04'` →
     `'ubuntu-24.04'`. The apt packages are the same names on noble
     (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `librsvg2-dev`, `patchelf`,
     `libappindicator3-dev`, build deps). This makes the bundled WebKit ~2.44+.

2. **Add `.rpm` to the build-artifact upload glob** (`release.yml`, "Upload
   installers", ~lines 108-113)
   - Add `target/**/release/bundle/**/*.rpm` to the `path:` list. Without it the
     rpm produced by `targets: "all"` never leaves the build job.
     (`if-no-files-found: ignore` already set, so a missing rpm is harmless.)

3. **Add `.rpm` to the publish-job `find` filter** (`release.yml`, "Stage
   installers", ~lines 173-175)
   - The `find artifacts ... \( -name '*.dmg' -o -name '*.deb' ... \)` omits
     `*.rpm`. Add `-o -name '*.rpm'`.

4. **Add the `.rpm` rename case** (`release.yml`, the `case "$lower" in` block,
   ~lines 157-171)
   - Alongside `*.deb) dest="zanto-$VERSION-Linux-x86_64.deb" ;;` add:
     `*.rpm) dest="zanto-$VERSION-Linux-x86_64.rpm" ;;`.

5. **Rewrite the GitHub Release notes** (`release.yml`, "Create draft GitHub
   Release", the `--notes "..."` heredoc, ~lines 188-192)
   - New content (terse; preserve the existing bash-double-quoted + escaped-
     backtick style so YAML/bash stays valid):
     - Lead: "Ad-hoc signed builds (not notarized by Apple). Install notes:"
     - **Linux:** "Ubuntu/Debian: `sudo dpkg -i zanto-*.deb`. Fedora:
       `sudo rpm -i zanto-*.rpm`. Arch / others: the AppImage —
       `chmod +x zanto-*.AppImage && ./zanto-*.AppImage`. If the AppImage shows a
       blank window on an older distro, try
       `WEBKIT_DISABLE_DMABUF_RENDERER=1 ./zanto-*.AppImage` or use the .deb/.rpm."
     - **macOS:** "Unsigned by Apple. First launch is blocked: click **Done**,
       then **System Settings → Privacy & Security → scroll down → Open Anyway**.
       (Or Terminal: `xattr -dr com.apple.quarantine /Applications/zanto.app`.)
       The old right-click→Open trick no longer works on macOS 15."
     - **Windows:** "SmartScreen → **More info** → **Run anyway**."

## Edge cases & risks

- **No new dependency.** Workflow-YAML + release-notes only.
- **glibc floor.** An AppImage/`.deb` built on ubuntu-24.04 needs host glibc
  ≥ 2.39 (noble's). This excludes distros older than ~2 years (e.g. Ubuntu 22.04
  itself, RHEL 8). **Accepted** per owner — current distros (Arch 2.43, Fedora 40+,
  Ubuntu 24.04, Debian 13) all satisfy it; older users can build from source. This
  is the normal AppImage base-version tradeoff. If wide-compat .deb is later
  needed, split the Linux matrix (24.04 for AppImage, 22.04 for .deb) — flagged,
  not done here (owner chose the single newer base).
- **rpm actually produced?** `targets: "all"` emits `.rpm` via Tauri's bundler
  when `rpmbuild` is on the runner. If ubuntu-24.04 lacks it, no rpm is emitted →
  the rpm glob matches nothing (harmless via `if-no-files-found: ignore`); the
  release ships .deb + AppImage. If the rpm is missing after the first run, a
  follow-up adds `rpm` to the apt-install step. (The sibling project ships rpm
  from an ubuntu base, so the bundler+runner combo is expected to produce it.)
- **WebKit 2.44 vs app.** The app already runs against system WebKit 2.52 in dev
  on this machine all session, so a 2.44 bundled WebKit is well within range — no
  app-side compatibility concern.
- **Notes string validity.** The `--notes` value is a multi-line bash string in
  YAML; an unescaped backtick or unbalanced quote breaks `gh release create`. Keep
  backticks escaped exactly as the current notes do (verified by YAML parse — test
  plan).

## Acceptance criteria

CI-workflow + release-notes change; no CLI to run. Verifiable on the next tagged
release (or a `workflow_dispatch` dry run) and by inspecting the workflow:

- [ ] The Linux build leg runs on `ubuntu-24.04` (matrix + deps-step guard both
      updated; no remaining `ubuntu-22.04` reference for the Linux leg).
- [ ] `release.yml` uploads `*.rpm` from the build job (glob includes it).
- [ ] The publish job's `find` filter includes `*.rpm`.
- [ ] A `.rpm` artifact, when present, is renamed to
      `zanto-<version>-Linux-x86_64.rpm`.
- [ ] The draft Release notes list .deb (Ubuntu/Debian), .rpm (Fedora), and the
      AppImage (Arch/portable) with the blank-screen fallback hint; the macOS
      lines document Open Anyway + `xattr` and drop the dead right-click→Open
      instruction.
- [ ] The workflow still parses: `actionlint` (or `yaml.safe_load`) reports no
      error; the `gh release create` notes string is valid.
- [ ] Only `.github/workflows/release.yml` changed; `cargo build` unaffected.

## Manual test plan

No CLI behavior change — verification is static-check + (optionally) a dry release.
Exact commands and expected results:

1. `git diff --stat` after the edit → only `.github/workflows/release.yml`.
2. `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('yaml ok')"`
   → `yaml ok` (the notes-string edit didn't break YAML). If `actionlint` is
   installed, `actionlint .github/workflows/release.yml` → no errors.
3. `grep -n "ubuntu-24.04\|ubuntu-22.04" .github/workflows/release.yml` → the
   Linux leg shows `24.04`; any remaining `22.04` is only a non-Linux context (none
   expected).
4. `grep -n "rpm" .github/workflows/release.yml` → three new rpm references
   (upload glob, find `-name '*.rpm'`, rename case).
5. `grep -n "Open Anyway\|com.apple.quarantine\|right-click" .github/workflows/release.yml`
   → new macOS lines present; old "right-click → Open" gone.
6. `cargo build` → `Finished` (unchanged).
7. (Optional, real) `workflow_dispatch` dry run → draft `v0.0.0-dev` release with
   `zanto-0.0.0-dev-Linux-x86_64.{deb,rpm,AppImage}` assets; install the AppImage
   on this Arch machine → it renders (no blank screen), proving the WebKit bump.
