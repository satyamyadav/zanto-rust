//! Session-management IPC commands (scoped to the active app).

use tauri::State;
use zanto_core::session::{unix_now_pub, Session, SessionMeta};
use super::{DesktopState, RenderMsg};

#[tauri::command]
pub fn list_sessions(state: State<'_, DesktopState>) -> Result<Vec<SessionMeta>, String> {
    let app_id = state.active_app_id();
    state
        .store
        .list_sessions(Some(&state.workspace), app_id.as_deref())
        .map_err(|e| e.to_string())
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
