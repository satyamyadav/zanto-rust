//! Finance-specific IPC. Statement parsing for the import pipeline lives here so
//! the file read is permission-checked (the pure finance app never touches the
//! filesystem); the parsed rows then flow to the `import_transactions` action.

use serde_json::{json, Value};
use tauri::State;
use zanto_core::permissions::Op;
use zanto_core::tools::docs::parse_table::parse_table;
use crate::apps::finance;
use super::DesktopState;

/// Parse a local CSV/XLSX statement into headers + rows + a suggested column
/// mapping. The path is permission-checked (Read) before the file is opened.
#[tauri::command]
pub async fn finance_parse_statement(
    state: State<'_, DesktopState>,
    path: String,
) -> Result<Value, String> {
    let resolved = state.permissions.check(&path, Op::Read).await.map_err(|e| e.to_string())?;
    let table = parse_table(&resolved)?;
    let rows: Vec<Value> = table.rows.iter().map(|r| json!(r)).collect();
    let preview: Vec<Value> = rows.iter().take(50).cloned().collect();
    Ok(json!({
        "columns": table.headers,
        "headers": table.headers,
        "preview": preview,
        "rows": rows,
        "row_count": table.rows.len(),
        "suggested_mapping": finance::suggest_mapping(&table.headers),
    }))
}
