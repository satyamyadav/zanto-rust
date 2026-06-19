use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use async_trait::async_trait;
use futures::future::join_all;
use futures::StreamExt;
use genai::{Client, ServiceTarget};
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, ContentPart, MessageContent, ToolCall, ToolResponse};
// Re-exported so downstream crates (the desktop app) can build tool schemas
// without depending on genai directly.
pub use genai::chat::Tool as GenaiTool;
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::config::{self, Provider};
use genai::adapter::AdapterKind;
use crate::permissions::PermissionGuard;
use crate::session::{ContextPolicy, Session, Store};
use crate::tools::ToolService;

// ---- Turn output (chat-block protocol) ----

/// The result of a chat turn: an ordered list of blocks. Generic prose is markdown;
/// app responses are component blocks (`{component_id, data}`) rendered by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub blocks: Vec<ChatBlock>,
    /// True when the turn was interrupted (cancel flag set); the blocks hold the
    /// partial output collected before the stop. Default false.
    #[serde(default)]
    pub stopped: bool,
    /// True when older history was folded into the running summary this turn
    /// (ContextPolicy::Auto/Summarize) — drives the "summarized to fit" indicator.
    #[serde(default)]
    pub summarized: bool,
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

/// An image attached to a turn, sent to the model as vision content (a base64
/// binary content part on the first user message). The caller is responsible for
/// gating by provider capability — `chat()` attaches whatever it is given.
#[derive(Debug, Clone)]
pub struct ImageAttachment {
    /// MIME type, e.g. "image/png".
    pub mime: String,
    /// Base64-encoded image bytes (no data-URL prefix).
    pub b64: String,
}

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
    /// Optional per-turn cancel flag. When set to `true` mid-turn, `chat` stops at
    /// the next safe check point and returns the partial turn with `stopped = true`.
    /// `None` (the default) means the turn cannot be interrupted.
    pub cancel: Option<Arc<AtomicBool>>,
    /// Images to attach to this turn as vision content on the first user message.
    /// Empty (the default) means a plain text user message. The caller decides
    /// whether the active provider can read images before populating this.
    pub images: Vec<ImageAttachment>,
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
            cancel: None,
            images: Vec::new(),
        }
    }
}

