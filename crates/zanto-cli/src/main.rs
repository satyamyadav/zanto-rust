use std::io::Write;
use std::sync::Arc;
use async_trait::async_trait;
use clap::Parser;
use zanto_core::chat::{chat, ChatConfig};
use zanto_core::config::Settings;
use zanto_core::permissions::{ApprovalResponse, Approver, PermissionGuard};

#[derive(Parser)]
#[command(name = "zanto", about = "AI assistant with filesystem tools")]
struct Cli {
    /// Question to ask; omit to start an interactive session
    question: Option<String>,

    /// LLM model (overrides settings file)
    #[arg(short, long)]
    model: Option<String>,

    /// Ollama endpoint URL (overrides settings file)
    #[arg(short, long)]
    endpoint: Option<String>,
}

struct StdinApprover;

#[async_trait]
impl Approver for StdinApprover {
    async fn confirm(&self, path: &str, op: &str, resolved: &str) -> ApprovalResponse {
        use tokio::io::AsyncBufReadExt;

        eprintln!("\n[zanto] permission required: {op} \"{path}\"");
        eprintln!("  resolved: {resolved}");
        eprintln!("  (a) allow once  (s) allow session  (f) allow forever  (d) deny");
        eprint!("> ");
        std::io::stderr().flush().ok();

        let mut line = String::new();
        if tokio::io::BufReader::new(tokio::io::stdin())
            .read_line(&mut line)
            .await
            .is_err()
        {
            return ApprovalResponse::Deny;
        }

        match line.trim() {
            "f" => ApprovalResponse::AllowForever,
            "s" => ApprovalResponse::AllowSession,
            "a" => ApprovalResponse::AllowOnce,
            _ => ApprovalResponse::Deny,
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let settings = Settings::load();

    let model = args
        .model
        .or_else(|| settings.model.clone())
        .unwrap_or_else(|| "qwen2.5:14b".to_string());

    let endpoint: &'static str = Box::leak(
        args.endpoint
            .or_else(|| settings.endpoint.clone())
            .unwrap_or_else(|| "http://192.168.1.66:11434/".to_string())
            .into_boxed_str(),
    );

    let permissions = Arc::new(PermissionGuard::new(&settings, StdinApprover));

    match args.question {
        Some(q) => {
            let config = ChatConfig { model, endpoint, permissions };
            match chat(config, &q).await {
                Ok(answer) => println!("{answer}"),
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        None => run_interactive(model, endpoint, permissions).await,
    }
}

async fn run_interactive(
    model: String,
    endpoint: &'static str,
    permissions: Arc<PermissionGuard>,
) {
    use tokio::io::AsyncBufReadExt;

    println!("zanto interactive — Ctrl+D or 'exit' to quit");

    let mut reader = tokio::io::BufReader::new(tokio::io::stdin());

    loop {
        print!("\n> ");
        std::io::stdout().flush().ok();

        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF / Ctrl+D
            Err(e) => {
                eprintln!("Error reading input: {e}");
                break;
            }
            Ok(_) => {}
        }

        let q = line.trim();
        if q.is_empty() {
            continue;
        }
        if q == "exit" || q == "quit" {
            break;
        }

        let config = ChatConfig {
            model: model.clone(),
            endpoint,
            permissions: Arc::clone(&permissions),
        };

        match chat(config, q).await {
            Ok(answer) => println!("\n{answer}"),
            Err(e) => eprintln!("Error: {e}"),
        }
    }
}
