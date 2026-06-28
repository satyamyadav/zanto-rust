# Linux .tar.gz artifact + one-line installer, replacing the AppImage

- **Date:** 2026-06-29

## Summary

Replace the AppImage — which hard-aborts on rolling-release distros because its
bundled graphics stack (libepoxy) skews against the system Mesa/EGL — with a plain
`.tar.gz` (binary + .desktop + icons + a local install script) that links the
system WebKitGTK/EGL like the .deb/.rpm, plus a repo-root `install.sh` for a
one-line `curl … | bash` install that fetches the latest release tarball
automatically.

## Two install scripts (named to avoid confusion)

- **`install.sh` (repo root)** — the *network* installer. Run via
  `curl -fsSL https://raw.githubusercontent.com/satyamyadav/zanto-rust/main/install.sh | bash`.
  Detects arch, queries the GitHub releases API for the newest tag (including
  pre-releases), downloads that tag's `.tar.gz`, extracts it, and runs the
  in-tarball installer. Lives in git, not in the tarball.
- **`tarball install.sh`** — the *local* installer SHIPPED INSIDE the `.tar.gz`
  (written by the CI step). Copies the already-extracted files to `~/.local` (or
  `/usr` with `--system`) and checks for `webkit2gtk-4.1`. No network.

The repo-root installer downloads + extracts, then delegates to the tarball
installer for the actual file copy — single source of truth for the install
logic.

## Motivation

Investigation (beta.1, on Arch + AwesomeWM + Mesa 26) found the AppImage aborts
with `Could not create default EGL display: EGL_BAD_PARAMETER` before rendering —
verified caused by the AppImage's **bundled libepoxy/graphics libs being
incompatible with the system Mesa libEGL** (the binary doesn't bundle libEGL but
does bundle the GL-dispatch layer). Proven decisively:
- A bare EGL probe on the same machine returns `EGL OK 1.5 vendor=Mesa Project`
  (system EGL is fine for normal apps).
- The **.deb's `zanto-desktop` binary** links `/usr/lib/libwebkit2gtk-4.1.so.0`,
  system `libepoxy`, system `libEGL.so.1` — and runs **without** the EGL abort.
- The .deb is 14.5 MB (5 files, system libs) vs the 87 MB AppImage (bundled
  stack). The 5 files: `usr/bin/zanto-desktop`, `usr/share/applications/
  zanto.desktop`, three `usr/share/icons/hicolor/*/apps/zanto-desktop.png`.

The official Tauri AppImage docs (v2.tauri.app/distribute/appimage) confirm there
is **no supported way to exclude the bundled graphics libs** — the only knob is
`bundleMediaFramework` (gstreamer), and the docs themselves warn that bundling
"frequently breaks compatibility," recommending an older build base (a
glibc-vs-newer-libs tradeoff, not a fix). So the AppImage is structurally a poor
fit for rolling distros, and a system-libs tarball is the correct portable Linux
artifact. The .deb (Ubuntu/Debian) and .rpm (Fedora) already cover their distros;
the tarball covers Arch + everything else.

## Scope

**In scope** (`.github/workflows/release.yml` only)
- **Remove the AppImage** entirely: the build-job upload glob, the publish-job
  `find` filter, the rename `*.appimage` case, and all AppImage lines in the
  release notes.
- **Add a `.tar.gz` build step** in the publish job, after "Stage installers"
  (where the renamed `.deb` is already in `release-files/`): extract the staged
  `.deb`, take its 5 payload files, add `install.sh` + `README.txt`, and tar to
  `release-files/zanto-<version>-Linux-x86_64.tar.gz`.
- **Rewrite the Linux release notes** to list the one-line installer first, then
  `.deb` (Ubuntu/Debian), `.rpm` (Fedora), `.tar.gz` (Arch / portable).
- **Add a repo-root `install.sh`** (the network installer) — a new file in git.

**Out of scope**
- Any app/source/`tauri.conf.json` change. Workflow-only.
- An AUR `PKGBUILD` (the proper native-Arch path) — separate, likely a
  packaging/zanto-site handoff.
