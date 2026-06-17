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

/// A tool call about to be dispatched, surfaced to the sink for live rendering.
pub struct ToolCallView {
    pub id: String,
    pub name: String,
    pub args: Value,
}

/// Receives a turn's output incrementally so the UI can render as it arrives.
/// `on_text` carries assistant text deltas; `on_block` carries a finished
/// component block (e.g. a `render_artifact` result). The reasoning / tool-call /
/// tool-result methods carry typed segments (default no-op so existing sinks keep
/// compiling). The final `ChatTurn` returned by `chat` is still authoritative;
/// sink calls are a live preview.
#[async_trait]
pub trait ChatSink: Send + Sync {
    /// A streamed assistant text delta.
    async fn on_text(&self, delta: &str);
    /// A streamed reasoning ("thinking") delta.
    async fn on_reasoning(&self, _delta: &str) {}
    /// A tool call about to be dispatched.
    async fn on_tool_call(&self, _call: &ToolCallView) {}
    /// A tool result, with its call id, output (or error text), and success flag.
    async fn on_tool_result(&self, _id: &str, _output: &str, _ok: bool) {}
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
    /// Loaded user context sources (see `context::load_context`). Injected into the
    /// system prompt after the system-info block. `None` when no sources configured.
    pub context: Option<String>,
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
            context: None,
            extra_tools: Vec::new(),
            app_dispatch: None,
            sink: None,
        }
    }
}

// ---- System prompt ----

