use std::borrow::Cow;
use rmcp::{ErrorData, schemars::JsonSchema};
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use serde::{Deserialize, Serialize};
use crate::permissions::Op;

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
        Some("Write content to a file, creating it and any missing parent directories".into())
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::FsTools> for WriteFile {
    async fn invoke(svc: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        svc.permissions.check(&args.path, Op::Write).await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let path = std::path::Path::new(&args.path);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        }

        std::fs::write(path, &args.content)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(format!("wrote {} bytes to {}", args.content.len(), args.path))
    }
}
