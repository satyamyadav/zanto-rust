//! Micro-app framework (desktop-only). An `App` is a full-stack module: a Rust
//! backend slice (this trait) + a Svelte frontend slice + a manifest. Apps are NOT
//! part of zanto-core; the core is parameterized at runtime by the active app's
//! profile (skill + tools + stores) via `ActiveDispatcher`.

use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use zanto_core::chat::{AppDispatcher, AppResult, GenaiTool};
use zanto_core::data::DataStore;

/// A component an app can render in chat / the right panel. The agent fills `data`
/// conforming to `schema`; the frontend renders the Svelte component registered
/// under `id`.
#[derive(Debug, Clone, Serialize)]
pub struct ComponentDecl {
    pub id: String,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stores: Vec<String>,
    pub components: Vec<ComponentDecl>,
}

/// A micro-app's backend. Agentic path: `agent_tools` + `dispatch_tool`. Manual
/// path (ungated, called directly over IPC by the Svelte views): `query` / `action`.
pub trait App: Send + Sync {
    fn manifest(&self) -> &AppManifest;
    /// System-prompt extension injected when this app is active.
    fn skill(&self) -> String;
    /// Agent tool schemas offered to the model when this app is active.
    fn agent_tools(&self) -> Vec<GenaiTool>;
    /// Execute an agent tool. Returns `None` if `name` is not this app's tool.
    fn dispatch_tool(&self, data: &DataStore, name: &str, args: Value) -> Option<Result<AppResult, String>>;
    /// Manual read-only query (ungated backend path).
    fn query(&self, data: &DataStore, name: &str, args: Value) -> Result<Value, String>;
    /// Manual action / flow (ungated backend path).
    fn action(&self, data: &DataStore, name: &str, args: Value) -> Result<Value, String>;
    /// Whether the agent gets the built-in fs/shell tools instead of app tools.
    /// The general "Chat" app returns true; verticals return false (default).
    fn uses_base_tools(&self) -> bool {
        false
    }
}

/// Registry of available apps + the single active one (single-active + general mode).
pub struct AppRegistry {
    apps: Vec<Arc<dyn App>>,
    active: Mutex<Option<String>>,
}

impl AppRegistry {
    pub fn new(apps: Vec<Arc<dyn App>>) -> Self {
        Self { apps, active: Mutex::new(None) }
    }

    pub fn manifests(&self) -> Vec<AppManifest> {
        self.apps.iter().map(|a| a.manifest().clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn App>> {
        self.apps.iter().find(|a| a.manifest().id == id).cloned()
    }

    pub fn mount(&self, id: &str) -> Result<(), String> {
        if self.get(id).is_some() {
            *self.active.lock().unwrap() = Some(id.to_string());
            Ok(())
        } else {
            Err(format!("unknown app: {id}"))
        }
    }

    pub fn unmount(&self) {
        *self.active.lock().unwrap() = None;
    }

    pub fn active(&self) -> Option<Arc<dyn App>> {
        let id = self.active.lock().unwrap().clone();
        id.and_then(|id| self.get(&id))
    }
}

/// Bridges the active app to the core chat loop's `AppDispatcher`. Holds the data
/// engine so app tools can read/write stores. Constructed per turn from the active app.
pub struct ActiveDispatcher {
    app: Arc<dyn App>,
    data: Arc<DataStore>,
}

impl ActiveDispatcher {
    pub fn new(app: Arc<dyn App>, data: Arc<DataStore>) -> Self {
        Self { app, data }
    }
}

#[async_trait]
impl AppDispatcher for ActiveDispatcher {
    async fn dispatch(&self, name: &str, args: Value) -> Option<Result<AppResult, String>> {
        self.app.dispatch_tool(&self.data, name, args)
    }
}