/// Compose the system prompt from its parts, in order:
/// 1. `base` — the base instruction prompt.
/// 2. `system_info` — A2's host/date block, under a `--- system ---` header.
/// 3. `context` — loaded user context sources (if any), under `--- context ---`.
/// 4. `skill` — the active app skill / selected preprompt, under `--- skill ---`.
///
/// Empty/absent sections are omitted. Pure function — unit-testable.
pub fn build_system_prompt(
    base: &str,
    system_info: &str,
    context: Option<&str>,
    skill: Option<&str>,
) -> String {
    let mut out = base.trim().to_string();

    let mut section = |header: &str, body: &str| {
        let body = body.trim();
        if body.is_empty() {
            return;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(header);
        out.push('\n');
        out.push_str(body);
    };

    section("--- system ---", system_info);
    if let Some(ctx) = context {
        section("--- context ---", ctx);
    }
    if let Some(sk) = skill {
        section("--- skill ---", sk);
    }

    out
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

    // Running-summary trigger (ContextPolicy::Summarize): once per turn, fold the
    // history beyond the last `keep_last` turns into the session's stored summary so
    // `effective_messages` can prepend it. A summarize failure is logged and skipped
    // — the turn proceeds without an updated summary rather than aborting.
    if let ContextPolicy::Summarize { keep_last } = policy {
        let turn_count = session
            .messages
            .iter()
            .filter(|m| matches!(m.role, genai::chat::ChatRole::User))
            .count();
        if crate::summarize::should_summarize(turn_count, *keep_last) {
            let older = crate::session::messages_before_last_turns(&session.messages, *keep_last);
            if !older.is_empty() {
                match crate::summarize::summarize_messages(&client, &config.model, &older).await {
                    Ok(summary) if !summary.trim().is_empty() => {
                        store.set_summary(&session.id, Some(&summary))?;
                        session.summary = Some(summary);
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("[zanto] warn: summarize failed, proceeding without summary: {e}"),
                }
            }
        }
    }

    let base_prompt =
        "You are a helpful assistant. Use the provided tools to answer questions about the filesystem. \
When a user message contains an @<path> token, treat it as a request to read that file with the read_file tool before answering.";
    let system_text = build_system_prompt(
        base_prompt,
        &crate::session::system_info(),
        config.context.as_deref(),
        config.skill.as_deref(),
    );
    let system_prompt = ChatMessage::system(system_text);

    // Tool schemas offered to the model: shared base fs/shell tools (always) plus
    // the desktop's extra tools (shared artifact tools + the app's domain tools).
    let mut request_tools = ToolService::all_tools();
    request_tools.extend(config.extra_tools.clone());

    // Capture the concatenated content and tool calls at stream end; text is also
    // accumulated per-chunk so the sink can render it live.
    let stream_options = ChatOptions::default()
        .with_capture_content(true)
        .with_capture_tool_calls(true)
        .with_capture_reasoning_content(true);

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
                ChatStreamEvent::ReasoningChunk(chunk) => {
                    if let Some(sink) = &config.sink {
                        sink.on_reasoning(&chunk.content).await;
                    }
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

            // Persist the turn's component blocks (the markdown text is already in
            // `content`) so reopening the thread restores the artifacts, not just text.
            let meta = component_blocks_meta(&blocks);
            push_msg_meta(store, session, ChatMessage::assistant(answer.clone()), meta.as_ref()).await?;
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
            flush_parallel(config, tools, store, session, &read_batch).await?;
            read_batch.clear();
            println!("[TOOL CALL mutating] {} ({:?})", call.fn_name, call.fn_arguments);
            emit_tool_call(config, call).await;
            let output = tools.dispatch(&call.fn_name, call.fn_arguments.clone()).await?;
            println!("[TOOL OUTPUT] {}", &output[..output.len().min(120)]);
            emit_tool_result(config, &call.call_id, &output, true).await;
            push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), output))).await?;
        } else {
            // Non-base tool (artifact / domain): drain reads, then dispatch to the app.
            flush_parallel(config, tools, store, session, &read_batch).await?;
            read_batch.clear();
            emit_tool_call(config, call).await;
            let (resp_text, ok) = match &config.app_dispatch {
                Some(disp) => match disp.dispatch(&call.fn_name, call.fn_arguments.clone()).await {
                    Some(Ok(AppResult::Data(v))) => (v.to_string(), true),
                    Some(Ok(AppResult::Block { component_id, data, target })) => {
                        let block = ChatBlock::Component { component_id, data: data.clone(), target };
                        if let Some(sink) = &config.sink {
                            sink.on_block(&block).await;
                        }
                        blocks.push(block);
                        (data.to_string(), true)
                    }
                    Some(Err(e)) => (format!("error: {e}"), false),
                    None => (format!("error: unknown tool {}", call.fn_name), false),
                },
                None => (format!("error: unknown tool {}", call.fn_name), false),
            };
            emit_tool_result(config, &call.call_id, &resp_text, ok).await;
            push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), resp_text))).await?;
        }
    }

    flush_parallel(config, tools, store, session, &read_batch).await
}

/// Notify the sink (if any) that a tool call is about to be dispatched.
async fn emit_tool_call(config: &ChatConfig, call: &ToolCall) {
    if let Some(sink) = &config.sink {
        sink.on_tool_call(&ToolCallView {
            id: call.call_id.clone(),
            name: call.fn_name.clone(),
            args: call.fn_arguments.clone(),
        })
        .await;
    }
}

/// Notify the sink (if any) that a tool call finished, with its output and outcome.
async fn emit_tool_result(config: &ChatConfig, id: &str, output: &str, ok: bool) {
    if let Some(sink) = &config.sink {
        sink.on_tool_result(id, output, ok).await;
    }
}

/// Append a message to the session and persist it.
async fn push_msg(
    store: &Store,
    session: &mut Session,
    msg: ChatMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    push_msg_meta(store, session, msg, None).await
}

/// Append a message to the session and persist it, with optional metadata JSON
/// (`None` is equivalent to `push_msg` — no metadata column written).
async fn push_msg_meta(
    store: &Store,
    session: &mut Session,
    msg: ChatMessage,
    metadata: Option<&Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pos = session.messages.len();
    session.messages.push(msg);
    store.append_message_meta(&session.id, pos, &session.messages[pos], metadata)?;
    Ok(())
}

