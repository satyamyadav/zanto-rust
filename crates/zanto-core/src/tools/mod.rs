pub mod fs;
pub mod shell;

use std::sync::Arc;
use genai::chat::Tool as GenaiTool;
use crate::permissions::PermissionGuard;

pub struct ToolService {
    fs: fs::FsTools,
    shell: shell::ShellTools,
}

impl ToolService {
    pub fn new(permissions: Arc<PermissionGuard>) -> Self {
        Self {
            fs: fs::FsTools::new(Arc::clone(&permissions)),
            shell: shell::ShellTools::new(permissions),
        }
    }

    pub fn all_tools() -> Vec<GenaiTool> {
        let mut tools = fs::schemas();
        tools.extend(shell::schemas());
        tools
    }

    pub async fn dispatch(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if let Ok(result) = fs::dispatch(&self.fs, name, args.clone()).await {
            return Ok(result);
        }
        shell::dispatch(&self.shell, name, args).await
    }

    pub fn is_readonly(name: &str) -> bool {
        fs::is_readonly(name) || shell::is_readonly(name)
    }
}
