//! Tauri IPC surface — domain submodules share `DesktopState` + shared types
//! defined here. `lib.rs` registers all commands.

pub mod apps;
pub mod artifacts;
pub mod chat;
pub mod config;
pub mod files;
pub mod finance;
pub mod session;
pub mod skills;

use crate::app::AppRegistry;
use crate::catalogue::Catalogue;
use crate::interaction::TauriInteractor;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;
use zanto_core::config::ContextSource;
pub use zanto_core::config::{GenerationParams, ProviderInfo};
use zanto_core::data::DataStore;
use zanto_core::permissions::PermissionGuard;
use zanto_core::session::{Session, Store};

pub struct DesktopState {
    pub store: Store,
    pub data: Arc<DataStore>,
    pub permissions: Arc<PermissionGuard>,
    pub registry: AppRegistry,
    pub catalogue: Arc<Catalogue>,
    pub interactor: TauriInteractor,
    pub session: Mutex<Session>,
    /// Runtime-mutable so Settings can change them live.
    pub model: StdMutex<String>,
    pub endpoint: StdMutex<String>,
    pub workspace: String,
    /// User-selected markdown skill (file stem), appended to the app skill on
    /// each turn. `None` means no extra skill.
    pub selected_skill: StdMutex<Option<String>>,
    /// Cancel flag for the in-flight turn (if any). `send_message` installs a fresh
    /// flag per turn and clears it on completion; `interrupt_turn` sets it to stop.
    pub active_cancel: StdMutex<Option<Arc<AtomicBool>>>,
}

impl DesktopState {
    pub fn active_app_id(&self) -> Option<String> {
        self.registry.active().map(|a| a.manifest().id.clone())
    }
}

/// A past message rendered for the chat thread. `blocks` carries the persisted
/// per-message metadata (D1: `{"blocks":[<Component ...>]}`) when present, so a
/// reopened thread restores artifacts, not just text. `segments` carries the full
/// ordered display-segment list (reasoning/tool_call/block/text) for a turn so it
/// restores exactly as it rendered live; `stopped` marks an interrupted turn. Both
/// are `None` for legacy sessions persisted before the segment metadata, where the
/// frontend falls back to text + `blocks`.
#[derive(Serialize, Deserialize)]
pub struct RenderMsg {
    pub role: String,
    pub text: String,
    pub blocks: Option<serde_json::Value>,
    pub segments: Option<serde_json::Value>,
    pub stopped: Option<bool>,
}

impl RenderMsg {
    /// Build a `RenderMsg` from a `display_messages_meta` triple, decoding the
    /// `segments` and `stopped` fields out of the raw per-message metadata. The
    /// raw metadata is still carried in `blocks` for the back-compat path.
    pub fn from_meta(role: String, text: String, meta: Option<serde_json::Value>) -> Self {
        let segments = meta
            .as_ref()
            .and_then(|m| m.get("segments"))
            .filter(|s| s.is_array())
            .cloned();
        let stopped = meta
            .as_ref()
            .and_then(|m| m.get("stopped"))
            .and_then(|s| s.as_bool());
        RenderMsg {
            role,
            text,
            blocks: meta,
            segments,
            stopped,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ProviderDto {
    pub provider: String,
    pub model: String,
    pub endpoint: Option<String>,
    pub has_key: bool,
    pub label: String,
    pub needs_key: bool,
    pub default_endpoint: Option<String>,
    pub generation: GenerationParams,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigDto {
    pub model: String,
    pub endpoint: String,
    pub allowed_paths: Vec<String>,
    /// The active project directory (outputs land in `<dir>/.zanto/artifacts`),
    /// or `None` when no project is set.
    pub project_dir: Option<String>,
    /// Context sources with per-source `enabled` toggles (serialized as
    /// `{ path, enabled }`).
    pub context_sources: Vec<ContextSource>,
    pub selected_skill: Option<String>,
    pub max_context_turns: Option<usize>,
    pub providers: Vec<ProviderDto>,
    pub active_provider: Option<String>,
    pub provider_registry: Vec<ProviderInfo>,
    pub generation: GenerationParams,
}

#[derive(Deserialize)]
pub struct ProviderPatch {
    pub provider: String,
    pub model: String,
    pub endpoint: Option<String>,
    #[serde(default)]
    pub generation: GenerationParams,
}

#[derive(Deserialize, Default)]
pub struct ConfigPatch {
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub max_context_turns: Option<usize>,
    pub providers: Option<Vec<ProviderPatch>>,
    pub active_provider: Option<String>,
    pub generation: Option<GenerationParams>,
}
