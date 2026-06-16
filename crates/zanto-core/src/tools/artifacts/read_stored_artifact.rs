use std::borrow::Cow;
use base64::Engine;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::artifacts::ArtifactKind;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Id of the stored artifact to read")]
    pub id: String,
}

pub struct ReadStoredArtifact;

impl ToolBase for ReadStoredArtifact {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "read_stored_artifact".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Read a stored artifact by id. Returns JSON `{ ref, content }`; content is text for \
             markdown/json/text and base64 for image."
                .into(),
        )
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::ArtifactTools> for ReadStoredArtifact {
    async fn invoke(svc: &super::ArtifactTools, args: Args) -> Result<String, ErrorData> {
        let (art, bytes) = svc
            .store
            .read(&args.id)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let content = if art.kind == ArtifactKind::Image {
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        } else {
            // Text kinds: lossless-decode; invalid UTF-8 is replaced rather than failing.
            String::from_utf8_lossy(&bytes).into_owned()
        };

        let out = json!({ "ref": art, "content": content });
        serde_json::to_string(&out)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}
