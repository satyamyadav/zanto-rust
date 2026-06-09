use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use crate::config::Settings;

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalResponse {
    AllowOnce,
    AllowSession,
    AllowForever,
    Deny,
}

#[async_trait]
pub trait Approver: Send + Sync {
    async fn confirm(&self, path: &str, op: &str, resolved: &str) -> ApprovalResponse;
}

pub enum Op {
    Read,
    Write,
}

pub struct PermissionGuard {
    allowed: Vec<PathBuf>,
    allow_read_outside: bool,
    allow_write_outside: bool,
    approver: Arc<dyn Approver>,
    session_grants: Mutex<HashSet<PathBuf>>,
}

impl PermissionGuard {
    pub fn new<A: Approver + 'static>(settings: &Settings, approver: A) -> Self {
        let allowed = settings.allowed_paths.iter().map(|p| resolve(p)).collect();
        Self {
            allowed,
            allow_read_outside: settings.allow_read_outside,
            allow_write_outside: settings.allow_write_outside,
            approver: Arc::new(approver),
            session_grants: Mutex::new(HashSet::new()),
        }
    }

    /// Check permission for a path. Returns the resolved absolute path on success.
    pub async fn check(&self, path: &str, op: Op) -> Result<PathBuf, String> {
        let resolved = resolve(path);

        let bypass = match op {
            Op::Read => self.allow_read_outside,
            Op::Write => self.allow_write_outside,
        };
        if bypass {
            return Ok(resolved);
        }

        if self.is_allowed(&resolved) {
            return Ok(resolved);
        }

        {
            let grants = self.session_grants.lock().unwrap();
            if grants.contains(&resolved) {
                return Ok(resolved.clone());
            }
        }

        let op_str = match op {
            Op::Read => "read",
            Op::Write => "write",
        };

        let response = self
            .approver
            .confirm(path, op_str, &resolved.display().to_string())
            .await;

        match response {
            ApprovalResponse::AllowForever => {
                self.session_grants.lock().unwrap().insert(resolved.clone());
                crate::config::Settings::persist_allowed_path(&resolved.to_string_lossy());
                Ok(resolved)
            }
            ApprovalResponse::AllowSession => {
                self.session_grants.lock().unwrap().insert(resolved.clone());
                Ok(resolved)
            }
            ApprovalResponse::AllowOnce => Ok(resolved),
            ApprovalResponse::Deny => Err(format!("permission denied: {op_str} \"{path}\"")),
        }
    }

    fn is_allowed(&self, path: &Path) -> bool {
        self.allowed.iter().any(|a| path.starts_with(a))
    }
}

/// Expands a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if path == "~" || path.starts_with("~/") || path.starts_with("~\\") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{}{}", home, &path[1..])
    } else {
        path.to_string()
    }
}

/// Resolves a path to canonical absolute form. For paths that don't exist yet
/// (e.g. a file about to be written), canonicalizes the parent and appends the filename.
fn resolve(path: &str) -> PathBuf {
    let expanded = expand_tilde(path);
    let p = PathBuf::from(&expanded);
    if let Ok(c) = std::fs::canonicalize(&p) {
        return c;
    }
    if let (Some(parent), Some(name)) = (p.parent(), p.file_name()) {
        let base = if parent == Path::new("") {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else if let Ok(c) = std::fs::canonicalize(parent) {
            c
        } else {
            parent.to_path_buf()
        };
        return base.join(name);
    }
    p
}
