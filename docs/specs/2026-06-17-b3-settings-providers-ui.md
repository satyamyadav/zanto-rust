# B3 — Provider / keys / model Settings UI

- **Date:** 2026-06-17
- **Wave:** B (shell scaffolding), batch 2 (after B1 + A1)
- **Owner of:** `SettingsDialog.svelte`, `ipc/config.rs`, config DTOs, `ipc.ts` config wrappers

## Summary
Expose A1's provider model + keychain keys in the app: pick the active provider, set its
model/endpoint, and enter API keys (stored via the keychain). Replaces the current
"model/endpoint text + env-var note" section.

## Affected files
- `crates/zanto-desktop/src-tauri/src/ipc/config.rs` — extend `ConfigDto`/`ConfigPatch`;
  new key commands.
- `crates/zanto-desktop/src-tauri/src/lib.rs` — register the new commands.
- `crates/zanto-desktop/src/lib/ipc.ts` — types + wrappers.
- `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` — provider/keys UI.
- `crates/zanto-desktop/src/lib/stores/app.svelte.ts` — if it caches config, extend it.

## Design

### Backend (`ipc/config.rs`)
```rust
#[derive(Serialize)] struct ProviderDto { provider: String, model: String, endpoint: Option<String>, has_key: bool }
// ConfigDto gains: providers: Vec<ProviderDto>, active_provider: Option<String>
// ConfigPatch gains: providers: Option<Vec<ProviderConfig-ish>>, active_provider: Option<String>
#[tauri::command] fn set_api_key(provider: String, key: String) -> Result<(), String>   // config::set_api_key
#[tauri::command] fn clear_api_key(provider: String) -> Result<(), String>
```
- `get_config`: populate `providers` from `Settings.providers` (or a default list of the
  four), each with `has_key = config::has_api_key(provider)`; never return key material.
- `set_config`: persist `providers`/`active_provider` into `Settings` (+ keep the legacy
  `model`/`endpoint` in sync with `Settings::active()` so the running `DesktopState.model`
  reflects the choice). Apply live to `state.model`/`state.endpoint`.
- Map provider string ↔ `Provider` enum; reject unknown.

### Frontend (`SettingsDialog.svelte`)
- A "Provider & model" section: a select for active provider; for the active provider,
  inputs for model + endpoint (endpoint only meaningful for Ollama); a password-style key
  input with a "Saved ✓" indicator when `has_key`, a Save-key button (→ `set_api_key`) and
  a Clear button (→ `clear_api_key`). Keys are write-only from the UI (never displayed).
- Keep the existing Appearance + Folder-access sections intact.

## Acceptance checks
- `cargo build` clean; `pnpm check` 0 errors; `pnpm build:web` clean.
- `get_config` returns providers with correct `has_key` and never leaks keys.
- Switching active provider + Save updates the running model (next turn uses it).
- Saving a key calls the keychain; on a headless box `set_api_key` surfaces the `Err`
  as a toast rather than crashing (build-check only verifies compile/serialize).

## Notes / handoff
- Depends on B1 (so `ipc/config.rs` exists as its own module) and A1 (provider/key API).
- The desktop still wires `config.context`/skill loading in a later unit — not here.
