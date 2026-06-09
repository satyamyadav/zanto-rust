use std::io::Write;
use std::sync::Arc;
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use zanto_core::chat::{chat, ChatConfig};
use zanto_core::config::Settings;
use zanto_core::permissions::{ApprovalResponse, Approver, PermissionGuard};
use zanto_core::session::{
    auto_title, format_ts_display, ContextPolicy, Session, Store,
};

// ---- CLI definition ----

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

    /// Resume a specific session by ID or prefix
    #[arg(short, long)]
    session: Option<String>,

    /// Force a new session instead of resuming the last one
    #[arg(short = 'n', long)]
    new: bool,

    /// Title for a new session
    #[arg(short, long)]
    title: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage conversation sessions
    Sessions {
        #[command(subcommand)]
        action: SessionAction,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List sessions (current workspace by default)
    List {
        /// Show sessions from all workspaces
        #[arg(long)]
        all: bool,
    },
    /// Print all messages in a session
    Show { id: String },
    /// Delete a session
    Delete { id: String },
    /// Delete sessions (current workspace by default)
    Clear {
        /// Clear sessions from all workspaces
        #[arg(long)]
        all: bool,
    },
}

// ---- StdinApprover ----

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

// ---- Entry point ----

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let settings = Settings::load();

    let workspace = std::fs::canonicalize(".")
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    // Handle sessions subcommand — no LLM needed
    if let Some(Commands::Sessions { action }) = args.command {
        handle_sessions(action, &workspace);
        return;
    }

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

    let policy = match settings.max_context_turns {
        Some(n) => ContextPolicy::LastNTurns { max_turns: n },
        None => ContextPolicy::default(),
    };

    let permissions = Arc::new(PermissionGuard::new(&settings, StdinApprover));
    let store = match Store::open() {
        Ok(s) => s,
        Err(e) => { eprintln!("Error opening session DB: {e}"); return; }
    };

    match args.question {
        Some(q) => {
            let mut session = resolve_session(&store, &workspace, args.session, args.new, args.title);
            run_once(&store, &mut session, model, endpoint, &permissions, &policy, &q).await;
            finalize_session(&store, &mut session);
        }
        None => {
            let mut session = resolve_session(&store, &workspace, args.session, args.new, args.title);
            run_interactive(&store, &mut session, model, endpoint, &permissions, &policy).await;
            finalize_session(&store, &mut session);
        }
    }
}

// ---- Session helpers ----

fn resolve_session(
    store: &Store,
    workspace: &str,
    session_arg: Option<String>,
    force_new: bool,
    title: Option<String>,
) -> Session {
    if force_new {
        return Session::new(title.unwrap_or_default(), workspace);
    }

    if let Some(ref id_or_prefix) = session_arg {
        match store.find_by_prefix(id_or_prefix) {
            Ok(id) => {
                if let Ok(s) = store.load_session(&id) {
                    eprintln!("[zanto] resumed session: {} — {}", s.id, s.title);
                    return s;
                }
            }
            Err(e) => eprintln!("[zanto] session not found: {e}"),
        }
    }

    if let Some(id) = store.last_session_id(Some(workspace)) {
        if let Ok(s) = store.load_session(&id) {
            eprintln!("[zanto] resumed session: {} — {}", s.id, s.title);
            return s;
        }
    }

    Session::new(title.unwrap_or_default(), workspace)
}

fn finalize_session(store: &Store, session: &mut Session) {
    if session.title.is_empty() {
        session.title = auto_title(&session.messages);
    }
    session.updated_at = zanto_core::session::unix_now_pub();
    if let Err(e) = store.save_session(session) {
        eprintln!("[zanto] warning: failed to save session: {e}");
    }
}

// ---- Chat runners ----

async fn run_once(
    store: &Store,
    session: &mut Session,
    model: String,
    endpoint: &'static str,
    permissions: &Arc<PermissionGuard>,
    policy: &ContextPolicy,
    question: &str,
) {
    let config = ChatConfig { model, endpoint, permissions: Arc::clone(permissions) };
    match chat(config, store, session, question, policy).await {
        Ok(answer) => println!("{answer}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}

async fn run_interactive(
    store: &Store,
    session: &mut Session,
    model: String,
    endpoint: &'static str,
    permissions: &Arc<PermissionGuard>,
    policy: &ContextPolicy,
) {
    use tokio::io::AsyncBufReadExt;

    println!("zanto interactive — Ctrl+D or 'exit' to quit");

    let mut reader = tokio::io::BufReader::new(tokio::io::stdin());

    loop {
        print!("\n> ");
        std::io::stdout().flush().ok();

        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Err(e) => { eprintln!("Error reading input: {e}"); break; }
            Ok(_) => {}
        }

        let q = line.trim();
        if q.is_empty() { continue; }
        if q == "exit" || q == "quit" { break; }

        let config = ChatConfig {
            model: model.clone(),
            endpoint,
            permissions: Arc::clone(permissions),
        };

        match chat(config, store, session, q, policy).await {
            Ok(answer) => println!("\n{answer}"),
            Err(e) => eprintln!("Error: {e}"),
        }
    }
}

// ---- Sessions subcommand ----

fn handle_sessions(action: SessionAction, workspace: &str) {
    let store = match Store::open() {
        Ok(s) => s,
        Err(e) => { eprintln!("Error opening session DB: {e}"); return; }
    };

    match action {
        SessionAction::List { all } => {
            let filter = if all { None } else { Some(workspace) };
            match store.list_sessions(filter) {
                Ok(sessions) if sessions.is_empty() => println!("No sessions found."),
                Ok(sessions) => {
                    println!("  {:<22}  {:<43}  {:>4}  UPDATED", "ID", "TITLE", "MSGS");
                    println!("  {:-<22}  {:-<43}  {:-<4}  {:-<16}", "", "", "", "");
                    for s in sessions {
                        let title = truncate(&s.title, 43);
                        println!(
                            "  {:<22}  {:<43}  {:>4}  {}",
                            s.id, title, s.message_count, format_ts_display(s.updated_at)
                        );
                    }
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }

        SessionAction::Show { id } => {
            let id = resolve_id(&store, &id);
            match store.load_session(&id) {
                Ok(session) => {
                    println!("Session: {} — {}", session.id, session.title);
                    println!("Workspace: {}", session.workspace);
                    println!();
                    for msg in &session.messages {
                        let role = format!("{:?}", msg.role).to_lowercase();
                        let text = msg.content.first_text().unwrap_or("[non-text content]");
                        println!("[{role}] {text}");
                        println!();
                    }
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }

        SessionAction::Delete { id } => {
            let id = resolve_id(&store, &id);
            match store.delete_session(&id) {
                Ok(_) => println!("Deleted session {id}."),
                Err(e) => eprintln!("Error: {e}"),
            }
        }

        SessionAction::Clear { all } => {
            let filter = if all { None } else { Some(workspace) };
            match store.clear(filter) {
                Ok(n) => println!("Deleted {n} session(s)."),
                Err(e) => eprintln!("Error: {e}"),
            }
        }
    }
}

fn resolve_id(store: &Store, id_or_prefix: &str) -> String {
    store.find_by_prefix(id_or_prefix).unwrap_or_else(|e| {
        eprintln!("[zanto] {e}");
        id_or_prefix.to_string()
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
