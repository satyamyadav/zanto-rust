pub mod list_directory;
pub mod read_file;
pub mod write_file;
pub mod search_files;

use genai::chat::Tool as GenaiTool;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase, ToolRouter};
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};

#[derive(Clone)]
pub struct FsTools;

impl FsTools {
    pub(super) fn tool_router() -> ToolRouter<Self> {
        ToolRouter::new()
            .with_async_tool::<list_directory::ListDirectory>()
            .with_async_tool::<read_file::ReadFile>()
            .with_async_tool::<write_file::WriteFile>()
            .with_async_tool::<search_files::SearchFiles>()
    }
}

impl ServerHandler for FsTools {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::tool_router().call(ToolCallContext::new(self, request, context)).await
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult { tools: Self::tool_router().list_all(), next_cursor: None, meta: None })
    }
}

pub(super) fn schemas() -> Vec<GenaiTool> {
    FsTools::tool_router().list_all().into_iter().map(|t| {
        let mut g = GenaiTool::new(t.name.as_ref());
        if let Some(ref desc) = t.description {
            g = g.with_description(desc.as_ref());
        }
        g.with_schema(t.schema_as_json_value())
    }).collect()
}

pub(super) async fn dispatch(
    name: &str,
    args: serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    macro_rules! try_invoke {
        ($T:ty) => {
            if name == <$T>::name() {
                return <$T>::invoke(&FsTools, serde_json::from_value(args)?)
                    .await
                    .map_err(|e| format!("{e:?}").into());
            }
        };
    }
    try_invoke!(list_directory::ListDirectory);
    try_invoke!(read_file::ReadFile);
    try_invoke!(write_file::WriteFile);
    try_invoke!(search_files::SearchFiles);
    Err(format!("unknown tool: {name}").into())
}

pub(super) fn is_readonly(name: &str) -> bool {
    name == list_directory::ListDirectory::name().as_ref()
        || name == read_file::ReadFile::name().as_ref()
        || name == search_files::SearchFiles::name().as_ref()
}
