//! Unified HITL interaction channel. The backend requests an interaction (a
//! permission **approval** or an agent **form**/clarification); the shell shows it
//! as an overlay above the composer and replies. Generalizes the old approver:
//! `Approver::confirm` is an approval interaction; the shared `ask` tool is a form.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use zanto_core::permissions::{ApprovalResponse, Approver};

struct Inner {
    app: AppHandle,
    pending: Mutex<HashMap<String, oneshot::Sender<Value>>>,
    counter: AtomicU64,
}

#[derive(Clone)]
pub struct TauriInteractor {
    inner: Arc<Inner>,
}

impl TauriInteractor {
    pub fn new(app: AppHandle) -> Self {
        Self {
            inner: Arc::new(Inner {
                app,
                pending: Mutex::new(HashMap::new()),
                counter: AtomicU64::new(0),
            }),
        }
    }

    /// Emit an `interaction_request` and await the user's response (JSON). Returns
    /// `Null` if the UI is unreachable / the channel is dropped.
    pub async fn request(&self, kind: &str, mut payload: Value) -> Value {
        let id = format!("ix-{}", self.inner.counter.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().unwrap().insert(id.clone(), tx);

        if !payload.is_object() {
            payload = json!({});
        }
        payload["id"] = json!(id);
        payload["kind"] = json!(kind);

        if self.inner.app.emit("interaction_request", &payload).is_err() {
            self.inner.pending.lock().unwrap().remove(&id);
            return Value::Null;
        }
        rx.await.unwrap_or(Value::Null)
    }

    pub fn resolve(&self, id: &str, value: Value) {
        if let Some(tx) = self.inner.pending.lock().unwrap().remove(id) {
            let _ = tx.send(value);
        }
    }
}

#[async_trait]
impl Approver for TauriInteractor {
    async fn confirm(&self, path: &str, op: &str, resolved: &str) -> ApprovalResponse {
        let v = self
            .request("approval", json!({ "op": op, "path": path, "resolved": resolved }))
            .await;
        match v.as_str() {
            Some("once") => ApprovalResponse::AllowOnce,
            Some("session") => ApprovalResponse::AllowSession,
            Some("forever") => ApprovalResponse::AllowForever,
            _ => ApprovalResponse::Deny,
        }
    }
}

/// Resolve a pending interaction from the UI (approval string or form answers).
#[tauri::command]
pub fn respond(state: tauri::State<'_, crate::ipc::DesktopState>, request_id: String, value: Value) {
    state.interactor.resolve(&request_id, value);
}
