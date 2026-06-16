//! Workspace-scoped data engine. Ungated library: internal features / sub-apps call
//! this directly as a backend. Schemaless JSON records in logical "stores", queried
//! with a structured filter API (no raw SQL exposed). Single physical DB (`zanto.db`,
//! shared with sessions); each store is a table, mapped friendly-name → table in a
//! `data_stores` meta-table.

use std::path::Path;
use std::sync::Mutex;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::session::{db_path, unix_now_pub};

// ---- Errors ----

#[derive(Debug)]
pub enum DataError {
    Db(rusqlite::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
    NoDataDir,
    UnknownStore(String),
    InvalidName(String),
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(e) => write!(f, "database error: {e}"),
            Self::Json(e) => write!(f, "json error: {e}"),
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::NoDataDir => write!(f, "could not resolve app data directory"),
            Self::UnknownStore(s) => write!(f, "unknown store: {s}"),
            Self::InvalidName(s) => write!(f, "invalid store name: {s}"),
        }
    }
}
impl std::error::Error for DataError {}
impl From<rusqlite::Error> for DataError {
    fn from(e: rusqlite::Error) -> Self { Self::Db(e) }
}
impl From<serde_json::Error> for DataError {
    fn from(e: serde_json::Error) -> Self { Self::Json(e) }
}
impl From<std::io::Error> for DataError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

// ---- Query model (structured filters; no raw SQL) ----

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Query {
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sort: Option<Sort>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub field: String,
    pub op: FilterOp,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains, // substring (LIKE %v%)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sort {
    pub field: String,
    #[serde(default)]
    pub dir: Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: i64,
    pub data: Value,
    pub created_at: u64,
}

// ---- Store ----

const META_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS data_stores (
    workspace   TEXT NOT NULL,
    name        TEXT NOT NULL,
    table_name  TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    UNIQUE(workspace, name)
);";

pub struct DataStore {
    conn: Mutex<Connection>,
    workspace: String,
}

impl DataStore {
    /// Open the data engine for a workspace against the OS-conventional `zanto.db`
    /// (or `$ZANTO_DB` if set). Shares the file with sessions; uses only
    /// `CREATE TABLE IF NOT EXISTS` so it never touches the session migration version.
    pub fn open(workspace: &str) -> Result<Self, DataError> {
        let path = match std::env::var("ZANTO_DB") {
            Ok(v) => std::path::PathBuf::from(v),
            Err(_) => db_path().map_err(|_| DataError::NoDataDir)?,
        };
        Self::open_at(&path, workspace)
    }

