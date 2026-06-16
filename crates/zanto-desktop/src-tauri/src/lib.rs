mod app;
mod approver;
mod apps;
mod ipc;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use zanto_core::config::Settings;
use zanto_core::data::DataStore;
use zanto_core::permissions::PermissionGuard;
use zanto_core::session::{ContextPolicy, Session, Store};
use crate::app::AppRegistry;
use crate::approver::{PendingApprovals, TauriApprover};
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

            let model = settings.model.clone().unwrap_or_else(|| "qwen2.5:14b".to_string());
            let endpoint = settings
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://192.168.1.66:11434/".to_string());
            let policy = match settings.max_context_turns {
                Some(n) => ContextPolicy::LastNTurns { max_turns: n },
                None => ContextPolicy::default(),
            };

            // HITL approval bridged to the UI. Pending map is shared with the
            // `approve` command (managed separately so the command can reach it).
            let pending: Arc<PendingApprovals> = Arc::new(Mutex::new(HashMap::new()));
            let approver = TauriApprover::new(app.handle().clone(), Arc::clone(&pending));
            let permissions = Arc::new(PermissionGuard::new(&settings, approver));

            let store = Store::open().expect("open sessions DB");
            let data = Arc::new(DataStore::open(WORKSPACE).expect("open data engine"));
            let registry = AppRegistry::new(vec![apps::finance::FinanceApp::new()]);
            let session = tokio::sync::Mutex::new(Session::new("", WORKSPACE));

            app.manage(Arc::clone(&pending));
            app.manage(DesktopState {
                store,
                data,
                permissions,
                registry,
                session,
                policy,
                model: Mutex::new(model),
                endpoint: Mutex::new(endpoint),
                workspace: WORKSPACE.to_string(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::send_message,
            ipc::list_apps,
            ipc::mount_app,
            ipc::unmount_app,
            ipc::query_app,
            ipc::run_app_action,
            ipc::list_sessions,
            ipc::load_session,
            ipc::new_session,
            ipc::delete_session,
            ipc::rename_session,
            ipc::get_config,
            ipc::set_config,
            ipc::pick_folder,
            approver::approve,
        ])
        .run(tauri::generate_context!())
        .expect("error while running zanto desktop");
}
