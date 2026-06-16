pub mod artifacts;
pub mod fs;
pub mod shell;

use std::path::PathBuf;
use std::sync::Arc;
use genai::chat::Tool as GenaiTool;
use crate::artifacts::ArtifactStore;
use crate::config::Settings;
use crate::permissions::PermissionGuard;

pub struct ToolService {
    fs: fs::FsTools,
    shell: shell::ShellTools,
    artifacts: artifacts::ArtifactTools,
}

impl ToolService {
    pub fn new(permissions: Arc<PermissionGuard>) -> Self {
        // Artifact store is ungated (managed roots only); its project scope is
        // rooted at the active project, loaded from settings here so the public
        // signature stays unchanged.
        let project_dir = Settings::load().project_dir.map(PathBuf::from);
        let store = Arc::new(ArtifactStore::new(project_dir.as_deref()));
        Self {
            fs: fs::FsTools::new(Arc::clone(&permissions)),
            shell: shell::ShellTools::new(permissions),
            artifacts: artifacts::ArtifactTools::new(store),
        }
    }

    pub fn all_tools() -> Vec<GenaiTool> {
        let mut tools = fs::schemas();
        tools.extend(shell::schemas());
        tools.extend(artifacts::schemas());
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
        } else if shell::owns(name) {
            shell::dispatch(&self.shell, name, args).await
        } else if artifacts::owns(name) {
            artifacts::dispatch(&self.artifacts, name, args).await
        } else {
            Err(format!("unknown tool: {name}").into())
        }
    }

    pub fn is_readonly(name: &str) -> bool {
        fs::is_readonly(name) || shell::is_readonly(name) || artifacts::is_readonly(name)
    }

    /// Whether `name` is a built-in base tool (fs, shell, or artifacts).
    pub fn owns(name: &str) -> bool {
        fs::owns(name) || shell::owns(name) || artifacts::owns(name)
    }
}
