use crate::permissions::Op;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Shell command to execute (passed to sh -c)")]
    pub command: String,
    #[schemars(
        description = "Working directory for the command. Defaults to current directory if omitted."
    )]
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
        // Read-only commands (git status, pacman -Qi, ls…) gate as reads, not
        // writes — fewer / less-alarming prompts. Best-effort, not a security
        // boundary: anything compound/redirected falls back to Write.
        let op = classify_op(&args.command);
        let resolved_wd = svc
            .permissions
            .check(wd_str, op)
            .await
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

/// Classify a shell command as read-only (`Op::Read`) or potentially mutating
/// (`Op::Write`). Conservative: compound/redirected commands and anything not on
/// the read-only allowlist are treated as writes. NOT a security boundary — a
/// read-classified command could still be crafted to mutate; the user already
/// approved the command content at the gate.
fn classify_op(command: &str) -> Op {
    // Shell metacharacters can chain a mutating command — can't reason, treat as write.
    if command.contains(['|', '&', ';', '>', '<', '`', '$', '\n']) {
        return Op::Write;
    }
    let tokens: Vec<&str> = command.split_whitespace().collect();
    if is_read_only(&tokens) {
        Op::Read
    } else {
        Op::Write
    }
}

fn is_read_only(tokens: &[&str]) -> bool {
    let Some(&cmd) = tokens.first() else {
        return false;
    };
    match cmd {
        "ls" | "cat" | "pwd" | "whoami" | "echo" | "which" | "head" | "tail" | "wc" | "stat"
        | "df" | "du" | "env" | "date" | "uname" | "grep" | "find" | "file" | "realpath" => true,
        "git" => matches!(
            tokens.get(1).copied(),
            Some(
                "status"
                    | "log"
                    | "diff"
                    | "show"
                    | "ls-remote"
                    | "remote"
                    | "branch"
                    | "rev-parse"
                    | "describe"
                    | "config"
                    | "ls-files"
            )
        ),
        "pacman" => tokens.get(1).is_some_and(|a| a.starts_with("-Q")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_commands_gate_as_read() {
        assert_eq!(classify_op("git status"), Op::Read);
        assert_eq!(classify_op("git log -n 5"), Op::Read);
        assert_eq!(classify_op("pacman -Qi google-chrome"), Op::Read);
        assert_eq!(classify_op("ls -la"), Op::Read);
        assert_eq!(classify_op("whoami"), Op::Read);
    }

    #[test]
    fn mutating_commands_gate_as_write() {
        assert_eq!(classify_op("git push"), Op::Write);
        assert_eq!(classify_op("git pull origin master"), Op::Write);
        assert_eq!(classify_op("rm -rf x"), Op::Write);
        assert_eq!(classify_op("makepkg -si"), Op::Write);
        assert_eq!(classify_op("pacman -S firefox"), Op::Write);
    }

    #[test]
    fn compound_commands_gate_as_write() {
        // A read-only leading token can still chain a mutation.
        assert_eq!(classify_op("ls && rm -rf x"), Op::Write);
        assert_eq!(classify_op("cat a > b"), Op::Write);
        assert_eq!(classify_op("git status; rm x"), Op::Write);
        assert_eq!(classify_op("echo $(rm x)"), Op::Write);
    }
}
