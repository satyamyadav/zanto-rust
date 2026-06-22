pub mod app;
mod apps;
pub mod catalogue;
mod interaction;
pub mod ipc;

use crate::app::AppRegistry;
use crate::interaction::TauriInteractor;
use crate::ipc::DesktopState;
use std::sync::Arc;
use tauri::Manager;
use zanto_core::config::Settings;
use zanto_core::data::DataStore;
use zanto_core::permissions::PermissionGuard;
use zanto_core::session::{Session, Store};

// Fixed workspace for the first slice. Directory picker / multi-workspace is future.
const WORKSPACE: &str = "default";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // single-instance MUST be registered first: a relaunch routes through this
        // callback, which surfaces and focuses the existing main window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings = Settings::load();

            // Resolve the active provider's model/endpoint so the running state
            // matches what get_config will report as active_provider.
            let (_, active_model, active_endpoint) = settings.active();
            let model = if active_model.is_empty() {
                settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "qwen2.5:14b".to_string())
            } else {
                active_model
            };
            let endpoint = active_endpoint
                .filter(|e| !e.is_empty())
                .or_else(|| settings.endpoint.clone())
                .filter(|e| !e.is_empty())
                .unwrap_or_else(|| "http://192.168.1.66:11434/".to_string());
            // One HITL interaction channel: powers permission approvals (Approver)
            // and agent `ask` forms. Shared by the permission guard and dispatcher.
            let interactor = TauriInteractor::new(app.handle().clone());
            let permissions = Arc::new(PermissionGuard::new(&settings, interactor.clone()));

            let store = Store::open().expect("open sessions DB");
            let data = Arc::new(DataStore::open(WORKSPACE).expect("open data engine"));
            let registry = AppRegistry::new(vec![
                apps::chat::ChatApp::new(),
                apps::finance::FinanceApp::new(),
            ]);
            let catalogue = Arc::new(catalogue::Catalogue::load());
            let session = tokio::sync::Mutex::new(Session::new("", WORKSPACE));

            app.manage(DesktopState {
                store,
                data,
                permissions,
                registry,
                catalogue,
                interactor,
                session,
                model: std::sync::Mutex::new(model),
                endpoint: std::sync::Mutex::new(endpoint),
                workspace: WORKSPACE.to_string(),
                selected_skill: std::sync::Mutex::new(settings.selected_skill.clone()),
                active_cancel: std::sync::Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::chat::send_message,
            ipc::chat::interrupt_turn,
            ipc::apps::list_apps,
            ipc::apps::get_catalogue,
            ipc::apps::mount_app,
            ipc::apps::unmount_app,
            ipc::apps::query_app,
            ipc::apps::run_app_action,
            ipc::apps::notify,
            ipc::finance::finance_parse_statement,
            ipc::finance::finance_import_statement,
            ipc::session::list_sessions,
            ipc::session::list_sessions_page,
            ipc::session::load_session,
            ipc::session::load_session_page,
            ipc::session::new_session,
            ipc::session::delete_session,
            ipc::session::rename_session,
            ipc::session::archive_session,
            ipc::session::unarchive_session,
            ipc::session::list_archived_sessions,
            ipc::config::get_config,
            ipc::config::set_config,
            ipc::config::pick_folder,
            ipc::config::add_allowed_path,
            ipc::config::add_context_source,
            ipc::config::remove_context_source,
            ipc::config::toggle_context_source,
            ipc::config::set_project_dir,
            ipc::config::set_api_key,
            ipc::config::clear_api_key,
            ipc::config::list_models,
            ipc::skills::list_skills,
            ipc::skills::set_active_skill,
            ipc::files::browse_dir,
            ipc::artifacts::list_stored_artifacts_cmd,
            ipc::artifacts::read_stored_artifact_cmd,
            ipc::artifacts::list_pinned_artifacts,
            ipc::artifacts::read_pinned_artifact,
            ipc::artifacts::pin_artifact_cmd,
            ipc::artifacts::save_artifact_copy,
            ipc::artifacts::reveal_artifact,
            interaction::respond,
        ])
        .run(tauri::generate_context!())
        .expect("error while running zanto desktop");
}
