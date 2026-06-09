use std::borrow::Cow;
use rmcp::{ErrorData, schemars::JsonSchema};
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Filesystem path of the file to read")]
    pub path: String,
}

pub struct ReadFile;

impl ToolBase for ReadFile {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "read_file".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some("Read the full text contents of a file".into())
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::FsTools> for ReadFile {
    async fn invoke(_: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        std::fs::read_to_string(&args.path)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}
