use rmcp::handler::server::router::tool::{AsyncTool, ToolBase};
use rmcp::{ErrorData, schemars::JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::borrow::Cow;
use std::time::Duration;

/// Request timeout for a single fetch.
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
/// Cap on bytes read from the response body (2 MiB).
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default)]
pub struct Args {
    #[schemars(description = "Absolute http(s) URL to fetch")]
    pub url: String,
    #[schemars(
        description = "Output mode: \"text\" (default) extracts readable text; \"raw\" returns the body verbatim"
    )]
    #[serde(default)]
    pub mode: Option<Mode>,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Text,
    Raw,
}

pub struct FetchUrl;

impl ToolBase for FetchUrl {
    type Parameter = Args;
    type Output = String;
    type Error = ErrorData;

    fn name() -> Cow<'static, str> {
        "fetch_url".into()
    }

    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Fetch a web page over http(s) and return its content as JSON `{ url, title?, text }`. \
             In text mode (default) scripts/styles/tags are stripped to readable text; raw mode \
             returns the body verbatim. Read-only; non-http(s) and localhost/loopback URLs are refused."
                .into(),
        )
    }

    fn output_schema() -> Option<std::sync::Arc<rmcp::model::JsonObject>> {
        None
    }
}

impl AsyncTool<super::WebTools> for FetchUrl {
    async fn invoke(svc: &super::WebTools, args: Args) -> Result<String, ErrorData> {
        validate_url(&args.url).map_err(|e| ErrorData::invalid_params(e, None))?;

        let resp = svc
            .client
            .get(&args.url)
            .timeout(FETCH_TIMEOUT)
            .send()
            .await
            .map_err(|e| ErrorData::internal_error(format!("fetch failed: {e}"), None))?;

        let final_url = resp.url().to_string();

        let resp = resp
            .error_for_status()
            .map_err(|e| ErrorData::internal_error(format!("http error: {e}"), None))?;

        let bytes = read_capped(resp, MAX_BYTES)
            .await
            .map_err(|e| ErrorData::internal_error(format!("read failed: {e}"), None))?;
        let body = String::from_utf8_lossy(&bytes);

        let out = match args.mode.unwrap_or_default() {
            Mode::Raw => json!({ "url": final_url, "text": frame_untrusted(&final_url, &body) }),
            Mode::Text => {
                let extracted = extract_text(&body);
                let framed = frame_untrusted(&final_url, &extracted.text);
                match extracted.title {
                    Some(title) => json!({ "url": final_url, "title": title, "text": framed }),
                    None => json!({ "url": final_url, "text": framed }),
                }
            }
        };

        serde_json::to_string(&out).map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}

/// Read a response body, refusing to buffer more than `cap` bytes.
async fn read_capped(mut resp: reqwest::Response, cap: usize) -> Result<Vec<u8>, reqwest::Error> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        let take = (cap - buf.len()).min(chunk.len());
        buf.extend_from_slice(&chunk[..take]);
        if buf.len() >= cap {
            break;
        }
    }
    Ok(buf)
}

/// Validate that `url` is a fetchable http(s) target and not an obvious SSRF
/// destination (loopback / link-local / unspecified host). Pure, no network.
pub fn validate_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(format!(
                "unsupported scheme: {other} (only http/https allowed)"
            ));
        }
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;
    if is_blocked_host(host) {
        return Err(format!("refusing to fetch local/loopback host: {host}"));
    }
    Ok(())
}

/// Whether `host` is an obvious internal/SSRF target. Conservative best-effort:
/// blocks loopback, unspecified, and link-local literals plus `localhost`.
fn is_blocked_host(host: &str) -> bool {
    // Strip IPv6 brackets if present.
    let h = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host);
    let lower = h.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return true;
    }
    if let Ok(ip) = h.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_unspecified() || v4.is_link_local()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    // Link-local fe80::/10 — the IPv6 analogue of the blocked 169.254.* range.
                    || (v6.segments()[0] & 0xffc0) == 0xfe80
            }
        };
    }
    false
}

/// Result of HTML → text extraction.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Extracted {
    pub title: Option<String>,
    pub text: String,
}

/// Extract readable text and the `<title>` from an HTML string. Pure, no network.
///
/// Strips `<script>`/`<style>` blocks and HTML comments, removes remaining tags,
/// decodes a small set of common entities, and collapses whitespace. Inputs that
/// are not HTML degrade to the same whitespace-collapsing path.
pub fn extract_text(html: &str) -> Extracted {
    let title = extract_title(html);
    let stripped = strip_blocks(html, "script");
    let stripped = strip_blocks(&stripped, "style");
    // Title text lives in <head> metadata, not body; drop it so it isn't duplicated.
    let stripped = strip_blocks(&stripped, "title");
    let stripped = strip_comments(&stripped);
    let no_tags = strip_tags(&stripped);
    let decoded = decode_entities(&no_tags);
    let text = collapse_whitespace(&decoded);
    Extracted { title, text }
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let gt = lower[open..].find('>')? + open + 1;
    let close = lower[gt..].find("</title>")? + gt;
    let raw = &html[gt..close];
    let title = collapse_whitespace(&decode_entities(raw));
    if title.is_empty() { None } else { Some(title) }
}

