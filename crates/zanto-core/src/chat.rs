use futures::future::join_all;
use genai::{Client, ServiceTarget};
use genai::chat::{ChatMessage, ChatRequest, ToolCall, ToolResponse};
use genai::resolver::{Endpoint, ServiceTargetResolver};
use super::tools::{all_tools, dispatch, is_readonly};

pub async fn chat() -> Result<String, Box<dyn std::error::Error>> {
    let target_resolver = ServiceTargetResolver::from_resolver_fn(
        |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            let ServiceTarget { endpoint: _, auth, model } = service_target;
            let endpoint = Endpoint::from_static("http://192.168.1.66:11434/");
            Ok(ServiceTarget { endpoint, auth, model })
        }
    );

    let client = Client::builder().with_service_target_resolver(target_resolver).build();

    let question = "Search for all '*.toml' files under '.', then write a file 'hello.txt' with the content 'hello from zanto', then read it back and confirm.";

    let mut messages = vec![
        ChatMessage::system("You are a helpful assistant. Use the provided tools to answer questions about the filesystem."),
        ChatMessage::user(question),
    ];

    println!("--- TURN 1: Sending prompt + tool schemas to Ollama ---");

    let chat_req = ChatRequest::new(messages.clone()).with_tools(all_tools());
    let chat_res = client.exec_chat("qwen2.5:14b", chat_req, None).await?;

    if chat_res.tool_calls().is_empty() {
        let answer = chat_res.first_text().unwrap_or_default().to_string();
        println!("\nAnswer:\n{}", answer);
        return Ok(answer);
    }

    let tool_calls = chat_res.into_tool_calls();
    messages.push(ChatMessage::from(tool_calls.clone()));

    execute_tool_calls(&tool_calls, &mut messages).await?;

    println!("\n--- TURN 2: Sending tool results back to Ollama ---");

    let final_req = ChatRequest::new(messages);
    let final_res = client.exec_chat("qwen2.5:14b", final_req, None).await?;

    let answer = final_res.first_text().unwrap_or_default().to_string();
    println!("\nFinal Answer:\n{}", answer);
    Ok(answer)
}

/// Execute tool calls respecting read/write ordering:
/// consecutive read-only calls run concurrently; mutating calls break batches and run one at a time.
async fn execute_tool_calls(
    tool_calls: &[ToolCall],
    messages: &mut Vec<ChatMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut read_batch: Vec<&ToolCall> = Vec::new();

    for call in tool_calls {
        if is_readonly(&call.fn_name) {
            read_batch.push(call);
        } else {
            flush_parallel(&read_batch, messages).await?;
            read_batch.clear();

            println!("[TOOL CALL mutating] {} ({:?})", call.fn_name, call.fn_arguments);
            let output = dispatch(&call.fn_name, call.fn_arguments.clone()).await?;
            println!("[TOOL OUTPUT] {}", &output[..output.len().min(120)]);
            messages.push(ChatMessage::from(ToolResponse::new(call.call_id.clone(), output)));
        }
    }

    flush_parallel(&read_batch, messages).await
}

async fn flush_parallel(
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
        async move { dispatch(&name, args).await }
    }))
    .await;

    for (call, result) in batch.iter().zip(results) {
        let output = result?;
        println!("[TOOL OUTPUT] {} → {}", call.fn_name, &output[..output.len().min(120)]);
        messages.push(ChatMessage::from(ToolResponse::new(call.call_id.clone(), output)));
    }

    Ok(())
}
