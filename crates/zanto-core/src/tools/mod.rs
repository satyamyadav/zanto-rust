pub mod artifacts;
pub mod docs;
pub mod fs;
pub mod shell;
pub mod web;

use crate::artifacts::ArtifactStore;
use crate::permissions::PermissionGuard;
use genai::chat::Tool as GenaiTool;
use std::path::Path;
use std::sync::Arc;

pub struct ToolService {
    fs: fs::FsTools,
    docs: docs::DocTools,
    shell: shell::ShellTools,
    artifacts: artifacts::ArtifactTools,
    web: web::WebTools,
}

impl ToolService {
    /// Create a `ToolService` with **no** project scope for artifact tools
    /// (`Scope::Project` artifact ops will fail with no project root). Prefer
    /// [`ToolService::with_project_dir`] in any path that has a loaded `Settings`
    /// — pass `settings.project_dir_path().as_deref()` so project-scoped
    /// artifacts work. Kept for tests and the no-config case.
    pub fn new(permissions: Arc<PermissionGuard>) -> Self {
        Self::with_project_dir(permissions, None)
    }

    /// Create a `ToolService` with an explicit project directory for artifact
    /// scoping. Prefer this over `new` when a loaded `Settings` is available —
    /// it avoids a redundant `Settings::load()` call (and its `ensure_project_config`
    /// side-effect) inside the library.
    pub fn with_project_dir(permissions: Arc<PermissionGuard>, project_dir: Option<&Path>) -> Self {
        let store = Arc::new(ArtifactStore::new(project_dir));
        Self {
            fs: fs::FsTools::new(Arc::clone(&permissions)),
            docs: docs::DocTools::new(Arc::clone(&permissions)),
            shell: shell::ShellTools::new(permissions),
            artifacts: artifacts::ArtifactTools::new(store),
            web: web::WebTools::new(),
        }
    }

    pub fn all_tools() -> Vec<GenaiTool> {
        let mut tools = fs::schemas();
        tools.extend(docs::schemas());
        tools.extend(shell::schemas());
        tools.extend(artifacts::schemas());
        tools.extend(web::schemas());
        tools
    }

    pub async fn dispatch(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Route by which category owns the tool name — explicit, not Err-fallthrough.
        if fs::owns(name) {
            fs::dispatch(&self.fs, name, args).await
        } else if docs::owns(name) {
            docs::dispatch(&self.docs, name, args).await
        } else if shell::owns(name) {
            shell::dispatch(&self.shell, name, args).await
        } else if artifacts::owns(name) {
            artifacts::dispatch(&self.artifacts, name, args).await
        } else if web::owns(name) {
            web::dispatch(&self.web, name, args).await
        } else {
            Err(format!("unknown tool: {name}").into())
        }
    }

    pub fn is_readonly(name: &str) -> bool {
        fs::is_readonly(name)
            || docs::is_readonly(name)
            || shell::is_readonly(name)
            || artifacts::is_readonly(name)
            || web::is_readonly(name)
    }

    /// Whether `name` is a built-in base tool (fs, docs, shell, artifacts, or web).
    pub fn owns(name: &str) -> bool {
        fs::owns(name)
            || docs::owns(name)
            || shell::owns(name)
            || artifacts::owns(name)
            || web::owns(name)
    }
}
