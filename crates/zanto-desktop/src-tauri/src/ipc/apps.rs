//! App-registry IPC commands.

use std::sync::Mutex;
use std::time::Instant;
use serde_json::Value;
use tauri::State;
use tauri_plugin_notification::NotificationExt;
use crate::app::AppManifest;
use crate::catalogue::ArtifactDef;
use super::DesktopState;

/// `notify` is an unauthenticated native-notification primitive (review A3): bound
/// it so a buggy or abusive caller can't spam the OS center or push huge strings.
const NOTIFY_MAX_PER_MIN: usize = 5;
const NOTIFY_TITLE_CAP: usize = 120;
const NOTIFY_BODY_CAP: usize = 500;

/// Timestamps of recent notifications, for the rolling-minute rate limit.
static NOTIFY_TIMES: Mutex<Vec<Instant>> = Mutex::new(Vec::new());

/// Truncate to at most `max` chars (char-safe), appending an ellipsis when cut.
fn cap_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Show a native OS notification. Generic; used e.g. for finance overspend nudges.
/// Rate-limited to NOTIFY_MAX_PER_MIN per rolling minute and length-capped.
#[tauri::command]
pub fn notify(app: tauri::AppHandle, title: String, body: String) {
    {
        let mut times = NOTIFY_TIMES.lock().unwrap();
        let now = Instant::now();
        times.retain(|t| now.duration_since(*t).as_secs() < 60);
        if times.len() >= NOTIFY_MAX_PER_MIN {
            return; // over budget this minute — drop silently
        }
        times.push(now);
    }
    let title = cap_chars(&title, NOTIFY_TITLE_CAP);
    let body = cap_chars(&body, NOTIFY_BODY_CAP);
    let _ = app.notification().builder().title(title).body(body).show();
}

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
