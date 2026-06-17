pub mod fetch_url;

use genai::chat::Tool as GenaiTool;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase, ToolRouter};
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};

/// Ungated web tools. Like artifacts, these hold no `PermissionGuard`:
/// network access is a non-path resource, expressed as read-only and guarded
/// by scheme/SSRF checks inside the tool rather than the path permission system.
#[derive(Clone)]
pub struct WebTools {
    client: reqwest::Client,
}

impl WebTools {
    pub fn new() -> Self {
        // Validate every redirect hop's target before following it, so a redirect
        // chain cannot smuggle a request to a blocked (loopback/link-local) host —
        // post-hoc validation of only the final URL would miss intermediate hops.
        let policy = reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 10 {
                return attempt.error("too many redirects");
            }
            match fetch_url::validate_url(attempt.url().as_str()) {
                Ok(()) => attempt.follow(),
                Err(e) => attempt.error(e),
            }
        });
        let client = reqwest::Client::builder()
            .user_agent(concat!("zanto/", env!("CARGO_PKG_VERSION")))
            .redirect(policy)
            .build()
            .unwrap_or_default();
        Self { client }
    }

    pub(super) fn tool_router() -> ToolRouter<Self> {
        ToolRouter::new().with_async_tool::<fetch_url::FetchUrl>()
    }
}

impl Default for WebTools {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandler for WebTools {
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
    WebTools::tool_router().list_all().into_iter().map(|t| {
        let mut g = GenaiTool::new(t.name.as_ref());
        if let Some(ref desc) = t.description {
            g = g.with_description(desc.as_ref());
        }
        g.with_schema(t.schema_as_json_value())
    }).collect()
}

pub(super) async fn dispatch(
    svc: &WebTools,
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
    try_invoke!(fetch_url::FetchUrl);
    Err(format!("unknown tool: {name}").into())
}

pub(super) fn is_readonly(name: &str) -> bool {
    name == fetch_url::FetchUrl::name().as_ref()
}

pub(super) fn owns(name: &str) -> bool {
    name == fetch_url::FetchUrl::name().as_ref()
}