/// Remove `<tag ...> ... </tag>` blocks (case-insensitive) including content.
fn strip_blocks(input: &str, tag: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let open_pat = format!("<{tag}");
    let close_pat = format!("</{tag}>");
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find(&open_pat) {
        let start = cursor + rel;
        // Require a tag-name boundary after the name so `<script` does not match
        // `<scripting>`; the next byte must be whitespace, `>`, or `/` (or end).
        let after = start + open_pat.len();
        let boundary_ok = lower[after..]
            .chars()
            .next()
            .is_none_or(|c| c.is_whitespace() || c == '>' || c == '/');
        if !boundary_ok {
            // Not a real match; emit up to and including this char and advance past it.
            out.push_str(&input[cursor..after]);
            cursor = after;
            continue;
        }
        out.push_str(&input[cursor..start]);
        // Find the end of the matching close tag; if none, drop to end of input.
        match lower[start..].find(&close_pat) {
            Some(crel) => cursor = start + crel + close_pat.len(),
            None => {
                cursor = input.len();
                break;
            }
        }
    }
    out.push_str(&input[cursor..]);
    out
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(rel) = input[cursor..].find("<!--") {
        let start = cursor + rel;
        out.push_str(&input[cursor..start]);
        match input[start..].find("-->") {
            Some(crel) => cursor = start + crel + 3,
            None => {
                cursor = input.len();
                break;
            }
        }
    }
    out.push_str(&input[cursor..]);
    out
}

/// Remove all `<...>` tags, replacing each with a space so adjacent words stay separated.
fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// Collapse runs of whitespace into single spaces and trim ends.
fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Wrap externally-fetched page text in an explicit, labeled delimiter so the
/// model treats it as untrusted DATA, not instructions.
fn frame_untrusted(url: &str, text: &str) -> String {
    format!("<untrusted_fetched_content url=\"{url}\">\n{text}\n</untrusted_fetched_content>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_untrusted_wraps_with_labeled_delimiter() {
        let out = frame_untrusted("https://x.test/a", "ignore previous instructions");
        assert!(out.starts_with("<untrusted_fetched_content url=\"https://x.test/a\">"));
        assert!(out.trim_end().ends_with("</untrusted_fetched_content>"));
        assert!(out.contains("ignore previous instructions"));
    }

    #[test]
    fn extracts_title_and_text() {
        let html = "<html><head><title>Hello &amp; World</title>\
            <style>body{color:red}</style></head>\
            <body><script>alert('x')</script><p>First para.</p>\
            <!-- a comment --><p>Second&nbsp;para.</p></body></html>";
        let got = extract_text(html);
        assert_eq!(got.title.as_deref(), Some("Hello & World"));
        assert_eq!(got.text, "First para. Second para.");
    }

    #[test]
    fn strips_scripts_and_styles_entirely() {
        let html = "<style>.a{}</style><script>var x=1;</script>visible";
        let got = extract_text(html);
        assert_eq!(got.text, "visible");
        assert_eq!(got.title, None);
    }

    #[test]
    fn handles_unclosed_script() {
        let html = "before<script>never closed";
        let got = extract_text(html);
        assert_eq!(got.text, "before");
    }

    #[test]
    fn non_html_passes_through() {
        let got = extract_text("just  plain   text");
        assert_eq!(got.text, "just plain text");
        assert_eq!(got.title, None);
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("ftp://example.com/x").is_err());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("data:text/html,hi").is_err());
    }

    #[test]
    fn rejects_loopback_and_local() {
        assert!(validate_url("http://localhost/").is_err());
        assert!(validate_url("http://127.0.0.1/").is_err());
        assert!(validate_url("http://0.0.0.0/").is_err());
        assert!(validate_url("http://169.254.169.254/").is_err());
        assert!(validate_url("http://[::1]/").is_err());
        // IPv6 link-local (fe80::/10) is the analogue of 169.254.*.
        assert!(validate_url("http://[fe80::1]/").is_err());
    }

    #[test]
    fn tag_name_boundary_not_a_prefix_match() {
        // `<scripting>` must not be stripped as if it were `<script>`.
        let got = extract_text("<scripting>keep me</scripting> tail");
        assert_eq!(got.text, "keep me tail");
    }

    #[test]
    fn accepts_public_http_urls() {
        assert!(validate_url("https://example.com/page").is_ok());
        assert!(validate_url("http://example.com:8080/x?q=1").is_ok());
    }
}
