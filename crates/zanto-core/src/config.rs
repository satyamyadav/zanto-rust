use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PROJECT_CONFIG: &str = ".zanto/settings.json";

/// Keyring service name under which API keys are stored.
const KEYRING_SERVICE: &str = "zanto";

/// An LLM provider. Each provider has its own model + auth scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Ollama,
    Gemini,
    Anthropic,
    OpenAI,
}

impl Provider {
    /// Snake-case identifier; used as the keyring username and env-var basis.
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::Gemini => "gemini",
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
        }
    }

    /// Environment variable consulted as a fallback for this provider's API key.
    /// `None` for providers that need no key (Ollama).
    fn env_var(self) -> Option<&'static str> {
        match self {
            Provider::Ollama => None,
            Provider::Gemini => Some("GEMINI_API_KEY"),
            Provider::Anthropic => Some("ANTHROPIC_API_KEY"),
            Provider::OpenAI => Some("OPENAI_API_KEY"),
        }
    }
}

/// Infer the provider for a model name from its prefix. Defaults to Ollama.
pub fn provider_of(model: &str) -> Provider {
    if model.starts_with("gemini-") {
        Provider::Gemini
    } else if model.starts_with("claude-") {
        Provider::Anthropic
    } else if model.starts_with("gpt-") || model.starts_with("o1") {
        Provider::OpenAI
    } else {
        Provider::Ollama
    }
}

/// Approximate context-window size (in tokens) for a model, by provider, used to
/// keep the live conversation within budget (`ContextPolicy::Auto`). These are
/// deliberately conservative round numbers, not exact per-model limits — the auto
/// policy only needs a ballpark, and a Settings override exists for outliers
/// (notably local Ollama models, whose windows vary widely).
pub fn model_context_window(model: &str) -> usize {
    match provider_of(model) {
        Provider::Anthropic => 200_000,
        Provider::Gemini => 1_000_000,
        Provider::OpenAI => 128_000,
        // Local models vary; assume a small window unless the user overrides it.
        Provider::Ollama => 8_192,
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
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    /// The selected provider; when set, its `ProviderConfig` is the effective active one.
    #[serde(skip_serializing_if = "Option::is_none")]
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
            None => (Provider::Ollama, String::new(), self.endpoint.clone()),
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
        self
    }
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
                    provider: Provider::Ollama,
                    model: "llama3".to_string(),
                    endpoint: Some("http://localhost:11434".to_string()),
                },
                ProviderConfig {
                    provider: Provider::Gemini,
                    model: "gemini-2.0-flash".to_string(),
                    endpoint: None,
                },
            ],
            active_provider: Some(Provider::Gemini),
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
        assert_eq!(back.active_provider, Some(Provider::Gemini));
        assert_eq!(back.project_dir.as_deref(), Some("/tmp/project"));
        assert_eq!(
            back.context_sources,
            vec![ContextSource {
                path: "notes.md".to_string(),
                enabled: true,
            }]
        );
        // snake_case rename for the provider enum.
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
                provider: Provider::Gemini,
                model: "gemini-2.0-flash".to_string(),
                endpoint: None,
            }],
            active_provider: Some(Provider::Gemini),
            ..Default::default()
        };

        let (provider, model, endpoint) = settings.active();
        assert_eq!(provider, Provider::Gemini);
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
        assert_eq!(provider, Provider::Gemini);
        assert_eq!(model, "gemini-2.0-flash");
        assert_eq!(endpoint.as_deref(), Some("http://legacy"));
    }

    #[test]
    fn active_defaults_to_ollama_without_model() {
        let settings = Settings::default();
        let (provider, model, _) = settings.active();
        assert_eq!(provider, Provider::Ollama);
        assert!(model.is_empty());
    }

    #[test]
    fn provider_of_prefix_map() {
        assert_eq!(provider_of("gemini-2.0-flash"), Provider::Gemini);
        assert_eq!(provider_of("claude-opus-4"), Provider::Anthropic);
        assert_eq!(provider_of("gpt-4o"), Provider::OpenAI);
        assert_eq!(provider_of("o1-mini"), Provider::OpenAI);
        assert_eq!(provider_of("llama3"), Provider::Ollama);
        assert_eq!(provider_of("qwen2.5"), Provider::Ollama);
    }

    #[test]
    fn api_key_env_fallback() {
        // No keychain entry expected in a headless test environment, so the env
        // var is the only source. Use OpenAI to avoid clobbering other tests.
        let var = "OPENAI_API_KEY";
        let prev = std::env::var(var).ok();
        unsafe { std::env::set_var(var, "sk-test-123") };

        assert_eq!(api_key(Provider::OpenAI).as_deref(), Some("sk-test-123"));
        assert!(has_api_key(Provider::OpenAI));

        match prev {
            Some(v) => unsafe { std::env::set_var(var, v) },
            None => unsafe { std::env::remove_var(var) },
        }
    }

    #[test]
    fn api_key_none_for_ollama() {
        assert_eq!(api_key(Provider::Ollama), None);
        assert!(!has_api_key(Provider::Ollama));
    }
}
