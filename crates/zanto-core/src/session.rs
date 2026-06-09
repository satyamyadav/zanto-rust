use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use directories::ProjectDirs;
use genai::chat::{ChatMessage, ChatRole};
use rusqlite::{Connection, params};
use rusqlite_migration::{Migrations, M};

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
    fn from(e: rusqlite::Error) -> Self { Self::Db(e) }
}
impl From<rusqlite_migration::Error> for SessionError {
    fn from(e: rusqlite_migration::Error) -> Self { Self::Migration(e) }
}
impl From<serde_json::Error> for SessionError {
    fn from(e: serde_json::Error) -> Self { Self::Json(e) }
}
impl From<std::io::Error> for SessionError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

// ---- Structs ----

pub struct Session {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<ChatMessage>,
}

pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
}

pub enum ContextPolicy {
    All,
    LastNTurns { max_turns: usize },
}

impl Default for ContextPolicy {
    fn default() -> Self {
        ContextPolicy::LastNTurns { max_turns: 20 }
    }
}

// ---- Session ----

impl Session {
    pub fn new(title: impl Into<String>, workspace: impl Into<String>) -> Self {
        let now = unix_now();
        Session {
            id: new_id(),
            title: title.into(),
            workspace: workspace.into(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    /// Returns the messages to send to the model: no system msgs (caller prepends),
    /// trimmed to policy.
    pub fn effective_messages(&self, policy: &ContextPolicy) -> Vec<ChatMessage> {
        match policy {
            ContextPolicy::All => self.messages.clone(),
            ContextPolicy::LastNTurns { max_turns } => trim_to_turns(&self.messages, *max_turns),
        }
    }
}

/// Auto-generate a title from the first user message in the session.
pub fn auto_title(messages: &[ChatMessage]) -> String {
    for msg in messages {
        if matches!(msg.role, ChatRole::User) {
            if let Some(text) = msg.content.first_text() {
                let t: String = text.chars().take(60).collect();
                if !t.is_empty() {
                    return t;
                }
            }
        }
    }
    "untitled".to_string()
}

fn trim_to_turns(messages: &[ChatMessage], max_turns: usize) -> Vec<ChatMessage> {
    // Split into turns at User boundaries; system messages are excluded (caller injects them).
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
    pub fn open() -> Result<Self, SessionError> {
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
        Ok(Store { conn: Mutex::new(conn) })
    }

    /// Upsert the session metadata row (does not touch messages).
    pub fn save_session(&self, session: &Session) -> Result<(), SessionError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, title, workspace, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, updated_at = excluded.updated_at",
            params![session.id, session.title, session.workspace, session.created_at as i64, session.updated_at as i64],
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
        let content = serde_json::to_string(msg)?;
        let role = role_str(&msg.role);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO messages (session_id, position, role, content) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, pos as i64, role, content],
        )?;
        Ok(())
    }

    /// Load a full session including all messages.
    pub fn load_session(&self, id: &str) -> Result<Session, SessionError> {
        let conn = self.conn.lock().unwrap();

        let row = conn.query_row(
            "SELECT title, workspace, created_at, updated_at FROM sessions WHERE id = ?1",
            params![id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)),
        );

        let (title, workspace, created_at, updated_at) = match row {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Err(SessionError::NotFound(id.to_string())),
            Err(e) => return Err(e.into()),
        };

        let mut stmt = conn.prepare(
            "SELECT content FROM messages WHERE session_id = ?1 ORDER BY position",
        )?;

        let messages: Result<Vec<ChatMessage>, SessionError> = stmt
            .query_map(params![id], |r| r.get::<_, String>(0))?
            .map(|r| r.map_err(SessionError::from).and_then(|s| serde_json::from_str(&s).map_err(SessionError::from)))
            .collect();

        Ok(Session {
            id: id.to_string(),
            title,
            workspace,
            created_at: created_at as u64,
            updated_at: updated_at as u64,
            messages: messages?,
        })
    }

    pub fn delete_session(&self, id: &str) -> Result<(), SessionError> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(SessionError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn list_sessions(&self, workspace_filter: Option<&str>) -> Result<Vec<SessionMeta>, SessionError> {
        let conn = self.conn.lock().unwrap();
        let (sql, count_params) = if workspace_filter.is_some() {
            (
                "SELECT s.id, s.title, s.workspace, s.created_at, s.updated_at, COUNT(m.id)
                 FROM sessions s LEFT JOIN messages m ON m.session_id = s.id
                 WHERE s.workspace = ?1
                 GROUP BY s.id ORDER BY s.updated_at DESC",
                true,
            )
        } else {
            (
                "SELECT s.id, s.title, s.workspace, s.created_at, s.updated_at, COUNT(m.id)
                 FROM sessions s LEFT JOIN messages m ON m.session_id = s.id
                 GROUP BY s.id ORDER BY s.updated_at DESC",
                false,
            )
        };

        let mut stmt = conn.prepare(sql)?;

        let rows: Result<Vec<SessionMeta>, SessionError> = if count_params {
            stmt.query_map(params![workspace_filter.unwrap()], row_to_meta)?
                .map(|r| r.map_err(SessionError::from))
                .collect()
        } else {
            stmt.query_map([], row_to_meta)?
                .map(|r| r.map_err(SessionError::from))
                .collect()
        };

        rows
    }

    pub fn last_session_id(&self, workspace_filter: Option<&str>) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        if let Some(ws) = workspace_filter {
            conn.query_row(
                "SELECT id FROM sessions WHERE workspace = ?1 ORDER BY updated_at DESC LIMIT 1",
                params![ws],
                |r| r.get::<_, String>(0),
            ).ok()
        } else {
            conn.query_row(
                "SELECT id FROM sessions ORDER BY updated_at DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            ).ok()
        }
    }

    /// Find a session ID by exact match or unique prefix. Returns error if ambiguous.
    pub fn find_by_prefix(&self, prefix: &str) -> Result<String, SessionError> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}%", prefix);
        let mut stmt = conn.prepare("SELECT id FROM sessions WHERE id LIKE ?1 ORDER BY updated_at DESC")?;
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
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays: [u64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &md in &mdays {
        if days < md { break; }
        days -= md;
        mo += 1;
    }
    let d = days + 1;
    format!("{y:04}{mo:02}{d:02}T{h:02}{m:02}{s:02}")
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Format unix seconds as "YYYY-MM-DD HH:MM" for display.
pub fn format_ts_display(secs: u64) -> String {
    let tod = secs % 86400;
    let mut days = secs / 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;

    let mut y = 1970u32;
    loop {
        let dy = if is_leap(y) { 366u64 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays: [u64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &md in &mdays {
        if days < md { break; }
        days -= md;
        mo += 1;
    }
    let d = days + 1;
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}
