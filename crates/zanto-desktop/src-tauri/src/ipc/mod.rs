//! Tauri IPC surface — domain submodules share `DesktopState` + shared types
//! defined here. `lib.rs` registers all commands.

pub mod apps;
pub mod chat;
pub mod config;
pub mod files;
pub mod session;

use std::sync::{Arc, Mutex as StdMutex};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zanto_core::data::DataStore;
use zanto_core::permissions::PermissionGuard;
use zanto_core::session::{ContextPolicy, Session, Store};
use crate::app::AppRegistry;
use crate::catalogue::Catalogue;
use crate::interaction::TauriInteractor;

pub struct DesktopState {
    pub store: Store,
    pub data: Arc<DataStore>,
    pub permissions: Arc<PermissionGuard>,
    pub registry: AppRegistry,
    pub catalogue: Arc<Catalogue>,
    pub interactor: TauriInteractor,
    pub session: Mutex<Session>,
    pub policy: ContextPolicy,
    /// Runtime-mutable so Settings can change them live.
    pub model: StdMutex<String>,
    pub endpoint: StdMutex<String>,
    pub workspace: String,
}

impl DesktopState {
    pub fn active_app_id(&self) -> Option<String> {
        self.registry.active().map(|a| a.manifest().id.clone())
    }
}

/// A past message rendered for the chat thread.
#[derive(Serialize)]
pub struct RenderMsg {
    pub role: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct ProviderDto {
    pub provider: String,
    pub model: String,
    pub endpoint: Option<String>,
    pub has_key: bool,
}

#[derive(Serialize)]
pub struct ConfigDto {
    pub model: String,
    pub endpoint: String,
    pub allowed_paths: Vec<String>,
    pub max_context_turns: Option<usize>,
    pub providers: Vec<ProviderDto>,
    pub active_provider: Option<String>,
}

#[derive(Deserialize)]
pub struct ProviderPatch {
    pub provider: String,
    pub model: String,
    pub endpoint: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ConfigPatch {
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub max_context_turns: Option<usize>,
    pub providers: Option<Vec<ProviderPatch>>,
    pub active_provider: Option<String>,
}

