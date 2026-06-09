pub mod fs;

use std::sync::Arc;
use genai::chat::Tool as GenaiTool;
use crate::permissions::PermissionGuard;

pub struct ToolService {
    fs: fs::FsTools,
}

impl ToolService {
    pub fn new(permissions: Arc<PermissionGuard>) -> Self {
        Self { fs: fs::FsTools::new(permissions) }
    }

    pub fn all_tools() -> Vec<GenaiTool> {
        fs::schemas()
    }

    pub async fn dispatch(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        fs::dispatch(&self.fs, name, args).await
    }

    pub fn is_readonly(name: &str) -> bool {
        fs::is_readonly(name)
        // future: || web::is_readonly(name)
    }
}