- Changing the build base (.deb/.rpm/tarball stay built on ubuntu-24.04; glibc
  ≥ 2.39 floor accepted, as for the .deb today).
- macOS/Windows notes (unchanged except the AppImage line removal touches only the
  Linux block).

## Affected files

- `.github/workflows/release.yml` — remove AppImage (4 spots), add the tarball
  build step, rewrite the Linux notes.
- `install.sh` (repo root) — **new file**: the one-line network installer.

## Implementation steps

1. **Remove AppImage from the build-job upload glob**
   (`release.yml` "Upload installers", ~line 112)
   - Delete the line `target/**/release/bundle/**/*.AppImage`. (The Linux build
     still produces .deb + .rpm; the AppImage is simply not uploaded. Tauri still
     *builds* it via `targets: "all"`, but it's dropped here — acceptable; the
     publish job won't see it. A later cleanup could set explicit targets, out of
     scope.)

2. **Remove AppImage from the publish-job `find` filter**
   (`release.yml` "Stage installers", ~line 175-177)
   - Remove `-o -name '*.AppImage'` from the `find` predicate so no AppImage is
     staged even if one is downloaded.

3. **Remove the AppImage rename case**
   (`release.yml` "Stage installers" `case` block, ~line 165)
   - Delete `*.appimage) dest="zanto-$VERSION-Linux-x86_64.AppImage" ;;`.

4. **Add the tarball build step** (`release.yml`, new step immediately AFTER
   "Stage installers", BEFORE "Create draft GitHub Release")
   - New step `- name: Build portable Linux tarball`, runs on the
     ubuntu-latest publish runner (has `ar`, `tar`; install `binutils`/`zstd` if
     the .deb payload is zstd — Tauri's .deb uses gzip `data.tar.gz`, verified, so
     `tar` suffices). Logic:
     ```bash
     set -euo pipefail
     DEB=$(ls release-files/zanto-*-Linux-x86_64.deb 2>/dev/null || true)
     if [ -z "$DEB" ]; then echo "no .deb to repack into a tarball; skipping"; exit 0; fi
     work="$(mktemp -d)"
     ar x "$DEB" --output="$work"            # → control.tar.gz data.tar.gz debian-binary
     mkdir -p "$work/payload"
     tar -xzf "$work/data.tar.gz" -C "$work/payload"
     # Stage the portable tree: the 5 files under a versioned dir
     pkg="zanto-$VERSION-Linux-x86_64"
     root="$work/$pkg"
     mkdir -p "$root"
     cp -r "$work/payload/usr" "$root/usr"   # usr/bin/zanto-desktop + .desktop + icons
     cp release-files/install.sh "$root/install.sh"  # written by the next sub-step OR heredoc here
     cp release-files/README.txt "$root/README.txt"
     chmod +x "$root/install.sh" "$root/usr/bin/zanto-desktop"
     tar -czf "release-files/$pkg.tar.gz" -C "$work" "$pkg"
     ls -la release-files/
     ```
   - The `install.sh` and `README.txt` are written as heredocs within this same
     step (so no extra repo files). Content below (steps 5–6 describe them; in the
     YAML they are `cat > "$root/install.sh" <<'EOF' … EOF` before the final tar).
   - VERSION comes from `steps.meta.outputs.version` (add it to this step's `env:`
     like the staging step).

