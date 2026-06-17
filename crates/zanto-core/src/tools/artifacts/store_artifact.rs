use std::borrow::Cow;
use base64::Engine;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};

use crate::artifacts::{ArtifactKind, Scope};

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Artifact kind: markdown, image, json, or text")]
    pub kind: ArtifactKind,
    #[schemars(description = "Human-readable title (image titles may carry an extension, e.g. chart.svg)")]
    pub title: String,
    #[schemars(
        description = "Content: UTF-8 text for markdown/json/text; base64-encoded bytes for image"
    )]
    pub content: String,
    #[schemars(description = "Scope: global (default) or project")]
    #[serde(default)]
    pub scope: Option<Scope>,
}

pub struct StoreArtifact;

impl ToolBase for StoreArtifact {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "store_artifact".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Persist a durable artifact (markdown, image, json, or text) the user can browse later. \
             Returns the artifact reference as JSON. Distinct from desktop render artifacts."
                .into(),
        )
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::ArtifactTools> for StoreArtifact {
    async fn invoke(svc: &super::ArtifactTools, args: Args) -> Result<String, ErrorData> {
        let scope = args.scope.unwrap_or(Scope::Global);

        // Images arrive base64-encoded; text kinds are stored verbatim. Models
        // often wrap base64 across lines, so strip ASCII whitespace and accept
        // both padded and unpadded input before decoding.
        let bytes: Vec<u8> = if args.kind == ArtifactKind::Image {
            let cleaned: String = args
                .content
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect();
            decode_base64(&cleaned).map_err(|e| {
                ErrorData::invalid_params(format!("invalid base64 content: {e}"), None)
            })?
        } else {
            args.content.into_bytes()
        };

        let art = svc
            .store
            .save(args.kind, &args.title, &bytes, scope)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        serde_json::to_string(&art)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}

/// Decode whitespace-stripped base64, tolerating missing padding (models emit
/// both padded and unpadded variants).
fn decode_base64(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
    STANDARD
        .decode(s)
        .or_else(|_| STANDARD_NO_PAD.decode(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_padded_and_unpadded() {
        // "hello" → aGVsbG8= (padded) / aGVsbG8 (unpadded).
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("aGVsbG8").unwrap(), b"hello");
    }
}
