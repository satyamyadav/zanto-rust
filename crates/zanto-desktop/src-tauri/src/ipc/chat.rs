//! `send_message` — runs a chat turn in the active app's context.

use std::sync::Arc;
use tauri::State;
use zanto_core::chat::{chat, ChatConfig, ChatTurn};
use crate::catalogue::{shared_tools, SharedDispatcher};
use crate::interaction::TauriSink;
use super::DesktopState;

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

    let mut config = match &active {
        Some(app) => {
            // Shared artifact tools + the app's domain tools; base fs/shell come
            // from core. SharedDispatcher routes artifact tools then delegates.
            let mut extra = shared_tools();
            extra.extend(app.agent_tools());
            ChatConfig {
                model,
                endpoint,
                permissions: Arc::clone(&state.permissions),
                skill: Some(app.skill()),
                context: None,
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
        None => ChatConfig::new(model, endpoint, Arc::clone(&state.permissions)),
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
