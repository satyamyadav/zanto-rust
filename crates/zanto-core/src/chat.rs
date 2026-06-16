use std::sync::Arc;
use async_trait::async_trait;
use futures::future::join_all;
use genai::{Client, ServiceTarget};
use genai::chat::{ChatMessage, ChatRequest, ToolCall, ToolResponse};
// Re-exported so downstream crates (the desktop app) can build tool schemas
// without depending on genai directly.
pub use genai::chat::Tool as GenaiTool;
use genai::resolver::{Endpoint, ServiceTargetResolver};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::permissions::PermissionGuard;
use crate::session::{ContextPolicy, Session, Store};
use crate::tools::ToolService;

// ---- Turn output (chat-block protocol) ----

/// The result of a chat turn: an ordered list of blocks. Generic prose is markdown;
/// app responses are component blocks (`{component_id, data}`) rendered by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub blocks: Vec<ChatBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatBlock {
    Markdown { text: String },
    Component { component_id: String, data: Value, target: Target },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Inline,
    Canvas,
}

impl ChatTurn {
    /// Concatenated markdown text (components ignored) — for the CLI / back-compat.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                ChatBlock::Markdown { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---- App tool dispatch (provided by the desktop in app mode) ----

/// Result of an app agent-tool: plain data fed back to the model, or a component
/// block rendered to the user. The app maps tool output to its component
/// deterministically (small-model-safe); the model only picks `target`.
pub enum AppResult {
    Data(Value),
    Block { component_id: String, data: Value, target: Target },
}

#[async_trait]
pub trait AppDispatcher: Send + Sync {
    /// Returns `Some(result)` if this dispatcher owns `name`; `None` if not its tool.
    async fn dispatch(&self, name: &str, args: Value) -> Option<Result<AppResult, String>>;
}

// ---- Config ----

pub struct ChatConfig {
    pub model: String,
    pub endpoint: String,
    pub permissions: Arc<PermissionGuard>,
    /// Extra system-prompt text (the active app's skill). Appended to the base prompt.
    pub skill: Option<String>,
    /// Extra tool schemas (the active app's agent tools).
    pub extra_tools: Vec<GenaiTool>,
    /// App tool dispatcher (app mode). When set with `include_base_tools = false`,
    /// all tool calls route here.
    pub app_dispatch: Option<Arc<dyn AppDispatcher>>,
    /// Include the built-in fs/shell tools (general mode). App mode sets this false.
    pub include_base_tools: bool,
}

impl ChatConfig {
    /// General-mode config (CLI): base fs/shell tools, no app skill/tools.
    pub fn new(model: String, endpoint: String, permissions: Arc<PermissionGuard>) -> Self {
        Self {
            model,
            endpoint,
            permissions,
            skill: None,
            extra_tools: Vec::new(),
            app_dispatch: None,
            include_base_tools: true,
        }
    }
}

// ---- Chat loop ----

pub async fn chat(
    config: ChatConfig,
    store: &Store,
    session: &mut Session,
    question: &str,
    policy: &ContextPolicy,
) -> Result<ChatTurn, Box<dyn std::error::Error + Send + Sync>> {
    let tools = ToolService::new(Arc::clone(&config.permissions));

    // Ensure the session row exists before appending messages
    session.updated_at = crate::session::unix_now_pub();
    store.save_session(session)?;

    // genai's resolver needs a 'static endpoint; leak the (rarely-changing) value.
    let endpoint_str: &'static str = Box::leak(config.endpoint.clone().into_boxed_str());
    // Cloud models (e.g. gemini-*) resolve their own endpoint + auth via genai
    // (API key from GEMINI_API_KEY). Only override the endpoint for local Ollama,
    // which genai would otherwise point at localhost instead of the configured host.
    let override_endpoint = !config.model.starts_with("gemini");
    let target_resolver = ServiceTargetResolver::from_resolver_fn(
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            if !override_endpoint {
                return Ok(service_target);
            }
            let ServiceTarget { endpoint: _, auth, model } = service_target;
            Ok(ServiceTarget { endpoint: Endpoint::from_static(endpoint_str), auth, model })
        },
    );

    let client = Client::builder()
        .with_service_target_resolver(target_resolver)
        .build();

    push_msg(store, session, ChatMessage::user(question)).await?;

    let base_prompt =
        "You are a helpful assistant. Use the provided tools to answer questions about the filesystem.";
    let system_text = match &config.skill {
        Some(skill) => format!("{base_prompt}\n\n{skill}"),
        None => base_prompt.to_string(),
    };
    let system_prompt = ChatMessage::system(system_text);

    // Tool schemas offered to the model: base (general mode) + the active app's tools.
    let mut request_tools = if config.include_base_tools {
        ToolService::all_tools()
    } else {
        Vec::new()
    };
    request_tools.extend(config.extra_tools.clone());

