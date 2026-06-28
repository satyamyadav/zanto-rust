# Release: split-arch matrix + human-readable package names

- **Date:** 2026-06-28

## Summary
Restructure `.github/workflows/release.yml` into a build→publish pipeline that builds macOS per-arch (two single-arch DMGs) and Linux/Windows, then renames the bundles to human-readable `zanto-<version>-<os>-<arch>` files and attaches them to a draft GitHub Release whose **version, title, and pre-release flag all derive from the pushed tag**.

## Motivation
- **The tag is not read (bug).** The current workflow sets `releaseName: "zanto __VERSION__"`, and `__VERSION__` is a tauri-action placeholder interpolated from `tauri.conf.json`'s `version` field (`1.0.0`) — **not the tag**. So pushing tag `v1.0.0-beta.0` produces a draft titled `zanto 1.0.0` (observed): the prerelease suffix is dropped and the release is not flagged as a pre-release. The fix is to derive the version/title from `GITHUB_REF_NAME` and pass `--prerelease` when the tag has a pre-release suffix.
- **Package names:** Tauri emits machine-style filenames (`zanto_1.0.0_aarch64.dmg`, `zanto_1.0.0_amd64.deb`, `zanto_1.0.0_x64-setup.exe`). Orbit-cowork ships human-readable names (`Orbit-Cowork-<ver>-macOS-Apple-Silicon.dmg`). We want the same readability for zanto downloads.
- **Matrix:** Orbit builds macOS as two single-arch rows (`aarch64-apple-darwin`, `x86_64-apple-darwin`) producing two named DMGs (Apple-Silicon / Intel), instead of one fat "universal" DMG. We're matching that so download names tell the user which Mac they need.
- **Why a restructure is required:** `tauri-action` attaches the *raw-named* bundles to the release when given a `tagName`. To insert a rename step, the build must instead **upload artifacts** (no `tagName`), and a separate **publish** job downloads, renames, and creates the release with `gh release create`. This is orbit's exact shape, ported onto zanto's existing **tag-push trigger** (we are *not* adopting orbit's `release:`-commit / version-bump-back machinery).

## Scope

**In scope** — `.github/workflows/release.yml` only:
1. Build matrix: `macos-latest` ×2 (aarch64 + x86_64), `ubuntu-22.04`, `windows-latest`; `fail-fast: true`.
2. `build` job: build per row, `tauri-action` with **no `tagName`** (build-only), then `upload-artifact` the bundles.
3. `publish` job: `download-artifact`, **rename** to `zanto-<version>-<os>-<arch>` stems, `gh release create --draft` with zanto's existing install notes.
4. Version source: pushed tag with leading `v` stripped (`v1.2.3` → `1.2.3`); `workflow_dispatch` dry run → `0.0.0-dev`.

**Out of scope**
- Orbit's `release: <bump>` commit trigger, `scripts/bump-version.mjs`, and the commit-bump-back-and-tag publish step. Zanto keeps **push-tag** triggering; the tag is authoritative for the version. No file in the repo is version-bumped by CI.
- Changing `tauri.conf.json` (ad-hoc macOS signing already present; `targets: "all"` unchanged).
- Any non-`.github/workflows/release.yml` file.
- RPM in the release assets: Tauri *does* emit `.rpm` under `targets: "all"`, but orbit's rename only surfaces dmg/appimage/deb/exe/msi. **Decision:** do not publish `.rpm` (omit from the rename map and the asset upload) to keep the asset list tidy — matches the download page (`.AppImage` / `.deb` only). The `.rpm` is still built but not attached.

## Affected files
- `.github/workflows/release.yml` — full rewrite into build→publish with rename.

## Background facts (verified)
- `crates/zanto-desktop/src-tauri/tauri.conf.json`: `productName: "zanto"`, `version: "1.0.0"`, `bundle.targets: "all"`, `bundle.macOS.signingIdentity: "-"` (ad-hoc).
- Cargo **workspace target is at the repo root** (`./target`), no `.cargo/config` override. So bundles land under `target/release/bundle/**` for the native-arch build and `target/<triple>/release/bundle/**` for the cross-targeted macOS builds. The artifact glob must be repo-root-relative (orbit's `src-tauri/target/...` path is wrong for zanto and must NOT be copied verbatim).
- `targets: "all"` on each OS emits: macOS `.app` + `.dmg`; Linux `.deb` + `.rpm` + `.AppImage`; Windows `.msi` (WiX) + NSIS `setup.exe`.

## Implementation steps

1. **Header, trigger, permissions** (`.github/workflows/release.yml`)
   - Keep `name: release`.
   - Keep trigger: `on: push: tags: ["v*"]` and `workflow_dispatch:`.
   - Keep `permissions: contents: write`.
   - Update the top comment to describe the new build→publish + rename flow.

2. **`build` job — matrix** (`.github/workflows/release.yml`)
   - `strategy.fail-fast: true`, `strategy.matrix.include`:
     ```yaml
     - platform: macos-latest
       args: "--target aarch64-apple-darwin"
       rust-targets: "aarch64-apple-darwin"
       name: macos-arm64
     - platform: macos-latest
       args: "--target x86_64-apple-darwin"
       rust-targets: "x86_64-apple-darwin"
       name: macos-x64
     - platform: ubuntu-22.04
       args: ""
       rust-targets: ""
       name: linux
     - platform: windows-latest
       args: ""
       rust-targets: ""
       name: windows
     ```
   - `runs-on: ${{ matrix.platform }}`.

3. **`build` job — setup steps** (unchanged from current, kept in order)
   - `actions/checkout@v4`.
   - Linux deps step (`if: matrix.platform == 'ubuntu-22.04'`) — keep the existing apt list.
   - `pnpm/action-setup@v4` (version 9).
   - `actions/setup-node@v4` node 20, `cache: pnpm`, `cache-dependency-path: crates/zanto-desktop/pnpm-lock.yaml`.
   - `dtolnay/rust-toolchain@stable` with `targets: ${{ matrix.rust-targets }}`.
   - `Swatinem/rust-cache@v2` with `workspaces: ". -> target"`.
   - Install frontend deps: `working-directory: crates/zanto-desktop`, `pnpm install --frozen-lockfile`.

4. **`build` job — build-only (no release)** (`.github/workflows/release.yml`)
   - `tauri-apps/tauri-action@v0` with:
     - `env: GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}`
     - `with: projectPath: crates/zanto-desktop`, `args: ${{ matrix.args }}`
     - **No `tagName` / `releaseName` / `releaseDraft` / `releaseBody`** — omitting `tagName` makes tauri-action build only and not create/upload a release.

5. **`build` job — upload artifacts** (`.github/workflows/release.yml`)
   - `actions/upload-artifact@v4`:
     ```yaml
     name: bundle-${{ matrix.name }}
     if-no-files-found: ignore
     path: |
       target/**/release/bundle/**/*.dmg
       target/**/release/bundle/**/*.deb
       target/**/release/bundle/**/*.AppImage
       target/**/release/bundle/**/*.msi
       target/**/release/bundle/**/*.exe
     ```
   - Note: repo-root `target/**` (NOT `src-tauri/target/**`). `.rpm` intentionally omitted (out-of-scope decision).

6. **`publish` job — guard + version + prerelease flag** (`.github/workflows/release.yml`)
   - `needs: build`, `runs-on: ubuntu-latest`.
   - Compute version + tag + prerelease from context in a step that sets outputs:
     - If `github.ref_type == 'tag'`: `TAG="$GITHUB_REF_NAME"`, `VERSION="${GITHUB_REF_NAME#v}"`.
     - Else (`workflow_dispatch`): `TAG="v0.0.0-dev"`, `VERSION="0.0.0-dev"`.
     - **Prerelease detection:** if `$VERSION` contains a `-` (e.g. `1.0.0-beta.0`, `0.0.0-dev`) → `prerelease=true`, else `prerelease=false`. This is what fixes the observed `v1.0.0-beta.0` → `zanto 1.0.0` bug: the suffix is preserved in the title *and* the release is flagged pre-release.
   - Expose `version`, `tag`, and `prerelease` as step outputs.
   - Implementation sketch:
     ```bash
     if [ "${{ github.ref_type }}" = "tag" ]; then
       TAG="$GITHUB_REF_NAME"; VERSION="${GITHUB_REF_NAME#v}"
     else
       TAG="v0.0.0-dev"; VERSION="0.0.0-dev"
     fi
     case "$VERSION" in *-*) PRE=true ;; *) PRE=false ;; esac
     { echo "tag=$TAG"; echo "version=$VERSION"; echo "prerelease=$PRE"; } >> "$GITHUB_OUTPUT"
     ```

7. **`publish` job — download artifacts** (`.github/workflows/release.yml`)
   - `actions/download-artifact@v4` with `path: artifacts` (downloads all `bundle-*` into `artifacts/<name>/...`).

8. **`publish` job — rename to human-readable names** (`.github/workflows/release.yml`)
   - A bash step (adapted from orbit's "Stage installers" step) that finds the bundles and copies them into `release-files/` with `zanto-<VERSION>-<os>-<arch>` stems:
     - `*.dmg` containing `aarch64`/`arm64` → `zanto-$VERSION-macOS-Apple-Silicon.dmg`
     - other `*.dmg` → `zanto-$VERSION-macOS-Intel.dmg`
     - `*.appimage` → `zanto-$VERSION-Linux-x86_64.AppImage`
     - `*.deb` → `zanto-$VERSION-Linux-x86_64.deb`
     - `*setup.exe` → `zanto-$VERSION-Windows-x64-setup.exe`
     - `*.msi` → `zanto-$VERSION-Windows-x64.msi`
     - any other `*.exe` (non-setup) → skip (NSIS `setup.exe` is the installer; a bare `.exe` is the raw binary we don't ship)
   - Match case-insensitively (lowercase the basename for the `case`), `mkdir -p release-files`, `cp` each match, `ls -la release-files` for the log.
   - `VERSION` comes from step 6's output via env.

9. **`publish` job — create draft release** (`.github/workflows/release.yml`)
   - Build the command from step 6's outputs:
     ```bash
     gh release create "$TAG" \
       --draft \
       ${PRERELEASE:+--prerelease} \
       --title "zanto $TAG" \
       --notes "<install notes>" \
       release-files/*
     ```
     where `PRERELEASE` is set to a non-empty value iff `steps.<id>.outputs.prerelease == 'true'` (e.g. `env: PRERELEASE: ${{ steps.meta.outputs.prerelease == 'true' && '1' || '' }}`), so `--prerelease` is added only for `-alpha`/`-beta`/`-dev` tags.
   - Title is `zanto $TAG` (e.g. `zanto v1.0.0-beta.0`) — derived from the **tag**, not `tauri.conf.json`. This is the core fix.
   - `env: GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}`, plus `TAG` and `PRERELEASE`.
   - `--notes` carries zanto's existing install copy (ad-hoc mac, no xattr), with the Linux glob updated to the **renamed** asset (`zanto-*.AppImage`, hyphen not underscore):
     > Unsigned/ad-hoc builds — installation notes:
     > - macOS: right-click zanto.app → **Open** → **Open** the first time (ad-hoc signed; no Terminal command needed).
     > - Windows: SmartScreen → "More info" → "Run anyway".
     > - Linux: `chmod +x zanto-*.AppImage` and run, or `sudo dpkg -i` the .deb.
   - Draft (not auto-published) so the maintainer reviews assets/notes before publishing — same posture as the current workflow's `releaseDraft: true`.

## Edge cases & risks
- **No new dependencies.** Uses existing actions (`tauri-action`, `upload`/`download-artifact`, `rust-cache`) + `gh` (preinstalled on `ubuntu-latest`).
- **Cannot run on a Linux dev box.** The workflow only executes on GitHub Actions runners (needs macOS/Windows). Local verification is limited to **YAML validity + static review**; the real proof is the next tagged release (or a `workflow_dispatch` dry run producing `0.0.0-dev` assets). This is called out in the test plan.
- **Artifact path divergence from orbit:** orbit globs `src-tauri/target/**`; zanto's workspace target is at the **repo root**, so the glob is `target/**`. Copying orbit's path verbatim would silently upload nothing (`if-no-files-found: ignore` would mask it → empty release). This is the single highest-risk line; the spec pins `target/**`.
- **macOS single-arch DMG naming:** with `--target aarch64-apple-darwin`/`x86_64-apple-darwin`, the bundle lands under `target/<triple>/release/bundle/dmg/…`. The arch is in the triple path, but the **dmg filename** itself (`zanto_1.0.0_aarch64.dmg` / `_x64.dmg`) also carries the arch — the rename matches on the filename (`aarch64`/`arm64` substring), which holds for Tauri's default dmg names. Risk: if a future Tauri changes dmg naming, the Intel/Silicon branch could mislabel. Low; acceptable.
- **`fail-fast: true`:** if any one OS build fails, the other in-flight builds are cancelled and `publish` won't run (its `needs: build` is unmet) → no partial release. This is the orbit behavior and is desirable (no half-built release). Trade-off: a flaky single-runner failure wastes the whole run; acceptable.
- **`workflow_dispatch` dry run** creates a **draft** `v0.0.0-dev` release. It won't be published automatically, but the maintainer must delete the draft after a dry run to avoid clutter. Documented in the test plan.
- **`.rpm` not attached** by decision — if a user later wants RPM, add a `*.rpm` case + glob line. Flagged so it's a known, not a bug.
- **Two DMGs instead of one universal:** users on the download page must pick Apple-Silicon vs Intel. The site/download copy currently says ".dmg — universal" — **out of scope here**, but note: after this change that copy is stale and should be updated in a follow-up (site repo).

## Acceptance criteria
This is CI-only; criteria are verified by YAML/static checks now and by an Actions run later (not by `cargo run`).
- [x] `.github/workflows/release.yml` parses as valid YAML and defines exactly two jobs: `build` and `publish`.
- [x] `build` matrix has 4 rows: `macos-arm64` (aarch64), `macos-x64` (x86_64), `linux`, `windows`; `fail-fast: true`.
- [x] The `tauri-action` step in `build` has **no `tagName`** key (build-only); an `upload-artifact` step uploads from `target/**/release/bundle/**` (repo-root, not `src-tauri/...`).
- [x] `publish` `needs: build`, downloads artifacts, renames to the six `zanto-<version>-…` stems, and runs `gh release create "$TAG" --draft`.
- [x] Version derivation: a `v*` tag yields `VERSION` without the leading `v`; `workflow_dispatch` yields `0.0.0-dev`.
- [x] **Tag is authoritative:** the release title is `zanto $TAG` (no `__VERSION__` placeholder anywhere in the file — `grep -c "__VERSION__"` → `0`). A `v1.0.0-beta.0` tag produces a release titled `zanto v1.0.0-beta.0`, NOT `zanto 1.0.0`.
- [x] **Prerelease flag:** a tag whose version contains `-` (`-alpha`/`-beta`/`-dev`) passes `--prerelease` to `gh release create`; a plain `vX.Y.Z` does not.
- [x] `.rpm` is not in the rename map nor the asset glob; `tauri.conf.json` is unchanged.
- [x] After a real `vX.Y.Z` tag run, the draft release lists assets named `zanto-X.Y.Z-macOS-Apple-Silicon.dmg`, `zanto-X.Y.Z-macOS-Intel.dmg`, `zanto-X.Y.Z-Linux-x86_64.AppImage`, `zanto-X.Y.Z-Linux-x86_64.deb`, `zanto-X.Y.Z-Windows-x64-setup.exe`, `zanto-X.Y.Z-Windows-x64.msi`. *(verified on Actions, not locally)*

## Manual test plan
CI-only workflow — no `cargo run` path exercises it. Verification is static + a CI dry run.

1. **YAML lint / structure (local):**
   `python3 -c "import yaml,sys; d=yaml.safe_load(open('.github/workflows/release.yml')); print(list(d['jobs'].keys()))"`
   → prints `['build', 'publish']`. (Confirms valid YAML + both jobs.)
2. **Static assertions (local greps):**
   - `grep -c "tagName" .github/workflows/release.yml` → `0` (build-only).
   - `grep -c "src-tauri/target" .github/workflows/release.yml` → `0` (uses repo-root `target/`).
   - `grep -c "gh release create" .github/workflows/release.yml` → `1`.
   - `grep -c "__VERSION__" .github/workflows/release.yml` → `0` (the tauri-action placeholder that ignored the tag is gone).
   - `grep -c "prerelease" .github/workflows/release.yml` → ≥ `1` (prerelease detection + flag present).
   - `grep -o "macos-arm64\|macos-x64\|name: linux\|name: windows" .github/workflows/release.yml | sort -u` → all four names present.
   - `grep -c "rpm" .github/workflows/release.yml` → `0`.
3. **CI dry run (on GitHub, optional before tagging):**
   Trigger `workflow_dispatch`. Expect: 4 build matrix legs, a `publish` job producing a **draft** `v0.0.0-dev` release whose assets are the six `zanto-0.0.0-dev-…` files. Delete the draft afterward.
4. **Real release (the actual proof):**
   `git tag v1.0.0 && git push origin v1.0.0` → workflow builds all 4 legs, publish renames + creates draft `zanto v1.0.0` with the six human-readable assets. Review, then publish the draft from the Releases UI.