/// Build the persisted metadata for an assistant turn: a `{"blocks": [...]}`
/// object holding only this turn's `Component` blocks (markdown text lives in the
/// message `content`). Returns `None` when the turn produced no component blocks,
/// so plain text turns keep an empty metadata column.
fn component_blocks_meta(blocks: &[ChatBlock]) -> Option<Value> {
    let components: Vec<&ChatBlock> = blocks
        .iter()
        .filter(|b| matches!(b, ChatBlock::Component { .. }))
        .collect();
    if components.is_empty() {
        return None;
    }
    Some(serde_json::json!({ "blocks": components }))
}

async fn flush_parallel(
    config: &ChatConfig,
    tools: &ToolService,
    store: &Store,
    session: &mut Session,
    batch: &[&ToolCall],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if batch.is_empty() {
        return Ok(());
    }

    println!("[TOOL BATCH {} read-only, concurrent]", batch.len());

    // Surface each read call before the concurrent batch runs.
    for call in batch {
        emit_tool_call(config, call).await;
    }

    let results = join_all(batch.iter().map(|call| {
        let name = call.fn_name.clone();
        let args = call.fn_arguments.clone();
        async move { tools.dispatch(&name, args).await }
    }))
    .await;

    for (call, result) in batch.iter().zip(results) {
        let output = result?;
        println!("[TOOL OUTPUT] {} → {}", call.fn_name, &output[..output.len().min(120)]);
        emit_tool_result(config, &call.call_id, &output, true).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_system_prompt_orders_sections() {
        let out = build_system_prompt(
            "BASE",
            "System: linux",
            Some("--- context: notes.md ---\nhello"),
            Some("be a planner"),
        );

        // All sections present.
        assert!(out.contains("BASE"));
        assert!(out.contains("--- system ---"));
        assert!(out.contains("System: linux"));
        assert!(out.contains("--- context ---"));
        assert!(out.contains("hello"));
        assert!(out.contains("--- skill ---"));
        assert!(out.contains("be a planner"));

        // Ordering: base < system < context < skill.
        let base = out.find("BASE").unwrap();
        let sys = out.find("--- system ---").unwrap();
        let ctx = out.find("--- context ---").unwrap();
        let skill = out.find("--- skill ---").unwrap();
        assert!(base < sys && sys < ctx && ctx < skill);
    }

    // A sink that records the order and payload of segment callbacks.
    #[derive(Default)]
    struct RecordingSink {
        events: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait]
    impl ChatSink for RecordingSink {
        async fn on_text(&self, _delta: &str) {}
        async fn on_block(&self, _block: &ChatBlock) {}
        async fn on_tool_call(&self, call: &ToolCallView) {
            self.events.lock().unwrap().push(format!("call:{}", call.id));
        }
        async fn on_tool_result(&self, id: &str, _output: &str, ok: bool) {
            self.events.lock().unwrap().push(format!("result:{id}:{ok}"));
        }
    }

    // An app dispatcher that owns one stub tool returning plain data.
    struct StubDispatcher;

    #[async_trait]
    impl AppDispatcher for StubDispatcher {
        async fn dispatch(&self, name: &str, _args: Value) -> Option<Result<AppResult, String>> {
            if name == "stub_tool" {
                Some(Ok(AppResult::Data(serde_json::json!({"ok": true}))))
            } else {
                None
            }
        }
    }

    struct DenyApprover;

    #[async_trait]
    impl crate::permissions::Approver for DenyApprover {
        async fn confirm(
            &self,
            _path: &str,
            _op: &str,
            _resolved: &str,
        ) -> crate::permissions::ApprovalResponse {
            crate::permissions::ApprovalResponse::Deny
        }
    }

    #[tokio::test]
    async fn route_tool_calls_emits_call_before_result() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open_at(&dir.path().join("test.db")).unwrap();
        let mut session = Session::new("test", dir.path().to_str().unwrap());
        store.save_session(&session).unwrap();

        let permissions = Arc::new(PermissionGuard::new(
            &crate::config::Settings::default(),
            DenyApprover,
        ));
        let tools = ToolService::new(Arc::clone(&permissions));
        let sink = Arc::new(RecordingSink::default());

        let mut config = ChatConfig::new(
            "stub-model".to_string(),
            "http://localhost".to_string(),
            permissions,
        );
        config.app_dispatch = Some(Arc::new(StubDispatcher));
        config.sink = Some(Arc::clone(&sink) as Arc<dyn ChatSink>);

        let call = ToolCall {
            call_id: "call-1".to_string(),
            fn_name: "stub_tool".to_string(),
            fn_arguments: serde_json::json!({}),
            thought_signatures: None,
        };

        let mut blocks = Vec::new();
        route_tool_calls(&config, &tools, &store, &mut session, &[call], &mut blocks)
            .await
            .unwrap();

        let events = sink.events.lock().unwrap().clone();
        assert_eq!(events, vec!["call:call-1".to_string(), "result:call-1:true".to_string()]);
    }

    #[test]
    fn component_blocks_meta_filters_to_components() {
        // Markdown-only turn → no metadata.
        let only_text = vec![ChatBlock::Markdown { text: "hi".into() }];
        assert!(component_blocks_meta(&only_text).is_none());

        // Mixed turn → metadata holds only the component block(s).
        let mixed = vec![
            ChatBlock::Markdown { text: "see chart".into() },
            ChatBlock::Component {
                component_id: "chart".into(),
                data: serde_json::json!({ "points": [1, 2, 3] }),
                target: Target::Canvas,
            },
        ];
        let meta = component_blocks_meta(&mixed).expect("component present");
        let arr = meta["blocks"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["kind"], "component");
        assert_eq!(arr[0]["component_id"], "chart");
    }

    #[tokio::test]
    async fn assistant_component_block_round_trips_through_store() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open_at(&dir.path().join("test.db")).unwrap();
        let mut session = Session::new("test", dir.path().to_str().unwrap());
        store.save_session(&session).unwrap();

        let blocks = vec![ChatBlock::Component {
            component_id: "chart".into(),
            data: serde_json::json!({ "points": [1, 2, 3] }),
            target: Target::Canvas,
        }];
        let meta = component_blocks_meta(&blocks);

        // Persist a plain user turn, then the assistant turn carrying block metadata.
        push_msg(&store, &mut session, ChatMessage::user("plot it")).await.unwrap();
        push_msg_meta(&store, &mut session, ChatMessage::assistant("here you go"), meta.as_ref())
            .await
            .unwrap();

        // Reload metadata aligned by position; the user row has none, the assistant
        // row carries the component block, which deserializes back to a ChatBlock.
        let loaded = store.load_message_meta(&session.id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded[0].is_none());

        let stored = loaded[1].as_ref().expect("assistant metadata present");
        let restored: Vec<ChatBlock> =
            serde_json::from_value(stored["blocks"].clone()).unwrap();
        assert_eq!(restored.len(), 1);
        match &restored[0] {
            ChatBlock::Component { component_id, data, target } => {
                assert_eq!(component_id, "chart");
                assert_eq!(data, &serde_json::json!({ "points": [1, 2, 3] }));
                assert_eq!(*target, Target::Canvas);
            }
            _ => panic!("expected component block"),
        }
    }

    #[test]
    fn build_system_prompt_omits_empty_sections() {
        let out = build_system_prompt("BASE", "System: linux", None, None);
        assert!(out.contains("BASE"));
        assert!(out.contains("--- system ---"));
        assert!(!out.contains("--- context ---"));
        assert!(!out.contains("--- skill ---"));

        // Empty/whitespace strings are treated as absent.
        let out2 = build_system_prompt("BASE", "", Some("   "), Some(""));
        assert_eq!(out2, "BASE");
    }
}
