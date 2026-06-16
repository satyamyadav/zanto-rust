//! Bridges zanto-core's HITL `Approver` to the Svelte UI over Tauri IPC. When the
//! core needs permission, `confirm()` emits an `approval_request` event and awaits a
//! oneshot channel; the `approve` command (called by the UI dialog) resolves it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;
use zanto_core::permissions::{ApprovalResponse, Approver};

/// Shared map of in-flight approval requests, keyed by request id. Held in Tauri
/// state so the `approve` command and the `TauriApprover` reference the same map.
pub type PendingApprovals = Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>;

#[derive(Clone, Serialize)]
struct ApprovalRequestEvent {
    id: String,
    path: String,
    op: String,
    resolved: String,
}

pub struct TauriApprover {
    app: AppHandle,
    pending: Arc<PendingApprovals>,
    counter: AtomicU64,
}

impl TauriApprover {
    pub fn new(app: AppHandle, pending: Arc<PendingApprovals>) -> Self {
        Self { app, pending, counter: AtomicU64::new(0) }
    }
}

#[async_trait]
impl Approver for TauriApprover {
    async fn confirm(&self, path: &str, op: &str, resolved: &str) -> ApprovalResponse {
        let id = format!("appr-{}", self.counter.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);

        let emitted = self.app.emit(
            "approval_request",
            ApprovalRequestEvent {
                id: id.clone(),
                path: path.to_string(),
                op: op.to_string(),
                resolved: resolved.to_string(),
            },
        );
        if emitted.is_err() {
            // Could not reach the UI — clean up and deny.
            self.pending.lock().unwrap().remove(&id);
            return ApprovalResponse::Deny;
        }

        // Block until the UI resolves (or the channel is dropped → deny).
        rx.await.unwrap_or(ApprovalResponse::Deny)
    }
}

/// Resolve a pending approval from the UI dialog.
#[tauri::command]
pub fn approve(pending: State<'_, Arc<PendingApprovals>>, request_id: String, response: String) {
    let resp = match response.as_str() {
        "once" => ApprovalResponse::AllowOnce,
        "session" => ApprovalResponse::AllowSession,
        "forever" => ApprovalResponse::AllowForever,
        _ => ApprovalResponse::Deny,
    };
    if let Some(tx) = pending.lock().unwrap().remove(&request_id) {
        let _ = tx.send(resp);
    }
}
