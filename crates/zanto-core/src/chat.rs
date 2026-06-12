use std::sync::Arc;
use futures::future::join_all;
use genai::{Client, ServiceTarget};
use genai::chat::{ChatMessage, ChatRequest, ToolCall, ToolResponse};
use genai::resolver::{Endpoint, ServiceTargetResolver};
use crate::permissions::PermissionGuard;
use crate::session::{ContextPolicy, Session, Store};
use crate::tools::ToolService;

pub struct ChatConfig {
    pub model: String,
    pub endpoint: &'static str,
    pub permissions: Arc<PermissionGuard>,
}

pub async fn chat(
    config: ChatConfig,
    store: &Store,
    session: &mut Session,
    question: &str,
    policy: &ContextPolicy,
) -> Result<String, Box<dyn std::error::Error>> {
    let tools = ToolService::new(Arc::clone(&config.permissions));

    // Ensure the session row exists before appending messages
    session.updated_at = crate::session::unix_now_pub();
    store.save_session(session)?;

    let endpoint_str = config.endpoint;
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
            Ok(ServiceTarget {
                endpoint: Endpoint::from_static(endpoint_str),
                auth,
                model,
            })
        },
    );

    let client = Client::builder()
        .with_service_target_resolver(target_resolver)
        .build();

    // Append user message to session
    push_msg(store, session, ChatMessage::user(question)).await?;

    let system_prompt = ChatMessage::system(
        "You are a helpful assistant. Use the provided tools to answer questions about the filesystem.",
    );

    let mut turn = 1;
    loop {
        println!("--- TURN {turn} ---");

        let mut send_messages = vec![system_prompt.clone()];
        send_messages.extend(session.effective_messages(policy));

        let req = ChatRequest::new(send_messages).with_tools(ToolService::all_tools());
        let res = client.exec_chat(&config.model, req, None).await?;

        if res.tool_calls().is_empty() {
            let answer = res.first_text().unwrap_or_default().to_string();

            // Fallback: some models (e.g. qwen2.5 via Ollama) occasionally return
            // tool calls as raw JSON text instead of structured tool call objects.
            // Detect and execute them rather than surfacing raw JSON to the user.
            let fallback = extract_raw_tool_calls(&answer);
            if !fallback.is_empty() {
                eprintln!("[zanto] warn: model returned unparsed tool call(s), applying fallback parser");
                push_msg(store, session, ChatMessage::from(fallback.clone())).await?;
                execute_tool_calls(&tools, store, session, &fallback).await?;
                turn += 1;
                continue;
            }

            push_msg(store, session, ChatMessage::assistant(answer.clone())).await?;
            return Ok(answer);
        }

        let tool_calls = res.into_tool_calls();
        push_msg(store, session, ChatMessage::from(tool_calls.clone())).await?;
        execute_tool_calls(&tools, store, session, &tool_calls).await?;

        turn += 1;
    }
}

/// Append a message to the session and persist it.
async fn push_msg(
    store: &Store,
    session: &mut Session,
    msg: ChatMessage,
) -> Result<(), Box<dyn std::error::Error>> {
    let pos = session.messages.len();
    session.messages.push(msg);
    store.append_message(&session.id, pos, &session.messages[pos])?;
    Ok(())
}

async fn execute_tool_calls(
    tools: &ToolService,
    store: &Store,
    session: &mut Session,
    tool_calls: &[ToolCall],
) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
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
/// Returns synthetic `ToolCall` values that can be executed normally.
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
                if let (Some(name), Some(args)) = (
                    v.get("name").and_then(|n| n.as_str()),
                    v.get("arguments"),
                ) {
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
