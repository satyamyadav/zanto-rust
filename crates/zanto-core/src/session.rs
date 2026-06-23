use directories::ProjectDirs;
use genai::chat::{ChatMessage, ChatRole};
use rusqlite::{Connection, params};
use rusqlite_migration::{M, Migrations};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

// ---- Error ----

#[derive(Debug)]
pub enum SessionError {
    Db(rusqlite::Error),
    Migration(rusqlite_migration::Error),
    Json(serde_json::Error),
    NotFound(String),
    AmbiguousPrefix(String),
    NoDataDir,
    Io(std::io::Error),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(e) => write!(f, "database error: {e}"),
            Self::Migration(e) => write!(f, "migration error: {e}"),
            Self::Json(e) => write!(f, "json error: {e}"),
            Self::NotFound(id) => write!(f, "session not found: {id}"),
            Self::AmbiguousPrefix(p) => write!(f, "ambiguous session prefix: {p}"),
            Self::NoDataDir => write!(f, "could not resolve app data directory"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}
impl From<rusqlite::Error> for SessionError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e)
    }
}
impl From<rusqlite_migration::Error> for SessionError {
    fn from(e: rusqlite_migration::Error) -> Self {
        Self::Migration(e)
    }
}
impl From<serde_json::Error> for SessionError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<std::io::Error> for SessionError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---- Structs ----

pub struct Session {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub app_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub archived: bool,
    pub summary: Option<String>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub app_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
    pub archived: bool,
}

pub enum ContextPolicy {
    All,
    LastNTurns {
        max_turns: usize,
    },
    /// Keep the last `keep_last` turns verbatim; when older turns exist, prepend
    /// the session's stored running `summary` (see `summarize` module) as a leading
    /// system message so dropped history is not lost.
    Summarize {
        keep_last: usize,
    },
    /// Automatic, model-aware management: keep as many recent messages verbatim as
    /// fit within `headroom_frac` of the model's `window_tokens`, summarizing the
    /// rest into the running summary. No manual turn count — the split is computed
    /// from estimated token usage at send time.
    Auto {
        window_tokens: usize,
        headroom_frac: f64,
    },
}

impl Default for ContextPolicy {
    fn default() -> Self {
        ContextPolicy::LastNTurns { max_turns: 20 }
    }
}

/// Rough token estimate for a string (~4 chars/token) — avoids pulling in a
/// per-provider tokenizer. Good enough to keep the live context within a model's
/// window for `ContextPolicy::Auto`.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count() / 4 + 1
}

/// Estimated tokens for one message: its text plus a small per-message overhead.
fn message_tokens(m: &ChatMessage) -> usize {
    m.content.first_text().map(estimate_tokens).unwrap_or(0) + 4
}

/// Index where the verbatim tail begins under `ContextPolicy::Auto`: keep the
/// newest messages whose cumulative estimated tokens fit `budget`, but always keep
/// at least the last two messages (one user+assistant turn). Returns 0 when the
/// whole history fits — nothing needs summarizing.
fn auto_split_index(messages: &[ChatMessage], budget: usize) -> usize {
    let n = messages.len();
    if n <= 2 {
        return 0;
    }
    let mut total = 0usize;
    let mut keep_start = 0usize;
    for i in (0..n).rev() {
        total += message_tokens(&messages[i]);
        if total > budget {
            keep_start = i + 1;
            break;
        }
        keep_start = i;
    }
    keep_start.min(n - 2)
}

// ---- Session ----

impl Session {
    pub fn new(title: impl Into<String>, workspace: impl Into<String>) -> Self {
        let now = unix_now();
        Session {
            id: new_id(),
            title: title.into(),
            workspace: workspace.into(),
            app_id: None,
            created_at: now,
            updated_at: now,
            archived: false,
            summary: None,
            messages: Vec::new(),
        }
    }

    /// (role, text) pairs for display in a UI — user/assistant text messages only
    /// (system and tool messages, and tool-call-only turns, are skipped).
    pub fn display_messages(&self) -> Vec<(String, String)> {
        self.messages
            .iter()
            .filter_map(|m| {
                let role = match m.role {
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                    _ => return None,
                };
                let text = m.content.first_text()?.to_string();
                if text.trim().is_empty() {
                    return None;
                }
                Some((role.to_string(), text))
            })
            .collect()
    }

