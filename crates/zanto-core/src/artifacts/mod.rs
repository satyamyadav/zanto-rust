//! Filesystem-backed artifact store for durable docs/assets (markdown, images,
//! json, text) the agent produces and the user browses. Ungated library: operates
//! only within managed roots (like `DataStore`), so callers wire it directly.
//!
//! Two scopes: project (`<project_dir>/.zanto/artifacts/`, when a project root is
//! set) and global (`<data_dir>/zanto/artifacts/`). Each root is indexed by a JSON
//! manifest (`index.json`); blobs live under `files/<id>.<ext>`. No SQLite, so this
//! never collides with the session migration version.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::session::unix_now_pub;

// ---- Errors ----

#[derive(Debug)]
pub enum ArtifactError {
    Io(std::io::Error),
    Json(serde_json::Error),
    NoProjectRoot,
    NotFound(String),
}

impl std::fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Json(e) => write!(f, "json error: {e}"),
            Self::NoProjectRoot => write!(f, "no project root set for project scope"),
            Self::NotFound(id) => write!(f, "artifact not found: {id}"),
        }
    }
}
impl std::error::Error for ArtifactError {}
impl From<std::io::Error> for ArtifactError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for ArtifactError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

type Result<T> = std::result::Result<T, ArtifactError>;

// ---- Types ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    #[default]
    Text,
    Markdown,
    Image,
    Json,
}

impl ArtifactKind {
    /// Default file extension for this kind. Image defaults to `png`; an explicit
    /// extension inferred from the title takes precedence at save time.
    fn ext(self) -> &'static str {
        match self {
            ArtifactKind::Markdown => "md",
            ArtifactKind::Image => "png",
            ArtifactKind::Json => "json",
            ArtifactKind::Text => "txt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Project,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// Short uuid.
    pub id: String,
    pub kind: ArtifactKind,
    pub title: String,
    /// Path of the blob relative to its scope root.
    pub rel_path: String,
    pub scope: Scope,
    pub created_at: u64,
}

// ---- Store ----

pub struct ArtifactStore {
    project_root: Option<PathBuf>,
    global_root: PathBuf,
}

impl ArtifactStore {
    /// Build a store. The global root is `$ZANTO_ARTIFACTS` when set (tests), else
    /// the OS-conventional `<data_dir>/zanto/artifacts`. `project_dir`, when given,
    /// roots the project scope at `<project_dir>/.zanto/artifacts`.
    pub fn new(project_dir: Option<&Path>) -> Self {
        let global_root = match std::env::var("ZANTO_ARTIFACTS") {
            Ok(v) => PathBuf::from(v),
            Err(_) => ProjectDirs::from("", "", "zanto")
                .map(|d| d.data_dir().join("artifacts"))
                // Fall back to a relative path; `save` surfaces a real IO error if
                // this location is unusable, rather than panicking at construction.
                .unwrap_or_else(|| PathBuf::from(".zanto-artifacts")),
        };
        Self {
            project_root: project_dir.map(|p| p.join(".zanto").join("artifacts")),
            global_root,
        }
    }

    /// Save a blob and upsert the manifest atomically. `bytes` is the raw content;
    /// for text kinds it is the UTF-8 text, for images the decoded image bytes.
    pub fn save(
        &self,
        kind: ArtifactKind,
        title: &str,
        bytes: &[u8],
        scope: Scope,
    ) -> Result<ArtifactRef> {
        let root = self.root(scope)?;
        let mut index = read_index(&root)?;

        // Upsert by (title, scope): if a same-title entry already exists in this
        // scope, reuse its id + rel_path, overwrite the blob, and refresh
        // created_at so it sorts as most-recent. Otherwise create a fresh entry.
        let existing = index
            .iter()
            .position(|a| a.title == title && a.scope == scope);

        let (id, rel_path) = match existing {
            Some(i) => (index[i].id.clone(), index[i].rel_path.clone()),
            None => {
                let id = new_id();
                let ext = ext_for(kind, title);
                let rel_path = format!("files/{id}.{ext}");
                (id, rel_path)
            }
        };

        let file_path = root.join(&rel_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, bytes)?;

        let art = ArtifactRef {
            id,
            kind,
            title: title.to_string(),
            rel_path,
            scope,
            created_at: unix_now_pub(),
        };

        match existing {
            Some(i) => index[i] = art.clone(),
            None => index.push(art.clone()),
        }
        write_index_atomic(&root, &index)?;
        Ok(art)
    }

