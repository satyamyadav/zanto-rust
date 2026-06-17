use std::borrow::Cow;
use std::path::Path;
use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::permissions::Op;

/// Default cap on extracted characters before truncation.
const DEFAULT_MAX_CHARS: usize = 32_000;
/// Cap on rows emitted per spreadsheet sheet.
const MAX_ROWS_PER_SHEET: usize = 200;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(
        description = "Filesystem path of the document to read (pdf, docx, xlsx/xls/ods, csv, html, or any text file)"
    )]
    pub path: String,
    #[schemars(description = "Maximum characters of extracted text to return (default 32000)")]
    #[serde(default)]
    pub max_chars: Option<usize>,
}

/// Detected document category. The string form is what we report as `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Csv,
    Html,
    Pdf,
    Docx,
    Spreadsheet,
    Image,
    Binary,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Csv => "csv",
            Kind::Html => "html",
            Kind::Pdf => "pdf",
            Kind::Docx => "docx",
            Kind::Spreadsheet => "spreadsheet",
            Kind::Image => "image",
            Kind::Binary => "binary",
        }
    }
}

pub struct ReadDocument;

impl ToolBase for ReadDocument {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "read_document".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Read and extract text from a document of any common format — PDF, Word (.docx), \
             Excel/OpenDocument spreadsheets (.xlsx/.xls/.ods), CSV, HTML, or any plaintext/code \
             file. Use this (not `read_file`) for binary office documents and PDFs. Returns JSON \
             `{ path, kind, text }`; text is best-effort, layout-lossy, and capped (default 32000 \
             chars) with a truncation note. Images are not OCR'd — attach them to a vision model \
             instead. Read-only; the path is permission-gated."
                .into(),
        )
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::DocTools> for ReadDocument {
    async fn invoke(svc: &super::DocTools, args: Args) -> Result<String, ErrorData> {
        let resolved = svc
            .permissions
            .check(&args.path, Op::Read)
            .await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let kind = detect_kind(&resolved);
        // Treat an explicit 0 as "unset" — a 0 cap would silently discard the
        // whole document and return only the truncation note.
        let max_chars = match args.max_chars {
            Some(n) if n > 0 => n,
            _ => DEFAULT_MAX_CHARS,
        };

        // Extraction is CPU-bound and the PDF/office crates are blocking; keep the
        // async runtime free by running it (and the truncation walk over the full
        // extracted string) on the blocking pool.
        let path = resolved.clone();
        let text = tokio::task::spawn_blocking(move || {
            truncate_chars(&extract(kind, &path), max_chars)
        })
        .await
        .map_err(|e| ErrorData::internal_error(format!("extraction task failed: {e}"), None))?;

        let out = json!({
            "path": resolved.to_string_lossy(),
            "kind": kind.as_str(),
            "text": text,
        });
        serde_json::to_string(&out).map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}

/// Classify a path by its (lowercased) extension. Pure.
pub(crate) fn detect_kind(path: &Path) -> Kind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "csv" | "tsv" => Kind::Csv,
        "html" | "htm" => Kind::Html,
        "pdf" => Kind::Pdf,
        "docx" => Kind::Docx,
        "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => Kind::Spreadsheet,
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tiff" | "tif" => Kind::Image,
        // Known binary formats we cannot extract from.
        "zip" | "tar" | "gz" | "7z" | "rar" | "exe" | "dll" | "so" | "dylib" | "bin" | "o"
        | "a" | "wasm" | "mp3" | "mp4" | "mov" | "avi" | "mkv" | "wav" | "flac" | "doc"
        | "ppt" | "pptx" => Kind::Binary,
        // Everything else (txt/md/json/log/yaml/toml/source code/…) is read as text.
        _ => Kind::Text,
    }
}

