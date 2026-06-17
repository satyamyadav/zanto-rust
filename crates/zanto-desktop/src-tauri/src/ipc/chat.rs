//! `send_message` — runs a chat turn in the active app's context.

use std::path::Path;
use std::sync::Arc;
use tauri::State;
use zanto_core::chat::{chat, ChatConfig, ChatTurn};
use zanto_core::config::Settings;
use crate::catalogue::{shared_tools, SharedDispatcher};
use crate::interaction::TauriSink;
use super::DesktopState;

/// Injected into the system prompt whenever the shared artifact tools are active.
/// Weaker local models otherwise discover artifacts but never call `render_artifact`,
/// narrating "a chart will appear" while the user sees nothing.
const ARTIFACT_PROTOCOL: &str = "\
To show the user any data UI — a table, chart, metric, list, key/values, or markdown \
document — you MUST call the `render_artifact` tool. That tool call is the only thing \
that displays an artifact; describing it in your reply shows the user nothing. Flow: \
`list_artifacts` to see options, `get_artifact(id)` to read its dataSchema, then \
`render_artifact({id, data, target})` with `data` matching that schema (target \
\"inline\" for the chat, \"canvas\" for the side panel). If `render_artifact` returns a \
schema error, fix `data` and call it again. Never announce a chart or table without \
calling `render_artifact` in the same turn.";

#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
    text: String,
) -> Result<ChatTurn, String> {
    let active = state.registry.active();
    let model = state.model.lock().unwrap().clone();
    let endpoint = state.endpoint.lock().unwrap().clone();

    let mut session = state.session.lock().await;
    session.app_id = active.as_ref().map(|a| a.manifest().id.clone());

    // Load settings once: drives context-source injection and skill resolution.
    let settings = Settings::load();

    // Inject the user's configured context sources into this turn (or None when
    // nothing is configured / readable).
    let ctx = zanto_core::context::load_context(&settings.context_sources);
    let context = (!ctx.is_empty()).then_some(ctx);

    // Resolve the active app's skill (if any) and append the user-selected
    // markdown skill body when one is active.
    let app_skill = active.as_ref().map(|a| a.skill());
    let selected = state.selected_skill.lock().unwrap().clone();
    let skill = match selected.and_then(|name| {
        zanto_core::context::get_skill(
            settings.project_dir.as_deref().map(Path::new),
            &name,
        )
        .map(|s| s.body)
    }) {
        Some(body) => match &app_skill {
            Some(app) => Some(format!("{app}\n\n{body}")),
            None => Some(body),
        },
        None => app_skill.clone(),
    };

    let mut config = match &active {
        Some(app) => {
            // Shared artifact tools + the app's domain tools; base fs/shell come
            // from core. SharedDispatcher routes artifact tools then delegates.
            let mut extra = shared_tools();
            extra.extend(app.agent_tools());
            // Prepend the artifact protocol since render_artifact is available here.
            let skill = Some(match &skill {
                Some(s) => format!("{ARTIFACT_PROTOCOL}\n\n{s}"),
                None => ARTIFACT_PROTOCOL.to_string(),
            });
            ChatConfig {
                model,
                endpoint,
                permissions: Arc::clone(&state.permissions),
                skill,
                context,
                extra_tools: extra,
                app_dispatch: Some(Arc::new(SharedDispatcher::new(
                    Arc::clone(&state.catalogue),
                    Arc::clone(app),
                    Arc::clone(&state.data),
                    state.interactor.clone(),
                ))),
                sink: None,
            }
        }
        None => {
            let mut c = ChatConfig::new(model, endpoint, Arc::clone(&state.permissions));
            c.skill = skill;
            c.context = context;
            c
        }
    };

    // Stream the turn to the shell: text deltas + blocks live, `chat_done` at end.
    let sink = TauriSink::new(app);
    config.sink = Some(Arc::new(sink.clone()));

    let result = chat(config, &state.store, &mut session, &text, &state.policy).await;
    sink.finish();
    let turn = result.map_err(|e| e.to_string())?;

    // Auto-title a fresh session from its first user message.
    if session.title.is_empty() {
        session.title = zanto_core::session::auto_title(&session.messages);
        let _ = state.store.save_session(&session);
    }
    Ok(turn)
}
