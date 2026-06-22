//! Finance-specific IPC. Statement parsing AND import both live here so the file
//! read is permission-checked (the pure finance app never touches the
//! filesystem) — the parsed rows never round-trip through the client.

use super::DesktopState;
use crate::apps::finance;
use serde_json::{json, Value};
use tauri::State;
use zanto_core::permissions::Op;
use zanto_core::tools::docs::parse_table::parse_table;

/// Parse a local CSV/XLSX statement into headers + a small PREVIEW + a suggested
/// column mapping. The path is permission-checked (Read) before the file is
/// opened. Full rows are NOT returned — import re-reads the (still
/// permission-checked) file server-side, so nothing large round-trips.
#[tauri::command]
pub async fn finance_parse_statement(
    state: State<'_, DesktopState>,
    path: String,
) -> Result<Value, String> {
    let resolved = state
        .permissions
        .check(&path, Op::Read)
        .await
        .map_err(|e| e.to_string())?;
    let table = parse_table(&resolved)?;
    let preview: Vec<Value> = table.rows.iter().take(50).map(|r| json!(r)).collect();
    Ok(json!({
        "columns": table.headers,
        "headers": table.headers,
        "preview": preview,
        "row_count": table.rows.len(),
        "total_rows": table.total_rows,
        "truncated": table.truncated,
        "malformed": table.malformed,
        "suggested_mapping": finance::suggest_mapping(&table.headers),
    }))
}

/// Import a statement: re-read + parse the permission-checked path server-side,
/// then run the finance app's `import_transactions` flow with the SERVER-parsed
/// rows. Closes the confused-deputy gap where the client could hand fabricated
/// rows to an un-gated action.
#[tauri::command]
pub async fn finance_import_statement(
    state: State<'_, DesktopState>,
    path: String,
    mapping: Value,
    account: String,
) -> Result<Value, String> {
    let resolved = state
        .permissions
        .check(&path, Op::Read)
        .await
        .map_err(|e| e.to_string())?;
    let table = parse_table(&resolved)?;
    let rows: Vec<Value> = table.rows.iter().map(|r| json!(r)).collect();
    let app = state
        .registry
        .get("finance")
        .ok_or_else(|| "finance app not mounted".to_string())?;
    app.action(
        &state.data,
        "import_transactions",
        json!({
            "headers": table.headers, "rows": rows, "mapping": mapping, "account": account,
            // Surface parse-level data loss so the import result can warn the user.
            "total_rows": table.total_rows, "truncated": table.truncated, "malformed": table.malformed,
        }),
    )
}
