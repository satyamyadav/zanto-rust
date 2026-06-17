//! Artifact-browser IPC commands (E4). Thin desktop wrappers over A3's
//! `ArtifactStore`: list stored artifacts and read one for preview. The store is
//! ungated and cheap to construct, so each command builds it from `Settings`
//! (project scope rooted at `project_dir`, global scope at the data dir).
//!
//! Command names are suffixed `_cmd` and prefixed `stored_` to stay distinct from
//! the core LLM artifact tools.

use std::path::Path;

use base64::Engine;
use serde_json::{json, Value};
use zanto_core::artifacts::{ArtifactKind, ArtifactRef, ArtifactStore, Scope};
use zanto_core::config::Settings;

/// Build a store rooted at the configured project dir (project scope) and the
/// data dir (global scope).
fn store() -> ArtifactStore {
    let settings = Settings::load();
    ArtifactStore::new(settings.project_dir.as_deref().map(Path::new))
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
