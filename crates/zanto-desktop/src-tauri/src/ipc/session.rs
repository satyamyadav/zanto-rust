//! Session-management IPC commands (scoped to the active app).

use super::{DesktopState, RenderMsg};
use tauri::State;
use zanto_core::session::{unix_now_pub, Session, SessionMeta};

#[tauri::command]
pub fn list_sessions(state: State<'_, DesktopState>) -> Result<Vec<SessionMeta>, String> {
    let app_id = state.active_app_id();
    state
        .store
        .list_sessions(Some(&state.workspace), app_id.as_deref())
        .map_err(|e| e.to_string())
}

/// One page of the active app's non-archived sessions (newest-first), windowed
/// by `offset`/`limit`. Backs the sidebar's infinite-scroll session list.
#[tauri::command]
pub fn list_sessions_page(
    state: State<'_, DesktopState>,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMeta>, String> {
    let app_id = state.active_app_id();
    state
        .store
        .list_sessions_page(
            Some(&state.workspace),
            app_id.as_deref(),
            false,
            offset,
            limit,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_session(
    state: State<'_, DesktopState>,
    id: String,
) -> Result<Vec<RenderMsg>, String> {
    let loaded = state.store.load_session(&id).map_err(|e| e.to_string())?;
    let meta = state
        .store
        .load_message_meta(&id)
        .map_err(|e| e.to_string())?;
    let msgs = loaded
        .display_messages_meta(&meta)
        .into_iter()
        .map(|(role, text, meta)| RenderMsg::from_meta(role, text, meta))
        .collect();
    *state.session.lock().await = loaded;
    Ok(msgs)
}

/// Load one windowed page of a session's display messages (newest-last). Also
/// sets the active session in app state, so paging in a session also "opens" it
/// (idempotent: the full session is loaded into state regardless of the page).
#[tauri::command]
pub async fn load_session_page(
    state: State<'_, DesktopState>,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<Vec<RenderMsg>, String> {
    let loaded = state.store.load_session(&id).map_err(|e| e.to_string())?;
    let meta = state
        .store
        .load_message_meta(&id)
        .map_err(|e| e.to_string())?;
    let page = loaded
        .display_messages_meta(&meta)
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(role, text, meta)| RenderMsg::from_meta(role, text, meta))
        .collect();
    *state.session.lock().await = loaded;
    Ok(page)
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
pub fn rename_session(
    state: State<'_, DesktopState>,
    id: String,
    title: String,
) -> Result<(), String> {
    let mut s = state.store.load_session(&id).map_err(|e| e.to_string())?;
    s.title = title;
    s.updated_at = unix_now_pub();
    state.store.save_session(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn archive_session(state: State<'_, DesktopState>, id: String) -> Result<(), String> {
    state
        .store
        .set_archived(&id, true)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unarchive_session(state: State<'_, DesktopState>, id: String) -> Result<(), String> {
    state
        .store
        .set_archived(&id, false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_archived_sessions(state: State<'_, DesktopState>) -> Result<Vec<SessionMeta>, String> {
    let app_id = state.active_app_id();
    state
        .store
        .list_sessions_archived(Some(&state.workspace), app_id.as_deref())
        .map_err(|e| e.to_string())
}
