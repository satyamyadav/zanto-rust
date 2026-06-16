pub mod list_stored_artifacts;
pub mod read_stored_artifact;
pub mod store_artifact;

use std::sync::Arc;
use genai::chat::Tool as GenaiTool;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase, ToolRouter};
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};
use crate::artifacts::ArtifactStore;

/// Ungated artifact tools. Unlike fs/shell, these hold no `PermissionGuard`:
/// they operate only within the store's managed roots (like `DataStore`).
#[derive(Clone)]
pub struct ArtifactTools {
    pub store: Arc<ArtifactStore>,
}

impl ArtifactTools {
    pub fn new(store: Arc<ArtifactStore>) -> Self {
        Self { store }
    }

    pub(super) fn tool_router() -> ToolRouter<Self> {
        ToolRouter::new()
            .with_async_tool::<store_artifact::StoreArtifact>()
            .with_async_tool::<list_stored_artifacts::ListStoredArtifacts>()
            .with_async_tool::<read_stored_artifact::ReadStoredArtifact>()
    }
}

impl ServerHandler for ArtifactTools {
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
    ArtifactTools::tool_router().list_all().into_iter().map(|t| {
        let mut g = GenaiTool::new(t.name.as_ref());
        if let Some(ref desc) = t.description {
            g = g.with_description(desc.as_ref());
        }
        g.with_schema(t.schema_as_json_value())
    }).collect()
}

pub(super) async fn dispatch(
    svc: &ArtifactTools,
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
    try_invoke!(store_artifact::StoreArtifact);
    try_invoke!(list_stored_artifacts::ListStoredArtifacts);
    try_invoke!(read_stored_artifact::ReadStoredArtifact);
    Err(format!("unknown tool: {name}").into())
}

pub(super) fn is_readonly(name: &str) -> bool {
    name == list_stored_artifacts::ListStoredArtifacts::name().as_ref()
        || name == read_stored_artifact::ReadStoredArtifact::name().as_ref()
}

pub(super) fn owns(name: &str) -> bool {
    name == store_artifact::StoreArtifact::name().as_ref()
        || name == list_stored_artifacts::ListStoredArtifacts::name().as_ref()
        || name == read_stored_artifact::ReadStoredArtifact::name().as_ref()
}
