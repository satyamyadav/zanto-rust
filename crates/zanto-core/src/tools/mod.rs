pub mod fs;

use genai::chat::Tool as GenaiTool;

pub fn all_tools() -> Vec<GenaiTool> {
    fs::schemas()
    // future: .into_iter().chain(web::schemas()).collect()
}

pub async fn dispatch(
    name: &str,
    args: serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    fs::dispatch(name, args).await
    // future: .or_else(|_| web::dispatch(name, args))
}

pub fn is_readonly(name: &str) -> bool {
    fs::is_readonly(name)
    // future: || web::is_readonly(name)
}
