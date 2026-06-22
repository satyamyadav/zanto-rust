//! Running-summary support: condense older turns into a compact recap so long
//! threads keep context cheaply.
//!
//! [`summarize_messages`] asks the model for a concise running summary of a slice
//! of (older) messages. The result is stored via [`crate::session::Store::set_summary`]
//! and re-injected as a leading system note by
//! [`crate::session::Session::effective_messages`] under
//! [`crate::session::ContextPolicy::Summarize`].
//!
//! There is no background task: callers decide *when* to summarize (e.g. when a
//! session's turn count exceeds the policy's `keep_last`) and then persist the
//! result. See [`should_summarize`] for the suggested trigger.

use genai::Client;
use genai::chat::{ChatMessage, ChatRequest, ChatRole};

/// System instruction steering the model toward a tight, factual recap.
const SUMMARY_PROMPT: &str = "You are summarizing an ongoing conversation so it can \
continue with less context. Write a concise running summary of the messages below: \
capture decisions, facts, open questions, and any state needed to keep going. Omit \
pleasantries and filler. Use terse prose or bullets. Do not add information that is \
not present.";

/// Ask `model` for a concise running summary of `messages` (the older turns being
/// folded out of the live window). Returns the summary text.
///
/// Network call — never invoke from unit tests. Callers persist the result via
/// `Store::set_summary`.
pub async fn summarize_messages(
    client: &Client,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Flatten the turns into a single transcript so providers that reject
    // arbitrary role orderings (or empty/tool-only messages) still accept the
    // request.
    let transcript = render_transcript(messages);

    let req = ChatRequest::new(vec![
        ChatMessage::system(SUMMARY_PROMPT),
        ChatMessage::user(transcript),
    ]);

    let res = client.exec_chat(model, req, None).await?;
    Ok(res.into_first_text().unwrap_or_default())
}

/// Render messages as a plain `role: text` transcript, skipping empty content.
fn render_transcript(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for msg in messages {
        let Some(text) = msg.content.first_text() else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let role = match msg.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::System => "system",
            ChatRole::Tool => "tool",
        };
        out.push_str(role);
        out.push_str(": ");
        out.push_str(text);
        out.push('\n');
    }
    out
}

/// Suggested trigger: returns `true` when `turn_count` exceeds `keep_last`, i.e.
/// there are older turns that would be dropped from the live window and are worth
/// folding into the running summary. A documented helper — wiring the actual
/// summarize/persist call is left to the caller.
pub fn should_summarize(turn_count: usize, keep_last: usize) -> bool {
    turn_count > keep_last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_summarize_trips_past_budget() {
        assert!(!should_summarize(3, 5));
        assert!(!should_summarize(5, 5));
        assert!(should_summarize(6, 5));
    }

    #[test]
    fn render_transcript_skips_empty_and_labels_roles() {
        let msgs = vec![
            ChatMessage::user("hello"),
            ChatMessage::assistant("   "),
            ChatMessage::assistant("hi there"),
        ];
        let t = render_transcript(&msgs);
        assert!(t.contains("user: hello"));
        assert!(t.contains("assistant: hi there"));
        // The whitespace-only assistant message is dropped.
        assert_eq!(t.matches("assistant:").count(), 1);
    }
}