5. **`install.sh` content** (heredoc inside step 4)
   - A POSIX `sh`/bash script, default install to `~/.local` (no sudo),
     `--system` flag for `/usr` (with sudo):
     ```sh
     #!/usr/bin/env bash
     set -euo pipefail
     PREFIX="$HOME/.local"; SUDO=""
     [ "${1:-}" = "--system" ] && { PREFIX="/usr"; SUDO="sudo"; }
     here="$(cd "$(dirname "$0")" && pwd)"
     # webkit2gtk-4.1 presence check (runtime dep we do NOT bundle)
     if ! ldconfig -p 2>/dev/null | grep -q "libwebkit2gtk-4.1.so.0"; then
       echo "WARNING: libwebkit2gtk-4.1 not found. Install it first:"
       echo "  Arch:   sudo pacman -S webkit2gtk-4.1"
       echo "  Fedora: sudo dnf install webkit2gtk4.1"
       echo "  Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-0"
       echo "Continuing anyway — the app will fail to start until it is installed."
     fi
     $SUDO install -Dm755 "$here/usr/bin/zanto-desktop" "$PREFIX/bin/zanto-desktop"
     $SUDO install -Dm644 "$here/usr/share/applications/zanto.desktop" "$PREFIX/share/applications/zanto.desktop"
     for png in "$here"/usr/share/icons/hicolor/*/apps/zanto-desktop.png; do
       rel="${png#"$here"/usr/share/}"
       $SUDO install -Dm644 "$png" "$PREFIX/share/$rel"
     done
     echo "Installed to $PREFIX. Launch: zanto-desktop  (ensure $PREFIX/bin is on PATH)"
     ```
   - Mirror exact escaping rules for the YAML heredoc; use `<<'EOF'` (single-quoted
     delimiter) so `$HOME`/`$1` are NOT expanded at YAML/CI time — they must reach
     the user's script literally.

6. **`README.txt` content** (heredoc inside step 4)
   - Short: what this is, the dependency, install:
     ```
     zanto — portable Linux build (uses your system libraries)

     Requires: webkit2gtk-4.1 (GTK3 WebKit) and glibc >= 2.39 already installed.
       Arch:   sudo pacman -S webkit2gtk-4.1
       Fedora: sudo dnf install webkit2gtk4.1
       Debian/Ubuntu 24.04+: sudo apt install libwebkit2gtk-4.1-0

     Install:  ./install.sh            (to ~/.local, no sudo)
               ./install.sh --system   (to /usr, needs sudo)
     Or run in place:  ./usr/bin/zanto-desktop
     ```

7. **Rewrite the Linux release notes**
   (`release.yml` "Create draft GitHub Release" `--notes`, the Linux block,
   ~lines 193-196)
   - Replace the AppImage lines. New Linux block (keep the existing
     escaped-backtick style):
     - "One-line install (any distro with webkit2gtk-4.1):
       `curl -fsSL https://raw.githubusercontent.com/satyamyadav/zanto-rust/main/install.sh | bash`"
     - "Ubuntu/Debian: `sudo dpkg -i zanto-*.deb`"
     - "Fedora: `sudo rpm -i zanto-*.rpm`"
     - "Arch / other (manual): extract `zanto-*-Linux-x86_64.tar.gz` and run
       `./install.sh` (needs `webkit2gtk-4.1`). Or run `./usr/bin/zanto-desktop`."
   - Remove both AppImage lines. macOS + Windows blocks unchanged.

