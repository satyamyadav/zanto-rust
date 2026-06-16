use std::borrow::Cow;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};

use crate::artifacts::Scope;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Scope to list: project, global, or omit for all scopes")]
    #[serde(default)]
    pub scope: Option<Scope>,
}

pub struct ListStoredArtifacts;

impl ToolBase for ListStoredArtifacts {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "list_stored_artifacts".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(
            "List stored artifacts (as a JSON array of references). Omit scope to list both \
             project and global scopes."
                .into(),
        )
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::ArtifactTools> for ListStoredArtifacts {
    async fn invoke(svc: &super::ArtifactTools, args: Args) -> Result<String, ErrorData> {
        let refs = svc
            .store
            .list(args.scope)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        serde_json::to_string(&refs)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}
