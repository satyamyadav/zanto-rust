//! Tauri IPC surface: the commands the Svelte frontend calls. Manual paths
//! (`query_app`/`run_app_action`) hit the data engine directly (ungated); the
//! agentic path (`send_message`) runs a chat turn in the active app's context.

use std::sync::Arc;
use serde_json::Value;
use tauri::State;
use tokio::sync::Mutex;
use zanto_core::chat::{chat, ChatConfig, ChatTurn};
use zanto_core::data::DataStore;
use zanto_core::permissions::PermissionGuard;
use zanto_core::session::{ContextPolicy, Session, SessionMeta, Store};
use crate::app::{ActiveDispatcher, AppManifest, AppRegistry};

pub struct DesktopState {
    pub store: Store,
    pub data: Arc<DataStore>,
    pub permissions: Arc<PermissionGuard>,
    pub registry: AppRegistry,
    pub session: Mutex<Session>,
    pub policy: ContextPolicy,
    pub model: String,
    pub endpoint: &'static str,
    pub workspace: String,
}

#[tauri::command]
pub async fn send_message(state: State<'_, DesktopState>, text: String) -> Result<ChatTurn, String> {
    let active = state.registry.active();
    let mut session = state.session.lock().await;

    let config = match &active {
        Some(app) => ChatConfig {
            model: state.model.clone(),
            endpoint: state.endpoint,
            permissions: Arc::clone(&state.permissions),
            skill: Some(app.skill()),
            extra_tools: app.agent_tools(),
            app_dispatch: Some(Arc::new(ActiveDispatcher::new(Arc::clone(app), Arc::clone(&state.data)))),
            include_base_tools: false,
        },
        None => ChatConfig::new(state.model.clone(), state.endpoint, Arc::clone(&state.permissions)),
    };

    chat(config, &state.store, &mut session, &text, &state.policy)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_apps(state: State<'_, DesktopState>) -> Vec<AppManifest> {
    state.registry.manifests()
}

#[tauri::command]
pub fn mount_app(state: State<'_, DesktopState>, id: String) -> Result<(), String> {
    state.registry.mount(&id)
}

#[tauri::command]
pub fn unmount_app(state: State<'_, DesktopState>) {
    state.registry.unmount();
}

#[tauri::command]
pub fn query_app(
    state: State<'_, DesktopState>,
    id: String,
    query: String,
    args: Value,
) -> Result<Value, String> {
    let app = state.registry.get(&id).ok_or_else(|| format!("unknown app: {id}"))?;
    app.query(&state.data, &query, args)
}

#[tauri::command]
pub fn run_app_action(
    state: State<'_, DesktopState>,
    id: String,
    action: String,
    args: Value,
) -> Result<Value, String> {
    let app = state.registry.get(&id).ok_or_else(|| format!("unknown app: {id}"))?;
    app.action(&state.data, &action, args)
}

#[tauri::command]
pub fn list_sessions(state: State<'_, DesktopState>) -> Result<Vec<SessionMeta>, String> {
    state.store.list_sessions(Some(&state.workspace)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_session(state: State<'_, DesktopState>, id: String) -> Result<(), String> {
    let loaded = state.store.load_session(&id).map_err(|e| e.to_string())?;
    *state.session.lock().await = loaded;
    Ok(())
}

#[tauri::command]
pub async fn new_session(state: State<'_, DesktopState>) -> Result<(), String> {
    *state.session.lock().await = Session::new("", &state.workspace);
    Ok(())
}
