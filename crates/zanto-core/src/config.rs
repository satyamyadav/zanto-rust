use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PROJECT_CONFIG: &str = ".zanto/settings.json";

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
}

impl Settings {
    pub fn load() -> Self {
        Self::ensure_project_config();
        let user = Self::load_file(Self::user_path()).unwrap_or_default();
        let project = Self::load_file(PathBuf::from(PROJECT_CONFIG)).unwrap_or_default();
        user.merge(project).resolve_paths()
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
        self
    }
}
