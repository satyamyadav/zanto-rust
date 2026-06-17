mod app;
mod apps;
mod catalogue;
mod interaction;
mod ipc;

use std::sync::Arc;
use tauri::Manager;
use zanto_core::config::Settings;
use zanto_core::data::DataStore;
use zanto_core::permissions::PermissionGuard;
use zanto_core::session::{ContextPolicy, Session, Store};
use crate::app::AppRegistry;
use crate::interaction::TauriInteractor;
use crate::ipc::DesktopState;

// Fixed workspace for the first slice. Directory picker / multi-workspace is future.
const WORKSPACE: &str = "default";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings = Settings::load();

            // Resolve the active provider's model/endpoint so the running state
            // matches what get_config will report as active_provider.
            let (_, active_model, active_endpoint) = settings.active();
            let model = if active_model.is_empty() {
                settings.model.clone().unwrap_or_else(|| "qwen2.5:14b".to_string())
            } else {
                active_model
            };
            let endpoint = active_endpoint
                .filter(|e| !e.is_empty())
                .or_else(|| settings.endpoint.clone())
                .filter(|e| !e.is_empty())
                .unwrap_or_else(|| "http://192.168.1.66:11434/".to_string());
            let policy = match settings.max_context_turns {
                Some(n) => ContextPolicy::LastNTurns { max_turns: n },
                None => ContextPolicy::default(),
            };

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
                policy,
                model: std::sync::Mutex::new(model),
                endpoint: std::sync::Mutex::new(endpoint),
                workspace: WORKSPACE.to_string(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::chat::send_message,
            ipc::apps::list_apps,
            ipc::apps::get_catalogue,
            ipc::apps::mount_app,
            ipc::apps::unmount_app,
            ipc::apps::query_app,
            ipc::apps::run_app_action,
            ipc::session::list_sessions,
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
            ipc::config::set_api_key,
            ipc::config::clear_api_key,
            ipc::files::browse_dir,
            ipc::artifacts::list_stored_artifacts_cmd,
            ipc::artifacts::read_stored_artifact_cmd,
            interaction::respond,
        ])
        .run(tauri::generate_context!())
        .expect("error while running zanto desktop");
}