/// True when this config's cancel flag has been set (the turn should stop).
fn cancelled(c: &ChatConfig) -> bool {
    c.cancel.as_ref().map_or(false, |f| f.load(Ordering::SeqCst))
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
    mut config: ChatConfig,
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
    let override_endpoint = provider == Provider(AdapterKind::Ollama);
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
            if provider == Provider(AdapterKind::Ollama) {
                return Ok(None);
            }
            Ok(config::api_key(provider).map(AuthData::from_single))
        },
    );

    let client = Client::builder()
        .with_service_target_resolver(target_resolver)
        .with_auth_resolver(auth_resolver)
        .build();

    // Build the first user message. With image attachments and a multimodal
    // provider, send a multipart message (text + one image binary part each) so
    // the model sees the images as vision input. Otherwise a plain text message.
    let user_msg = if config.images.is_empty() {
        ChatMessage::user(question)
    } else {
        let mut parts: Vec<ContentPart> = Vec::with_capacity(config.images.len() + 1);
        if !question.is_empty() {
            parts.push(ContentPart::from_text(question));
        }
        // Move each base64 payload into the content part (no clone of the bytes).
        for img in std::mem::take(&mut config.images) {
            parts.push(ContentPart::from_binary_base64(img.mime, img.b64, None));
        }
        ChatMessage::user(MessageContent::from_parts(parts))
    };
    push_msg(store, session, user_msg).await?;

    // Running-summary trigger: once per turn, fold the history that falls outside
    // the live window into the session's stored summary so `effective_messages` can
    // prepend it. `Summarize` drops by a fixed turn count; `Auto` drops by estimated
    // token usage against the model's window. A summarize failure is logged and
    // skipped — the turn proceeds without an updated summary rather than aborting.
    let mut summarized = false;
    let older: Vec<ChatMessage> = match policy {
        ContextPolicy::Summarize { keep_last } => {
            let turn_count = session
                .messages
                .iter()
                .filter(|m| matches!(m.role, genai::chat::ChatRole::User))
                .count();
            if crate::summarize::should_summarize(turn_count, *keep_last) {
                crate::session::messages_before_last_turns(&session.messages, *keep_last)
            } else {
                Vec::new()
            }
        }
        ContextPolicy::Auto { window_tokens, headroom_frac } => {
            session.auto_older(*window_tokens, *headroom_frac)
        }
        _ => Vec::new(),
    };
    if !older.is_empty() {
        match crate::summarize::summarize_messages(&client, &config.model, &older).await {
            Ok(summary) if !summary.trim().is_empty() => {
                store.set_summary(&session.id, Some(&summary))?;
                session.summary = Some(summary);
                summarized = true;
            }
            Ok(_) => {}
            Err(e) => eprintln!("[zanto] warn: summarize failed, proceeding without summary: {e}"),
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
    // Ordered display segments for this assistant turn (reasoning/tool_call/block/
    // text), assembled as events occur so a reopened turn restores exactly.
    let mut display = TurnDisplay::default();
    let mut turn = 1;
    loop {
        // Check point 1 — loop top: if cancelled before building the next request,
        // persist whatever we have and return a stopped turn without a model call.
        if cancelled(&config) {
            let meta = assistant_turn_meta(&display, &blocks, true);
            push_msg_meta(store, session, ChatMessage::assistant(String::new()), meta.as_ref()).await?;
            return Ok(ChatTurn { blocks, stopped: true, summarized });
        }

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
            // Check point 2 — per stream event: on cancel, drop the stream (which
            // aborts the genai/reqwest request) and break out.
            if cancelled(&config) {
                break;
            }
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
                    // Accumulate reasoning into the persisted display segments
                    // (coalescing consecutive deltas) — dropped before this change.
                    display.push_reasoning(&chunk.content);
                }
                ChatStreamEvent::End(end) => {
                    tool_calls = end.captured_into_tool_calls().unwrap_or_default();
                }
                _ => {}
            }
        }
        // Dropping the stream aborts an in-flight request; do it before returning.
        drop(stream);

        // Record this iteration's prose in the display segments NOW, in document
        // order — before any tool calls of this iteration. The live frontend pushes
        // text segments via `onChatChunk` as they stream (interleaved with tool
        // calls across iterations); appending only at the terminal branch would drop
        // every intermediate tool-loop iteration's prose. Whitespace-only text is a
        // no-op (push_text skips it).
        display.push_text(&answer);

        // Cancelled mid-stream: persist the partial assistant text and return it.
        if cancelled(&config) {
            let meta = assistant_turn_meta(&display, &blocks, true);
            push_msg_meta(store, session, ChatMessage::assistant(answer.clone()), meta.as_ref()).await?;
            if !answer.trim().is_empty() || blocks.is_empty() {
                blocks.push(ChatBlock::Markdown { text: answer });
            }
            return Ok(ChatTurn { blocks, stopped: true, summarized });
        }

        if tool_calls.is_empty() {
            // Fallback: some models (e.g. qwen2.5 via Ollama) emit tool calls as raw
            // JSON text instead of structured calls. Detect and execute them.
            let fallback = extract_raw_tool_calls(&answer);
            if !fallback.is_empty() {
                eprintln!("[zanto] warn: model returned unparsed tool call(s), applying fallback parser");
                push_msg(store, session, ChatMessage::from(fallback.clone())).await?;
                route_tool_calls(&config, &tools, store, session, &fallback, &mut blocks, &mut display).await?;
                turn += 1;
                continue;
            }

            // Persist the turn's full ordered display segments (reasoning/tool calls/
            // blocks/text) + the stopped flag so reopening restores it exactly. The
            // back-compat `blocks` list rides along for old readers.
            let meta = assistant_turn_meta(&display, &blocks, false);
            push_msg_meta(store, session, ChatMessage::assistant(answer.clone()), meta.as_ref()).await?;
            if !answer.trim().is_empty() || blocks.is_empty() {
                blocks.push(ChatBlock::Markdown { text: answer });
            }
            return Ok(ChatTurn { blocks, stopped: false, summarized });
        }

        push_msg(store, session, ChatMessage::from(tool_calls.clone())).await?;
        route_tool_calls(&config, &tools, store, session, &tool_calls, &mut blocks, &mut display).await?;

        // Check point 3 (coupling) — `route_tool_calls` stops dispatching on cancel;
        // if cancelled, return the partial turn rather than looping into another model
        // request.
        if cancelled(&config) {
            let meta = assistant_turn_meta(&display, &blocks, true);
            push_msg_meta(store, session, ChatMessage::assistant(String::new()), meta.as_ref()).await?;
            return Ok(ChatTurn { blocks, stopped: true, summarized });
        }

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
    display: &mut TurnDisplay,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut read_batch: Vec<&ToolCall> = Vec::new();

    for (i, call) in calls.iter().enumerate() {
        // Stop dispatching on cancel: flush any pending reads (so their tool
        // responses are persisted), then synthesize a response for every
        // not-yet-dispatched call. The assistant message persisted before this fn
        // carries all N tool_use ids; every one needs a matching tool_result or the
        // next request to a strict provider is rejected for an unanswered tool_use.
        if cancelled(config) {
            // Reads in `read_batch` come from calls[..i]; flushing answers them.
            flush_parallel(config, tools, store, session, &read_batch, display).await?;
            // Every call from `i` onward is undispatched — synthesize a tool_result
            // for each so no tool_use id is left unanswered in the persisted log.
            for pending in &calls[i..] {
                let msg = ChatMessage::from(ToolResponse::new(
                    pending.call_id.clone(),
                    "interrupted: turn stopped by user".to_string(),
                ));
                push_msg(store, session, msg).await?;
            }
            return Ok(());
        }
        if ToolService::owns(&call.fn_name) {
            if ToolService::is_readonly(&call.fn_name) {
                read_batch.push(call);
                continue;
            }
            // Mutating base tool: drain pending reads first, then run it.
            flush_parallel(config, tools, store, session, &read_batch, display).await?;
            read_batch.clear();
            println!("[TOOL CALL mutating] {} ({:?})", call.fn_name, call.fn_arguments);
            emit_tool_call(config, display, call).await;
            let output = tools.dispatch(&call.fn_name, call.fn_arguments.clone()).await?;
            println!("[TOOL OUTPUT] {}", &output[..output.len().min(120)]);
            emit_tool_result(config, display, &call.call_id, &output, true).await;
            push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), output))).await?;
        } else {
            // Non-base tool (artifact / domain): drain reads, then dispatch to the app.
            flush_parallel(config, tools, store, session, &read_batch, display).await?;
            read_batch.clear();
            emit_tool_call(config, display, call).await;
            // `llm_text` is the tool result the MODEL sees (full data). `display_text`
            // is what the UI/persistence stores: for a block-rendered tool it's a short
            // reference, because the block segment already carries the data — storing it
            // again in the tool-call output would persist + re-parse a second full copy
            // (review A2 / B5-2).
            let (llm_text, display_text, ok) = match &config.app_dispatch {
                Some(disp) => match disp.dispatch(&call.fn_name, call.fn_arguments.clone()).await {
                    Some(Ok(AppResult::Data(v))) => {
                        let s = v.to_string();
                        (s.clone(), s, true)
                    }
                    Some(Ok(AppResult::Block { component_id, data, target })) => {
                        let reference = format!("[rendered {component_id} block]");
                        let block = ChatBlock::Component { component_id, data: data.clone(), target };
                        if let Some(sink) = &config.sink {
                            sink.on_block(&block).await;
                        }
                        // Record the block segment in document order: after the
                        // tool_call, before its result fill — matching the live sink.
                        display.push_block(&block);
                        // Flag the tool-call segment as block-rendering so the UI hides
                        // its card authoritatively, by this flag — not by tool name (B5-1).
                        display.mark_renders_as_block(&call.call_id);
                        blocks.push(block);
                        (data.to_string(), reference, true)
                    }
                    Some(Err(e)) => {
                        let s = format!("error: {e}");
                        (s.clone(), s, false)
                    }
                    None => {
                        let s = format!("error: unknown tool {}", call.fn_name);
                        (s.clone(), s, false)
                    }
                },
                None => {
                    let s = format!("error: unknown tool {}", call.fn_name);
                    (s.clone(), s, false)
                }
            };
            emit_tool_result(config, display, &call.call_id, &display_text, ok).await;
            push_msg(store, session, ChatMessage::from(ToolResponse::new(call.call_id.clone(), llm_text))).await?;
        }
    }

    flush_parallel(config, tools, store, session, &read_batch, display).await
}

