use crate::config::Settings;
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Read,
    Write,
}

pub struct PermissionGuard {
    allowed: Mutex<Vec<PathBuf>>,
    allow_read_outside: bool,
    allow_write_outside: bool,
    approver: Arc<dyn Approver>,
    session_grants: Mutex<HashSet<PathBuf>>,
    // Serializes interactive approval prompts so concurrent checks (e.g. a batch of
    // read-only tools) don't race on the approver's input (stdin).
    prompt_lock: tokio::sync::Mutex<()>,
}

impl PermissionGuard {
    pub fn new<A: Approver + 'static>(settings: &Settings, approver: A) -> Self {
        let allowed = settings.allowed_paths.iter().map(|p| resolve(p)).collect();
        Self {
            allowed: Mutex::new(allowed),
            allow_read_outside: settings.allow_read_outside,
            allow_write_outside: settings.allow_write_outside,
            approver: Arc::new(approver),
            session_grants: Mutex::new(HashSet::new()),
            prompt_lock: tokio::sync::Mutex::new(()),
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

        // Serialize prompts: only one approval is solicited at a time. After
        // acquiring, re-check the cache — a concurrent prompt may have already
        // granted this exact path while we waited.
        let _prompt = self.prompt_lock.lock().await;
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
        self.allowed
            .lock()
            .unwrap()
            .iter()
            .any(|a| path.starts_with(a))
    }

    /// Grant a folder (and its children) for the rest of this process. The caller
    /// persists it to config separately for future launches.
    pub fn add_allowed(&self, path: &str) {
        self.allowed.lock().unwrap().push(resolve(path));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct PanicApprover;
    #[async_trait::async_trait]
    impl Approver for PanicApprover {
        async fn confirm(&self, _: &str, _: &str, _: &str) -> ApprovalResponse {
            panic!("approver should not have been called");
        }
    }

    struct CountingApprover {
        count: Arc<AtomicUsize>,
        response: ApprovalResponse,
    }
    #[async_trait::async_trait]
    impl Approver for CountingApprover {
        async fn confirm(&self, _: &str, _: &str, _: &str) -> ApprovalResponse {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.response.clone()
        }
    }

    fn guard_with_allowed(path: &str, approver: impl Approver + 'static) -> PermissionGuard {
        let settings = Settings {
            allowed_paths: vec![path.to_string()],
            ..Default::default()
        };
        PermissionGuard::new(&settings, approver)
    }

    #[tokio::test]
    async fn tilde_expands_to_home() {
        let settings = Settings {
            allow_read_outside: true,
            ..Default::default()
        };
        let guard = PermissionGuard::new(&settings, PanicApprover);
        // With allow_read_outside the guard bypasses allowed_paths check.
        // resolve() must expand ~ so the path is absolute.
        let result = guard.check("~/some_nonexistent_path_xyz", Op::Read).await;
        let home = std::env::var("HOME").unwrap_or_default();
        let resolved = result.unwrap();
        assert!(
            resolved.to_string_lossy().starts_with(&home),
            "expected path to start with HOME, got: {}",
            resolved.display()
        );
    }

    #[tokio::test]
    async fn allowed_path_passes_without_prompt() {
        let dir = tempfile::TempDir::new().unwrap();
        let dir_str = dir.path().to_string_lossy().to_string();
        let guard = guard_with_allowed(&dir_str, PanicApprover);
        // Create a file so canonicalize succeeds
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "x").unwrap();
        let result = guard.check(file.to_str().unwrap(), Op::Read).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn deny_returns_error() {
        let count = Arc::new(AtomicUsize::new(0));
        let approver = CountingApprover {
            count: Arc::clone(&count),
            response: ApprovalResponse::Deny,
        };
        let guard = guard_with_allowed("/never/matches", approver);
        let result = guard.check("/some/other/path", Op::Read).await;
        assert!(result.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn allow_once_does_not_cache() {
        let count = Arc::new(AtomicUsize::new(0));
        let approver = CountingApprover {
            count: Arc::clone(&count),
            response: ApprovalResponse::AllowOnce,
        };
        let guard = guard_with_allowed("/never/matches", approver);
        guard.check("/some/path", Op::Read).await.unwrap();
        guard.check("/some/path", Op::Read).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn allow_session_caches() {
        let count = Arc::new(AtomicUsize::new(0));
        let approver = CountingApprover {
            count: Arc::clone(&count),
            response: ApprovalResponse::AllowSession,
        };
        let guard = guard_with_allowed("/never/matches", approver);
        guard.check("/some/path", Op::Read).await.unwrap();
        guard.check("/some/path", Op::Read).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