8. **Add the repo-root `install.sh`** (the network installer — new git file)
   - A bash script that detects arch, finds the newest release tag (including
     pre-releases), downloads its `.tar.gz`, extracts to a temp dir, and runs the
     in-tarball `install.sh` (so install logic lives in one place). Passes through
     a `--system` flag. Uses only `curl`, `tar`, and either `python3`/`grep` to
     parse the GitHub API JSON (no `jq` dependency):
     ```bash
     #!/usr/bin/env bash
     set -euo pipefail
     REPO="satyamyadav/zanto-rust"
     # Only x86_64 is published today.
     arch="$(uname -m)"
     if [ "$arch" != "x86_64" ]; then
       echo "Unsupported arch: $arch (only x86_64 builds are published)."; exit 1
     fi
     echo "Finding the latest zanto release…"
     # /releases (all) — first entry is newest; include pre-releases. Pull the
     # browser_download_url of the Linux tarball without jq.
     api="https://api.github.com/repos/$REPO/releases"
     url="$(curl -fsSL "$api" \
       | grep -oE '"browser_download_url": *"[^"]*Linux-x86_64\.tar\.gz"' \
       | head -1 | sed -E 's/.*"(https[^"]+)"/\1/')"
     if [ -z "$url" ]; then
       echo "No Linux tarball found in the latest releases."; exit 1
     fi
     echo "Downloading $url"
     tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
     curl -fsSL "$url" -o "$tmp/zanto.tar.gz"
     tar -xzf "$tmp/zanto.tar.gz" -C "$tmp"
     dir="$(find "$tmp" -maxdepth 1 -type d -name 'zanto-*-Linux-x86_64' | head -1)"
     if [ -z "$dir" ] || [ ! -x "$dir/install.sh" ]; then
       echo "Extracted tarball missing install.sh"; exit 1
     fi
     exec "$dir/install.sh" "$@"
     ```
   - NOTE the GitHub API is unauthenticated here (60 req/hr/IP — fine for an
     installer). The `grep`/`sed` JSON parse avoids a `jq` dependency; it keys on
     the stable `…Linux-x86_64.tar.gz` asset name this spec defines. The newest
     release is the first array element from `/releases` (GitHub returns them
     newest-first), so `head -1` on the matched URLs picks the latest tarball.
   - `chmod +x install.sh` in the repo (committed executable).

## Edge cases & risks

- **No new dependency.** Workflow-YAML + a repo-root bash script; `ar`/`tar` are on
  the ubuntu publish runner; the installer uses only `curl`/`tar`/`grep`/`sed`
  (no `jq`).
- **Installer parses GitHub JSON with grep/sed.** Brittle if GitHub changes the
  JSON shape, but `browser_download_url` is stable and the asset-name pattern
  (`*Linux-x86_64.tar.gz`) is defined by this spec. A draft release's assets are
  NOT in `/releases` until published — fine, the installer should only see
  published releases (the workflow creates drafts; the owner publishes them).
  **Risk:** if the newest published release has no tarball (e.g. an old release
  predating this change), `head -1` could match an OLDER release's tarball or
  none. Since this change introduces the tarball, the first release WITH a tarball
  is the newest going forward — acceptable. Documented.
- **curl | bash trust.** Standard for this install pattern (nvm, rustup). The
  script is short, readable in the repo, and does no privileged action without
  `--system` (default install is to `~/.local`, no sudo). Users can read it at the
  raw URL before piping. No mitigation needed beyond keeping it minimal.
- **Rate limit.** Unauthenticated GitHub API = 60 req/hr/IP. One install = one API
  call + one asset download. Fine.
- **`raw.githubusercontent.com/.../main/install.sh`** serves whatever is on `main`
  — so the installer must be committed to `main` (this change does that) and stays
  in sync with the asset-naming the workflow produces. If the asset name changes
  later, update both. Noted.
- **.deb payload compression.** Tauri's bundler writes `data.tar.gz` (gzip),
  confirmed by extracting beta.1's .deb (`data.tar.gz` present). If a future Tauri
  switches to `data.tar.xz`/`.zst`, the `tar -xzf` fails. Mitigation: the step can
  detect (`ls $work/data.tar.*`) and pick the right flag; the spec uses gzip per
  the verified current output, with a one-line guard noted in implementation.
- **No .deb → no tarball.** Step 4 guards (`if [ -z "$DEB" ]; … skipping`), so a
  missing .deb degrades to "no tarball" rather than failing the release.
- **webkit2gtk-4.1 not installed on the user's machine.** The tarball cannot
  auto-install it (unlike .deb/.rpm). `install.sh` warns + prints the per-distro
  command; the README states the dependency. This is the accepted tradeoff for a
  system-libs portable artifact — and still strictly better than the AppImage,
  which fails *with* the libs present.
- **glibc ≥ 2.39 floor** (24.04 build base) — same as the current .deb; excludes
  distros older than ~2 years. Accepted (owner decided earlier). Old distros use
  the .deb on their own glibc… (note: the .deb is also 24.04-built, so the floor is
  uniform across all Linux artifacts — consistent, not a regression).
