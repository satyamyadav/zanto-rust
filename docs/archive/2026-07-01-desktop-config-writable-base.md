# Desktop config: writable base dir (fix macOS read-only FS error)

- **Date:** 2026-07-01

## Summary
Give the project config (`.zanto/settings.json`) a resolvable base directory so the packaged desktop app writes to an OS-writable location instead of the process CWD, which is `/` (read-only) on a macOS `.app` launch.

## Motivation
On macOS, double-clicking the `.app` starts the process with CWD `/`. All project-config persistence in `zanto-core::config` writes to the **CWD-relative** path `.zanto/settings.json`:

- `Settings::save()` — writes provider config, active provider, model, generation, `project_dir`.
- `Settings::persist_allowed_path()` — appends to `allowed_paths`.
- `Settings::ensure_project_config()` — creates the default file at startup.

All three call `std::fs::write(".zanto/settings.json")` / `create_dir_all(".zanto")` against CWD. On the packaged mac app this is `/.zanto/…` → **`Read-only file system (os error 30)`**. The user sees "read-only filesystem" when saving provider config (`set_config` → `save()`) and when selecting a directory (`add_allowed_path` / `set_project_dir` → `persist_allowed_path` + `save()`).

The CLI is unaffected because its CWD is the user's writable working directory — that behavior must be preserved.

## Scope
**In scope**
- Add a process-global, overridable base directory for the project config in `config.rs`.
- Route the four config path sites through it.
- Set the override from the desktop app at startup to the OS app-data dir (the same base already used for `zanto.db`, skills, and artifacts via `directories::ProjectDirs::from("", "", "zanto").data_dir()`).

**Out of scope**
- CLI behavior (unchanged: base defaults to CWD → `.zanto/settings.json` relative to where the user runs it).
- The user-layer config (`~/.config/zanto/settings.json`) — already OS-writable, untouched.
- Migrating any existing `/.zanto` file (none can exist — the write always failed).
- The two-layer (user + project) merge model — preserved as-is.
- Skills/artifacts project scoping (keyed off `project_dir`, a user-chosen writable path — not CWD — so unaffected).

## Affected files
- `crates/zanto-core/src/config.rs` — add the base-dir override + resolver; route `load`, `save`, `persist_allowed_path`, `ensure_project_config` through it.
- `crates/zanto-desktop/src-tauri/src/lib.rs` — set the override once at startup, before the first `Settings::load()`.
- `crates/zanto-desktop/src-tauri/src/ipc/config.rs` — `load_project_settings` reads `config::PROJECT_CONFIG` directly (line ~175); route it through the same resolver so it reads the moved file.

## Implementation steps

1. **Add a config-base override + path resolver** (`crates/zanto-core/src/config.rs`)
   - Add a process-global override: `static PROJECT_CONFIG_BASE: OnceLock<PathBuf>` (or `RwLock<Option<PathBuf>>` if it must be settable after first read; a `OnceLock` set once at startup is sufficient and simpler).
   - Add `pub fn set_project_config_base(dir: PathBuf)` — stores the override. Idempotent; first-set-wins is fine (desktop sets it once before any load).
   - Add `fn project_config_path() -> PathBuf` — returns `<base>/.zanto/settings.json`, where `base` is the override if set, else `PathBuf::new()` (empty → the existing CWD-relative `.zanto/settings.json`). Keep `PROJECT_CONFIG` as the relative suffix constant.
   - Add `fn project_config_dir() -> PathBuf` — the `.zanto` parent, for `create_dir_all`.

2. **Route all four sites through the resolver** (`crates/zanto-core/src/config.rs`)
   - `load()` (line 432): `Self::load_file(Self::project_config_path())`.
   - `persist_allowed_path()` (line 438): `let config_path = Self::project_config_path();` and `create_dir_all(project_config_dir())` before the write (the CWD version relied on `.zanto` already existing from `ensure_project_config`; make it self-sufficient), then `std::fs::write(&config_path, …)`.
   - `save()` (line 450): `let path = Self::project_config_path();` — its existing `create_dir_all(path.parent())` then works for the app-data base too.
   - `ensure_project_config()` (line 480): use `project_config_path()` / `project_config_dir()`.

