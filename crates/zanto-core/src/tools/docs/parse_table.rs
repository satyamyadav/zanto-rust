//! Structured tabular parsing (CSV/TSV + spreadsheets) for the finance import
//! pipeline. Unlike `read_document` (which flattens a file to text), this keeps
//! the header row and cells so a statement file can be mapped to transactions.

use std::path::Path;

/// A parsed table: the header row plus data rows, all cells as trimmed strings.
#[derive(Debug, Clone, PartialEq)]
pub struct TableData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Defensive cap on returned data rows for very large files.
const MAX_ROWS: usize = 5000;

/// Parse a CSV/TSV or spreadsheet (xlsx/xls/ods) file into headers + rows. The
/// first row is treated as the header. Errors on an unsupported extension or an
/// empty file.
pub fn parse_table(path: &Path) -> Result<TableData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "csv" => parse_delimited(path, b','),
        "tsv" => parse_delimited(path, b'\t'),
        "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => parse_spreadsheet(path),
        other => Err(format!("unsupported table file type: .{other} (use CSV/TSV/XLSX/ODS)")),
    }
}

fn parse_delimited(path: &Path, delim: u8) -> Result<TableData, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_path(path)
        .map_err(|e| format!("could not read file: {e}"))?;
    let mut records = rdr.records();
    let headers: Vec<String> = match records.next() {
        Some(Ok(r)) => r.iter().map(|s| s.trim().to_string()).collect(),
        Some(Err(e)) => return Err(format!("malformed CSV header: {e}")),
        None => return Err("file is empty".to_string()),
    };
    let mut rows = Vec::new();
    for rec in records.take(MAX_ROWS) {
        // Skip a malformed row rather than failing the whole import.
        if let Ok(r) = rec {
            rows.push(r.iter().map(|s| s.trim().to_string()).collect());
        }
    }
    Ok(TableData { headers, rows })
}

fn parse_spreadsheet(path: &Path) -> Result<TableData, String> {
    use calamine::Reader;
    let mut wb =
        calamine::open_workbook_auto(path).map_err(|e| format!("could not open spreadsheet: {e}"))?;
    let range = wb
        .worksheets()
        .into_iter()
        .next()
        .map(|(_, r)| r)
        .ok_or_else(|| "spreadsheet has no sheets".to_string())?;
    let mut iter = range.rows();
    let headers: Vec<String> = match iter.next() {
        Some(row) => row.iter().map(|c| c.to_string().trim().to_string()).collect(),
        None => return Err("spreadsheet is empty".to_string()),
    };
    let rows: Vec<Vec<String>> = iter
        .take(MAX_ROWS)
        .map(|row| row.iter().map(|c| c.to_string().trim().to_string()).collect())
        .collect();
    Ok(TableData { headers, rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_csv_headers_and_rows() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("s.csv");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "Date,Description,Amount").unwrap();
        writeln!(f, "2026-06-01,Cafe,-12.50").unwrap();
        writeln!(f, "2026-06-02,Payroll,2000").unwrap();
        let t = parse_table(&path).unwrap();
        assert_eq!(t.headers, vec!["Date", "Description", "Amount"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], vec!["2026-06-01", "Cafe", "-12.50"]);
    }

    #[test]
    fn unsupported_extension_errs() {
        assert!(parse_table(Path::new("/tmp/x.pdf")).is_err());
    }
}
