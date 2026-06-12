use std::borrow::Cow;
use rmcp::{ErrorData, schemars::JsonSchema};
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use serde::{Deserialize, Serialize};
use crate::permissions::Op;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Shell command to execute (passed to sh -c)")]
    pub command: String,
    #[schemars(description = "Working directory for the command. Defaults to current directory if omitted.")]
    pub working_dir: Option<String>,
}

pub struct RunCommand;

impl ToolBase for RunCommand {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "run_command".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some("Run a shell command and return its stdout, stderr, and exit code".into())
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::ShellTools> for RunCommand {
    async fn invoke(svc: &super::ShellTools, args: Args) -> Result<String, ErrorData> {
        let wd_str = args.working_dir.as_deref().unwrap_or(".");
        let resolved_wd = svc.permissions.check(wd_str, Op::Write).await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let command = args.command.clone();
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("sh")
                .args(["-c", &command])
                .current_dir(&resolved_wd)
                .output()
        })
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        let mut result = format!("exit {exit_code}");
        if !stdout.is_empty() {
            result.push('\n');
            result.push_str(stdout.trim_end());
        }
        if !stderr.is_empty() {
            result.push_str("\n[stderr]\n");
            result.push_str(stderr.trim_end());
        }

        Ok(result)
    }
}
