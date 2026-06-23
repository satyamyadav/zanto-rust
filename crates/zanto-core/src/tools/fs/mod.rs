pub mod edit_file;
pub mod list_directory;
pub mod read_file;
pub mod search_files;
pub mod write_file;

use crate::permissions::PermissionGuard;
use genai::chat::Tool as GenaiTool;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase, ToolRouter};
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct FsTools {
    pub permissions: Arc<PermissionGuard>,
    project_dir: Option<PathBuf>,
}

impl FsTools {
    pub fn new(permissions: Arc<PermissionGuard>, project_dir: Option<PathBuf>) -> Self {
        Self {
            permissions,
            project_dir,
        }
    }

    /// Resolve a model-supplied path for the working-directory model: a relative
    /// path joins onto `project_dir` (when set); absolute and `~`-prefixed paths
    /// are returned unchanged (the permission layer expands `~` and canonicalizes).
    /// With no `project_dir`, the path is returned unchanged (resolved against cwd
    /// downstream, as before).
    pub fn resolve_input(&self, path: &str) -> String {
        match &self.project_dir {
            Some(base) if !path.starts_with('~') && Path::new(path).is_relative() => {
                if path == "." || path == "./" {
                    base.to_string_lossy().into_owned()
                } else {
                    base.join(path).to_string_lossy().into_owned()
                }
            }
            _ => path.to_string(),
        }
    }

    pub(super) fn tool_router() -> ToolRouter<Self> {
        ToolRouter::new()
            .with_async_tool::<edit_file::EditFile>()
            .with_async_tool::<list_directory::ListDirectory>()
            .with_async_tool::<read_file::ReadFile>()
            .with_async_tool::<search_files::SearchFiles>()
            .with_async_tool::<write_file::WriteFile>()
    }
}

impl ServerHandler for FsTools {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_router()
            .call(ToolCallContext::new(self, request, context))
            .await
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: Self::tool_router().list_all(),
            next_cursor: None,
            meta: None,
        })
    }
}

pub(super) fn schemas() -> Vec<GenaiTool> {
    FsTools::tool_router()
        .list_all()
        .into_iter()
        .map(|t| {
            let mut g = GenaiTool::new(t.name.as_ref());
            if let Some(ref desc) = t.description {
                g = g.with_description(desc.as_ref());
            }
            g.with_schema(t.schema_as_json_value())
        })
        .collect()
}

pub(super) async fn dispatch(
    svc: &FsTools,
    name: &str,
    args: serde_json::Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    macro_rules! try_invoke {
        ($T:ty) => {
            if name == <$T>::name() {
                let param = match serde_json::from_value(args) {
                    Ok(p) => p,
                    Err(e) => return Ok(format!("invalid arguments: {e}")),
                };
                return Ok(<$T>::invoke(svc, param)
                    .await
                    .unwrap_or_else(|e| format!("error: {}", e.message)));
            }
        };
    }
    try_invoke!(edit_file::EditFile);
    try_invoke!(list_directory::ListDirectory);
    try_invoke!(read_file::ReadFile);
    try_invoke!(search_files::SearchFiles);
    try_invoke!(write_file::WriteFile);
    Err(format!("unknown tool: {name}").into())
}

pub(super) fn is_readonly(name: &str) -> bool {
    name == list_directory::ListDirectory::name().as_ref()
        || name == read_file::ReadFile::name().as_ref()
        || name == search_files::SearchFiles::name().as_ref()
}

pub(super) fn owns(name: &str) -> bool {
    name == edit_file::EditFile::name().as_ref()
        || name == list_directory::ListDirectory::name().as_ref()
        || name == read_file::ReadFile::name().as_ref()
        || name == search_files::SearchFiles::name().as_ref()
        || name == write_file::WriteFile::name().as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::permissions::{ApprovalResponse, Approver, PermissionGuard};

    struct PanicApprover;
    #[async_trait::async_trait]
    impl Approver for PanicApprover {
        async fn confirm(&self, _: &str, _: &str, _: &str) -> ApprovalResponse {
            panic!("approver should not have been called");
        }
    }

    fn make_guard() -> Arc<PermissionGuard> {
        let settings = Settings {
            allow_read_outside: true,
            allow_write_outside: true,
            ..Default::default()
        };
        Arc::new(PermissionGuard::new(&settings, PanicApprover))
    }

    #[test]
    fn resolve_input_joins_relative_under_project_dir() {
        let perms = make_guard();
        let fs = FsTools::new(Arc::clone(&perms), Some(PathBuf::from("/proj")));
        assert_eq!(fs.resolve_input("src/main.rs"), "/proj/src/main.rs");
        assert_eq!(fs.resolve_input("."), "/proj");
        assert_eq!(fs.resolve_input("/etc/hosts"), "/etc/hosts");
        assert_eq!(fs.resolve_input("~/notes.md"), "~/notes.md");
        let fs_none = FsTools::new(perms, None);
        assert_eq!(fs_none.resolve_input("src/main.rs"), "src/main.rs");
    }
}