- **install.sh PATH.** `~/.local/bin` may not be on the user's PATH; the script
  prints a reminder. Non-fatal.
- **AppImage still built but unused.** `targets: "all"` keeps producing it in the
  build job; it's just not uploaded. Harmless (wasted build time only). A later
  change can set explicit Linux targets to skip it — out of scope.

## Acceptance criteria

Workflow + release-notes change; no CLI. Verifiable by inspecting the workflow and
on the next tagged release (or a `workflow_dispatch` dry run):

- [ ] No AppImage anywhere in `release.yml` (no `*.AppImage` glob, no rename case,
      no `find` predicate, no notes line).
- [ ] A new "Build portable Linux tarball" step produces
      `release-files/zanto-<version>-Linux-x86_64.tar.gz` from the staged `.deb`.
- [ ] The tarball contains `usr/bin/zanto-desktop`, the `.desktop`, the 3 icons,
      `install.sh` (executable), and `README.txt`.
- [ ] `install.sh` installs to `~/.local` by default and `/usr` with `--system`,
      and warns when `libwebkit2gtk-4.1.so.0` is absent.
- [ ] The Linux release notes list the one-line installer + .deb / .rpm / .tar.gz
      and no AppImage.
- [ ] A repo-root `install.sh` exists (executable), and
      `curl … /main/install.sh | bash` (once on `main`) downloads the latest
      tarball, extracts it, and delegates to the in-tarball `install.sh`.
- [ ] The workflow still parses (`actionlint` / YAML load); the notes + the new
      step's bash + the repo-root `install.sh` are valid (`bash -n` clean).
- [ ] Only `.github/workflows/release.yml` + the new `install.sh` changed;
      `cargo build` unaffected.

## Manual test plan

No CLI behavior change. Verify statically + (optionally) a real dry run, and
locally reproduce the tarball-build logic against the already-downloaded beta.1
.deb to prove the packaging works on this machine:

1. `git diff --stat` → `.github/workflows/release.yml` + new `install.sh`.
2. YAML parse: `ruby -ryaml -e "YAML.load_file('.github/workflows/release.yml'); puts 'ok'"`
   → `ok`. Bash-check the new step + notes + repo-root installer:
   `bash -n install.sh` → clean; extract each workflow `run:` and `bash -n` it.
3. `grep -ni "appimage" .github/workflows/release.yml` → no matches.
4. `grep -n "tar.gz\|install.sh\|README\|raw.githubusercontent" .github/workflows/release.yml`
   → the new step + the notes (incl. the one-liner URL) reference them.
5. **Local repro of the tarball logic** (proves the CI step works) against the
   real beta.1 .deb already in the scratchpad:
   `ar x zanto-*-Linux-x86_64.deb && tar -xzf data.tar.gz -C payload && ls payload/usr/bin/zanto-desktop`
   → the binary extracts; then `payload/usr/bin/zanto-desktop` run on this Arch
   machine renders (no EGL abort) — the same binary the tarball ships.
6. **Repro the network installer's discovery** without piping to bash:
   `curl -fsSL https://api.github.com/repos/satyamyadav/zanto-rust/releases | grep -oE '"browser_download_url": *"[^"]*Linux-x86_64\.tar\.gz"' | head -1`
   → once a release with a tarball exists, prints the latest tarball URL (proves
   the grep/sed discovery logic). Before that release exists, prints nothing
   (expected).
7. `cargo build` → `Finished` (unchanged).
8. (Optional, real) `workflow_dispatch` dry run → draft `v0.0.0-dev` release with
   `zanto-0.0.0-dev-Linux-x86_64.{deb,rpm,tar.gz}` and NO AppImage. After
   publishing a release with the tarball, run
   `curl -fsSL https://raw.githubusercontent.com/satyamyadav/zanto-rust/main/install.sh | bash`
   on Arch → downloads, installs to `~/.local`, `zanto-desktop` launches +
   renders.
