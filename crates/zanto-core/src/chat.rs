use std::sync::Arc;
use async_trait::async_trait;
use futures::future::join_all;
use futures::StreamExt;
use genai::{Client, ServiceTarget};
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, ToolCall, ToolResponse};
// Re-exported so downstream crates (the desktop app) can build tool schemas
// without depending on genai directly.
pub use genai::chat::Tool as GenaiTool;
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::config::{self, Provider};
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

// ---- Streaming sink (provided by the frontend to render a turn live) ----

/// Receives a turn's output incrementally so the UI can render as it arrives.
/// `on_text` carries assistant text deltas; `on_block` carries a finished
/// component block (e.g. a `render_artifact` result). The final `ChatTurn`
/// returned by `chat` is still authoritative; sink calls are a live preview.
#[async_trait]
pub trait ChatSink: Send + Sync {
    /// A streamed assistant text delta.
    async fn on_text(&self, delta: &str);
    /// A component block produced mid-turn (tool/app output).
    async fn on_block(&self, block: &ChatBlock);
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
    /// Dispatcher for non-base tools (shared artifact tools + the app's domain
    /// tools). Base fs/shell tools are always available and handled separately.
    pub app_dispatch: Option<Arc<dyn AppDispatcher>>,
    /// Optional streaming sink. When set, the turn's text deltas and blocks are
    /// emitted live; when `None`, `chat` behaves as a synchronous turn.
    pub sink: Option<Arc<dyn ChatSink>>,
}

impl ChatConfig {
    /// General-mode config (CLI): base fs/shell tools, no app skill/extra tools.
    pub fn new(model: String, endpoint: String, permissions: Arc<PermissionGuard>) -> Self {
        Self {
            model,
            endpoint,
            permissions,
            skill: None,
            extra_tools: Vec::new(),
            app_dispatch: None,
            sink: None,
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
    let provider = config::provider_of(&config.model);
    // Cloud providers resolve their own endpoint via genai; only override it for
    // local Ollama, which genai would otherwise point at localhost instead of the
    // configured host.
    let override_endpoint = provider == Provider::Ollama;
    let target_resolver = ServiceTargetResolver::from_resolver_fn(
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            if !override_endpoint {
                return Ok(service_target);
            }
            let ServiceTarget { endpoint: _, auth, model } = service_target;
            Ok(ServiceTarget { endpoint: Endpoint::from_static(endpoint_str), auth, model })
        },
    );

    // Resolve auth for cloud providers from the keychain/env (Ollama needs none).
    let auth_resolver = AuthResolver::from_resolver_fn(
        move |_model_iden: genai::ModelIden| -> Result<Option<AuthData>, genai::resolver::Error> {
            if provider == Provider::Ollama {
                return Ok(None);
            }
            Ok(config::api_key(provider).map(AuthData::from_single))
        },
    );

    let client = Client::builder()
        .with_service_target_resolver(target_resolver)
        .with_auth_resolver(auth_resolver)
        .build();

    push_msg(store, session, ChatMessage::user(question)).await?;

    let base_prompt =
        "You are a helpful assistant. Use the provided tools to answer questions about the filesystem.";
    let system_text = match &config.skill {
        Some(skill) => format!("{base_prompt}\n\n{skill}"),
        None => base_prompt.to_string(),
    };
    let system_prompt = ChatMessage::system(system_text);

    // Tool schemas offered to the model: shared base fs/shell tools (always) plus
    // the desktop's extra tools (shared artifact tools + the app's domain tools).
    let mut request_tools = ToolService::all_tools();
    request_tools.extend(config.extra_tools.clone());

    // Capture the concatenated content and tool calls at stream end; text is also
    // accumulated per-chunk so the sink can render it live.
    let stream_options = ChatOptions::default()
        .with_capture_content(true)
        .with_capture_tool_calls(true);

    let mut blocks: Vec<ChatBlock> = Vec::new();
    let mut turn = 1;
    loop {
        println!("--- TURN {turn} ---");

        let mut send_messages = vec![system_prompt.clone()];
        send_messages.extend(session.effective_messages(policy));

        let req = ChatRequest::new(send_messages).with_tools(request_tools.clone());
        let res = client
            .exec_chat_stream(&config.model, req, Some(&stream_options))
            .await?;

        // Consume the stream: forward text deltas to the sink, accumulate the full
        // text, and grab the captured tool calls from the terminal event.
        let mut stream = res.stream;
        let mut answer = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        while let Some(event) = stream.next().await {
            match event? {
                ChatStreamEvent::Chunk(chunk) => {
                    if let Some(sink) = &config.sink {
                        sink.on_text(&chunk.content).await;
                    }
                    answer.push_str(&chunk.content);
                }
                ChatStreamEvent::End(end) => {
                    tool_calls = end.captured_into_tool_calls().unwrap_or_default();
                }
                _ => {}
            }
        }

        if tool_calls.is_empty() {
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

        push_msg(store, session, ChatMessage::from(tool_calls.clone())).await?;
        route_tool_calls(&config, &tools, store, session, &tool_calls, &mut blocks).await?;

        turn += 1;
    }
}

/// Execute a turn's tool calls. Base fs/shell tools go to `ToolService` (read-only
/// calls batched concurrently, mutating calls serialized); everything else
/// (shared artifact tools + the app's domain tools) goes to `app_dispatch`, which
/// may return a component block.
async fn route_tool_calls(
    config: &ChatConfig,
    tools: &ToolService,
    store: &Store,
    session: &mut Session,
    calls: &[ToolCall],
    blocks: &mut Vec<ChatBlock>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut read_batch: Vec<&ToolCall> = Vec::new();

    for call in calls {
        if ToolService::owns(&call.fn_name) {
            if ToolService::is_readonly(&call.fn_name) {
                read_batch.push(call);
                continue;
            }
            // Mutating base tool: drain pending reads first, then run it.
            flush_parallel(tools, store, session, &read_batch).await?;
            read_batch.clear();
            println!("[TOOL CALL mutating] {} ({:?})", call.fn_name, call.fn_arguments);
            let output = tools.dispatch(&call.fn_name, call.fn_arguments.clone()).await?;
            println!("[TOOL OUTPUT] {}", &output[..output.len().min(120)]);
            push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), output))).await?;
        } else {
            // Non-base tool (artifact / domain): drain reads, then dispatch to the app.
            flush_parallel(tools, store, session, &read_batch).await?;
            read_batch.clear();
            let resp_text = match &config.app_dispatch {
                Some(disp) => match disp.dispatch(&call.fn_name, call.fn_arguments.clone()).await {
                    Some(Ok(AppResult::Data(v))) => v.to_string(),
                    Some(Ok(AppResult::Block { component_id, data, target })) => {
                        let block = ChatBlock::Component { component_id, data: data.clone(), target };
                        if let Some(sink) = &config.sink {
                            sink.on_block(&block).await;
                        }
                        blocks.push(block);
                        data.to_string()
                    }
                    Some(Err(e)) => format!("error: {e}"),
                    None => format!("error: unknown tool {}", call.fn_name),
                },
                None => format!("error: unknown tool {}", call.fn_name),
            };
            push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), resp_text))).await?;
        }
    }

    flush_parallel(tools, store, session, &read_batch).await
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
