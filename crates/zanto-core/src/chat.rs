use std::sync::Arc;
use futures::future::join_all;
use genai::{Client, ServiceTarget};
use genai::chat::{ChatMessage, ChatRequest, ToolCall, ToolResponse};
use genai::resolver::{Endpoint, ServiceTargetResolver};
use crate::permissions::PermissionGuard;
use crate::tools::ToolService;

pub struct ChatConfig {
    pub model: String,
    pub endpoint: &'static str,
    pub permissions: Arc<PermissionGuard>,
}

pub async fn chat(
    config: ChatConfig,
    question: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let tools = ToolService::new(Arc::clone(&config.permissions));

    let endpoint_str = config.endpoint;
    let target_resolver = ServiceTargetResolver::from_resolver_fn(
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
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

    let mut messages = vec![
        ChatMessage::system(
            "You are a helpful assistant. Use the provided tools to answer questions about the filesystem.",
        ),
        ChatMessage::user(question),
    ];

    let mut turn = 1;
    loop {
        println!("--- TURN {} ---", turn);

        let req = ChatRequest::new(messages.clone()).with_tools(ToolService::all_tools());
        let res = client.exec_chat(&config.model, req, None).await?;

        if res.tool_calls().is_empty() {
            return Ok(res.first_text().unwrap_or_default().to_string());
        }

        let tool_calls = res.into_tool_calls();
        messages.push(ChatMessage::from(tool_calls.clone()));
        execute_tool_calls(&tools, &tool_calls, &mut messages).await?;

        turn += 1;
    }
}

/// Consecutive read-only calls run concurrently; mutating calls break batches and run sequentially.
async fn execute_tool_calls(
    tools: &ToolService,
    tool_calls: &[ToolCall],
    messages: &mut Vec<ChatMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut read_batch: Vec<&ToolCall> = Vec::new();

    for call in tool_calls {
        if ToolService::is_readonly(&call.fn_name) {
            read_batch.push(call);
        } else {
            flush_parallel(tools, &read_batch, messages).await?;
            read_batch.clear();

            println!("[TOOL CALL mutating] {} ({:?})", call.fn_name, call.fn_arguments);
            let output = tools.dispatch(&call.fn_name, call.fn_arguments.clone()).await?;
            println!("[TOOL OUTPUT] {}", &output[..output.len().min(120)]);
            messages.push(ChatMessage::from(ToolResponse::new(call.call_id.clone(), output)));
        }
    }

    flush_parallel(tools, &read_batch, messages).await
}

async fn flush_parallel(
    tools: &ToolService,
    batch: &[&ToolCall],
    messages: &mut Vec<ChatMessage>,
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
        messages.push(ChatMessage::from(ToolResponse::new(call.call_id.clone(), output)));
    }

    Ok(())
}
