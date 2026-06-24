use crate::permissions::Op;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Filesystem path of the file to write")]
    pub path: String,
    #[schemars(description = "Content to write into the file")]
    pub content: String,
}

pub struct WriteFile;

impl ToolBase for WriteFile {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "write_file".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Write content to a specific file the user named or that you are editing \
             (source code, a config file, an existing file at a known path). Do NOT use \
             this to save a document, article, report, or notes you generated — persist \
             those with store_artifact (durable, browsable in Artifacts) and show them \
             with render_artifact. Creates any missing parent directories."
                .into(),
        )
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::FsTools> for WriteFile {
    async fn invoke(svc: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        let input = svc.resolve_input(&args.path);
        let resolved = svc
            .permissions
            .check(&input, Op::Write)
            .await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        }

        std::fs::write(&resolved, &args.content)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(format!(
            "wrote {} bytes to {}",
            args.content.len(),
            resolved.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_steers_generated_docs_to_artifacts() {
        let d = WriteFile::description().expect("description present");
        assert!(
            d.contains("store_artifact"),
            "should redirect generated docs to store_artifact: {d}"
        );
        assert!(d.contains("Do NOT"), "should discourage doc drops: {d}");
    }
}