    /// Like `display_messages`, but each entry also carries the per-message
    /// metadata aligned by position. `meta` must be positionally parallel to
    /// `self.messages` (as returned by `Store::load_message_meta`); entries shorter
    /// than `messages` yield `None`. Mirrors `display_messages` (system/tool messages
    /// skipped) with one deliberate difference: an assistant message whose text is
    /// empty is still emitted when it carries persisted block metadata, so a
    /// blocks-only turn (e.g. an artifact with no trailing prose) restores its
    /// artifacts on reopen instead of being dropped with the empty text.
    pub fn display_messages_meta(
        &self,
        meta: &[Option<serde_json::Value>],
    ) -> Vec<(String, String, Option<serde_json::Value>)> {
        self.messages
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let role = match m.role {
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                    _ => return None,
                };
                let text = m.content.first_text().unwrap_or("").to_string();
                let blocks = meta.get(i).and_then(|o| o.clone());
                // Keep the entry if it has visible text OR persisted blocks to render.
                if text.trim().is_empty() && blocks.is_none() {
                    return None;
                }
                Some((role.to_string(), text, blocks))
            })
            .collect()
    }

    /// Returns the messages to send to the model: no system msgs (caller prepends),
    /// trimmed to policy.
    pub fn effective_messages(&self, policy: &ContextPolicy) -> Vec<ChatMessage> {
        match policy {
            ContextPolicy::All => self.messages.clone(),
            ContextPolicy::LastNTurns { max_turns } => trim_to_turns(&self.messages, *max_turns),
            ContextPolicy::Summarize { keep_last } => {
                let trimmed = trim_to_turns(&self.messages, *keep_last);
                // Only prepend the summary when older turns were actually dropped
                // and we have a summary to inject.
                if count_turns(&self.messages) > *keep_last
                    && let Some(summary) = self.summary.as_deref().filter(|s| !s.trim().is_empty())
                {
                    let note = format!("Summary of earlier conversation:\n{summary}");
                    let mut out = Vec::with_capacity(trimmed.len() + 1);
                    out.push(ChatMessage::system(note));
                    out.extend(trimmed);
                    return out;
                }
                trimmed
            }
            ContextPolicy::Auto {
                window_tokens,
                headroom_frac,
            } => {
                let split =
                    auto_split_index(&self.messages, auto_budget(*window_tokens, *headroom_frac));
                let tail = self.messages[split..].to_vec();
                if split > 0
                    && let Some(summary) = self.summary.as_deref().filter(|s| !s.trim().is_empty())
                {
                    let note = format!("Summary of earlier conversation:\n{summary}");
                    let mut out = Vec::with_capacity(tail.len() + 1);
                    out.push(ChatMessage::system(note));
                    out.extend(tail);
                    return out;
                }
                tail
            }
        }
    }

    /// The older messages that `ContextPolicy::Auto` would fold out of the live
    /// window (everything before the verbatim tail). Empty when the whole history
    /// fits the budget. The chat loop summarizes these before sending the turn.
    pub fn auto_older(&self, window_tokens: usize, headroom_frac: f64) -> Vec<ChatMessage> {
        let split = auto_split_index(&self.messages, auto_budget(window_tokens, headroom_frac));
        self.messages[..split].to_vec()
    }
}

/// Token budget for the verbatim tail: `headroom_frac` of the window, clamped to a
/// sane fraction so a bad setting can't drop everything or disable trimming.
fn auto_budget(window_tokens: usize, headroom_frac: f64) -> usize {
    (window_tokens as f64 * headroom_frac.clamp(0.1, 0.95)) as usize
}

/// Auto-generate a title from the first user message in the session.
pub fn auto_title(messages: &[ChatMessage]) -> String {
    for msg in messages {
        if matches!(msg.role, ChatRole::User)
            && let Some(text) = msg.content.first_text()
        {
            let t: String = text.chars().take(60).collect();
            if !t.is_empty() {
                return t;
            }
        }
    }
    "untitled".to_string()
}

/// Count conversation turns (a turn starts at each non-system `User` message).
/// System messages are excluded, matching `trim_to_turns`.
fn count_turns(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .filter(|m| matches!(m.role, ChatRole::User))
        .count()
}

/// The older messages that `trim_to_turns(.., keep_last)` would drop: everything
/// except the last `keep_last` turns. System messages are excluded (the inverse of
/// the kept tail). Empty when there are `keep_last` turns or fewer. Used to feed the
/// summarizer the history being folded out of the live window.
pub fn messages_before_last_turns(messages: &[ChatMessage], keep_last: usize) -> Vec<ChatMessage> {
    let turns = split_into_turns(messages);
    let end = turns.len().saturating_sub(keep_last);
    turns[..end].iter().flatten().cloned().collect()
}

/// Split messages into conversation turns at non-system `User` boundaries; system
/// messages are excluded (the caller injects them).
fn split_into_turns(messages: &[ChatMessage]) -> Vec<Vec<ChatMessage>> {
    let conv: Vec<&ChatMessage> = messages
        .iter()
        .filter(|m| !matches!(m.role, ChatRole::System))
        .collect();

    let mut turns: Vec<Vec<ChatMessage>> = Vec::new();
    let mut current: Vec<ChatMessage> = Vec::new();
    for msg in conv {
        if matches!(msg.role, ChatRole::User) && !current.is_empty() {
            turns.push(std::mem::take(&mut current));
        }
        current.push(msg.clone());
    }
    if !current.is_empty() {
        turns.push(current);
    }
    turns
}

fn trim_to_turns(messages: &[ChatMessage], max_turns: usize) -> Vec<ChatMessage> {
    let turns = split_into_turns(messages);
    let start = turns.len().saturating_sub(max_turns);
    turns[start..].iter().flatten().cloned().collect()
}

// ---- Store ----

fn migrations() -> &'static Migrations<'static> {
    static MIGRATIONS: OnceLock<Migrations<'static>> = OnceLock::new();
    MIGRATIONS.get_or_init(|| {
        Migrations::new(vec![
            M::up(
                "CREATE TABLE IF NOT EXISTS sessions (
                    id          TEXT PRIMARY KEY,
                    title       TEXT NOT NULL DEFAULT '',
                    workspace   TEXT NOT NULL DEFAULT '',
                    created_at  INTEGER NOT NULL,
                    updated_at  INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS messages (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    position    INTEGER NOT NULL,
                    role        TEXT NOT NULL,
                    content     TEXT NOT NULL,
                    UNIQUE(session_id, position)
                );",
            ),
            // Sessions are scoped to the active micro-app (NULL = general mode).
            M::up("ALTER TABLE sessions ADD COLUMN app_id TEXT;"),
            // Archive flag (0/1), session summary text, per-message metadata JSON.
            M::up("ALTER TABLE sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;"),
            M::up("ALTER TABLE sessions ADD COLUMN summary TEXT;"),
            M::up("ALTER TABLE messages ADD COLUMN metadata TEXT;"),
        ])
    })
}

