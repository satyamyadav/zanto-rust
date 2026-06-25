//! Artifact-browser IPC commands (E4). Thin desktop wrappers over A3's
//! `ArtifactStore`: list stored artifacts and read one for preview. The store is
//! ungated and cheap to construct, so each command builds it from `Settings`
//! (project scope rooted at `project_dir`, global scope at the data dir).
//!
//! Command names are suffixed `_cmd` and prefixed `stored_` to stay distinct from
//! the core LLM artifact tools.

use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;
use zanto_core::artifacts::{ArtifactKind, ArtifactRef, ArtifactStore, Scope};
use zanto_core::config::Settings;
use zanto_core::data::{Dir, Query, Sort};

use super::DesktopState;
use crate::catalogue::PINNED_STORE;

/// A pinned view+data artifact persisted in the `pinned_artifacts` DataStore (4b).
/// The browser (4d) re-renders it by building a `{kind:"component", component_id,
/// data, target}` block from these fields.
#[derive(Serialize, Deserialize)]
pub struct PinnedArtifact {
    pub id: i64,
    pub component_id: String,
    pub title: Option<String>,
    pub target: String,
    pub created_at: u64,
    pub data: Value,
}

/// Map a DataStore record into a `PinnedArtifact`. The persisted JSON shape is
/// `{ component_id, data, target, title, created_at }` (see `pin_artifact`).
fn pinned_from_record(rec: zanto_core::data::Record) -> PinnedArtifact {
    let obj = &rec.data;
    PinnedArtifact {
        id: rec.id,
        component_id: obj
            .get("component_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        title: obj
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        target: obj
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("inline")
            .to_string(),
        created_at: obj
            .get("created_at")
            .and_then(|v| v.as_u64())
            .unwrap_or(rec.created_at),
        data: obj.get("data").cloned().unwrap_or(Value::Null),
    }
}

/// List pinned view+data artifacts, newest first. Empty when nothing pinned yet
/// (the store may not exist until the first `pin_artifact`).
#[tauri::command]
pub fn list_pinned_artifacts(
    state: State<'_, DesktopState>,
) -> Result<Vec<PinnedArtifact>, String> {
    // Ensure the store exists so a never-pinned workspace returns [] not an error.
    state
        .data
        .create_store(PINNED_STORE)
        .map_err(|e| e.to_string())?;
    let q = Query {
        sort: Some(Sort {
            field: "created_at".into(),
            dir: Dir::Desc,
        }),
        ..Default::default()
    };
    let rows = state
        .data
        .query(PINNED_STORE, &q)
        .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(pinned_from_record).collect())
}

/// Pin a view+data artifact from the UI (A-5 user "Pin" button). Mirrors the
/// agent `pin_artifact` write: persists `{ component_id, data, target:"inline",
/// title, created_at }` to the `pinned_artifacts` DataStore so it reopens from the
/// Artifacts browser. Returns the new record id.
#[tauri::command]
pub fn pin_artifact_cmd(
    state: State<'_, DesktopState>,
    component_id: String,
    data: Value,
    title: Option<String>,
) -> Result<i64, String> {
    state
        .data
        .create_store(PINNED_STORE)
        .map_err(|e| e.to_string())?;
    let record = json!({
        "component_id": component_id,
        "data": data,
        "target": "inline",
        "title": title,
        "created_at": zanto_core::session::unix_now_pub(),
    });
    state
        .data
        .insert(PINNED_STORE, &record)
        .map_err(|e| e.to_string())
}

/// Read one pinned artifact by its record id.
#[tauri::command]
pub fn read_pinned_artifact(
    state: State<'_, DesktopState>,
    id: i64,
) -> Result<PinnedArtifact, String> {
    state
        .data
        .create_store(PINNED_STORE)
        .map_err(|e| e.to_string())?;
    let rows = state
        .data
        .query(PINNED_STORE, &Query::default())
        .map_err(|e| e.to_string())?;
    rows.into_iter()
        .find(|r| r.id == id)
        .map(pinned_from_record)
        .ok_or_else(|| format!("pinned artifact not found: {id}"))
}

