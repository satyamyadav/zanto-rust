# A1 — Provider / model / keys foundation

- **Date:** 2026-06-17
- **Wave:** A (core foundations), batch 1 (∥ A2)
- **Owner of:** `crates/zanto-core/src/config.rs` (entirely, this wave), `chat.rs` client build

## Summary
Generalize `Settings` from a single `model`/`endpoint` to a **provider model**:
multiple providers (Ollama, Gemini, Anthropic, OpenAI), each with its own model +
optional endpoint, an active selection, and **API keys stored in the OS keychain**
(`keyring` crate) with env-var fallback. The genai client resolves auth from the
keychain for the active provider.

A1 also adds the two config fields the rest of Wave A consumes (`project_dir`,
`context_sources`) so no other unit edits `config.rs` this wave.

## Affected files
- `crates/zanto-core/Cargo.toml` — add `keyring = "3"`.
- `crates/zanto-core/src/config.rs` — Provider model, key access, new fields, merge/serde.
- `crates/zanto-core/src/chat.rs` — client build: provider detection + `AuthResolver`.

## Design

### Provider + Settings
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider { Ollama, Gemini, Anthropic, OpenAI }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: Provider,
    pub model: String,                 // active model for this provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,      // override (Ollama needs it)
}
```
Add to `Settings` (keep existing fields for back-compat):
```rust
#[serde(default)] pub providers: Vec<ProviderConfig>,
#[serde(skip_serializing_if = "Option::is_none")] pub active_provider: Option<Provider>,
#[serde(skip_serializing_if = "Option::is_none")] pub project_dir: Option<String>,
#[serde(default)] pub context_sources: Vec<String>,
```
- **Back-compat:** existing `model`/`endpoint` remain the *effective active* values.
  Add `Settings::active() -> (Provider, String /*model*/, Option<String> /*endpoint*/)`
  that prefers `active_provider` + matching `ProviderConfig`, else falls back to
  legacy `model`/`endpoint` (infer provider from model prefix; default Ollama).
- `merge` extends `providers`, takes `active_provider`/`project_dir` if `Some`,
  extends `context_sources`. `resolve_paths` also canonicalizes `project_dir`.

### Keys (keychain + env fallback)
```rust
const KEYRING_SERVICE: &str = "zanto";
pub fn api_key(p: Provider) -> Option<String>     // keyring → else env (GEMINI_API_KEY / ANTHROPIC_API_KEY / OPENAI_API_KEY)
pub fn set_api_key(p: Provider, key: &str) -> Result<(), String>  // keyring write; Err on no secret service
pub fn clear_api_key(p: Provider) -> Result<(), String>
pub fn has_api_key(p: Provider) -> bool
```
Keyring entry: service `"zanto"`, username = provider snake_case. Ollama needs none.

### chat.rs client build
- Replace `override_endpoint = !model.starts_with("gemini")` with
  `provider_of(model)` (prefix map: `gemini-`→Gemini, `claude-`→Anthropic,
  `gpt-`/`o1`→OpenAI, else Ollama). **Override endpoint only for Ollama.**
- Add an `AuthResolver` (genai `AuthResolver::from_resolver_fn`) returning
  `AuthData::from_single(key)` from `config::api_key(provider)` for cloud providers;
  no auth for Ollama. Build client with both the existing `ServiceTargetResolver`
  and the new auth resolver.

## Acceptance checks
- `cargo build` clean; existing 28 core tests pass.
- New tests: `Settings` serde round-trips with `providers`/`active_provider`;
  `active()` falls back to legacy `model`/`endpoint`; `provider_of()` prefix map.
- Key path: test the **env fallback** of `api_key` (set env, assert read). Do **not**
  assert keychain writes in tests — headless CI has no secret service; `set_api_key`
  must return `Err` gracefully there, not panic.

## Notes / handoff
- B3 (Settings UI) calls `set_api_key`/`has_api_key` + reads/writes `providers`.
- A4 consumes `project_dir` + `context_sources`. A3 consumes `project_dir`.
- Do **not** remove legacy `model`/`endpoint` — desktop `DesktopState` still reads them
  until B1/B3 migrate; keep them in sync via `active()`.
