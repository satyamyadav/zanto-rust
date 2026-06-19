use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PROJECT_CONFIG: &str = ".zanto/settings.json";

/// Keyring service name under which API keys are stored.
const KEYRING_SERVICE: &str = "zanto";

// Re-exported so downstream crates (the desktop app) can name provider kinds
// without depending on genai directly.
pub use genai::adapter::AdapterKind;

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
    let kind = AdapterKind::from_model(model).unwrap_or(AdapterKind::Ollama);
    // genai may classify some OpenAI models (e.g. gpt-5*) to the OpenAIResp
    // adapter, whose lowercase id differs from "openai". Our keychain username
    // is derived from this id, and OpenAI keys are stored under "openai", so
    // collapse the Responses variant to OpenAI to keep auth resolution stable.
    // (genai still selects the correct adapter when executing the request.)
    let kind = match kind {
        AdapterKind::OpenAIResp => AdapterKind::OpenAI,
        other => other,
    };
    Provider(kind)
}

/// Approximate context-window size (in tokens) for a model, by provider, used to
/// keep the live conversation within budget (`ContextPolicy::Auto`). These are
/// deliberately conservative round numbers, not exact per-model limits — the auto
/// policy only needs a ballpark, and a Settings override exists for outliers
/// (notably local Ollama models, whose windows vary widely).
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
    /// Field-wise overlay: each value set in `other` overrides `self`; unset
    /// fields in `other` keep `self`'s value. Used to layer project config over
    /// user config.
    pub fn overlay(mut self, other: GenerationParams) -> GenerationParams {
        if other.temperature.is_some() { self.temperature = other.temperature; }
        if other.max_tokens.is_some() { self.max_tokens = other.max_tokens; }
        if other.top_p.is_some() { self.top_p = other.top_p; }
        if other.reasoning_effort.is_some() { self.reasoning_effort = other.reasoning_effort; }
        if other.seed.is_some() { self.seed = other.seed; }
        if !other.stop_sequences.is_empty() { self.stop_sequences = other.stop_sequences; }
        if other.extra_body.is_some() { self.extra_body = other.extra_body; }
        self
    }

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

/// A context source (file or dir) fed to the assistant, with an enable toggle.
///
/// Serializes as `{ "path": "...", "enabled": true }`. Deserialization is
/// back-compat: a bare JSON string (the old `Vec<String>` shape) parses as an
/// enabled source. See the `Deserialize` impl below.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextSource {
    pub path: String,
    pub enabled: bool,
}

impl<'de> Deserialize<'de> for ContextSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        /// Accepts both the old `"path"` string and the new `{path,enabled}` object.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Shape {
            Bare(String),
            Full {
                path: String,
                #[serde(default = "default_true")]
                enabled: bool,
            },
        }
        Ok(match Shape::deserialize(deserializer)? {
            Shape::Bare(path) => ContextSource { path, enabled: true },
            Shape::Full { path, enabled } => ContextSource { path, enabled },
        })
    }
}

fn default_true() -> bool {
    true
}

/// A single provider's active model and optional endpoint override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: Provider,
    /// Active model for this provider.
    pub model: String,
    /// Endpoint override (Ollama needs it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Deserialize the provider list, silently dropping entries whose id genai does
/// not recognize (rather than failing the whole settings file). Unknown ids are
/// dropped on load and — because `set_config` re-saves — the drop is persisted
/// to disk on the next save. This is intentional: stale ids don't accumulate.
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

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub allow_read_outside: bool,
    #[serde(default)]
    pub allow_write_outside: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_turns: Option<usize>,
    /// Optional override for the automatic context window (tokens). When unset, the
    /// window is inferred from the active model via `model_context_window`. Useful
    /// for local models whose true window isn't in the lookup table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<usize>,
    /// Per-provider model + endpoint configs.
    #[serde(default, deserialize_with = "de_providers")]
    pub providers: Vec<ProviderConfig>,
    /// The selected provider; when set, its `ProviderConfig` is the effective active one.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_active_provider")]
    pub active_provider: Option<Provider>,
    /// Root directory of the active project (canonicalized on load).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    /// Extra context sources (files/dirs) fed to the assistant. Each carries an
    /// `enabled` toggle; `load_context` honors only enabled ones. Back-compat:
    /// an old `["/a","/b"]` list deserializes as enabled sources.
    #[serde(default)]
    pub context_sources: Vec<ContextSource>,
    /// The user-selected skill (file stem) appended to the system prompt each
    /// turn. Persisted so the choice survives an app restart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_skill: Option<String>,
    /// Global generation parameters applied to every turn.
    #[serde(default)]
    pub generation: GenerationParams,
}

