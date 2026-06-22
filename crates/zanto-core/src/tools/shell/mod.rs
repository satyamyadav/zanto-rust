pub mod run_command;

use crate::permissions::PermissionGuard;
use genai::chat::Tool as GenaiTool;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase, ToolRouter};
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};
use std::sync::Arc;

#[derive(Clone)]
pub struct ShellTools {
    pub permissions: Arc<PermissionGuard>,
}

impl ShellTools {
    pub fn new(permissions: Arc<PermissionGuard>) -> Self {
        Self { permissions }
    }

    pub(super) fn tool_router() -> ToolRouter<Self> {
        ToolRouter::new().with_async_tool::<run_command::RunCommand>()
    }
}

impl ServerHandler for ShellTools {
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
    ShellTools::tool_router()
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
    svc: &ShellTools,
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
    try_invoke!(run_command::RunCommand);
    Err(format!("unknown tool: {name}").into())
}

pub(super) fn is_readonly(_name: &str) -> bool {
    false
}

pub(super) fn owns(name: &str) -> bool {
    name == run_command::RunCommand::name().as_ref()
}
