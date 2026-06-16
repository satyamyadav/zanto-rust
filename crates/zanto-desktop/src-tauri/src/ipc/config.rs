//! Configuration IPC commands.

use tauri::State;
use zanto_core::config::Settings;
use super::{ConfigDto, ConfigPatch, DesktopState};

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