    /// List artifacts, newest first. `None` lists every scope, `Some` one scope.
    /// Missing roots list as empty rather than erroring.
    pub fn list(&self, scope: Option<Scope>) -> Result<Vec<ArtifactRef>> {
        let mut out = Vec::new();
        match scope {
            Some(Scope::Project) => out.extend(self.list_root(Scope::Project)?),
            Some(Scope::Global) => out.extend(self.list_root(Scope::Global)?),
            None => {
                out.extend(self.list_root(Scope::Project)?);
                out.extend(self.list_root(Scope::Global)?);
            }
        }
        // Newest first across whatever scopes were collected.
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    /// Read an artifact's manifest entry and its raw bytes. Searches both scopes.
    pub fn read(&self, id: &str) -> Result<(ArtifactRef, Vec<u8>)> {
        let (root, art) = self.locate(id)?;
        let bytes = std::fs::read(root.join(&art.rel_path))?;
        Ok((art, bytes))
    }

    /// Absolute path of an artifact's blob. Searches both scopes.
    pub fn path(&self, id: &str) -> Result<PathBuf> {
        let (root, art) = self.locate(id)?;
        Ok(root.join(&art.rel_path))
    }

    fn root(&self, scope: Scope) -> Result<PathBuf> {
        match scope {
            Scope::Global => Ok(self.global_root.clone()),
            Scope::Project => self
                .project_root
                .clone()
                .ok_or(ArtifactError::NoProjectRoot),
        }
    }

    /// List one scope's manifest; a project scope without a root, or any absent
    /// root, yields an empty list (not an error) so `list(None)` is robust.
    fn list_root(&self, scope: Scope) -> Result<Vec<ArtifactRef>> {
        let root = match scope {
            Scope::Global => self.global_root.clone(),
            Scope::Project => match &self.project_root {
                Some(r) => r.clone(),
                None => return Ok(Vec::new()),
            },
        };
        read_index(&root)
    }

    /// Find the root + manifest entry for an id across both scopes.
    fn locate(&self, id: &str) -> Result<(PathBuf, ArtifactRef)> {
        let mut roots: Vec<PathBuf> = Vec::new();
        if let Some(p) = &self.project_root {
            roots.push(p.clone());
        }
        roots.push(self.global_root.clone());

        for root in roots {
            if let Some(art) = read_index(&root)?.into_iter().find(|a| a.id == id) {
                return Ok((root, art));
            }
        }
        Err(ArtifactError::NotFound(id.to_string()))
    }
}

// ---- Helpers ----

fn index_path(root: &Path) -> PathBuf {
    root.join("index.json")
}

/// Read a root's manifest. A missing manifest is an empty index.
fn read_index(root: &Path) -> Result<Vec<ArtifactRef>> {
    let path = index_path(root);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(ArtifactError::Io(e)),
    }
}

/// Write the manifest atomically: serialize to a sibling tmp file, then rename
/// over `index.json` (rename is atomic within a directory).
fn write_index_atomic(root: &Path, index: &[ArtifactRef]) -> Result<()> {
    std::fs::create_dir_all(root)?;
    let json = serde_json::to_string_pretty(index)?;
    let tmp = root.join(format!(".index.json.{}.tmp", new_id()));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, index_path(root))?;
    Ok(())
}

/// Pick the blob extension: for images, honor an explicit extension in the title
/// (`chart.svg` → `svg`); otherwise use the kind's default.
fn ext_for(kind: ArtifactKind, title: &str) -> String {
    if kind == ArtifactKind::Image
        && let Some(ext) = Path::new(title).extension().and_then(|e| e.to_str())
        && !ext.is_empty()
    {
        return ext.to_ascii_lowercase();
    }
    kind.ext().to_string()
}