/// Notify the sink (if any) that a tool call is about to be dispatched, and record
/// it in the turn's display segments (in the same order the sink sees it).
async fn emit_tool_call(config: &ChatConfig, display: &mut TurnDisplay, call: &ToolCall) {
    display.push_tool_call(&call.call_id, &call.fn_name, &call.fn_arguments);
    if let Some(sink) = &config.sink {
        sink.on_tool_call(&ToolCallView {
            id: call.call_id.clone(),
            name: call.fn_name.clone(),
            args: call.fn_arguments.clone(),
        })
        .await;
    }
}

/// Notify the sink (if any) that a tool call finished, with its output and outcome,
/// and fill the matching display segment's output/ok.
async fn emit_tool_result(config: &ChatConfig, display: &mut TurnDisplay, id: &str, output: &str, ok: bool) {
    display.complete_tool_call(id, output, ok);
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

/// Ordered display-segment list for an assistant turn, mirroring the frontend's
/// `ChatSegment` shape. Built as the turn streams (reasoning deltas, tool calls,
/// blocks, final text) so a reopened turn restores exactly as it rendered live:
/// the reasoning Thinking block, inline tool calls, artifacts, and text — in
/// document order. Persisted into the assistant message metadata alongside the
/// (back-compat) `blocks` list and the `stopped` flag.
#[derive(Default)]
struct TurnDisplay {
    segments: Vec<Value>,
}

impl TurnDisplay {
    /// Append a reasoning delta, coalescing into a trailing reasoning segment so
    /// consecutive `ReasoningChunk`s form one block (matching the live assembly).
    fn push_reasoning(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(last) = self.segments.last_mut() {
            if last.get("kind").and_then(Value::as_str) == Some("reasoning") {
                if let Some(Value::String(t)) = last.get_mut("text") {
                    t.push_str(delta);
                    return;
                }
            }
        }
        self.segments.push(serde_json::json!({ "kind": "reasoning", "text": delta }));
    }

    /// Append a tool-call segment with its inputs (output/ok filled in later).
    fn push_tool_call(&mut self, id: &str, name: &str, args: &Value) {
        self.segments.push(serde_json::json!({
            "kind": "tool_call", "id": id, "name": name, "args": args,
        }));
    }

    /// Fill in a previously-pushed tool call's output/outcome, matched by id.
    fn complete_tool_call(&mut self, id: &str, output: &str, ok: bool) {
        for seg in self.segments.iter_mut().rev() {
            if seg.get("kind").and_then(Value::as_str) == Some("tool_call")
                && seg.get("id").and_then(Value::as_str) == Some(id)
            {
                if let Value::Object(obj) = seg {
                    obj.insert("output".into(), Value::String(output.to_string()));
                    obj.insert("ok".into(), Value::Bool(ok));
                }
                return;
            }
        }
    }

    /// Mark a tool-call segment (by id) as one whose result renders as a block, so
    /// the UI hides its tool-call card by this authoritative flag rather than by
    /// matching against a set of bare tool names (review A1 / B5-1).
    fn mark_renders_as_block(&mut self, id: &str) {
        for seg in self.segments.iter_mut().rev() {
            if seg.get("kind").and_then(Value::as_str) == Some("tool_call")
                && seg.get("id").and_then(Value::as_str) == Some(id)
            {
                if let Value::Object(obj) = seg {
                    obj.insert("renders_as_block".into(), Value::Bool(true));
                }
                return;
            }
        }
    }

    /// Append a component block segment, in order.
    fn push_block(&mut self, block: &ChatBlock) {
        if let Ok(v) = serde_json::to_value(block) {
            self.segments.push(serde_json::json!({ "kind": "block", "block": v }));
        }
    }

    /// Append the final assistant text answer (skipping empty/whitespace).
    fn push_text(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.segments.push(serde_json::json!({ "kind": "text", "text": text }));
    }
}

/// Build the persisted metadata for an assistant turn: the full ordered display
/// segment list, the `stopped` flag, and (for back-compat) the component blocks.
/// The frontend prefers `segments` and falls back to `blocks` for legacy sessions.
///
/// Returns `None` only for a NON-stopped turn that produced nothing to render (no
/// segments and no component blocks), so the metadata column stays empty and
/// `display_messages_meta` drops the empty turn — matching the live path, which
/// never spawns a bubble for such a turn. A STOPPED turn always persists (even
/// with no output) so its `stopped:true` flag — and the "Stopped" marker — survive
/// a reload, matching the live path which DOES show the marker for an early stop.
fn assistant_turn_meta(display: &TurnDisplay, blocks: &[ChatBlock], stopped: bool) -> Option<Value> {
    let components: Vec<&ChatBlock> = blocks
        .iter()
        .filter(|b| matches!(b, ChatBlock::Component { .. }))
        .collect();
    if !stopped && display.segments.is_empty() && components.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "segments": display.segments,
        "stopped": stopped,
        "blocks": components,
    }))
}

