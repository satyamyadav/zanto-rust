// =========================================================================
// Multi-turn orchestration loop
// =========================================================================

use genai::{Client, ServiceTarget};
use genai::chat::{ChatMessage, ChatRequest, ToolResponse};
use genai::resolver::{Endpoint, ServiceTargetResolver};
use super::{FsTools, fs_tools, dispatch};

pub async fn chat() -> Result<String, Box<dyn std::error::Error>> {
    let target_resolver = ServiceTargetResolver::from_resolver_fn(
        |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            let ServiceTarget { endpoint: _, auth, model } = service_target;
            let endpoint = Endpoint::from_static("http://192.168.1.66:11434/");
            Ok(ServiceTarget { endpoint, auth, model })
        }
    );

    let client = Client::builder().with_service_target_resolver(target_resolver).build();
    let handler = FsTools;

    let question = "List the files in the current directory '.', then read the file 'Cargo.toml' and give me a one-sentence summary.";

    let mut messages = vec![
        ChatMessage::system("You are a helpful assistant. Use the provided tools to answer questions about the filesystem."),
        ChatMessage::user(question),
    ];

    println!("--- TURN 1: Sending prompt + tool schemas to Ollama ---");

    let chat_req = ChatRequest::new(messages.clone()).with_tools(fs_tools());
    let chat_res = client.exec_chat("qwen2.5:14b", chat_req, None).await?;

    // No tool calls — model answered directly
    if chat_res.tool_calls().is_empty() {
        let answer = chat_res.first_text().unwrap_or_default().to_string();
        println!("\nAnswer:\n{}", answer);
        return Ok(answer);
    }

    // Execute every tool call the model requested
    let tool_calls = chat_res.into_tool_calls();
    messages.push(ChatMessage::from(tool_calls.clone()));


    println!("{:?}", tool_calls);

    for call in &tool_calls {
        println!("[TOOL CALL] {} ({:?})", call.fn_name, call.fn_arguments);
        let output = dispatch(&handler, &call.fn_name, call.fn_arguments.clone()).await?;
        println!("[TOOL OUTPUT] {}", &output[..output.len().min(120)]);
        messages.push(ChatMessage::from(ToolResponse::new(call.call_id.clone(), output)));
    }

    println!("\n--- TURN 2: Sending tool results back to Ollama ---");

    let final_req = ChatRequest::new(messages);
    let final_res = client.exec_chat("qwen2.5:14b", final_req, None).await?;

    let answer = final_res.first_text().unwrap_or_default().to_string();
    println!("\nFinal Answer:\n{}", answer);
    Ok(answer)
}