/// Build a store rooted at the configured project dir (project scope) and the
/// data dir (global scope).
fn store() -> ArtifactStore {
    let settings = Settings::load();
    ArtifactStore::new(settings.project_dir.as_deref().map(Path::new))
}

/// Persist a generated markdown document to the artifact store (the deliberate
/// "Save" action from a chat message). Upserts by title (see
/// `ArtifactStore::save`), so re-saving the same document updates it in place.
/// Saves to the project scope when a project dir is set, else the global store
/// (project scope errors without a project root).
#[tauri::command]
pub fn store_document_artifact(title: String, text: String) -> Result<Value, String> {
    let scope = if Settings::load().project_dir.is_some() {
        Scope::Project
    } else {
        Scope::Global
    };
    let art = store()
        .save(ArtifactKind::Markdown, &title, text.as_bytes(), scope)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&art).map_err(|e| e.to_string())
}

/// Delete a stored document artifact (blob + index entry) by id.
#[tauri::command]
pub fn delete_stored_artifact(id: String) -> Result<(), String> {
    store().delete(&id).map_err(|e| e.to_string())
}

fn parse_scope(scope: Option<String>) -> Result<Option<Scope>, String> {
    match scope.as_deref() {
        None => Ok(None),
        Some("project") => Ok(Some(Scope::Project)),
        Some("global") => Ok(Some(Scope::Global)),
        Some(other) => Err(format!("unknown scope: {other}")),
    }
}

/// JSON view of an `ArtifactRef` for the browser list.
fn ref_json(art: &ArtifactRef) -> Value {
    json!({
        "id": art.id,
        "kind": art.kind,
        "title": art.title,
        "rel_path": art.rel_path,
        "scope": art.scope,
        "created_at": art.created_at,
    })
}

/// List stored artifacts, optionally filtered to a scope (`"project"`/`"global"`).
#[tauri::command]
pub fn list_stored_artifacts_cmd(scope: Option<String>) -> Result<Vec<Value>, String> {
    let scope = parse_scope(scope)?;
    let refs = store().list(scope).map_err(|e| e.to_string())?;
    Ok(refs.iter().map(ref_json).collect())
}

/// Read one stored artifact for preview. Text/markdown/json return UTF-8 `content`;
/// images return base64 `content` with `is_image: true` and a `mime` hint.
#[tauri::command]
pub fn read_stored_artifact_cmd(id: String) -> Result<Value, String> {
    let (art, bytes) = store().read(&id).map_err(|e| e.to_string())?;
    let mut out = ref_json(&art);

    if art.kind == ArtifactKind::Image {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        out["is_image"] = json!(true);
        out["mime"] = json!(mime_for(&art.rel_path));
        out["content"] = json!(b64);
    } else {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        out["is_image"] = json!(false);
        out["content"] = json!(text);
    }
    Ok(out)
}

/// Save a copy of a stored artifact to a user-chosen path. Reads the artifact's
/// bytes, pops a save-file dialog seeded with the artifact's blob filename, and
/// writes the bytes there. Returns `true` if a file was written, `false` if the
/// user cancelled the dialog.
#[tauri::command]
pub async fn save_artifact_copy(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let (art, bytes) = store().read(&id).map_err(|e| e.to_string())?;

    // Suggest the blob's filename (id.ext) so the saved copy keeps the correct
    // extension; the title may carry no extension.
    let suggested = Path::new(&art.rel_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&art.id)
        .to_string();

    let chosen = app
        .dialog()
        .file()
        .set_file_name(suggested)
        .set_title(format!("Save a copy of {}", art.title))
        .blocking_save_file();

    let Some(file_path) = chosen else {
        return Ok(false); // cancelled
    };
    let dest = file_path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Reveal a stored artifact's blob in the OS file manager (Finder/Explorer/etc.)
/// via the opener plugin. Resolves the absolute blob path through the store.
#[tauri::command]
pub fn reveal_artifact(app: tauri::AppHandle, id: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    let path = store().path(&id).map_err(|e| e.to_string())?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}

/// Best-effort MIME type from a blob's extension, for the `data:` image URL.
fn mime_for(rel_path: &str) -> &'static str {
    match Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