3. **Set the override at desktop startup** (`crates/zanto-desktop/src-tauri/src/lib.rs`)
   - In the Tauri `setup` closure, **before** the first `Settings::load()` (currently line ~37), resolve the app-data base with `directories::ProjectDirs::from("", "", "zanto").map(|d| d.data_dir().to_path_buf())` and call `zanto_core::config::set_project_config_base(base)`.
   - Match the existing convention: the DB uses `ProjectDirs…data_dir()`. Put the project config file at `<data_dir>/.zanto/settings.json` (so `set_project_config_base(<data_dir>)`), keeping the `.zanto/` suffix uniform with the CLI layout. Confirm `directories` is already a desktop dep (it's a core dep; add to desktop only if `ProjectDirs` is referenced there — otherwise expose a helper from core, see 3b).
   - **3b (preferred to avoid a new desktop dep):** add `pub fn default_desktop_config_base() -> Option<PathBuf>` to `zanto-core::config` (wrapping `ProjectDirs::from("", "", "zanto").map(|d| d.data_dir().to_path_buf())`), and have the desktop call `set_project_config_base(...)` with it. This keeps `directories` usage inside core.

4. **Route the desktop's direct read through the resolver** (`crates/zanto-desktop/src-tauri/src/ipc/config.rs`)
   - `load_project_settings` (line ~175) does `std::fs::read_to_string(config::PROJECT_CONFIG)`. Replace with a core accessor that reads `project_config_path()` — add `pub fn project_config_raw() -> std::io::Result<String>` to core, or expose `pub fn project_config_path()` and read that. Prefer exposing `project_config_path()` `pub` so the desktop reads the same resolved path.

## Edge cases & risks
- **First-set-wins ordering:** the override must be set before any `Settings::load()` on the desktop. `lib.rs` calls `Settings::load()` at line ~37 in setup; the `set_project_config_base` call must precede it. If any earlier code path loads settings, it will use CWD. Audit: grep `Settings::load` in desktop startup — the setup closure is the first. Low risk, but call out.
- **No new desktop dependency** if step 3b is used (resolver stays in core). If `ProjectDirs` is used directly in `lib.rs`, add `directories` to `zanto-desktop/src-tauri/Cargo.toml`.
- **Tests that write `.zanto` in CWD:** existing config tests rely on the CWD-relative default (override unset). Since the override defaults to unset → CWD, CLI/test behavior is byte-identical. No test changes expected; verify `cargo test -p zanto-core config` stays green.
- **Concurrency:** `OnceLock<PathBuf>` is thread-safe; the getter is lock-free after set.
- **Not a data migration:** any pre-existing user `.zanto/settings.json` in a CLI working dir keeps working for the CLI. The desktop simply starts reading/writing its own app-data copy. (Provider keys live in the OS keyring, not this file, so no key loss.)

## Acceptance criteria
- [ ] `cargo build` compiles both crates.
- [ ] CLI unchanged: `cargo run -p zanto-cli -- "hi"` still reads/writes `./.zanto/settings.json` relative to CWD (override unset).
- [ ] Desktop dev (`cargo run -p zanto-desktop` from repo root) writes provider config to `<app-data>/.zanto/settings.json`, NOT `./.zanto/…`. Verify the file appears under the OS data dir after saving a provider in Settings.
- [ ] On a packaged macOS `.app` (CWD `/`): saving provider config succeeds (no "read-only file system"), and selecting a project directory succeeds — both `set_config` and `add_allowed_path`/`set_project_dir` complete without error.
- [ ] `cargo test -p zanto-core` green (config tests unaffected).

## Manual test plan
1. `cargo run -p zanto-cli -- "ping"` → confirm `.zanto/settings.json` created in CWD (unchanged CLI behavior).
2. `cargo run -p zanto-desktop` from repo root → open Settings, save a provider → confirm the file lands under the OS data dir (`~/.local/share/zanto/.zanto/settings.json` on Linux; `~/Library/Application Support/zanto/.zanto/settings.json` on macOS), and NOT in the repo root.
3. macOS packaged build: `pnpm tauri build`, launch the `.app` by double-click, open Settings → save provider (expect success), select a folder (expect success). Before the fix both throw "read-only file system".
