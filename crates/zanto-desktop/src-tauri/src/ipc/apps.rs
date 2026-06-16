//! App-registry IPC commands.

use serde_json::Value;
use tauri::State;
use crate::app::AppManifest;
use crate::catalogue::ArtifactDef;
use super::DesktopState;

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