pub struct Store {
    conn: Mutex<Connection>,
}

// Connection is Send but not Sync; wrapping in Mutex makes Store Send + Sync.
unsafe impl Sync for Store {}

impl Store {
    /// Open the DB at the OS-conventional app-data path.
    /// If `ZANTO_DB` env var is set, uses that path instead (for test isolation).
    pub fn open() -> Result<Self, SessionError> {
        if let Ok(val) = std::env::var("ZANTO_DB") {
            return Self::open_at(Path::new(&val));
        }
        let path = db_path()?;
        Self::open_at(&path)
    }

    /// Open the DB at a specific path (useful for tests or overrides).
    pub fn open_at(db_path: &Path) -> Result<Self, SessionError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations().to_latest(&mut conn)?;
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    /// Upsert the session metadata row (does not touch messages).
    pub fn save_session(&self, session: &Session) -> Result<(), SessionError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, title, workspace, app_id, created_at, updated_at, archived, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title, app_id = excluded.app_id, updated_at = excluded.updated_at,
                 archived = excluded.archived, summary = excluded.summary",
            params![session.id, session.title, session.workspace, session.app_id, session.created_at as i64, session.updated_at as i64, session.archived as i64, session.summary],
        )?;
        Ok(())
    }

    /// Append a single message at the given position. Ignores duplicate (session_id, position).
    pub fn append_message(
        &self,
        session_id: &str,
        pos: usize,
        msg: &ChatMessage,
    ) -> Result<(), SessionError> {
        self.append_message_meta(session_id, pos, msg, None)
    }

    /// Append a single message with optional metadata JSON. Ignores duplicate (session_id, position).
    pub fn append_message_meta(
        &self,
        session_id: &str,
        pos: usize,
        msg: &ChatMessage,
        metadata: Option<&serde_json::Value>,
    ) -> Result<(), SessionError> {
        let content = serde_json::to_string(msg)?;
        let role = role_str(&msg.role);
        let metadata = match metadata {
            Some(v) => Some(serde_json::to_string(v)?),
            None => None,
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO messages (session_id, position, role, content, metadata) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, pos as i64, role, content, metadata],
        )?;
        Ok(())
    }

    /// Load per-message metadata, positionally parallel to `Session.messages`.
    pub fn load_message_meta(
        &self,
        session_id: &str,
    ) -> Result<Vec<Option<serde_json::Value>>, SessionError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT metadata FROM messages WHERE session_id = ?1 ORDER BY position")?;
        stmt.query_map(params![session_id], |r| r.get::<_, Option<String>>(0))?
            .map(|r| {
                r.map_err(SessionError::from).and_then(|opt| match opt {
                    Some(s) => Ok(Some(serde_json::from_str(&s)?)),
                    None => Ok(None),
                })
            })
            .collect()
    }

    /// Load a full session including all messages.
    pub fn load_session(&self, id: &str) -> Result<Session, SessionError> {
        let conn = self.conn.lock().unwrap();

        let row = conn.query_row(
            "SELECT title, workspace, app_id, created_at, updated_at, archived, summary FROM sessions WHERE id = ?1",
            params![id],
            |r| Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
            )),
        );

        let (title, workspace, app_id, created_at, updated_at, archived, summary) = match row {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(SessionError::NotFound(id.to_string()));
            }
            Err(e) => return Err(e.into()),
        };

        let mut stmt =
            conn.prepare("SELECT content FROM messages WHERE session_id = ?1 ORDER BY position")?;

        let messages: Result<Vec<ChatMessage>, SessionError> = stmt
            .query_map(params![id], |r| r.get::<_, String>(0))?
            .map(|r| {
                r.map_err(SessionError::from)
                    .and_then(|s| serde_json::from_str(&s).map_err(SessionError::from))
            })
            .collect();

        Ok(Session {
            id: id.to_string(),
            title,
            workspace,
            app_id,
            created_at: created_at as u64,
            updated_at: updated_at as u64,
            archived: archived != 0,
            summary,
            messages: messages?,
        })
    }

    /// Load a window of display (role, text) pairs for a session, ordered
    /// newest-last. Applies the same user/assistant text filtering as
    /// `Session::display_messages`, then windows the *filtered* list by
    /// `offset`/`limit` counting from the start (oldest). An `offset` past the
    /// end yields an empty vec.
    pub fn load_messages_page(
        &self,
        session_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<(String, String)>, SessionError> {
        let filtered = self.load_session(session_id)?.display_messages();
        Ok(filtered.into_iter().skip(offset).take(limit).collect())
    }

    pub fn delete_session(&self, id: &str) -> Result<(), SessionError> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(SessionError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// List non-archived sessions, optionally filtered by workspace/app.
    /// Unpaginated (all rows). Prefer `list_sessions_page` for large lists.
    pub fn list_sessions(
        &self,
        workspace_filter: Option<&str>,
        app_filter: Option<&str>,
    ) -> Result<Vec<SessionMeta>, SessionError> {
        self.list_sessions_filtered(workspace_filter, app_filter, false, 0, None)
    }

    /// List archived sessions, optionally filtered by workspace/app.
    pub fn list_sessions_archived(
        &self,
        workspace_filter: Option<&str>,
        app_filter: Option<&str>,
    ) -> Result<Vec<SessionMeta>, SessionError> {
        self.list_sessions_filtered(workspace_filter, app_filter, true, 0, None)
    }

    /// List one page of sessions (newest-first), windowed by `offset`/`limit`.
    /// `archived` selects the active (false) or archived (true) list.
    pub fn list_sessions_page(
        &self,
        workspace_filter: Option<&str>,
        app_filter: Option<&str>,
        archived: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SessionMeta>, SessionError> {
        self.list_sessions_filtered(workspace_filter, app_filter, archived, offset, Some(limit))
    }

    /// Shared list query. `limit == None` returns all rows from `offset`
    /// onward; `Some(n)` windows to at most `n` rows. Ordered newest-first.
    fn list_sessions_filtered(
        &self,
        workspace_filter: Option<&str>,
        app_filter: Option<&str>,
        archived: bool,
        offset: usize,
        limit: Option<usize>,
    ) -> Result<Vec<SessionMeta>, SessionError> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT s.id, s.title, s.workspace, s.created_at, s.updated_at, COUNT(m.id), s.app_id, s.archived
             FROM sessions s LEFT JOIN messages m ON m.session_id = s.id",
        );
        // archived flag is always constrained; workspace/app are optional.
        let mut conds: Vec<&str> = vec![if archived {
            "s.archived = 1"
        } else {
            "s.archived = 0"
        }];
        let mut binds: Vec<String> = Vec::new();
        if let Some(ws) = workspace_filter {
            conds.push("s.workspace = ?");
            binds.push(ws.to_string());
        }
        if let Some(app) = app_filter {
            conds.push("s.app_id = ?");
            binds.push(app.to_string());
        }
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
        sql.push_str(" GROUP BY s.id ORDER BY s.updated_at DESC");
        // LIMIT/OFFSET bound as params. SQLite needs a LIMIT to honour an OFFSET,
        // so an unbounded page (limit None) uses LIMIT -1 (all rows).
        match limit {
            Some(n) => binds.push(n.to_string()),
            None => binds.push("-1".to_string()),
        }
        binds.push(offset.to_string());
        sql.push_str(" LIMIT ? OFFSET ?");

        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map(rusqlite::params_from_iter(binds.iter()), row_to_meta)?
            .map(|r| r.map_err(SessionError::from))
            .collect()
    }

    /// Set or clear the archived flag on a session.
    pub fn set_archived(&self, id: &str, archived: bool) -> Result<(), SessionError> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions SET archived = ?1 WHERE id = ?2",
            params![archived as i64, id],
        )?;
        if n == 0 {
            return Err(SessionError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Set or clear the session summary text.
    pub fn set_summary(&self, id: &str, summary: Option<&str>) -> Result<(), SessionError> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions SET summary = ?1 WHERE id = ?2",
            params![summary, id],
        )?;
        if n == 0 {
            return Err(SessionError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn last_session_id(&self, workspace_filter: Option<&str>) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        if let Some(ws) = workspace_filter {
            conn.query_row(
                "SELECT id FROM sessions WHERE workspace = ?1 ORDER BY updated_at DESC LIMIT 1",
                params![ws],
                |r| r.get::<_, String>(0),
            )
            .ok()
        } else {
            conn.query_row(
                "SELECT id FROM sessions ORDER BY updated_at DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
        }
    }

    /// Find a session ID by exact match or unique prefix. Returns error if ambiguous.
    pub fn find_by_prefix(&self, prefix: &str) -> Result<String, SessionError> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}%", prefix);
        let mut stmt =
            conn.prepare("SELECT id FROM sessions WHERE id LIKE ?1 ORDER BY updated_at DESC")?;
        let ids: Vec<String> = stmt
            .query_map(params![pattern], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        match ids.len() {
            0 => Err(SessionError::NotFound(prefix.to_string())),
            1 => Ok(ids.into_iter().next().unwrap()),
            _ => {
                if let Some(exact) = ids.iter().find(|id| id.as_str() == prefix) {
                    Ok(exact.clone())
                } else {
                    Err(SessionError::AmbiguousPrefix(prefix.to_string()))
                }
            }
        }
    }

    pub fn clear(&self, workspace_filter: Option<&str>) -> Result<usize, SessionError> {
        let conn = self.conn.lock().unwrap();
        let n = if let Some(ws) = workspace_filter {
            conn.execute("DELETE FROM sessions WHERE workspace = ?1", params![ws])?
        } else {
            conn.execute("DELETE FROM sessions", [])?
        };
        Ok(n)
    }
}

// ---- Helpers ----

fn row_to_meta(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionMeta> {
    Ok(SessionMeta {
        id: r.get(0)?,
        title: r.get(1)?,
        workspace: r.get(2)?,
        created_at: r.get::<_, i64>(3)? as u64,
        updated_at: r.get::<_, i64>(4)? as u64,
        message_count: r.get::<_, i64>(5)? as usize,
        app_id: r.get::<_, Option<String>>(6)?,
        archived: r.get::<_, i64>(7)? != 0,
    })
}

fn role_str(role: &ChatRole) -> &'static str {
    match role {
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::System => "system",
        ChatRole::Tool => "tool",
    }
}

pub fn db_path() -> Result<PathBuf, SessionError> {
    ProjectDirs::from("", "", "zanto")
        .map(|d| d.data_dir().join("zanto.db"))
        .ok_or(SessionError::NoDataDir)
}

pub fn unix_now_pub() -> u64 {
    unix_now()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn new_id() -> String {
    let secs = unix_now();
    let ts = format_ts(secs);
    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
    format!("{}-{}", ts, suffix)
}

fn format_ts(secs: u64) -> String {
    let tod = secs % 86400;
    let mut days = secs / 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;
    let s = tod % 60;

    let mut y = 1970u32;
    loop {
        let dy = if is_leap(y) { 366u64 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u32;
    for &md in &mdays {
        if days < md {
            break;
        }
        days -= md;
        mo += 1;
    }
    let d = days + 1;
    format!("{y:04}{mo:02}{d:02}T{h:02}{m:02}{s:02}")
}

fn is_leap(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Format unix seconds as "YYYY-MM-DD HH:MM" for display.
/// System (IANA) local UTC offset in seconds, e.g. `+14400` for `+04:00`.
/// Read from the OS timezone via chrono (std exposes no local-time API).
fn local_offset_secs() -> i64 {
    chrono::Local::now().offset().local_minus_utc() as i64
}

/// Format a UTC timestamp for display in the user's **local** timezone as
/// `YYYY-MM-DD HH:MM`. A UTC-based calendar date runs a day behind local
/// wall-clock near midnight in `+offset` zones, so the model's "today",
/// finance's notion of today/this-month, and session-list timestamps all use
/// local time via this one formatter.
pub fn format_ts_display(secs: u64) -> String {
    format_ts_display_at_offset(secs, local_offset_secs())
}

/// Apply a fixed UTC offset (seconds) to a UTC epoch, then format as
/// `YYYY-MM-DD HH:MM`. Pure (offset is supplied) — unit-testable. A pre-epoch
/// result clamps to the epoch rather than wrapping.
fn format_ts_display_at_offset(secs: u64, offset_secs: i64) -> String {
    let shifted = (secs as i64 + offset_secs).max(0) as u64;
    format_epoch_ymd_hm(shifted)
}

/// Format a UTC epoch-seconds value as a naive `YYYY-MM-DD HH:MM`, with no
/// timezone shift. Pure.
fn format_epoch_ymd_hm(secs: u64) -> String {
    let tod = secs % 86400;
    let mut days = secs / 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;

    let mut y = 1970u32;
    loop {
        let dy = if is_leap(y) { 366u64 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u32;
    for &md in &mdays {
        if days < md {
            break;
        }
        days -= md;
        mo += 1;
    }
    let d = days + 1;
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}

/// Short single-line system-info block: OS, arch, cwd, shell, today's date.
/// Pure function — reads env/cwd/clock, mutates nothing.
pub fn system_info() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".to_string());
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "?".to_string());
    // Date portion ("YYYY-MM-DD") of the current local-naive timestamp.
    let date: String = format_ts_display(unix_now()).chars().take(10).collect();
    format!("System: {os} {arch} · cwd: {cwd} · shell: {shell} · date: {date}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use genai::chat::ChatMessage;
    use tempfile::TempDir;

    fn temp_store() -> (Store, TempDir) {
        let dir = TempDir::new().unwrap();
        let store = Store::open_at(&dir.path().join("test.db")).unwrap();
        (store, dir)
    }

    fn make_session(workspace: &str) -> Session {
        Session::new("test session", workspace)
    }

    #[test]
    fn open_at_creates_schema() {
        let (_store, _dir) = temp_store();
        // if schema creation failed, open_at would have panicked
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (store, _dir) = temp_store();
        let mut s = make_session("/ws/a");
        s.title = "my title".to_string();
        store.save_session(&s).unwrap();

        let loaded = store.load_session(&s.id).unwrap();
        assert_eq!(loaded.id, s.id);
        assert_eq!(loaded.title, "my title");
        assert_eq!(loaded.workspace, "/ws/a");
        assert!(loaded.messages.is_empty());
    }

    #[test]
    fn append_and_load_messages() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws/a");
        store.save_session(&s).unwrap();

        let m0 = ChatMessage::user("hello");
        let m1 = ChatMessage::assistant("hi there");
        store.append_message(&s.id, 0, &m0).unwrap();
        store.append_message(&s.id, 1, &m1).unwrap();

        let loaded = store.load_session(&s.id).unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content.first_text().unwrap(), "hello");
        assert_eq!(loaded.messages[1].content.first_text().unwrap(), "hi there");
    }

    #[test]
    fn load_messages_page_windows_filtered_list() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws");
        store.save_session(&s).unwrap();

        // 10 display messages (alternating user/assistant) plus a system + an
        // empty-text message that the filter must drop.
        store
            .append_message(&s.id, 0, &ChatMessage::system("you are a bot"))
            .unwrap();
        let mut pos = 1;
        for i in 0..10u8 {
            let m = if i % 2 == 0 {
                ChatMessage::user(format!("u{i}"))
            } else {
                ChatMessage::assistant(format!("a{i}"))
            };
            store.append_message(&s.id, pos, &m).unwrap();
            pos += 1;
        }
        store
            .append_message(&s.id, pos, &ChatMessage::assistant("   "))
            .unwrap();

        // Full list (system + blank filtered out) = 10 entries, newest-last.
        let all = store.load_messages_page(&s.id, 0, 100).unwrap();
        assert_eq!(all.len(), 10);
        assert_eq!(all[0], ("user".to_string(), "u0".to_string()));
        assert_eq!(all[9], ("assistant".to_string(), "a9".to_string()));

        // Most-recent page of 4 = offset 6.
        let page = store.load_messages_page(&s.id, 6, 4).unwrap();
        assert_eq!(page.len(), 4);
        assert_eq!(page[0], ("user".to_string(), "u6".to_string()));
        assert_eq!(page[3], ("assistant".to_string(), "a9".to_string()));

        // Older page of 4 = offset 2.
        let older = store.load_messages_page(&s.id, 2, 4).unwrap();
        assert_eq!(older.len(), 4);
        assert_eq!(older[0].1, "u2");
        assert_eq!(older[3].1, "a5");

        // Offset past the end yields empty.
        assert!(store.load_messages_page(&s.id, 50, 4).unwrap().is_empty());
    }

    #[test]
    fn last_session_id_returns_most_recent() {
        let (store, _dir) = temp_store();
        let mut s1 = make_session("/ws");
        let mut s2 = make_session("/ws");
        s1.updated_at = 100;
        s2.updated_at = 200;
        store.save_session(&s1).unwrap();
        store.save_session(&s2).unwrap();

        assert_eq!(store.last_session_id(None).unwrap(), s2.id);
    }

    #[test]
    fn find_by_prefix_exact() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws");
        store.save_session(&s).unwrap();
        assert_eq!(store.find_by_prefix(&s.id).unwrap(), s.id);
    }

    #[test]
    fn find_by_prefix_partial() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws");
        store.save_session(&s).unwrap();
        let prefix = &s.id[..10];
        assert_eq!(store.find_by_prefix(prefix).unwrap(), s.id);
    }

    #[test]
    fn find_by_prefix_ambiguous() {
        let (store, _dir) = temp_store();
        // Force two sessions with the same timestamp prefix by saving both
        // before any time can elapse. IDs share the same timestamp portion.
        let s1 = make_session("/ws");
        let s2 = make_session("/ws");
        store.save_session(&s1).unwrap();
        store.save_session(&s2).unwrap();
        // Both start with the date portion (15 chars). If UUIDs differ, an
        // 8-char prefix of s1 might not be shared — only test ambiguity if
        // the first 8 chars actually match both IDs.
        let prefix8 = &s1.id[..8];
        let both_match = s2.id.starts_with(prefix8);
        if both_match {
            assert!(matches!(
                store.find_by_prefix(prefix8),
                Err(SessionError::AmbiguousPrefix(_))
            ));
        }
        // If they don't share the prefix, the test is vacuously satisfied —
        // UUID randomness makes forced collision impractical without seeding.
    }

    #[test]
    fn list_sessions_workspace_filter() {
        let (store, _dir) = temp_store();
        let s_a = make_session("/ws/a");
        let s_b = make_session("/ws/b");
        store.save_session(&s_a).unwrap();
        store.save_session(&s_b).unwrap();

        let list = store.list_sessions(Some("/ws/a"), None).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, s_a.id);
    }

    #[test]
    fn delete_cascades_messages() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws");
        store.save_session(&s).unwrap();
        store
            .append_message(&s.id, 0, &ChatMessage::user("hi"))
            .unwrap();

        store.delete_session(&s.id).unwrap();
        assert!(matches!(
            store.load_session(&s.id),
            Err(SessionError::NotFound(_))
        ));
    }

    #[test]
    fn context_policy_last_n_turns() {
        let policy = ContextPolicy::LastNTurns { max_turns: 2 };
        let mut s = make_session("/ws");

        // 3 turns: user+assistant each
        for i in 0..3u8 {
            s.messages.push(ChatMessage::user(format!("q{i}")));
            s.messages.push(ChatMessage::assistant(format!("a{i}")));
        }

        let effective = s.effective_messages(&policy);
        // Should contain only last 2 turns (4 messages)
        assert_eq!(effective.len(), 4);
        assert_eq!(effective[0].content.first_text().unwrap(), "q1");
        assert_eq!(effective[2].content.first_text().unwrap(), "q2");
    }

    #[test]
    fn context_policy_summarize_prepends_summary_and_keeps_last() {
        let policy = ContextPolicy::Summarize { keep_last: 2 };
        let mut s = make_session("/ws");
        s.summary = Some("earlier recap".to_string());

        // 4 turns: user+assistant each.
        for i in 0..4u8 {
            s.messages.push(ChatMessage::user(format!("q{i}")));
            s.messages.push(ChatMessage::assistant(format!("a{i}")));
        }

        let effective = s.effective_messages(&policy);
        // Leading system summary + last 2 turns (4 messages) = 5.
        assert_eq!(effective.len(), 5);
        assert!(matches!(effective[0].role, ChatRole::System));
        assert!(
            effective[0]
                .content
                .first_text()
                .unwrap()
                .contains("earlier recap")
        );
        assert_eq!(effective[1].content.first_text().unwrap(), "q2");
        assert_eq!(effective[3].content.first_text().unwrap(), "q3");
    }

    #[test]
    fn estimate_tokens_is_roughly_chars_over_four() {
        assert_eq!(estimate_tokens(""), 1);
        assert_eq!(estimate_tokens("abcd"), 2); // 4/4 + 1
        assert!(estimate_tokens(&"x".repeat(400)) >= 100);
    }

    #[test]
    fn context_policy_auto_summarizes_when_over_budget() {
        // window 200 @ 0.5 headroom → ~100-token budget; six ~40-char messages
        // (~15 tokens each) overflow it, so a verbatim tail + summary is produced.
        let policy = ContextPolicy::Auto {
            window_tokens: 200,
            headroom_frac: 0.5,
        };
        let mut s = make_session("/ws");
        s.summary = Some("earlier recap".to_string());
        for _ in 0..6 {
            s.messages.push(ChatMessage::user("x".repeat(40)));
            s.messages.push(ChatMessage::assistant("y".repeat(40)));
        }
        let eff = s.effective_messages(&policy);
        assert!(matches!(eff[0].role, ChatRole::System));
        assert!(
            eff[0]
                .content
                .first_text()
                .unwrap()
                .contains("earlier recap")
        );
        // Older messages were dropped from the live window → non-empty complement.
        assert!(!s.auto_older(200, 0.5).is_empty());
    }

    #[test]
    fn context_policy_auto_keeps_everything_when_it_fits() {
        let policy = ContextPolicy::Auto {
            window_tokens: 1_000_000,
            headroom_frac: 0.75,
        };
        let mut s = make_session("/ws");
        for i in 0..3u8 {
            s.messages.push(ChatMessage::user(format!("q{i}")));
            s.messages.push(ChatMessage::assistant(format!("a{i}")));
        }
        assert_eq!(s.effective_messages(&policy).len(), 6); // all verbatim
        assert!(s.auto_older(1_000_000, 0.75).is_empty());
    }

    #[test]
    fn context_policy_auto_always_keeps_a_full_last_turn() {
        // Even a tiny window keeps the most recent turn (2 messages) whole.
        let policy = ContextPolicy::Auto {
            window_tokens: 1,
            headroom_frac: 0.5,
        };
        let mut s = make_session("/ws"); // no summary set
        for i in 0..4u8 {
            s.messages.push(ChatMessage::user(format!("q{i}")));
            s.messages.push(ChatMessage::assistant(format!("a{i}")));
        }
        let eff = s.effective_messages(&policy);
        assert_eq!(eff.len(), 2);
        assert_eq!(eff[0].content.first_text().unwrap(), "q3");
        assert_eq!(eff[1].content.first_text().unwrap(), "a3");
    }

    #[test]
    fn messages_before_last_turns_returns_older_complement() {
        let mut msgs = Vec::new();
        // 4 turns: user+assistant each.
        for i in 0..4u8 {
            msgs.push(ChatMessage::user(format!("q{i}")));
            msgs.push(ChatMessage::assistant(format!("a{i}")));
        }

        // keep_last=2 → older = turns 0,1 (4 messages: q0,a0,q1,a1).
        let older = messages_before_last_turns(&msgs, 2);
        assert_eq!(older.len(), 4);
        assert_eq!(older[0].content.first_text().unwrap(), "q0");
        assert_eq!(older[3].content.first_text().unwrap(), "a1");
        // The kept tail (q2,q3) must NOT appear in the older slice.
        assert!(!older.iter().any(|m| m.content.first_text() == Some("q2")));

        // Within budget → nothing older.
        assert!(messages_before_last_turns(&msgs, 4).is_empty());
        assert!(messages_before_last_turns(&msgs, 10).is_empty());
        // System messages are excluded from the older slice.
        let mut with_sys = vec![ChatMessage::system("sys")];
        with_sys.extend(msgs.clone());
        let older_sys = messages_before_last_turns(&with_sys, 2);
        assert!(!older_sys.iter().any(|m| matches!(m.role, ChatRole::System)));
    }

    #[test]
    fn context_policy_summarize_no_summary_when_within_budget() {
        let policy = ContextPolicy::Summarize { keep_last: 5 };
        let mut s = make_session("/ws");
        s.summary = Some("earlier recap".to_string());

        // Only 2 turns — within budget, so no summary prepended.
        for i in 0..2u8 {
            s.messages.push(ChatMessage::user(format!("q{i}")));
            s.messages.push(ChatMessage::assistant(format!("a{i}")));
        }

        let effective = s.effective_messages(&policy);
        assert_eq!(effective.len(), 4);
        assert!(!matches!(effective[0].role, ChatRole::System));
    }

    #[test]
    fn auto_title_takes_first_user_message() {
        let msgs = vec![
            ChatMessage::assistant("hi"),
            ChatMessage::user("what is the capital of France?"),
        ];
        assert_eq!(auto_title(&msgs), "what is the capital of France?");
    }

    #[test]
    fn auto_title_truncates_at_60_chars() {
        let long = "a".repeat(80);
        let msgs = vec![ChatMessage::user(long)];
        assert_eq!(auto_title(&msgs).len(), 60);
    }

    #[test]
    fn archived_round_trip() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws");
        store.save_session(&s).unwrap();
        assert!(!store.load_session(&s.id).unwrap().archived);

        store.set_archived(&s.id, true).unwrap();
        assert!(store.load_session(&s.id).unwrap().archived);

        store.set_archived(&s.id, false).unwrap();
        assert!(!store.load_session(&s.id).unwrap().archived);
    }

    #[test]
    fn summary_persist_and_load() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws");
        store.save_session(&s).unwrap();
        assert!(store.load_session(&s.id).unwrap().summary.is_none());

        store.set_summary(&s.id, Some("a recap")).unwrap();
        assert_eq!(
            store.load_session(&s.id).unwrap().summary.as_deref(),
            Some("a recap")
        );

        store.set_summary(&s.id, None).unwrap();
        assert!(store.load_session(&s.id).unwrap().summary.is_none());
    }

    #[test]
    fn message_metadata_round_trip() {
        let (store, _dir) = temp_store();
        let s = make_session("/ws");
        store.save_session(&s).unwrap();

        let meta = serde_json::json!({ "kind": "artifact", "n": 7 });
        store
            .append_message(&s.id, 0, &ChatMessage::user("hi"))
            .unwrap();
        store
            .append_message_meta(&s.id, 1, &ChatMessage::assistant("yo"), Some(&meta))
            .unwrap();

        let loaded = store.load_message_meta(&s.id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded[0].is_none());
        assert_eq!(loaded[1].as_ref().unwrap(), &meta);
    }

    #[test]
    fn display_messages_meta_aligns_and_keeps_blocks_only_turn() {
        let mut s = make_session("/ws");
        // system (skipped), user, assistant-with-text, blocks-only assistant (empty
        // text but has metadata → must be kept), trailing user.
        s.messages.push(ChatMessage::system("sys"));
        s.messages.push(ChatMessage::user("draw it"));
        s.messages.push(ChatMessage::assistant("here"));
        s.messages.push(ChatMessage::assistant(""));
        s.messages.push(ChatMessage::user("thanks"));

        let blocks = serde_json::json!({ "blocks": [{ "kind": "component" }] });
        // meta is positionally parallel to messages: only the blocks-only assistant
        // (index 3) carries metadata.
        let meta = vec![None, None, None, Some(blocks.clone()), None];

        let out = s.display_messages_meta(&meta);
        // system dropped; the empty-text assistant is kept because it has blocks.
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], ("user".into(), "draw it".into(), None));
        assert_eq!(out[1], ("assistant".into(), "here".into(), None));
        assert_eq!(out[2], ("assistant".into(), "".into(), Some(blocks)));
        assert_eq!(out[3], ("user".into(), "thanks".into(), None));
    }

    #[test]
    fn display_messages_meta_drops_empty_turn_without_blocks() {
        let mut s = make_session("/ws");
        s.messages.push(ChatMessage::user("hi"));
        s.messages.push(ChatMessage::assistant("   ")); // whitespace, no meta → dropped

        let meta = vec![None, None];
        let out = s.display_messages_meta(&meta);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "user");
    }

    #[test]
    fn list_sessions_excludes_archived() {
        let (store, _dir) = temp_store();
        let active = make_session("/ws");
        let archived = make_session("/ws");
        store.save_session(&active).unwrap();
        store.save_session(&archived).unwrap();
        store.set_archived(&archived.id, true).unwrap();

        let list = store.list_sessions(Some("/ws"), None).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, active.id);
        assert!(!list[0].archived);

        let arch = store.list_sessions_archived(Some("/ws"), None).unwrap();
        assert_eq!(arch.len(), 1);
        assert_eq!(arch[0].id, archived.id);
        assert!(arch[0].archived);
    }

    #[test]
    fn list_sessions_page_windows_newest_first() {
        let (store, _dir) = temp_store();
        // Insert 25 sessions with strictly increasing updated_at so order is
        // deterministic (newest-first = highest updated_at first).
        let mut ids = Vec::new();
        for i in 0..25u64 {
            let mut s = make_session("/ws");
            s.updated_at = 1000 + i;
            store.save_session(&s).unwrap();
            ids.push(s.id);
        }
        // ids[24] is newest. Page through in chunks of 10.
        let page_size = 10;
        let p0 = store
            .list_sessions_page(Some("/ws"), None, false, 0, page_size)
            .unwrap();
        assert_eq!(p0.len(), 10);
        assert_eq!(p0[0].id, ids[24]); // newest first
        assert_eq!(p0[9].id, ids[15]);

        let p1 = store
            .list_sessions_page(Some("/ws"), None, false, 10, page_size)
            .unwrap();
        assert_eq!(p1.len(), 10);
        assert_eq!(p1[0].id, ids[14]);
        assert_eq!(p1[9].id, ids[5]);

        let p2 = store
            .list_sessions_page(Some("/ws"), None, false, 20, page_size)
            .unwrap();
        assert_eq!(p2.len(), 5); // remainder
        assert_eq!(p2[0].id, ids[4]);
        assert_eq!(p2[4].id, ids[0]);

        // Offset past the end yields empty.
        assert!(
            store
                .list_sessions_page(Some("/ws"), None, false, 100, page_size)
                .unwrap()
                .is_empty()
        );

        // Unpaginated list still returns all rows.
        assert_eq!(store.list_sessions(Some("/ws"), None).unwrap().len(), 25);
    }

    #[test]
    fn local_offset_shifts_display_across_midnight() {
        // No offset: unchanged from the UTC formatter.
        assert_eq!(format_ts_display_at_offset(0, 0), "1970-01-01 00:00");
        assert_eq!(format_ts_display_at_offset(0, 3600), "1970-01-01 01:00");
        // +04:00 just before UTC midnight rolls the LOCAL date forward a day:
        // 1970-01-01 22:00 UTC (79200s) + 4h → 1970-01-02 02:00 local.
        assert_eq!(
            format_ts_display_at_offset(79200, 4 * 3600),
            "1970-01-02 02:00"
        );
        // Negative offset rolls back: 1970-01-02 02:00 UTC (93600s) - 4h → 1970-01-01 22:00.
        assert_eq!(
            format_ts_display_at_offset(93600, -4 * 3600),
            "1970-01-01 22:00"
        );
        // Pre-epoch underflow clamps to the epoch rather than wrapping.
        assert_eq!(format_ts_display_at_offset(0, -3600), "1970-01-01 00:00");
    }

    #[test]
    fn system_info_is_non_empty_and_dated() {
        let info = system_info();
        assert!(!info.is_empty());
        let date: String = format_ts_display(unix_now()).chars().take(10).collect();
        assert!(
            info.contains(&date),
            "info {info:?} should contain date {date}"
        );
    }
}