/// Dispatch extraction by detected kind. Blocking (PDF/office crates are sync).
fn extract(kind: Kind, path: &Path) -> String {
    match kind {
        Kind::Text => read_utf8(path),
        Kind::Csv => {
            // The extension tells us the delimiter with certainty (.tsv → tab),
            // so don't re-sniff it from the content.
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let delim = if ext.eq_ignore_ascii_case("tsv") { '\t' } else { ',' };
            csv_to_table(&read_utf8(path), delim)
        }
        Kind::Html => {
            let raw = read_utf8(path);
            crate::tools::web::fetch_url::extract_text(&raw).text
        }
        Kind::Pdf => extract_pdf(path),
        Kind::Docx => extract_docx(path),
        Kind::Spreadsheet => extract_spreadsheet(path),
        Kind::Image => "<note: this is an image; attach it to a vision-capable model to use it. \
             read_document does not OCR or describe images.>"
            .to_string(),
        Kind::Binary => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown");
            format!("unsupported binary format: {ext}")
        }
    }
}

/// Read a file as UTF-8 (lossy). Returns a clear error string on IO failure.
fn read_utf8(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => format!("error reading file: {e}"),
    }
}

/// Render delimiter-separated content as a compact text table. Pure.
///
/// Splits each line on `delim`, trims fields, and joins cells with " | " per
/// row. Does not handle quoted fields containing the delimiter — best-effort,
/// layout-lossy.
pub(crate) fn csv_to_table(content: &str, delim: char) -> String {
    content
        .lines()
        .map(|line| {
            line.split(delim)
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract text from a PDF. Best-effort and layout-lossy; never panics.
fn extract_pdf(path: &Path) -> String {
    // pdf-extract can panic on malformed input; isolate it so a bad PDF returns
    // an error string instead of aborting the process.
    let path = path.to_path_buf();
    match std::panic::catch_unwind(move || pdf_extract::extract_text(&path)) {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => format!("could not extract PDF text: {e}"),
        Err(_) => "could not extract PDF text: parser panicked on malformed PDF".to_string(),
    }
}

/// Extract paragraph (and table cell) text from a .docx. Best-effort.
fn extract_docx(path: &Path) -> String {
    use docx_rust::document::{BodyContent, TableRowContent};

    let docx_file = match docx_rust::DocxFile::from_file(path) {
        Ok(f) => f,
        Err(e) => return format!("could not open docx: {e:?}"),
    };
    let docx = match docx_file.parse() {
        Ok(d) => d,
        Err(e) => return format!("could not parse docx: {e:?}"),
    };

    let mut out: Vec<String> = Vec::new();
    for content in &docx.document.body.content {
        match content {
            BodyContent::Paragraph(p) => out.push(p.text()),
            BodyContent::Table(t) => {
                for row in &t.rows {
                    let cells: Vec<String> = row
                        .cells
                        .iter()
                        .filter_map(|c| match c {
                            TableRowContent::TableCell(cell) => {
                                Some(cell.iter_text().map(|s| s.as_ref()).collect::<String>())
                            }
                            _ => None,
                        })
                        .collect();
                    out.push(cells.join(" | "));
                }
            }
            _ => {}
        }
    }
    out.join("\n")
}

/// Extract all sheets of a spreadsheet as text tables. Best-effort; caps rows.
fn extract_spreadsheet(path: &Path) -> String {
    use calamine::Reader;

    let mut workbook = match calamine::open_workbook_auto(path) {
        Ok(wb) => wb,
        Err(e) => return format!("could not open spreadsheet: {e}"),
    };

    let mut out = String::new();
    for (name, range) in workbook.worksheets() {
        out.push_str(&format!("# Sheet: {name}\n"));
        out.push_str(&range_to_table(&range));
        out.push_str("\n\n");
    }
    if out.is_empty() {
        return "spreadsheet has no sheets".to_string();
    }
    out.trim_end().to_string()
}

/// Render a calamine range as a text table, capping at MAX_ROWS_PER_SHEET rows.
fn range_to_table(range: &calamine::Range<calamine::Data>) -> String {
    let total = range.height();
    let mut lines: Vec<String> = Vec::new();
    for row in range.rows().take(MAX_ROWS_PER_SHEET) {
        let cells: Vec<String> = row.iter().map(|c| c.to_string()).collect();
        lines.push(cells.join(" | "));
    }
    if total > MAX_ROWS_PER_SHEET {
        lines.push(format!(
            "… [{} more rows truncated]",
            total - MAX_ROWS_PER_SHEET
        ));
    }
    lines.join("\n")
}

/// Truncate `text` to at most `max_chars` characters (Unicode scalar values),
/// appending a note when content was cut. UTF-8-safe — never splits a char.
pub(crate) fn truncate_chars(text: &str, max_chars: usize) -> String {
    // char_indices yields byte offsets at char boundaries; the (max_chars)-th
    // entry is the boundary at which to cut.
    match text.char_indices().nth(max_chars) {
        None => text.to_string(),
        Some((byte_idx, _)) => format!("{}… [truncated]", &text[..byte_idx]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_format_by_extension() {
        let cases = [
            ("a.csv", Kind::Csv),
            ("a.TSV", Kind::Csv),
            ("page.html", Kind::Html),
            ("page.HTM", Kind::Html),
            ("doc.pdf", Kind::Pdf),
            ("doc.docx", Kind::Docx),
            ("sheet.xlsx", Kind::Spreadsheet),
            ("sheet.ods", Kind::Spreadsheet),
            ("img.PNG", Kind::Image),
            ("img.jpeg", Kind::Image),
            ("archive.zip", Kind::Binary),
            ("legacy.doc", Kind::Binary),
            ("notes.md", Kind::Text),
            ("data.json", Kind::Text),
            ("main.rs", Kind::Text),
            ("noext", Kind::Text),
        ];
        for (name, want) in cases {
            assert_eq!(detect_kind(&PathBuf::from(name)), want, "for {name}");
        }
    }

    #[test]
    fn csv_renders_as_table() {
        let got = csv_to_table("a,b,c\n1, 2 ,3\nx,y,z", ',');
        assert_eq!(got, "a | b | c\n1 | 2 | 3\nx | y | z");
    }

    #[test]
    fn tsv_splits_on_tab_even_when_first_line_has_no_tab() {
        // Single-column first line must not flip the delimiter to comma:
        // the extension (.tsv → '\t') is authoritative, not content sniffing.
        let got = csv_to_table("header\nval1\tval2", '\t');
        assert_eq!(got, "header\nval1 | val2");
    }

    #[test]
    fn html_strips_to_readable_text() {
        // Reuses the web crate's extractor.
        let html = "<html><body><script>x()</script><p>Hello</p><p>World</p></body></html>";
        let got = crate::tools::web::fetch_url::extract_text(html).text;
        assert_eq!(got, "Hello World");
    }

    #[test]
    fn text_passthrough_via_extract() {
        // Round-trip a plaintext file through the real extract() path.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("notes.md");
        std::fs::write(&p, "# Title\nbody line").unwrap();
        assert_eq!(extract(Kind::Text, &p), "# Title\nbody line");
    }

    #[test]
    fn image_returns_vision_note_not_extraction() {
        let got = extract(Kind::Image, &PathBuf::from("photo.png"));
        assert!(got.contains("vision"), "got: {got}");
        assert!(got.contains("does not OCR"), "got: {got}");
    }

    #[test]
    fn unknown_binary_returns_clear_message() {
        let got = extract(Kind::Binary, &PathBuf::from("blob.zip"));
        assert_eq!(got, "unsupported binary format: zip");
    }

    #[test]
    fn truncation_is_noted_when_cut() {
        let got = truncate_chars("abcdef", 3);
        assert_eq!(got, "abc… [truncated]");
    }

    #[test]
    fn no_truncation_when_under_cap() {
        let got = truncate_chars("abc", 10);
        assert_eq!(got, "abc");
        assert_eq!(truncate_chars("abc", 3), "abc");
    }

    #[test]
    fn truncation_is_utf8_safe() {
        // Multi-byte chars: cutting at a char boundary must not panic or split.
        let s = "héllo wörld 你好";
        let got = truncate_chars(s, 7);
        // First 7 chars: 'h','é','l','l','o',' ','w'
        assert_eq!(got, "héllo w… [truncated]");
    }
}