/// A fresh artifact id: the full 32-hex-char v4 uuid (no hyphens). Full width
/// keeps collisions astronomically unlikely, so `save` needs no uniqueness check.
fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // `$ZANTO_ARTIFACTS` is process-global; serialize env-mutating tests so a
    // tempdir set by one isn't clobbered by another running in parallel.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// A store whose global root is an isolated tempdir (via `$ZANTO_ARTIFACTS`)
    /// and no project root. The store captures the root at construction, so the
    /// env var is cleared immediately; the `TempDir` guard outlives the store.
    fn global_store() -> (ArtifactStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("ZANTO_ARTIFACTS", dir.path()) };
        let store = ArtifactStore::new(None);
        unsafe { std::env::remove_var("ZANTO_ARTIFACTS") };
        (store, dir)
    }

    #[test]
    fn save_list_read_markdown() {
        let (store, _dir) = global_store();
        let art = store
            .save(ArtifactKind::Markdown, "Notes", b"# hello", Scope::Global)
            .unwrap();
        assert_eq!(art.kind, ArtifactKind::Markdown);
        assert!(art.rel_path.ends_with(".md"));

        let listed = store.list(Some(Scope::Global)).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, art.id);

        let (read_ref, bytes) = store.read(&art.id).unwrap();
        assert_eq!(read_ref.id, art.id);
        assert_eq!(bytes, b"# hello");
    }

    #[test]
    fn save_upserts_same_title_in_scope() {
        let (store, _dir) = global_store();
        store
            .save(ArtifactKind::Markdown, "Doc", b"v1", Scope::Global)
            .unwrap();
        store
            .save(ArtifactKind::Markdown, "Doc", b"v2", Scope::Global)
            .unwrap();
        let listed = store.list(Some(Scope::Global)).unwrap();
        let docs: Vec<_> = listed.iter().filter(|a| a.title == "Doc").collect();
        assert_eq!(docs.len(), 1, "same-title save should upsert, not duplicate");
        let (_, bytes) = store.read(&docs[0].id).unwrap();
        assert_eq!(bytes, b"v2", "content should be the latest save");
    }

    #[test]
    fn list_is_sorted_newest_first() {
        let (store, _dir) = global_store();
        // Three entries; their created_at may tie at 1s resolution, so assert the
        // invariant that holds regardless of ties: the returned list is ordered
        // by created_at descending (newest first).
        store
            .save(ArtifactKind::Markdown, "A", b"a", Scope::Global)
            .unwrap();
        store
            .save(ArtifactKind::Markdown, "B", b"b", Scope::Global)
            .unwrap();
        store
            .save(ArtifactKind::Markdown, "C", b"c", Scope::Global)
            .unwrap();
        let listed = store.list(Some(Scope::Global)).unwrap();
        assert!(
            listed.windows(2).all(|w| w[0].created_at >= w[1].created_at),
            "list must be sorted by created_at descending: {:?}",
            listed.iter().map(|a| a.created_at).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn manifest_survives_reopen() {
        let dir = TempDir::new().unwrap();
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("ZANTO_ARTIFACTS", dir.path()) };
        // Both stores capture the same tempdir root at construction.
        let store = ArtifactStore::new(None);
        let reopened = ArtifactStore::new(None);
        unsafe { std::env::remove_var("ZANTO_ARTIFACTS") };

        let id = store
            .save(ArtifactKind::Text, "t", b"data", Scope::Global)
            .unwrap()
            .id;

        // The fresh store reads the persisted manifest from disk.
        let listed = reopened.list(Some(Scope::Global)).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);
        let (_, bytes) = reopened.read(&id).unwrap();
        assert_eq!(bytes, b"data");
    }

    #[test]
    fn project_scope_without_root_errs() {
        let (store, _dir) = global_store();
        let res = store.save(ArtifactKind::Markdown, "x", b"y", Scope::Project);
        assert!(matches!(res, Err(ArtifactError::NoProjectRoot)));
    }

    #[test]
    fn image_ext_inferred_from_title() {
        let (store, _dir) = global_store();
        let art = store
            .save(ArtifactKind::Image, "chart.svg", b"<svg/>", Scope::Global)
            .unwrap();
        assert!(art.rel_path.ends_with(".svg"));

        let dflt = store
            .save(ArtifactKind::Image, "no-ext", b"\x89PNG", Scope::Global)
            .unwrap();
        assert!(dflt.rel_path.ends_with(".png"));
    }

    #[test]
    fn read_unknown_id_errs() {
        let (store, _dir) = global_store();
        assert!(matches!(
            store.read("deadbeef"),
            Err(ArtifactError::NotFound(_))
        ));
    }
}