impl Settings {
    pub fn load() -> Self {
        Self::ensure_project_config();
        let user = Self::load_file(Self::user_path()).unwrap_or_default();
        let project = Self::load_file(PathBuf::from(PROJECT_CONFIG)).unwrap_or_default();
        user.merge(project).resolve_paths()
    }

    /// Persist an absolute path into the project config's allowed_paths.
    pub fn persist_allowed_path(abs_path: &str) {
        let config_path = Path::new(PROJECT_CONFIG);
        let mut settings = Self::load_file(config_path.to_path_buf()).unwrap_or_default();
        if !settings.allowed_paths.contains(&abs_path.to_string()) {
            settings.allowed_paths.push(abs_path.to_string());
            if let Ok(content) = serde_json::to_string_pretty(&settings) {
                let _ = std::fs::write(config_path, content);
            }
        }
    }

    /// Persist this Settings to the project config (`.zanto/settings.json`).
    pub fn save(&self) -> std::io::Result<()> {
        let path = Path::new(PROJECT_CONFIG);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, content)
    }

    /// The effective active `(provider, model, endpoint)`.
    ///
    /// Prefers `active_provider` + its matching `ProviderConfig`; otherwise falls
    /// back to the legacy `model`/`endpoint` fields (inferring the provider from
    /// the model prefix, defaulting to Ollama).
    pub fn active(&self) -> (Provider, String, Option<String>) {
        if let Some(active) = self.active_provider {
            if let Some(pc) = self.providers.iter().find(|p| p.provider == active) {
                return (pc.provider, pc.model.clone(), pc.endpoint.clone());
            }
        }
        match &self.model {
            Some(model) => (provider_of(model), model.clone(), self.endpoint.clone()),
            None => (Provider(AdapterKind::Ollama), String::new(), self.endpoint.clone()),
        }
    }

    fn ensure_project_config() {
        let path = Path::new(PROJECT_CONFIG);
        if !path.exists() {
            let _ = std::fs::create_dir_all(".zanto");
            let default = serde_json::to_string_pretty(&Self::default()).unwrap_or_default();
            let _ = std::fs::write(path, default);
        }
    }

    fn user_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            });
        base.join("zanto/settings.json")
    }

    fn load_file(path: PathBuf) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn resolve_paths(mut self) -> Self {
        self.allowed_paths = self
            .allowed_paths
            .into_iter()
            .map(|p| {
                std::fs::canonicalize(&p)
                    .unwrap_or_else(|_| PathBuf::from(&p))
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        self.project_dir = self.project_dir.map(|p| {
            std::fs::canonicalize(&p)
                .unwrap_or_else(|_| PathBuf::from(&p))
                .to_string_lossy()
                .into_owned()
        });
        self
    }

    fn merge(mut self, other: Self) -> Self {
        self.allowed_paths.extend(other.allowed_paths);
        if other.allow_read_outside {
            self.allow_read_outside = true;
        }
        if other.allow_write_outside {
            self.allow_write_outside = true;
        }
        if other.model.is_some() {
            self.model = other.model;
        }
        if other.endpoint.is_some() {
            self.endpoint = other.endpoint;
        }
        if other.max_context_turns.is_some() {
            self.max_context_turns = other.max_context_turns;
        }
        if other.context_window_tokens.is_some() {
            self.context_window_tokens = other.context_window_tokens;
        }
        self.providers.extend(other.providers);
        if other.active_provider.is_some() {
            self.active_provider = other.active_provider;
        }
        if other.project_dir.is_some() {
            self.project_dir = other.project_dir;
        }
        self.context_sources.extend(other.context_sources);
        self.generation = std::mem::take(&mut self.generation).overlay(other.generation);
        self
    }
}

// ---- Model listing ----

/// List model names a provider exposes, using the saved key/endpoint.
/// Errors (missing key, offline, no list endpoint) are returned as `Err`.
pub async fn list_models(p: Provider) -> Result<Vec<String>, String> {
    use genai::resolver::{AuthData, Endpoint, ProviderConfig};
    let auth = api_key(p).map(AuthData::from_single);
    let endpoint = p.default_endpoint().map(Endpoint::from_owned);
    genai::Client::default()
        .all_model_names(p.0, ProviderConfig { endpoint, auth })
        .await
        .map_err(|e| e.to_string())
}

// ---- API keys (OS keychain with env-var fallback) ----

/// The keyring entry for a provider, or an error string if the entry could not
/// be constructed (e.g. no secret service backend available).
fn key_entry(p: Provider) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, p.as_str()).map_err(|e| e.to_string())
}