async fn flush_parallel(
    config: &ChatConfig,
    tools: &ToolService,
    store: &Store,
    session: &mut Session,
    batch: &[&ToolCall],
    display: &mut TurnDisplay,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if batch.is_empty() {
        return Ok(());
    }

    println!("[TOOL BATCH {} read-only, concurrent]", batch.len());

    // Surface each read call before the concurrent batch runs.
    for call in batch {
        emit_tool_call(config, display, call).await;
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
        emit_tool_result(config, display, &call.call_id, &output, true).await;
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
        let mut display = TurnDisplay::default();
        route_tool_calls(&config, &tools, &store, &mut session, &[call], &mut blocks, &mut display)
            .await
            .unwrap();

        let events = sink.events.lock().unwrap().clone();
        assert_eq!(events, vec!["call:call-1".to_string(), "result:call-1:true".to_string()]);
    }

    #[test]
    fn assistant_turn_meta_back_compat_blocks_filter_to_components() {
        // The back-compat `blocks` field holds only this turn's Component blocks
        // (markdown lives in the message content), so old readers still restore them.
        let mixed = vec![
            ChatBlock::Markdown { text: "see chart".into() },
            ChatBlock::Component {
                component_id: "chart".into(),
                data: serde_json::json!({ "points": [1, 2, 3] }),
                target: Target::Canvas,
            },
        ];
        let meta = assistant_turn_meta(&TurnDisplay::default(), &mixed, false)
            .expect("component block present → metadata");
        let arr = meta["blocks"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["kind"], "component");
        assert_eq!(arr[0]["component_id"], "chart");
        assert_eq!(meta["stopped"], serde_json::json!(false));

        // A NON-stopped turn with no segments/blocks → no metadata column.
        assert!(assistant_turn_meta(&TurnDisplay::default(), &[], false).is_none());
        // A STOPPED empty turn still persists, so the "Stopped" marker survives a
        // reload (otherwise the empty assistant message is dropped on restore).
        let stopped_meta = assistant_turn_meta(&TurnDisplay::default(), &[], true)
            .expect("stopped empty turn → metadata with stopped flag");
        assert_eq!(stopped_meta["stopped"], serde_json::json!(true));
        assert_eq!(stopped_meta["segments"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn turn_display_assembles_ordered_segments() {
        let mut d = TurnDisplay::default();
        // Reasoning deltas coalesce into one segment.
        d.push_reasoning("Let me ");
        d.push_reasoning("think.");
        // A tool call, then its result fills output/ok by id.
        d.push_tool_call("c1", "read_file", &serde_json::json!({ "path": "x" }));
        d.complete_tool_call("c1", "contents", true);
        // A block, then the final text answer.
        d.push_block(&ChatBlock::Markdown { text: "inline".into() });
        d.push_text("done");
        // Empty/whitespace text is dropped (no trailing empty segment).
        d.push_text("   ");

        let segs = &d.segments;
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], serde_json::json!({ "kind": "reasoning", "text": "Let me think." }));
        assert_eq!(segs[1]["kind"], "tool_call");
        assert_eq!(segs[1]["id"], "c1");
        assert_eq!(segs[1]["output"], "contents");
        assert_eq!(segs[1]["ok"], true);
        assert_eq!(segs[2]["kind"], "block");
        assert_eq!(segs[3], serde_json::json!({ "kind": "text", "text": "done" }));
    }

    #[tokio::test]
    async fn assistant_turn_segments_round_trip_through_store() {
        // An assistant turn's full display segments + stopped flag must survive a
        // store write/read so a reopened session restores the turn exactly.
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open_at(&dir.path().join("test.db")).unwrap();
        let mut session = Session::new("test", dir.path().to_str().unwrap());
        store.save_session(&session).unwrap();

        let mut display = TurnDisplay::default();
        display.push_reasoning("thinking");
        display.push_tool_call("c1", "search_files", &serde_json::json!({ "q": "foo" }));
        display.complete_tool_call("c1", "3 hits", true);
        display.push_text("here");
        let meta = assistant_turn_meta(&display, &[], true).expect("non-empty turn → metadata");

        push_msg(&store, &mut session, ChatMessage::user("find foo")).await.unwrap();
        push_msg_meta(&store, &mut session, ChatMessage::assistant("here"), Some(&meta))
            .await
            .unwrap();

        let loaded = store.load_message_meta(&session.id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded[0].is_none());
        let stored = loaded[1].as_ref().expect("assistant metadata present");

        // The stopped flag round-trips.
        assert_eq!(stored["stopped"], serde_json::json!(true));
        // The ordered segments round-trip intact.
        let segs = stored["segments"].as_array().expect("segments array");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], serde_json::json!({ "kind": "reasoning", "text": "thinking" }));
        assert_eq!(segs[1]["kind"], "tool_call");
        assert_eq!(segs[1]["name"], "search_files");
        assert_eq!(segs[1]["output"], "3 hits");
        assert_eq!(segs[1]["ok"], true);
        assert_eq!(segs[2], serde_json::json!({ "kind": "text", "text": "here" }));
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
        let meta = assistant_turn_meta(&TurnDisplay::default(), &blocks, false)
            .expect("component block present → metadata");

        // Persist a plain user turn, then the assistant turn carrying block metadata.
        push_msg(&store, &mut session, ChatMessage::user("plot it")).await.unwrap();
        push_msg_meta(&store, &mut session, ChatMessage::assistant("here you go"), Some(&meta))
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

    #[tokio::test]
    async fn chat_returns_stopped_when_pre_cancelled_without_model_call() {
        // A pre-set cancel flag must be seen at the loop top, so chat() returns a
        // stopped turn promptly without ever hitting the network. The endpoint is
        // unreachable on purpose — if the loop tried a request, this would error.
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open_at(&dir.path().join("test.db")).unwrap();
        let mut session = Session::new("test", dir.path().to_str().unwrap());

        let permissions = Arc::new(PermissionGuard::new(
            &crate::config::Settings::default(),
            DenyApprover,
        ));

        let mut config = ChatConfig::new(
            "qwen2.5:0.5b".to_string(),
            "http://127.0.0.1:1/".to_string(),
            permissions,
        );
        config.cancel = Some(Arc::new(AtomicBool::new(true)));

        let turn = chat(
            config,
            &store,
            &mut session,
            "hello",
            &ContextPolicy::default(),
        )
        .await
        .expect("pre-cancelled chat returns Ok");

        assert!(turn.stopped, "turn should be marked stopped");
        assert!(turn.blocks.is_empty(), "no model output, no blocks");
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
