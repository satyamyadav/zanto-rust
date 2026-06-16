//! Tauri IPC surface. Manual paths (`query_app`/`run_app_action`) hit the data
//! engine directly (ungated); the agentic path (`send_message`) runs a chat turn in
//! the active app's context. Sessions are scoped to the active app.

use std::sync::{Arc, Mutex as StdMutex};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use tokio::sync::Mutex;
use zanto_core::chat::{chat, ChatConfig, ChatTurn};
use zanto_core::config::Settings;
use zanto_core::data::DataStore;
use zanto_core::permissions::PermissionGuard;
use zanto_core::session::{auto_title, unix_now_pub, ContextPolicy, Session, SessionMeta, Store};
use crate::app::{AppManifest, AppRegistry};
use crate::catalogue::{shared_tools, ArtifactDef, Catalogue, SharedDispatcher};
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
    // Runtime-mutable so Settings can change them live.
    pub model: StdMutex<String>,
    pub endpoint: StdMutex<String>,
    pub workspace: String,
}

impl DesktopState {
    fn active_app_id(&self) -> Option<String> {
        self.registry.active().map(|a| a.manifest().id.clone())
    }
}

// ---- Chat ----

#[tauri::command]
pub async fn send_message(state: State<'_, DesktopState>, text: String) -> Result<ChatTurn, String> {
    let active = state.registry.active();
    let model = state.model.lock().unwrap().clone();
    let endpoint = state.endpoint.lock().unwrap().clone();

    let mut session = state.session.lock().await;
    session.app_id = active.as_ref().map(|a| a.manifest().id.clone());

    let config = match &active {
        Some(app) => {
            // Shared artifact tools + the app's domain tools; base fs/shell come
            // from core. SharedDispatcher routes artifact tools then delegates.
            let mut extra = shared_tools();
            extra.extend(app.agent_tools());
            ChatConfig {
                model,
                endpoint,
                permissions: Arc::clone(&state.permissions),
                skill: Some(app.skill()),
                extra_tools: extra,
                app_dispatch: Some(Arc::new(SharedDispatcher::new(
                    Arc::clone(&state.catalogue),
                    Arc::clone(app),
                    Arc::clone(&state.data),
                    state.interactor.clone(),
                ))),
            }
        }
        None => ChatConfig::new(model, endpoint, Arc::clone(&state.permissions)),
    };

    let turn = chat(config, &state.store, &mut session, &text, &state.policy)
        .await
        .map_err(|e| e.to_string())?;

    // Auto-title a fresh session from its first user message.
    if session.title.is_empty() {
        session.title = auto_title(&session.messages);
        let _ = state.store.save_session(&session);
    }
    Ok(turn)
}

// ---- Apps ----

#[tauri::command]
pub fn list_apps(state: State<'_, DesktopState>) -> Vec<AppManifest> {
    state.registry.manifests()
}

/// The shared artifact catalogue (id, description, dataSchema) — for the shell to
/// AJV-validate and mount components by id.
#[tauri::command]
pub fn get_catalogue(state: State<'_, DesktopState>) -> Vec<ArtifactDef> {
    state.catalogue.all()
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

// ---- Sessions (scoped to the active app) ----

#[tauri::command]
pub fn list_sessions(state: State<'_, DesktopState>) -> Result<Vec<SessionMeta>, String> {
    let app_id = state.active_app_id();
    state
        .store
        .list_sessions(Some(&state.workspace), app_id.as_deref())
        .map_err(|e| e.to_string())
}

/// A past message rendered for the chat thread.
#[derive(Serialize)]
pub struct RenderMsg {
    pub role: String,
    pub text: String,
}

#[tauri::command]
pub async fn load_session(state: State<'_, DesktopState>, id: String) -> Result<Vec<RenderMsg>, String> {
    let loaded = state.store.load_session(&id).map_err(|e| e.to_string())?;
    let msgs = loaded
        .display_messages()
        .into_iter()
        .map(|(role, text)| RenderMsg { role, text })
        .collect();
    *state.session.lock().await = loaded;
    Ok(msgs)
}

#[tauri::command]
pub async fn new_session(state: State<'_, DesktopState>) -> Result<String, String> {
    let mut s = Session::new("", &state.workspace);
    s.app_id = state.active_app_id();
    let id = s.id.clone();
    *state.session.lock().await = s;
    Ok(id)
}

#[tauri::command]
pub fn delete_session(state: State<'_, DesktopState>, id: String) -> Result<(), String> {
    state.store.delete_session(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_session(state: State<'_, DesktopState>, id: String, title: String) -> Result<(), String> {
    let mut s = state.store.load_session(&id).map_err(|e| e.to_string())?;
    s.title = title;
    s.updated_at = unix_now_pub();
    state.store.save_session(&s).map_err(|e| e.to_string())
}

// ---- Config ----

#[derive(Serialize)]
pub struct ConfigDto {
    pub model: String,
    pub endpoint: String,
    pub allowed_paths: Vec<String>,
    pub max_context_turns: Option<usize>,
}

#[derive(Deserialize, Default)]
pub struct ConfigPatch {
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub max_context_turns: Option<usize>,
}

#[tauri::command]
pub fn get_config(state: State<'_, DesktopState>) -> ConfigDto {
    let settings = Settings::load();
    ConfigDto {
        model: state.model.lock().unwrap().clone(),
        endpoint: state.endpoint.lock().unwrap().clone(),
        allowed_paths: settings.allowed_paths,
        max_context_turns: settings.max_context_turns,
    }
}

#[tauri::command]
pub fn set_config(state: State<'_, DesktopState>, patch: ConfigPatch) -> Result<(), String> {
    // Apply live to running state.
    if let Some(m) = &patch.model {
        *state.model.lock().unwrap() = m.clone();
    }
    if let Some(e) = &patch.endpoint {
        *state.endpoint.lock().unwrap() = e.clone();
    }
    // Persist to .zanto/settings.json.
    let mut settings = Settings::load();
    if let Some(m) = patch.model {
        settings.model = Some(m);
    }
    if let Some(e) = patch.endpoint {
        settings.endpoint = Some(e);
    }
    if let Some(t) = patch.max_context_turns {
        settings.max_context_turns = Some(t);
    }
    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_folder().map(|p| p.to_string())
}

/// Grant a folder (and children) for this session and persist it to project config.
#[tauri::command]
pub fn add_allowed_path(state: State<'_, DesktopState>, path: String) -> Result<(), String> {
    state.permissions.add_allowed(&path);
    Settings::persist_allowed_path(&path);
    Ok(())
}
