//! Structured tabular parsing (CSV/TSV + spreadsheets) for the finance import
//! pipeline. Unlike `read_document` (which flattens a file to text), this keeps
//! the header row and cells so a statement file can be mapped to transactions.

use std::path::Path;

/// A parsed table: the header row plus data rows, all cells as trimmed strings.
///
/// `total_rows`/`truncated`/`malformed` make silent data loss visible: the import
/// UI can warn when a file was capped at `MAX_ROWS` or had unparseable rows,
/// instead of importing a quietly-incomplete statement.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Total data rows seen in the file (before the `MAX_ROWS` cap).
    pub total_rows: usize,
    /// True when more than `MAX_ROWS` data rows existed and `rows` was capped.
    pub truncated: bool,
    /// Rows the parser could not read (malformed CSV records), counted not dropped-silently.
    pub malformed: usize,
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
        other => Err(format!(
            "unsupported table file type: .{other} (use CSV/TSV/XLSX/ODS)"
        )),
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
    let mut total_rows = 0usize;
    let mut malformed = 0usize;
    for rec in records {
        total_rows += 1;
        match rec {
            // Count, don't silently drop, a malformed row; keep good rows up to the cap.
            Ok(r) if rows.len() < MAX_ROWS => {
                rows.push(r.iter().map(|s| s.trim().to_string()).collect());
            }
            Ok(_) => {} // over the cap — counted in total_rows, reported via `truncated`
            Err(_) => malformed += 1,
        }
    }
    let truncated = total_rows.saturating_sub(malformed) > MAX_ROWS;
    Ok(TableData {
        headers,
        rows,
        total_rows,
        truncated,
        malformed,
    })
}

fn parse_spreadsheet(path: &Path) -> Result<TableData, String> {
    use calamine::Reader;
    let mut wb = calamine::open_workbook_auto(path)
        .map_err(|e| format!("could not open spreadsheet: {e}"))?;
    let range = wb
        .worksheets()
        .into_iter()
        .next()
        .map(|(_, r)| r)
        .ok_or_else(|| "spreadsheet has no sheets".to_string())?;
    let mut iter = range.rows();
    let headers: Vec<String> = match iter.next() {
        Some(row) => row.iter().map(cell_to_string).collect(),
        None => return Err("spreadsheet is empty".to_string()),
    };
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total_rows = 0usize;
    for row in iter {
        total_rows += 1;
        if rows.len() < MAX_ROWS {
            rows.push(row.iter().map(cell_to_string).collect());
        }
    }
    let truncated = total_rows > MAX_ROWS;
    Ok(TableData {
        headers,
        rows,
        total_rows,
        truncated,
        malformed: 0,
    })
}

/// Render a spreadsheet cell to a trimmed string. Date-typed cells are emitted as
/// `YYYY-MM-DD` rather than their raw Excel serial number (`45810`) — otherwise
/// every date-derived feature breaks for XLSX imports (review H1).
fn cell_to_string(c: &calamine::Data) -> String {
    use calamine::Data;
    match c {
        Data::DateTime(dt) => {
            excel_serial_to_ymd(dt.as_f64()).unwrap_or_else(|| dt.as_f64().to_string())
        }
        // ISO datetime (ODS) — keep just the date part.
        Data::DateTimeIso(s) => s.split('T').next().unwrap_or(s).trim().to_string(),
        other => other.to_string().trim().to_string(),
    }
}

/// Convert an Excel/ODS date serial to `YYYY-MM-DD`. Excel counts days from the
/// 1899-12-30 epoch (the offset that already absorbs the fictional 1900-02-29
/// leap-day bug for any real statement date). Returns None for non-positive serials.
fn excel_serial_to_ymd(serial: f64) -> Option<String> {
    if !serial.is_finite() || serial < 1.0 {
        return None;
    }
    // Days since the Unix epoch: Excel serial 25569 == 1970-01-01.
    let days = serial.floor() as i64 - 25569;
    let (y, m, d) = civil_from_days(days);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// Civil date (year, month, day) from a day count since 1970-01-01
/// (Howard Hinnant's algorithm — the inverse of `days_from_civil`).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
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
        assert_eq!(t.total_rows, 2);
        assert!(!t.truncated);
        assert_eq!(t.malformed, 0);
    }

    #[test]
    fn unsupported_extension_errs() {
        assert!(parse_table(Path::new("/tmp/x.pdf")).is_err());
    }

    #[test]
    fn excel_serial_converts_to_iso_date() {
        // Canonical anchors: serial 44197 == 2021-01-01; 45810 == 2025-06-02.
        assert_eq!(excel_serial_to_ymd(44197.0).as_deref(), Some("2021-01-01"));
        assert_eq!(excel_serial_to_ymd(45810.0).as_deref(), Some("2025-06-02"));
        assert_eq!(excel_serial_to_ymd(0.0), None);
    }

    #[test]
    fn reports_truncation_and_total_rows() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("big.csv");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "Date,Amount").unwrap();
        for i in 0..(MAX_ROWS + 10) {
            writeln!(f, "2026-06-01,{i}").unwrap();
        }
        let t = parse_table(&path).unwrap();
        assert_eq!(t.rows.len(), MAX_ROWS);
        assert_eq!(t.total_rows, MAX_ROWS + 10);
        assert!(t.truncated);
    }
}
