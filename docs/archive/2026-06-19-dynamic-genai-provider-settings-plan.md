# Dynamic genai Provider Settings — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the desktop Settings provider panel dynamic — driven by genai's `AdapterKind` registry, with live model lists and exposed generation parameters.

**Architecture:** Replace the closed `Provider` enum with a thin newtype over `genai::adapter::AdapterKind`, curated to a tested ~10-provider registry. Drive the Settings UI from that registry over IPC. Add a `list_models` IPC backed by `Client::all_model_names`, and a global `GenerationParams` block wired into the chat loop's `ChatOptions`.

**Tech Stack:** Rust (zanto-core lib + Tauri backend), genai 0.6.4, SvelteKit 5 + shadcn-svelte frontend.

Spec: `docs/specs/2026-06-19-dynamic-genai-provider-settings.md`.

## Global Constraints

- genai version is pinned at `0.6` (`crates/zanto-core/Cargo.toml:12`) — use only APIs present in 0.6.4.
- `cargo build` is the compile gate (both crates). A green build does not prove behavior — the final task runs the app manually.
- Provider ids persisted in settings.json are genai `AdapterKind::as_lower_str()` values (e.g. `"openai"`, `"anthropic"`, `"gemini"`, `"ollama"`).
- Legacy migration: a stored provider id of `"open_ai"` must map to `AdapterKind::OpenAI`.
- Keychain service name stays `"zanto"`; username stays the provider id (`as_str()`), so existing stored keys keep working.
- Curated provider set (v1), in this order: Anthropic, OpenAI, Gemini, Groq, Xai, DeepSeek, Together, Fireworks, Cohere, Ollama.
- Generation params are **global** (one block in `Settings`), all-`None`/empty by default → identical behavior to today when unset.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/zanto-core/src/config.rs` | `Provider(AdapterKind)` newtype + serde + key/env helpers; `SUPPORTED` registry + descriptors + labels + default-model map; lenient provider deserialization + `open_ai` migration; `GenerationParams` + `Settings.generation` + `to_chat_options` |
| `crates/zanto-core/src/chat.rs` | Apply `GenerationParams` to `stream_options`; `ChatConfig.generation` field |
| `crates/zanto-desktop/src-tauri/src/ipc/mod.rs` | DTO additions (`ProviderDto` label/needs_key/default_endpoint; `ConfigDto`/`ConfigPatch` generation; `GenerationParamsDto`) |
| `crates/zanto-desktop/src-tauri/src/ipc/config.rs` | Registry-driven `get_config`; `parse_provider` via `from_lower_str`; carry `generation`; new `list_models` command |
| `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` | Populate `ChatConfig.generation` from `Settings` |
| `crates/zanto-desktop/src-tauri/src/lib.rs` | Register `list_models` in the invoke handler |
| `crates/zanto-desktop/src/lib/ipc.ts` | Type updates + `listModels` |
| `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` | Registry-driven provider select; model combobox + Refresh; Generation section |

---

## Task 1: `Provider` newtype over `AdapterKind` + registry + migration

**Files:**
- Modify: `crates/zanto-core/src/config.rs` (lines 9–68 provider block; 108–117 `ProviderConfig`; 119–156 `Settings`; 285–325 key fns; tests)

**Interfaces:**
- Produces:
  - `pub struct Provider(pub genai::adapter::AdapterKind)` — `Copy`, `PartialEq`, `Eq`, custom `Serialize`/`Deserialize`.
  - `impl Provider { pub fn as_str(self) -> &'static str; pub fn env_var(self) -> Option<&'static str>; pub fn needs_key(self) -> bool; pub fn label(self) -> &'static str; pub fn default_model(self) -> &'static str; pub fn default_endpoint(self) -> Option<&'static str>; }`
  - `pub const SUPPORTED: &[genai::adapter::AdapterKind]`
  - `pub struct ProviderInfo { pub id: String, pub label: String, pub needs_key: bool, pub default_endpoint: Option<String> }`
  - `pub fn provider_registry() -> Vec<ProviderInfo>`
  - `pub fn provider_of(model: &str) -> Provider` (unchanged signature)
  - Existing `api_key`/`set_api_key`/`clear_api_key`/`has_api_key`/`model_context_window` keep their signatures but now take/return the newtype.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `crates/zanto-core/src/config.rs`:

```rust
#[test]
fn provider_serde_roundtrips_lower_str() {
    use genai::adapter::AdapterKind;
    for kind in SUPPORTED {
        let p = Provider(*kind);
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, format!("\"{}\"", kind.as_lower_str()));
        let back: Provider = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}

#[test]
fn provider_deserializes_legacy_open_ai() {
    let p: Provider = serde_json::from_str("\"open_ai\"").unwrap();
    assert_eq!(p, Provider(genai::adapter::AdapterKind::OpenAI));
}

#[test]
fn settings_drops_unknown_provider_id() {
    let json = r#"{
        "providers": [
            {"provider":"anthropic","model":"claude-opus-4-5"},
            {"provider":"totally-bogus","model":"x"}
        ],
        "active_provider": "totally-bogus"
    }"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(s.providers.len(), 1);
    assert_eq!(s.providers[0].provider, Provider(genai::adapter::AdapterKind::Anthropic));
    assert!(s.active_provider.is_none());
}

#[test]
fn registry_has_curated_set_in_order() {
    let reg = provider_registry();
    let ids: Vec<&str> = reg.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec![
        "anthropic","openai","gemini","groq","xai","deepseek","together","fireworks","cohere","ollama"
    ]);
    let ollama = reg.iter().find(|p| p.id == "ollama").unwrap();
    assert!(!ollama.needs_key);
    assert_eq!(ollama.default_endpoint.as_deref(), Some("http://localhost:11434/"));
    let anthropic = reg.iter().find(|p| p.id == "anthropic").unwrap();
    assert!(anthropic.needs_key);
    assert!(anthropic.default_endpoint.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p zanto-core config::tests 2>&1 | tail -20`
Expected: compile errors / FAIL — `Provider` is still the enum, `SUPPORTED`/`provider_registry` undefined.

- [ ] **Step 3: Replace the provider block (lines 9–53)**

Replace the `Provider` enum, its `impl`, and `provider_of` with:

```rust
use genai::adapter::AdapterKind;

/// An LLM provider — a thin newtype over genai's `AdapterKind`, which is the
/// single source of truth for which providers and protocols exist. Persisted in
/// settings.json as the lowercase id (`AdapterKind::as_lower_str`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provider(pub AdapterKind);

/// Providers surfaced in the UI (curated to the set we test). Adding one here is
/// the only edit needed to offer a new provider.
pub const SUPPORTED: &[AdapterKind] = &[
    AdapterKind::Anthropic,
    AdapterKind::OpenAI,
    AdapterKind::Gemini,
    AdapterKind::Groq,
    AdapterKind::Xai,
    AdapterKind::DeepSeek,
    AdapterKind::Together,
    AdapterKind::Fireworks,
    AdapterKind::Cohere,
    AdapterKind::Ollama,
];

impl Provider {
    /// Lowercase id; the keyring username and the persisted/UI identifier.
    pub fn as_str(self) -> &'static str {
        self.0.as_lower_str()
    }

    /// Env var consulted as an API-key fallback; `None` when no key is needed.
    pub fn env_var(self) -> Option<&'static str> {
        self.0.default_key_env_name()
    }

    /// Whether this provider requires an API key.
    pub fn needs_key(self) -> bool {
        self.env_var().is_some()
    }

    /// Human-friendly label for the UI.
    pub fn label(self) -> &'static str {
        match self.0 {
            AdapterKind::Anthropic => "Anthropic",
            AdapterKind::OpenAI => "OpenAI",
            AdapterKind::Gemini => "Gemini",
            AdapterKind::Groq => "Groq",
            AdapterKind::Xai => "xAI",
            AdapterKind::DeepSeek => "DeepSeek",
            AdapterKind::Together => "Together AI",
            AdapterKind::Fireworks => "Fireworks",
            AdapterKind::Cohere => "Cohere",
            AdapterKind::Ollama => "Ollama",
            other => other.as_str(),
        }
    }

    /// A sensible default model to seed a freshly-added provider config.
    pub fn default_model(self) -> &'static str {
        match self.0 {
            AdapterKind::Anthropic => "claude-opus-4-5",
            AdapterKind::OpenAI => "gpt-4o",
            AdapterKind::Gemini => "gemini-2.0-flash",
            AdapterKind::Groq => "llama-3.3-70b-versatile",
            AdapterKind::Xai => "grok-2",
            AdapterKind::DeepSeek => "deepseek-chat",
            AdapterKind::Together => "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            AdapterKind::Fireworks => "accounts/fireworks/models/llama-v3p3-70b-instruct",
            AdapterKind::Cohere => "command-r-plus",
            AdapterKind::Ollama => "qwen2.5:14b",
            _ => "",
        }
    }

    /// Default endpoint override (only Ollama needs one).
    pub fn default_endpoint(self) -> Option<&'static str> {
        match self.0 {
            AdapterKind::Ollama => Some("http://localhost:11434/"),
            _ => None,
        }
    }
}

/// Custom serde: persist/parse as the lowercase id, remapping the legacy
/// `"open_ai"` form to `AdapterKind::OpenAI`.
impl serde::Serialize for Provider {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Provider {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        provider_from_id(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown provider: {raw}")))
    }
}

/// Parse a stored/UI provider id into a `Provider`, applying legacy remaps.
/// Returns `None` for ids genai does not recognize.
pub fn provider_from_id(s: &str) -> Option<Provider> {
    let id = if s == "open_ai" { "openai" } else { s };
    AdapterKind::from_lower_str(id).map(Provider)
}

/// A registry entry describing a provider for the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub label: String,
    pub needs_key: bool,
    pub default_endpoint: Option<String>,
}

/// The curated provider registry, in display order.
pub fn provider_registry() -> Vec<ProviderInfo> {
    SUPPORTED
        .iter()
        .map(|k| {
            let p = Provider(*k);
            ProviderInfo {
                id: p.as_str().to_string(),
                label: p.label().to_string(),
                needs_key: p.needs_key(),
                default_endpoint: p.default_endpoint().map(str::to_string),
            }
        })
        .collect()
}

/// Infer the provider for a model name. Falls back to Ollama (local) when genai
/// can't classify the name from its prefix.
pub fn provider_of(model: &str) -> Provider {
    AdapterKind::from_model(model)
        .ok()
        .map(Provider)
        .unwrap_or(Provider(AdapterKind::Ollama))
}
```

- [ ] **Step 4: Update `model_context_window` (was lines 60–68)**

```rust
pub fn model_context_window(model: &str) -> usize {
    match provider_of(model).0 {
        AdapterKind::Anthropic => 200_000,
        AdapterKind::Gemini => 1_000_000,
        AdapterKind::OpenAI => 128_000,
        AdapterKind::Ollama => 8_192,
        // New providers: a conservative default until a real value is known.
        _ => 32_000,
    }
}
```

- [ ] **Step 5: Make `Settings` provider fields lenient**

In the `Settings` struct, change the two provider-bearing fields to use lenient deserializers (keep their `#[serde(...)]` attrs, add `deserialize_with`):

```rust
    #[serde(default, deserialize_with = "de_providers")]
    pub providers: Vec<ProviderConfig>,
```
```rust
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_active_provider")]
    pub active_provider: Option<Provider>,
```

Add these free functions near `provider_from_id`:

```rust
/// Deserialize the provider list, silently dropping entries whose id genai does
/// not recognize (rather than failing the whole settings file).
fn de_providers<'de, D>(d: D) -> Result<Vec<ProviderConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Raw {
        provider: String,
        model: String,
        #[serde(default)]
        endpoint: Option<String>,
    }
    let raw = Vec::<Raw>::deserialize(d)?;
    Ok(raw
        .into_iter()
        .filter_map(|r| {
            provider_from_id(&r.provider).map(|p| ProviderConfig {
                provider: p,
                model: r.model,
                endpoint: r.endpoint,
            })
        })
        .collect())
}

/// Deserialize the active provider, dropping it to `None` if unrecognized.
fn de_active_provider<'de, D>(d: D) -> Result<Option<Provider>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(d)?;
    Ok(opt.as_deref().and_then(provider_from_id))
}
```

`ProviderConfig` keeps `#[derive(Debug, Clone, Serialize, Deserialize)]` — `de_providers` builds it directly, and `Provider`'s `Serialize` handles writing.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p zanto-core config:: 2>&1 | tail -25`
Expected: PASS for the four new tests and the pre-existing `provider_of_prefix_map`, `model_context_window_by_provider`, `api_key_*` tests.

> Note: the pre-existing tests reference `Provider::Gemini` etc. Update those literals to `Provider(AdapterKind::Gemini)` (and add `use genai::adapter::AdapterKind;` to the test module). `provider_of("o1-mini")` now classifies via genai; if genai does not map `o1-*` to OpenAI, change that assertion's input to `"gpt-4o"` and drop the `o1-mini` line.

- [ ] **Step 7: Compile the whole core crate (catches enum-reference fallout)**

Run: `cargo build -p zanto-core 2>&1 | tail -30`
Expected: errors only in `chat.rs` (handled in Task 3). Fix any remaining `Provider::Variant` references inside `config.rs` itself now. If `cargo build -p zanto-core` is clean except chat.rs, proceed.

- [ ] **Step 8: Commit**

```bash
git add crates/zanto-core/src/config.rs
git commit -m "feat(core): Provider newtype over genai AdapterKind + curated registry"
```

---

## Task 2: `GenerationParams` + `to_chat_options`

**Files:**
- Modify: `crates/zanto-core/src/config.rs` (add struct + `Settings.generation` + helper + test)

**Interfaces:**
- Consumes: `Provider` (Task 1).
- Produces:
  - `pub struct GenerationParams { temperature: Option<f64>, max_tokens: Option<u32>, top_p: Option<f64>, reasoning_effort: Option<String>, seed: Option<u64>, stop_sequences: Vec<String>, extra_body: Option<serde_json::Value> }` — `Default`, `Clone`, serde.
  - `impl GenerationParams { pub fn apply(&self, opts: genai::chat::ChatOptions) -> genai::chat::ChatOptions; }`
  - `Settings.generation: GenerationParams` (`#[serde(default)]`).

- [ ] **Step 1: Write the failing test**

Add to `config.rs` tests:

```rust
#[test]
fn generation_params_apply_only_set_fields() {
    use genai::chat::ChatOptions;
    let gp = GenerationParams {
        temperature: Some(0.3),
        max_tokens: Some(1024),
        reasoning_effort: Some("high".into()),
        stop_sequences: vec!["STOP".into()],
        ..Default::default()
    };
    let opts = gp.apply(ChatOptions::default());
    assert_eq!(opts.temperature, Some(0.3));
    assert_eq!(opts.max_tokens, Some(1024));
    assert_eq!(opts.top_p, None);
    assert_eq!(opts.stop_sequences, Some(vec!["STOP".to_string()]));
}

#[test]
fn generation_params_default_is_empty() {
    let gp = GenerationParams::default();
    assert!(gp.temperature.is_none());
    assert!(gp.stop_sequences.is_empty());
    let s: Settings = serde_json::from_str("{}").unwrap();
    assert!(s.generation.temperature.is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p zanto-core generation_params 2>&1 | tail -15`
Expected: FAIL — `GenerationParams` undefined.

- [ ] **Step 3: Add the struct + helper**

```rust
/// Global generation parameters surfaced in Settings and applied to every turn.
/// All-`None`/empty by default → genai's defaults (today's behavior).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerationParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// One of none|minimal|low|medium|high|xhigh. Unparseable values are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
    /// Advanced escape hatch: merged into the provider request body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<serde_json::Value>,
}

impl GenerationParams {
    /// Apply the set fields onto an existing `ChatOptions` (capture flags etc.
    /// are preserved by the caller building the base options first).
    pub fn apply(&self, mut opts: genai::chat::ChatOptions) -> genai::chat::ChatOptions {
        use genai::chat::ReasoningEffort;
        if let Some(t) = self.temperature {
            opts = opts.with_temperature(t);
        }
        if let Some(m) = self.max_tokens {
            opts = opts.with_max_tokens(m);
        }
        if let Some(p) = self.top_p {
            opts = opts.with_top_p(p);
        }
        if let Some(s) = self.seed {
            opts = opts.with_seed(s);
        }
        if let Some(eff) = self.reasoning_effort.as_deref() {
            let parsed = match eff {
                "none" => Some(ReasoningEffort::None),
                "minimal" => Some(ReasoningEffort::Minimal),
                "low" => Some(ReasoningEffort::Low),
                "medium" => Some(ReasoningEffort::Medium),
                "high" => Some(ReasoningEffort::High),
                "xhigh" => Some(ReasoningEffort::XHigh),
                _ => None,
            };
            if let Some(e) = parsed {
                opts = opts.with_reasoning_effort(e);
            }
        }
        if !self.stop_sequences.is_empty() {
            opts = opts.with_stop_sequences(self.stop_sequences.clone());
        }
        if let Some(body) = &self.extra_body {
            opts = opts.with_extra_body(body.clone());
        }
        opts
    }
}
```

- [ ] **Step 4: Add the field to `Settings`**

After the `selected_skill` field:

```rust
    /// Global generation parameters applied to every turn.
    #[serde(default)]
    pub generation: GenerationParams,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p zanto-core generation_params 2>&1 | tail -15`
Expected: PASS.

> If `ChatOptions` field names differ (e.g. `stop_sequences` typed as `Option<Vec<String>>` vs a wrapper), adjust the assertions to match the genai 0.6.4 field types; the `with_*` builders are confirmed present.

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-core/src/config.rs
git commit -m "feat(core): global GenerationParams + ChatOptions apply helper"
```

---

## Task 3: Wire generation params into the chat loop

**Files:**
- Modify: `crates/zanto-core/src/chat.rs` (struct `ChatConfig` ~122–147; `ChatConfig::new` ~150–164; `stream_options` build ~332–335; provider checks ~230–252)

**Interfaces:**
- Consumes: `GenerationParams` (Task 2), `Provider` (Task 1).
- Produces: `ChatConfig.generation: GenerationParams`.

- [ ] **Step 1: Add the field to `ChatConfig`**

After the `images` field in `pub struct ChatConfig`:

```rust
    /// Global generation parameters applied to this turn's request options.
    pub generation: crate::config::GenerationParams,
```

- [ ] **Step 2: Default it in `ChatConfig::new`**

In `ChatConfig::new`, add to the struct literal:

```rust
            generation: crate::config::Settings::load().generation,
```

This makes the CLI honor the configured params too.

- [ ] **Step 3: Fix the `Provider` comparisons (newtype fallout)**

In the auth/endpoint resolver block (~230–252), the comparisons `provider == Provider::Ollama` must become:

```rust
    let provider = config::provider_of(&config.model);
    let override_endpoint = provider.0 == genai::adapter::AdapterKind::Ollama;
```
and inside the auth resolver:
```rust
            if provider.0 == genai::adapter::AdapterKind::Ollama {
                return Ok(None);
            }
```
Add `use genai::adapter::AdapterKind;` to the imports if not already present.

- [ ] **Step 4: Apply generation params to `stream_options`**

Replace the `stream_options` builder (~332–335) with:

```rust
    let stream_options = config.generation.apply(
        ChatOptions::default()
            .with_capture_content(true)
            .with_capture_tool_calls(true)
            .with_capture_reasoning_content(true),
    );
```

Leave the summarize call (`summarize_messages`) untouched — it keeps its own minimal options.

- [ ] **Step 5: Compile**

Run: `cargo build -p zanto-core 2>&1 | tail -20`
Expected: clean build.

- [ ] **Step 6: Run the core test suite**

Run: `cargo test -p zanto-core 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/zanto-core/src/chat.rs
git commit -m "feat(core): apply GenerationParams to chat stream options"
```

---

## Task 4: IPC — registry-driven config, `parse_provider`, `list_models`

**Files:**
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/mod.rs` (DTOs ~83–120)
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/config.rs` (whole file)
- Modify: `crates/zanto-desktop/src-tauri/src/lib.rs` (invoke handler ~104)

**Interfaces:**
- Consumes: `provider_registry`, `provider_from_id`, `GenerationParams`, `api_key`, `Provider` (Tasks 1–2).
- Produces: IPC `list_models(provider: String) -> Result<Vec<String>, String>`; `ProviderDto`/`ConfigDto`/`ConfigPatch` carry the new fields; `get_config().provider_registry`.

- [ ] **Step 1: Extend DTOs in `ipc/mod.rs`**

`ProviderDto` — add fields:
```rust
pub struct ProviderDto {
    pub provider: String,
    pub model: String,
    pub endpoint: Option<String>,
    pub has_key: bool,
    pub label: String,
    pub needs_key: bool,
    pub default_endpoint: Option<String>,
}
```

Add a generation DTO that mirrors `GenerationParams` (reuse the core type directly to avoid drift):
```rust
pub use zanto_core::config::{GenerationParams, ProviderInfo};
```
Add to `ConfigDto`:
```rust
    pub provider_registry: Vec<ProviderInfo>,
    pub generation: GenerationParams,
```
Add to `ConfigPatch`:
```rust
    pub generation: Option<GenerationParams>,
```

- [ ] **Step 2: Rewrite `parse_provider` and `default_providers` in `ipc/config.rs`**

Replace `parse_provider` (lines 22–31) with:
```rust
fn parse_provider(s: &str) -> Result<Provider, String> {
    config::provider_from_id(s).ok_or_else(|| format!("unknown provider: {s}"))
}
```
Replace `default_providers` (lines 8–15) with a registry-derived default:
```rust
fn default_providers() -> Vec<ProviderConfig> {
    config::SUPPORTED
        .iter()
        .map(|k| {
            let p = Provider(*k);
            ProviderConfig {
                provider: p,
                model: p.default_model().to_string(),
                endpoint: p.default_endpoint().map(str::to_string),
            }
        })
        .collect()
}
```
Update imports at the top of `ipc/config.rs`:
```rust
use zanto_core::config::{self, ContextSource, Provider, ProviderConfig, Settings};
```

- [ ] **Step 3: Populate the new fields in `get_config`**

In the `providers` map closure, add the registry-derived fields:
```rust
        .map(|pc| {
            let has_key = config::has_api_key(pc.provider);
            ProviderDto {
                provider: pc.provider.as_str().to_string(),
                label: pc.provider.label().to_string(),
                needs_key: pc.provider.needs_key(),
                default_endpoint: pc.provider.default_endpoint().map(str::to_string),
                model: pc.model,
                endpoint: pc.endpoint,
                has_key,
            }
        })
```
In the returned `ConfigDto`, add:
```rust
        provider_registry: config::provider_registry(),
        generation: settings.generation.clone(),
```

- [ ] **Step 4: Apply the generation patch in `set_config`**

Before `settings.save()` at the end of `set_config`:
```rust
    if let Some(gen) = patch.generation {
        settings.generation = gen;
    }
```

- [ ] **Step 5: Add the `list_models` command**

Append to `ipc/config.rs`:
```rust
/// List the models a provider exposes, using the saved key/endpoint. Errors
/// (missing key, offline, no list endpoint) surface to the UI as `Err`.
#[tauri::command]
pub async fn list_models(provider: String) -> Result<Vec<String>, String> {
    use genai::adapter::AdapterKind;
    use genai::resolver::{AuthData, Endpoint};
    use genai::ModelIden;

    let p = parse_provider(&provider)?;
    let kind: AdapterKind = p.0;

    // Build a ProviderConfig (endpoint + auth) the way the chat loop resolves it:
    // Ollama uses its local endpoint and no key; cloud uses the keychain/env key.
    let auth = config::api_key(p).map(AuthData::from_single);
    let endpoint = p.default_endpoint().map(Endpoint::from_owned);

    let client = genai::Client::default();
    let provider_config = genai::ModelIden::new(kind, "")  // placeholder model
        .into();
    // Prefer explicit endpoint/auth when we have them.
    let provider_config = genai::adapter::inter::ProviderConfig { endpoint, auth };
    let _ = (ModelIden::new(kind, ""), provider_config);

    client
        .all_model_names(kind, genai::adapter::inter::ProviderConfig {
            endpoint: p.default_endpoint().map(Endpoint::from_owned),
            auth: config::api_key(p).map(AuthData::from_single),
        })
        .await
        .map_err(|e| e.to_string())
}
```

> **Implementer note (verify against genai 0.6.4):** `all_model_names(adapter_kind, provider_config: impl Into<ProviderConfig>)` is confirmed (`client_impl.rs:26`), where `ProviderConfig { endpoint: Option<Endpoint>, auth: Option<AuthData> }`. Resolve the exact import path of `ProviderConfig` with:
> `grep -rn "pub struct ProviderConfig" ~/.cargo/registry/src/*/genai-0.6.4/src`
> Replace the placeholder/dead lines above with a single clean construction:
> ```rust
> let provider_config = <ProviderConfig>::from((endpoint, auth)); // or { endpoint, auth }
> client.all_model_names(kind, provider_config).await.map_err(|e| e.to_string())
> ```
> Delete the `ModelIden`/placeholder scaffolding — it is only there to show intent. The final body must compile with no unused bindings.

- [ ] **Step 6: Register the command in `lib.rs`**

In the `tauri::generate_handler![...]` list near line 104, add:
```rust
            ipc::config::list_models,
```

- [ ] **Step 7: Compile the backend**

Run: `cargo build -p zanto-desktop 2>&1 | tail -30`
Expected: clean build. Fix any `ProviderConfig` import-path errors in `list_models` per the implementer note until green.

- [ ] **Step 8: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/ipc/mod.rs crates/zanto-desktop/src-tauri/src/ipc/config.rs crates/zanto-desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): registry-driven config IPC + list_models + generation patch"
```

---

## Task 5: Populate `ChatConfig.generation` from Settings (desktop)

**Files:**
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/chat.rs` (`ChatConfig { ... }` literal ~174)

**Interfaces:**
- Consumes: `ChatConfig.generation` (Task 3), `Settings` (Task 2).

- [ ] **Step 1: Set the field in the `ChatConfig` literal**

In the `ChatConfig { ... }` construction (~line 174), add:
```rust
                generation: zanto_core::config::Settings::load().generation,
```
(Match the existing import style in this file; if `Settings` is already imported, use `Settings::load().generation`.)

- [ ] **Step 2: Compile**

Run: `cargo build -p zanto-desktop 2>&1 | tail -20`
Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/ipc/chat.rs
git commit -m "feat(desktop): pass configured generation params into chat turns"
```

---

## Task 6: Frontend IPC types + `listModels`

**Files:**
- Modify: `crates/zanto-desktop/src/lib/ipc.ts` (types ~60–93; `ipc` object ~170–244)

**Interfaces:**
- Produces: `ProviderInfo`, `GenerationParams` TS types; `Config.provider_registry`, `Config.generation`; `ConfigPatch.generation`; `ipc.listModels`.

- [ ] **Step 1: Add/extend types**

```ts
export type ProviderDto = {
  provider: string;
  model: string;
  endpoint: string | null;
  has_key: boolean;
  label: string;
  needs_key: boolean;
  default_endpoint: string | null;
};

export type ProviderInfo = {
  id: string;
  label: string;
  needs_key: boolean;
  default_endpoint: string | null;
};

export type GenerationParams = {
  temperature?: number | null;
  max_tokens?: number | null;
  top_p?: number | null;
  reasoning_effort?: string | null;
  seed?: number | null;
  stop_sequences?: string[];
  extra_body?: unknown;
};
```

Add to `Config`:
```ts
  provider_registry: ProviderInfo[];
  generation: GenerationParams;
```

Change `ConfigPatch`:
```ts
export type ConfigPatch = Partial<Pick<Config, "model" | "endpoint" | "max_context_turns">> & {
  providers?: ProviderPatch[];
  active_provider?: string;
  generation?: GenerationParams;
};
```

- [ ] **Step 2: Add the `listModels` method**

In the `ipc` object, next to `getConfig`:
```ts
  listModels: (provider: string) => invoke<string[]>("list_models", { provider }),
```

- [ ] **Step 3: Type-check**

Run: `cd crates/zanto-desktop && npm run check 2>&1 | tail -20` (or `npx svelte-check --tsconfig ./tsconfig.json`)
Expected: no new type errors in `ipc.ts`. (Errors in `SettingsDialog.svelte` are expected until Task 7.)

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/ipc.ts
git commit -m "feat(desktop/ui): ipc types for provider registry + generation + listModels"
```

---

## Task 7: SettingsDialog — dynamic provider select, model combobox, Generation section

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte`

**Interfaces:**
- Consumes: `Config.provider_registry`, `ProviderDto.label/needs_key/default_endpoint`, `Config.generation`, `ipc.listModels`, `ipc.setConfig` (Tasks 4–6).

- [ ] **Step 1: Drive the provider select from the registry**

Delete the hardcoded `providerLabels` map (lines 164–169) and the `activeProviderLabel` derivation that uses it. Replace with registry lookups:
```ts
  const registry = $derived(appStore.config?.provider_registry ?? []);
  function providerLabel(id: string): string {
    return registry.find((r) => r.id === id)?.label ?? id;
  }
  const activeProviderLabel = $derived(providerLabel(activeProvider));
```
In the provider `<Select.Content>`, iterate the registry instead of `appStore.config.providers`:
```svelte
            {#each registry as r (r.id)}
              <Select.Item value={r.id} label={r.label} />
            {/each}
```

- [ ] **Step 2: Replace the literal `"ollama"` endpoint branch with capability checks**

Compute the active registry entry and use `needs_key` / `default_endpoint`:
```ts
  const activeInfo = $derived(registry.find((r) => r.id === activeProvider) ?? null);
```
Change `{#if activeProvider === "ollama"}` to `{#if activeInfo && !activeInfo.needs_key}` for the endpoint block, and `{:else}` (API-key block) stays for `needs_key` providers. Use `activeInfo?.default_endpoint ?? "http://localhost:11434/"` as the endpoint placeholder.

- [ ] **Step 3: Add model list state + Refresh**

Add state and a loader:
```ts
  let modelList = $state<string[]>([]);
  let modelsLoading = $state(false);
  let modelsError = $state("");

  async function refreshModels() {
    if (!activeProvider) return;
    modelsLoading = true;
    modelsError = "";
    try {
      modelList = await ipc.listModels(activeProvider);
    } catch (e) {
      modelList = [];
      modelsError = "Couldn't load models — type the name manually.";
    } finally {
      modelsLoading = false;
    }
  }
```
Reset `modelList = []; modelsError = ""` inside the existing provider-change `$effect` (where `resetKeyState()` is called) so a stale list never shows for the wrong provider.

- [ ] **Step 4: Turn the model field into a combobox**

Keep the existing free-text `<Input>` for `model` (custom entry stays possible). Add, directly under it, a Refresh button and a native datalist/dropdown of fetched names:
```svelte
          <div class="flex gap-2 items-center">
            <Input
              id="cfg-prov-model"
              class="font-mono flex-1 focus-visible:ring-2 focus-visible:ring-ring"
              list="cfg-model-options"
              value={activeProviderPatch()?.model ?? ""}
              oninput={(e) => setActiveModel((e.target as HTMLInputElement).value)}
              placeholder="model name"
            />
            <Button size="sm" variant="outline" onclick={refreshModels} disabled={modelsLoading}>
              {modelsLoading ? "Loading…" : "Refresh"}
            </Button>
          </div>
          <datalist id="cfg-model-options">
            {#each modelList as m (m)}<option value={m}></option>{/each}
          </datalist>
          {#if modelsError}
            <p class="text-xs text-muted-foreground">{modelsError}</p>
          {/if}
```

- [ ] **Step 5: Add the Generation section**

Add state seeded from config in the `open` effect (alongside `contextTurns`):
```ts
  let gen = $state<GenerationParams>({});
```
In the `$effect` that runs on `open`:
```ts
      gen = { ...(appStore.config.generation ?? {}) };
```
Add a save handler:
```ts
  async function saveGeneration() {
    try {
      await ipc.setConfig({ generation: gen });
      await refreshConfig();
      toast.success("Generation settings saved");
    } catch (e) {
      toast.error("Could not save generation settings", { description: `${e}` });
    }
  }
```
Add a new `<section>` after the Context section:
```svelte
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Generation</h3>
        <div class="grid grid-cols-2 gap-3">
          <label class="space-y-1.5 text-xs text-muted-foreground">Temperature
            <Input type="number" step="0.1" min="0" bind:value={gen.temperature} class="font-mono" />
          </label>
          <label class="space-y-1.5 text-xs text-muted-foreground">Max tokens
            <Input type="number" step="1" min="1" bind:value={gen.max_tokens} class="font-mono" />
          </label>
          <label class="space-y-1.5 text-xs text-muted-foreground">Top-p
            <Input type="number" step="0.05" min="0" max="1" bind:value={gen.top_p} class="font-mono" />
          </label>
          <label class="space-y-1.5 text-xs text-muted-foreground">Seed
            <Input type="number" step="1" bind:value={gen.seed} class="font-mono" />
          </label>
        </div>
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-reasoning-label">Reasoning effort</span>
          <Select.Root type="single" bind:value={gen.reasoning_effort}>
            <Select.Trigger class="w-full" aria-labelledby="cfg-reasoning-label">
              {gen.reasoning_effort ?? "default"}
            </Select.Trigger>
            <Select.Content>
              {#each ["none","minimal","low","medium","high","xhigh"] as e (e)}
                <Select.Item value={e} label={e} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
        <Button size="sm" onclick={saveGeneration}>Save generation</Button>
        <p class="text-xs text-muted-foreground">
          Empty fields use the provider default. Unsupported options are ignored per provider.
        </p>
      </section>
```
Import `GenerationParams` from `$lib/ipc` at the top with the other type imports.

> Numeric `<Input bind:value>` yields `""` when cleared; before sending, coerce empties to omit them. In `saveGeneration`, normalize: `const clean = Object.fromEntries(Object.entries(gen).filter(([,v]) => v !== "" && v != null)); await ipc.setConfig({ generation: clean });`. (Stop-sequences and `extra_body` are out of this v1 UI; the fields exist in the type for the backend but are not edited here.)

- [ ] **Step 6: Type-check + build**

Run: `cd crates/zanto-desktop && npm run check 2>&1 | tail -20`
Expected: no type errors.

- [ ] **Step 7: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/SettingsDialog.svelte
git commit -m "feat(desktop/ui): dynamic provider select, model combobox, generation section"
```

---

## Task 8: End-to-end manual verification (per CLAUDE.md)

**Files:** none (verification only).

- [ ] **Step 1: Full build**

Run: `cargo build 2>&1 | tail -10`
Expected: both crates compile clean.

- [ ] **Step 2: Run the app**

Run the desktop app (`cargo tauri dev` or the project's run command). Open Settings.

- [ ] **Step 3: Verify the scenarios**

- [ ] Provider dropdown lists all 10 curated providers with proper labels (xAI, Together AI, etc.).
- [ ] Select a provider with a saved key → click **Refresh** → real model names populate the datalist; pick one; **Save changes**; send a message end-to-end and confirm a normal streamed reply.
- [ ] Select a provider with no key → **Refresh** shows the graceful "type the name manually" hint; typing a model still saves and works.
- [ ] Switch providers → the model list, key field, and any confirm banner reset (no cross-provider bleed).
- [ ] Set Temperature = 0.2 and Reasoning effort = high → **Save generation** → send a message; confirm the turn streams with no provider error.
- [ ] Quit and relaunch with an existing settings.json containing `"open_ai"` (hand-edit one in if needed) → it resolves to OpenAI, not an error/reset.

- [ ] **Step 4: Run the full test suite**

Run: `cargo test 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 5: Commit any fixes, then summarize**

```bash
git add -A && git commit -m "test: verify dynamic provider settings end-to-end"
```

---

## Self-Review

**Spec coverage:**
- A (dynamic registry): Tasks 1 (newtype + registry + migration), 4 (IPC + parse_provider + default_providers), 7 (UI select). ✓
- B (live model dropdown): Tasks 4 (`list_models`), 6 (`listModels`), 7 (combobox + Refresh + fallback). ✓
- C (generation params): Tasks 2 (`GenerationParams` + apply), 3 (chat wiring), 4 (IPC carry), 5 (desktop populate), 7 (UI section). ✓
- Migration `open_ai`→`openai`: Task 1 (`provider_from_id`, `de_providers`, `de_active_provider`) + test. ✓
- `model_context_window` default arm for new providers: Task 1 Step 4. ✓
- Non-goals (per-provider params, custom endpoint, tool_choice UI): excluded. ✓

**Placeholder scan:** The only intentional non-final code is the `list_models` body in Task 4 Step 5, explicitly flagged with a verify-the-import-path note and a clean target form — the genai `all_model_names` signature and `ProviderConfig { endpoint, auth }` shape are confirmed from source; only the module path needs a one-line grep to pin. No other placeholders.

**Type consistency:** `Provider(AdapterKind)` used uniformly; `provider_from_id` is the single parse entry point (used by serde, `de_providers`, `de_active_provider`, and `parse_provider`). `GenerationParams` is the one shared type across core/IPC/TS. `ProviderInfo` fields (`id/label/needs_key/default_endpoint`) match between Rust and TS. `ChatConfig.generation` set in all three construction paths (`new`, desktop chat literal — CLI via `new`).