/// Read the API key for a provider: keychain first, then env-var fallback.
/// Returns `None` for providers that need no key (Ollama) when none is set.
pub fn api_key(p: Provider) -> Option<String> {
    if let Ok(entry) = key_entry(p) {
        if let Ok(key) = entry.get_password() {
            if !key.is_empty() {
                return Some(key);
            }
        }
    }
    p.env_var()
        .and_then(|var| std::env::var(var).ok())
        .filter(|k| !k.is_empty())
}

/// Store the API key for a provider in the OS keychain. Returns `Err` gracefully
/// (no panic) when no secret service is available.
pub fn set_api_key(p: Provider, key: &str) -> Result<(), String> {
    key_entry(p)?.set_password(key).map_err(|e| e.to_string())
}

/// Remove the API key for a provider from the OS keychain.
pub fn clear_api_key(p: Provider) -> Result<(), String> {
    let entry = key_entry(p)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        // Deleting a non-existent entry is a no-op success.
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Whether an API key is available for a provider (keychain or env fallback).
pub fn has_api_key(p: Provider) -> bool {
    api_key(p).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use genai::adapter::AdapterKind;

    #[test]
    fn provider_serde_roundtrips_lower_str() {
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
        assert_eq!(p, Provider(AdapterKind::OpenAI));
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
        assert_eq!(s.providers[0].provider, Provider(AdapterKind::Anthropic));
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

    #[test]
    fn model_context_window_by_provider() {
        assert_eq!(model_context_window("claude-opus-4-8"), 200_000);
        assert_eq!(model_context_window("gpt-4o"), 128_000);
        assert_eq!(model_context_window("gemini-1.5-pro"), 1_000_000);
        // Unknown / local model → conservative small window.
        assert_eq!(model_context_window("llama3"), 8_192);
    }

    #[test]
    fn settings_serde_round_trip_with_providers() {
        let settings = Settings {
            model: Some("llama3".to_string()),
            providers: vec![
                ProviderConfig {
                    provider: Provider(AdapterKind::Ollama),
                    model: "llama3".to_string(),
                    endpoint: Some("http://localhost:11434".to_string()),
                },
                ProviderConfig {
                    provider: Provider(AdapterKind::Gemini),
                    model: "gemini-2.0-flash".to_string(),
                    endpoint: None,
                },
            ],
            active_provider: Some(Provider(AdapterKind::Gemini)),
            project_dir: Some("/tmp/project".to_string()),
            context_sources: vec![ContextSource {
                path: "notes.md".to_string(),
                enabled: true,
            }],
            ..Default::default()
        };

        let json = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(back.providers.len(), 2);
        assert_eq!(back.active_provider, Some(Provider(AdapterKind::Gemini)));
        assert_eq!(back.project_dir.as_deref(), Some("/tmp/project"));
        assert_eq!(
            back.context_sources,
            vec![ContextSource {
                path: "notes.md".to_string(),
                enabled: true,
            }]
        );
        // lowercase id for the provider newtype.
        assert!(json.contains("\"gemini\""));
    }

    #[test]
    fn context_sources_back_compat_deserialize() {
        // Old shape: a flat list of path strings → all enabled.
        let old = r#"{ "context_sources": ["/a", "/b"] }"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(
            s.context_sources,
            vec![
                ContextSource { path: "/a".to_string(), enabled: true },
                ContextSource { path: "/b".to_string(), enabled: true },
            ]
        );

        // New shape: objects with explicit `enabled`.
        let new = r#"{ "context_sources": [
            { "path": "/a", "enabled": true },
            { "path": "/b", "enabled": false }
        ] }"#;
        let s: Settings = serde_json::from_str(new).unwrap();
        assert_eq!(
            s.context_sources,
            vec![
                ContextSource { path: "/a".to_string(), enabled: true },
                ContextSource { path: "/b".to_string(), enabled: false },
            ]
        );
    }

    #[test]
    fn active_prefers_active_provider_config() {
        let settings = Settings {
            model: Some("llama3".to_string()),
            endpoint: Some("http://legacy".to_string()),
            providers: vec![ProviderConfig {
                provider: Provider(AdapterKind::Gemini),
                model: "gemini-2.0-flash".to_string(),
                endpoint: None,
            }],
            active_provider: Some(Provider(AdapterKind::Gemini)),
            ..Default::default()
        };

        let (provider, model, endpoint) = settings.active();
        assert_eq!(provider, Provider(AdapterKind::Gemini));
        assert_eq!(model, "gemini-2.0-flash");
        assert_eq!(endpoint, None);
    }

    #[test]
    fn active_falls_back_to_legacy_fields() {
        let settings = Settings {
            model: Some("gemini-2.0-flash".to_string()),
            endpoint: Some("http://legacy".to_string()),
            ..Default::default()
        };

        let (provider, model, endpoint) = settings.active();
        assert_eq!(provider, Provider(AdapterKind::Gemini));
        assert_eq!(model, "gemini-2.0-flash");
        assert_eq!(endpoint.as_deref(), Some("http://legacy"));
    }

    #[test]
    fn active_defaults_to_ollama_without_model() {
        let settings = Settings::default();
        let (provider, model, _) = settings.active();
        assert_eq!(provider, Provider(AdapterKind::Ollama));
        assert!(model.is_empty());
    }

    #[test]
    fn provider_of_prefix_map() {
        assert_eq!(provider_of("gemini-2.0-flash"), Provider(AdapterKind::Gemini));
        assert_eq!(provider_of("claude-opus-4"), Provider(AdapterKind::Anthropic));
        assert_eq!(provider_of("gpt-4o"), Provider(AdapterKind::OpenAI));
        assert_eq!(provider_of("o1-mini"), Provider(AdapterKind::OpenAI));
        assert_eq!(provider_of("llama3"), Provider(AdapterKind::Ollama));
        assert_eq!(provider_of("qwen2.5"), Provider(AdapterKind::Ollama));
    }

    #[test]
    fn provider_of_collapses_openai_resp_to_openai() {
        // gpt-5* classifies to OpenAIResp in genai 0.6.4 (adapter_kind.rs:297-300);
        // we normalize so the keychain username stays "openai".
        assert_eq!(provider_of("gpt-5").as_str(), "openai");
        assert_eq!(provider_of("gpt-4o").as_str(), "openai");
    }

    #[test]
    fn api_key_env_fallback() {
        // No keychain entry expected in a headless test environment, so the env
        // var is the only source. Use OpenAI to avoid clobbering other tests.
        let var = "OPENAI_API_KEY";
        let prev = std::env::var(var).ok();
        unsafe { std::env::set_var(var, "sk-test-123") };

        assert_eq!(api_key(Provider(AdapterKind::OpenAI)).as_deref(), Some("sk-test-123"));
        assert!(has_api_key(Provider(AdapterKind::OpenAI)));

        match prev {
            Some(v) => unsafe { std::env::set_var(var, v) },
            None => unsafe { std::env::remove_var(var) },
        }
    }

    #[test]
    fn api_key_none_for_ollama() {
        assert_eq!(api_key(Provider(AdapterKind::Ollama)), None);
        assert!(!has_api_key(Provider(AdapterKind::Ollama)));
    }

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
        assert_eq!(opts.stop_sequences, vec!["STOP".to_string()]);
    }

    #[test]
    fn merge_overlays_generation_params() {
        let mut user = Settings::default();
        user.generation.temperature = Some(0.2);
        let mut project = Settings::default();
        project.generation.max_tokens = Some(500);
        let merged = user.merge(project);
        assert_eq!(merged.generation.temperature, Some(0.2)); // preserved from user
        assert_eq!(merged.generation.max_tokens, Some(500));  // overridden by project
    }

    #[test]
    fn generation_params_default_is_empty() {
        let gp = GenerationParams::default();
        assert!(gp.temperature.is_none());
        assert!(gp.stop_sequences.is_empty());
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(s.generation.temperature.is_none());
    }
}
