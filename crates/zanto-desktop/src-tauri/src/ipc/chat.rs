//! `send_message` — runs a chat turn in the active app's context.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Manager, State};
use tauri_plugin_notification::NotificationExt;
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
schema error, fix `data` and call it again. The `id` must be one returned by \
`list_artifacts` (e.g. \"chart\") — never invent an id, and never use `store_artifact` \
(which only saves a file to disk) to display something. Never announce a chart or table \
without calling `render_artifact` in the same turn. \
Tool roles differ: `render_artifact` SHOWS a view (table/chart/metric/etc) — it is \
ephemeral and is not saved or browsable. `store_artifact` SAVES a durable document (a \
markdown file or note) that the user can later open in the Artifacts browser; it displays \
nothing. Use render_artifact to display, store_artifact to persist a document. \
`pin_artifact` KEEPS a view+data artifact so the user can reopen it later from the \
Artifacts browser — use it for a view worth saving (vs render_artifact, which only \
shows it now, and store_artifact, which saves a file document). Pinning does not display; \
call render_artifact too if you also want to show it now. \
To read a non-plaintext document — PDF, Word (.docx), Excel/OpenDocument spreadsheet \
(.xlsx/.xls/.ods) — call `read_document` (not `read_file`, which only handles UTF-8 text). \
`read_document` also handles CSV, HTML, and plain text, so prefer it whenever a path might \
be a binary document. Images are not OCR'd; attach them to a vision-capable model instead.";

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
                cancel: None,
            }
        }
        None => {
            let mut c = ChatConfig::new(model, endpoint, Arc::clone(&state.permissions));
            c.skill = skill;
            c.context = context;
            c
        }
    };

    // Per-turn cancel flag: a fresh Arc so a prior interrupt never kills this turn.
    // Stored in `active_cancel` so `interrupt_turn` can set it mid-turn, then cleared
    // on completion.
    let cancel = Arc::new(AtomicBool::new(false));
    *state.active_cancel.lock().unwrap() = Some(Arc::clone(&cancel));
    config.cancel = Some(Arc::clone(&cancel));

    // Stream the turn to the shell: text deltas + blocks live, `chat_done` at end.
    let sink = TauriSink::new(app.clone());
    config.sink = Some(Arc::new(sink.clone()));

    let result = chat(config, &state.store, &mut session, &text, &state.policy).await;

    // Clear the active cancel flag now the turn is done (success, stop, or error).
    *state.active_cancel.lock().unwrap() = None;

    // Emit `chat_stopped` (before `chat_done`) when the turn was interrupted so the
    // shell can mark the assistant entry.
    if let Ok(turn) = &result {
        if turn.stopped {
            sink.stopped();
        }
    }
    sink.finish();
    let turn = result.map_err(|e| e.to_string())?;

    // Notify the user a turn finished while the window is in the background, so a
    // long-running reply isn't missed. Skip when focused; never fail the turn on a
    // notification error.
    if !app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false)
    {
        let body = if turn.stopped { "Turn stopped" } else { "Reply ready" };
        let _ = app.notification().builder().title("zanto").body(body).show();
    }

    // Auto-title a fresh session from its first user message.
    if session.title.is_empty() {
        session.title = zanto_core::session::auto_title(&session.messages);
        let _ = state.store.save_session(&session);
    }
    Ok(turn)
}

/// Interrupt the in-flight turn (if any). Sets the active cancel flag so the core
/// loop stops at its next check point, and drains pending HITL interactions so a
/// turn parked on an approval/form unblocks. No-op when no turn is running. Does not
/// take the session lock, so it is callable mid-turn while `send_message` holds it.
#[tauri::command]
pub fn interrupt_turn(state: State<'_, DesktopState>) {
    if let Some(flag) = state.active_cancel.lock().unwrap().as_ref() {
        flag.store(true, Ordering::SeqCst);
    }
    state.interactor.cancel_all();
}
