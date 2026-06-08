use genai::chat::Tool;
use rmcp::{tool_router, tool, schemars::JsonSchema};
use rmcp::handler::server::wrapper::Parameters;
use serde::{Deserialize, Serialize};

mod chat;

// =========================================================================
// Tool argument schemas
// =========================================================================
#[derive(Deserialize, Serialize, JsonSchema, Debug)]
struct ListDirArgs {
    #[schemars(description = "Filesystem path of the directory to list")]
    path: String,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
struct ReadFileArgs {
    #[schemars(description = "Filesystem path of the file to read")]
    path: String,
}

// =========================================================================
// MCP tool implementation
// =========================================================================
#[derive(Clone)]
pub struct FsTools;

#[tool_router(server_handler)]
impl FsTools {
    #[tool(description = "List files and subdirectories at a given path")]
    async fn list_directory(&self, Parameters(args): Parameters<ListDirArgs>) -> Result<String, rmcp::ErrorData> {
        let entries = std::fs::read_dir(&args.path)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let lines: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if e.path().is_dir() { format!("{}/", name) } else { name }
            })
            .collect();

        Ok(if lines.is_empty() {
            "(empty directory)".to_string()
        } else {
            lines.join("\n")
        })
    }

    #[tool(description = "Read the full text contents of a file")]
    async fn read_file(&self, Parameters(args): Parameters<ReadFileArgs>) -> Result<String, rmcp::ErrorData> {
        std::fs::read_to_string(&args.path)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
    }
}

// =========================================================================
// Genai tool definitions (schema forwarded to Ollama)
// =========================================================================
fn fs_tools() -> Vec<Tool> {
    vec![
        Tool::new("list_directory")
            .with_description("List files and subdirectories at a given path")
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Filesystem path of the directory to list" }
                },
                "required": ["path"]
            })),
        Tool::new("read_file")
            .with_description("Read the full text contents of a file")
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Filesystem path of the file to read" }
                },
                "required": ["path"]
            })),
    ]
}

// =========================================================================
// Dispatch a single tool call locally
// =========================================================================
async fn dispatch(handler: &FsTools, name: &str, args: serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    match name {
        "list_directory" => {
            let a: ListDirArgs = serde_json::from_value(args)?;
            Ok(handler.list_directory(Parameters(a)).await?)
        }
        "read_file" => {
            let a: ReadFileArgs = serde_json::from_value(args)?;
            Ok(handler.read_file(Parameters(a)).await?)
        }
        other => Err(format!("unknown tool: {other}").into()),
    }
}


#[tokio::main]
async fn main() {
    match chat::chat().await {
        Ok(_) => println!("Done."),
        Err(e) => eprintln!("Error: {}", e),
    }
}
