use crate::permissions::Op;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Path of the file to edit")]
    pub path: String,
    #[schemars(description = "Exact string to find in the file — must match exactly once")]
    pub old_str: String,
    #[schemars(description = "String to replace it with")]
    pub new_str: String,
}

pub struct EditFile;

impl ToolBase for EditFile {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "edit_file".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some("Replace an exact string in a file. old_str must appear exactly once.".into())
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::FsTools> for EditFile {
    async fn invoke(svc: &super::FsTools, args: Args) -> Result<String, ErrorData> {
        let input = svc.resolve_input(&args.path);
        let resolved = svc
            .permissions
            .check(&input, Op::Write)
            .await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let content = std::fs::read_to_string(&resolved)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let count = content.matches(&*args.old_str).count();
        match count {
            0 => {
                return Err(ErrorData::invalid_params(
                    format!("old_str not found in {}", resolved.display()),
                    None,
                ));
            }
            n if n > 1 => {
                return Err(ErrorData::invalid_params(
                    format!(
                        "old_str matches {n} times in {} — must be unique",
                        resolved.display()
                    ),
                    None,
                ));
            }
            _ => {}
        }

        let updated = content.replacen(&*args.old_str, &args.new_str, 1);
        std::fs::write(&resolved, updated)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(format!("edited {}", resolved.display()))
    }
}
