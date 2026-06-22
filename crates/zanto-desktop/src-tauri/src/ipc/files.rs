//! File-system browsing IPC command.

use super::DesktopState;
use serde::Serialize;
use tauri::State;
use zanto_core::config::Settings;
use zanto_core::permissions::Op;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// List directory contents, gated by the permission guard.
///
/// - `path = None` → return the configured allowed roots (allowed_paths +
///   project_dir) as top-level entries without descending into them.
/// - `path = Some(p)` → list immediate children of `p`. The path must be
///   within an allowed root; violations are rejected.
///
/// Within each group (dirs / files) entries are sorted by name.
#[tauri::command]
pub async fn browse_dir(
    state: State<'_, DesktopState>,
    path: Option<String>,
) -> Result<Vec<FileEntry>, String> {
    match path {
        None => {
            let settings = Settings::load();
            let mut roots: Vec<String> = settings.allowed_paths.clone();
            if let Some(proj) = settings.project_dir {
                if !roots.contains(&proj) {
                    roots.push(proj);
                }
            }
            let mut entries: Vec<FileEntry> = roots
                .into_iter()
                .map(|p| {
                    let name = std::path::Path::new(&p)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.clone());
                    let is_dir = std::path::Path::new(&p).is_dir();
                    FileEntry {
                        name,
                        path: p,
                        is_dir,
                    }
                })
                .collect();
            // Dirs first, then files; stable sort within each group by name.
            entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
            Ok(entries)
        }
        Some(raw_path) => {
            // Gate the read through the permission guard — this resolves tilde,
            // checks against allowed roots, and may prompt the user if needed.
            let resolved = state
                .permissions
                .check(&raw_path, Op::Read)
                .await
                .map_err(|e| e.to_string())?;

            let read_dir =
                std::fs::read_dir(&resolved).map_err(|e| format!("cannot read directory: {e}"))?;

            let mut dirs: Vec<FileEntry> = Vec::new();
            let mut files: Vec<FileEntry> = Vec::new();

            for entry in read_dir.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().into_owned();
                let path = entry.path().to_string_lossy().into_owned();
                let is_dir = entry.path().is_dir();
                let fe = FileEntry { name, path, is_dir };
                if is_dir {
                    dirs.push(fe);
                } else {
                    files.push(fe);
                }
            }

            dirs.sort_by(|a, b| a.name.cmp(&b.name));
            files.sort_by(|a, b| a.name.cmp(&b.name));
            dirs.extend(files);
            Ok(dirs)
        }
    }
}