    let mut blocks: Vec<ChatBlock> = Vec::new();
    let mut turn = 1;
    loop {
        println!("--- TURN {turn} ---");

        let mut send_messages = vec![system_prompt.clone()];
        send_messages.extend(session.effective_messages(policy));

        let req = ChatRequest::new(send_messages).with_tools(request_tools.clone());
        let res = client.exec_chat(&config.model, req, None).await?;

        if res.tool_calls().is_empty() {
            let answer = res.first_text().unwrap_or_default().to_string();

            // Fallback: some models (e.g. qwen2.5 via Ollama) emit tool calls as raw
            // JSON text instead of structured calls. Detect and execute them.
            let fallback = extract_raw_tool_calls(&answer);
            if !fallback.is_empty() {
                eprintln!("[zanto] warn: model returned unparsed tool call(s), applying fallback parser");
                push_msg(store, session, ChatMessage::from(fallback.clone())).await?;
                route_tool_calls(&config, &tools, store, session, &fallback, &mut blocks).await?;
                turn += 1;
                continue;
            }

            push_msg(store, session, ChatMessage::assistant(answer.clone())).await?;
            if !answer.trim().is_empty() || blocks.is_empty() {
                blocks.push(ChatBlock::Markdown { text: answer });
            }
            return Ok(ChatTurn { blocks });
        }

        let tool_calls = res.into_tool_calls();
        push_msg(store, session, ChatMessage::from(tool_calls.clone())).await?;
        route_tool_calls(&config, &tools, store, session, &tool_calls, &mut blocks).await?;

        turn += 1;
    }
}

/// Route tool calls to the base tool service (general mode) or the app dispatcher
/// (app mode). General and app modes are mutually exclusive for a given turn.
async fn route_tool_calls(
    config: &ChatConfig,
    tools: &ToolService,
    store: &Store,
    session: &mut Session,
    calls: &[ToolCall],
    blocks: &mut Vec<ChatBlock>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if config.include_base_tools {
        execute_tool_calls(tools, store, session, calls).await
    } else if let Some(disp) = &config.app_dispatch {
        execute_app_tool_calls(disp.as_ref(), store, session, calls, blocks).await
    } else {
        for call in calls {
            push_msg(
                store,
                session,
                ChatMessage::from(ToolResponse::new(call.call_id.clone(), "error: no tools available".to_string())),
            )
            .await?;
        }
        Ok(())
    }
}

/// Append a message to the session and persist it.
async fn push_msg(
    store: &Store,
    session: &mut Session,
    msg: ChatMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pos = session.messages.len();
    session.messages.push(msg);
    store.append_message(&session.id, pos, &session.messages[pos])?;
    Ok(())
}

// ---- App-mode execution (sequential; collects component blocks) ----

async fn execute_app_tool_calls(
    dispatch: &dyn AppDispatcher,
    store: &Store,
    session: &mut Session,
    tool_calls: &[ToolCall],
    blocks: &mut Vec<ChatBlock>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for call in tool_calls {
        let resp_text = match dispatch.dispatch(&call.fn_name, call.fn_arguments.clone()).await {
            Some(Ok(AppResult::Data(v))) => v.to_string(),
            Some(Ok(AppResult::Block { component_id, data, target })) => {
                blocks.push(ChatBlock::Component { component_id, data: data.clone(), target });
                // Feed the data back so the model can summarize/continue.
                data.to_string()
            }
            Some(Err(e)) => format!("error: {e}"),
            None => format!("error: unknown tool {}", call.fn_name),
        };
        push_msg(
            store,
            session,
            ChatMessage::from(ToolResponse::new(call.call_id.clone(), resp_text)),
        )
        .await?;
    }
    Ok(())
}

// ---- General-mode execution (read batch concurrent, writes sequential) ----

async fn execute_tool_calls(
    tools: &ToolService,
    store: &Store,
    session: &mut Session,
    tool_calls: &[ToolCall],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut read_batch: Vec<&ToolCall> = Vec::new();

    for call in tool_calls {
        if ToolService::is_readonly(&call.fn_name) {
            read_batch.push(call);
        } else {
            flush_parallel(tools, store, session, &read_batch).await?;
            read_batch.clear();

            println!("[TOOL CALL mutating] {} ({:?})", call.fn_name, call.fn_arguments);
            let output = tools.dispatch(&call.fn_name, call.fn_arguments.clone()).await?;
            println!("[TOOL OUTPUT] {}", &output[..output.len().min(120)]);
            push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), output))).await?;
        }
    }

    flush_parallel(tools, store, session, &read_batch).await
}

async fn flush_parallel(
    tools: &ToolService,
    store: &Store,
    session: &mut Session,
    batch: &[&ToolCall],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if batch.is_empty() {
        return Ok(());
    }

    println!("[TOOL BATCH {} read-only, concurrent]", batch.len());

    let results = join_all(batch.iter().map(|call| {
        let name = call.fn_name.clone();
        let args = call.fn_arguments.clone();
        async move { tools.dispatch(&name, args).await }
    }))
    .await;

    for (call, result) in batch.iter().zip(results) {
        let output = result?;
        println!("[TOOL OUTPUT] {} → {}", call.fn_name, &output[..output.len().min(120)]);
        push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), output))).await?;
    }

    Ok(())
}

/// Scan `text` for JSON objects with `name` + `arguments` keys that look like
/// tool calls the model produced in raw text form (genai failed to parse them).
fn extract_raw_tool_calls(text: &str) -> Vec<ToolCall> {
    let mut result = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }

        let start = i;
        let mut depth = 0i32;
        let mut j = i;
        let mut found_end = false;

        while j < bytes.len() {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        found_end = true;
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }

        if found_end {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text[start..=j]) {
                if let (Some(name), Some(args)) =
                    (v.get("name").and_then(|n| n.as_str()), v.get("arguments"))
                {
                    result.push(ToolCall {
                        call_id: format!("fallback-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]),
                        fn_name: name.to_string(),
                        fn_arguments: args.clone(),
                        thought_signatures: None,
                    });
                }
            }
            i = j + 1;
        } else {
            break;
        }
    }

    result
}