    pub fn open_at(db_path: &Path, workspace: &str) -> Result<Self, DataError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(META_SCHEMA)?;
        Ok(DataStore { conn: Mutex::new(conn), workspace: workspace.to_string() })
    }

    /// Create a logical store (idempotent). Friendly name must be `[A-Za-z_][A-Za-z0-9_]*`.
    pub fn create_store(&self, name: &str) -> Result<(), DataError> {
        if !valid_name(name) {
            return Err(DataError::InvalidName(name.to_string()));
        }
        let table = table_name(&self.workspace, name);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {table} (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    data        TEXT NOT NULL,
                    created_at  INTEGER NOT NULL)"
            ),
            [],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO data_stores (workspace, name, table_name, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![self.workspace, name, table, unix_now_pub() as i64],
        )?;
        Ok(())
    }

    /// Friendly names of stores in this workspace.
    pub fn list_stores(&self) -> Result<Vec<String>, DataError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM data_stores WHERE workspace = ?1 ORDER BY name")?;
        let rows = stmt.query_map(params![self.workspace], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Insert a JSON record; returns its row id.
    pub fn insert(&self, store: &str, record: &Value) -> Result<i64, DataError> {
        let conn = self.conn.lock().unwrap();
        let table = resolve_table(&conn, &self.workspace, store)?;
        let json = serde_json::to_string(record)?;
        conn.execute(
            &format!("INSERT INTO {table} (data, created_at) VALUES (?1, ?2)"),
            params![json, unix_now_pub() as i64],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Query a store with structured filters. All values are bound as parameters;
    /// field names go in as bound `json_extract` paths — no SQL injection surface.
    pub fn query(&self, store: &str, q: &Query) -> Result<Vec<Record>, DataError> {
        let conn = self.conn.lock().unwrap();
        let table = resolve_table(&conn, &self.workspace, store)?;

        let mut sql = format!("SELECT id, data, created_at FROM {table}");
        let mut binds: Vec<SqlValue> = Vec::new();

        if !q.filters.is_empty() {
            let mut clauses = Vec::new();
            for f in &q.filters {
                binds.push(SqlValue::Text(format!("$.{}", f.field)));
                if f.op == FilterOp::Contains {
                    clauses.push("json_extract(data, ?) LIKE ?".to_string());
                    binds.push(json_to_like(&f.value));
                } else {
                    clauses.push(format!("json_extract(data, ?) {} ?", op_to_sql(f.op)));
                    binds.push(json_to_sql(&f.value));
                }
            }
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }

        if let Some(sort) = &q.sort {
            binds.push(SqlValue::Text(format!("$.{}", sort.field)));
            sql.push_str(" ORDER BY json_extract(data, ?) ");
            sql.push_str(if sort.dir == Dir::Desc { "DESC" } else { "ASC" });
        }

        if let Some(lim) = q.limit {
            binds.push(SqlValue::Integer(lim as i64));
            sql.push_str(" LIMIT ?");
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(binds.iter()), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (id, data_str, created_at) = row?;
            out.push(Record {
                id,
                data: serde_json::from_str(&data_str)?,
                created_at: created_at as u64,
            });
        }
        Ok(out)
    }
}

// ---- Helpers ----

fn resolve_table(conn: &Connection, workspace: &str, name: &str) -> Result<String, DataError> {
    conn.query_row(
        "SELECT table_name FROM data_stores WHERE workspace = ?1 AND name = ?2",
        params![workspace, name],
        |r| r.get::<_, String>(0),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => DataError::UnknownStore(name.to_string()),
        other => DataError::Db(other),
    })
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn table_name(workspace: &str, name: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    workspace.hash(&mut h);
    format!("ds_{:08x}_{}", h.finish() as u32, name)
}

fn op_to_sql(op: FilterOp) -> &'static str {
    match op {
        FilterOp::Eq => "=",
        FilterOp::Ne => "!=",
        FilterOp::Gt => ">",
        FilterOp::Gte => ">=",
        FilterOp::Lt => "<",
        FilterOp::Lte => "<=",
        FilterOp::Contains => "LIKE",
    }
}

fn json_to_sql(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else {
                SqlValue::Real(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

fn json_to_like(v: &Value) -> SqlValue {
    let s = match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    SqlValue::Text(format!("%{s}%"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn store(workspace: &str) -> (DataStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let ds = DataStore::open_at(&dir.path().join("test.db"), workspace).unwrap();
        (ds, dir)
    }

    #[test]
    fn open_creates_meta() {
        let (_ds, _dir) = store("/ws");
    }

    #[test]
    fn create_and_list_stores() {
        let (ds, _dir) = store("/ws");
        ds.create_store("transactions").unwrap();
        ds.create_store("transactions").unwrap(); // idempotent
        assert_eq!(ds.list_stores().unwrap(), vec!["transactions".to_string()]);
    }

    #[test]
    fn invalid_name_rejected() {
        let (ds, _dir) = store("/ws");
        assert!(matches!(ds.create_store("1bad"), Err(DataError::InvalidName(_))));
        assert!(matches!(ds.create_store("bad-name"), Err(DataError::InvalidName(_))));
        assert!(matches!(ds.create_store(""), Err(DataError::InvalidName(_))));
    }

    #[test]
    fn insert_and_query_eq() {
        let (ds, _dir) = store("/ws");
        ds.create_store("transactions").unwrap();
        ds.insert("transactions", &json!({"merchant":"DMart","amount":4200,"category":"groceries"})).unwrap();
        ds.insert("transactions", &json!({"merchant":"Shell","amount":2000,"category":"fuel"})).unwrap();
        ds.insert("transactions", &json!({"merchant":"BigBasket","amount":1800,"category":"groceries"})).unwrap();

        let q = Query {
            filters: vec![Filter { field: "category".into(), op: FilterOp::Eq, value: json!("groceries") }],
            ..Default::default()
        };
        let rows = ds.query("transactions", &q).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.data["category"] == "groceries"));
    }

    #[test]
    fn query_gt_numeric() {
        let (ds, _dir) = store("/ws");
        ds.create_store("t").unwrap();
        for a in [500, 1500, 4200] {
            ds.insert("t", &json!({"amount": a})).unwrap();
        }
        let q = Query {
            filters: vec![Filter { field: "amount".into(), op: FilterOp::Gt, value: json!(1000) }],
            ..Default::default()
        };
        let rows = ds.query("t", &q).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.data["amount"].as_i64().unwrap() > 1000));
    }

    #[test]
    fn query_contains_and_sort_limit() {
        let (ds, _dir) = store("/ws");
        ds.create_store("t").unwrap();
        ds.insert("t", &json!({"merchant":"DMart","amount":300})).unwrap();
        ds.insert("t", &json!({"merchant":"DMart Express","amount":100})).unwrap();
        ds.insert("t", &json!({"merchant":"Shell","amount":200})).unwrap();

        let q = Query {
            filters: vec![Filter { field: "merchant".into(), op: FilterOp::Contains, value: json!("DMart") }],
            sort: Some(Sort { field: "amount".into(), dir: Dir::Asc }),
            limit: Some(5),
        };
        let rows = ds.query("t", &q).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].data["amount"], 100); // sorted ascending
        assert_eq!(rows[1].data["amount"], 300);
    }

    #[test]
    fn unknown_store_errors() {
        let (ds, _dir) = store("/ws");
        assert!(matches!(ds.query("nope", &Query::default()), Err(DataError::UnknownStore(_))));
        assert!(matches!(ds.insert("nope", &json!({})), Err(DataError::UnknownStore(_))));
    }

    #[test]
    fn workspaces_isolated() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let a = DataStore::open_at(&path, "/ws/a").unwrap();
        let b = DataStore::open_at(&path, "/ws/b").unwrap();
        a.create_store("transactions").unwrap();
        b.create_store("transactions").unwrap();
        a.insert("transactions", &json!({"merchant":"A"})).unwrap();

        // Same store name, different workspace → isolated rows.
        assert_eq!(a.query("transactions", &Query::default()).unwrap().len(), 1);
        assert_eq!(b.query("transactions", &Query::default()).unwrap().len(), 0);
        assert_eq!(a.list_stores().unwrap(), vec!["transactions".to_string()]);
        assert_eq!(b.list_stores().unwrap(), vec!["transactions".to_string()]);
    }
}
