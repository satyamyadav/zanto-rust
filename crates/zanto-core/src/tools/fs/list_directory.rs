use std::borrow::Cow;
use rmcp::{ErrorData, schemars::JsonSchema};
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use serde::{Deserialize, Serialize};
use crate::permissions::Op;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Filesystem path of the directory to list")]
    pub path: String,
}

pub struct ListDirectory;

impl ToolBase for ListDirectory {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "list_directory".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some("List files and subdirectories at a given path".into())
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::FsTools> for ListDirectory {
    async fn invoke(svc: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        svc.permissions.check(&args.path, Op::Read).await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let entries = std::fs::read_dir(&args.path)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let lines: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if e.path().is_dir() { format!("{}/", name) } else { name }
            })
            .collect();

        Ok(if lines.is_empty() {
            "(empty directory)".to_string()
        } else {
            lines.join("\n")
        })
    }
}
