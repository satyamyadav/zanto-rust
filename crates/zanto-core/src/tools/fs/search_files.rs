use std::borrow::Cow;
use rmcp::{ErrorData, schemars::JsonSchema};
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Directory to search in")]
    pub path: String,
    #[schemars(description = "Glob pattern to match filenames, e.g. '**/*.rs' or '*.toml'")]
    pub pattern: String,
}

pub struct SearchFiles;

impl ToolBase for SearchFiles {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "search_files".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some("Recursively search for files matching a glob pattern under a directory".into())
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::FsTools> for SearchFiles {
    async fn invoke(_: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        let glob = globset::Glob::new(&args.pattern)
            .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?
            .compile_matcher();

        let matches: Vec<String> = walkdir::WalkDir::new(&args.path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| glob.is_match(e.path()))
            .map(|e| e.path().display().to_string())
            .collect();

        Ok(if matches.is_empty() {
            format!("no files matching '{}' found under '{}'", args.pattern, args.path)
        } else {
            matches.join("\n")
        })
    }
}
