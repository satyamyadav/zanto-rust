//! Typed SQLite schema for the finance app (W5). Replaces the schemaless JSON
//! `DataStore` blobs with real `fin_*` tables — foreign keys + CHECK constraints
//! the JSON store can't enforce. Created idempotently over the same connection
//! `DataStore` already holds (via `DataStore::with_conn`), so it shares the WAL
//! file with sessions/JSON-stores and never touches their migration version.
//!
//! Scoping: the app is single-user/local-first, but the engine isolates by
//! `workspace`, so top-level tables carry a `workspace` column and all queries
//! filter by it — matching how the JSON stores stay workspace-isolated.

use zanto_core::rusqlite::{self, Connection};

/// Idempotent DDL: every table/index is `IF NOT EXISTS`, so this is safe to run on
/// every finance access (like the JSON `create_store`). FK + CHECK enforce the
/// integrity the audit found missing (orphan accounts, invalid types, budget→cat).
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS fin_categories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace  TEXT NOT NULL,
    parent_id  INTEGER REFERENCES fin_categories(id),
    name       TEXT NOT NULL,
    type       TEXT NOT NULL CHECK(type IN ('income','expense')),
    UNIQUE(workspace, name)
);

CREATE TABLE IF NOT EXISTS fin_accounts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace       TEXT NOT NULL,
    name            TEXT NOT NULL,
    type            TEXT NOT NULL CHECK(type IN ('checking','savings','credit','cash','investment')),
    institution     TEXT,
    opening_balance REAL NOT NULL DEFAULT 0,
    currency        TEXT NOT NULL DEFAULT 'USD',
    UNIQUE(workspace, name)
);

CREATE TABLE IF NOT EXISTS fin_transactions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace        TEXT NOT NULL,
    account_id       INTEGER NOT NULL REFERENCES fin_accounts(id),
    category_id      INTEGER REFERENCES fin_categories(id),
    amount           REAL NOT NULL,
    transaction_type TEXT NOT NULL CHECK(transaction_type IN ('income','expense')),
    merchant         TEXT,
    notes            TEXT,
    transaction_date TEXT NOT NULL,
    source           TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual','import')),
    import_hash      TEXT,
    created_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_fin_txn_ws_date  ON fin_transactions(workspace, transaction_date);
CREATE INDEX IF NOT EXISTS idx_fin_txn_account  ON fin_transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_fin_txn_category ON fin_transactions(category_id);
CREATE INDEX IF NOT EXISTS idx_fin_txn_hash     ON fin_transactions(workspace, import_hash);

CREATE TABLE IF NOT EXISTS fin_transfers (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace       TEXT NOT NULL,
    from_account_id INTEGER NOT NULL REFERENCES fin_accounts(id),
    to_account_id   INTEGER NOT NULL REFERENCES fin_accounts(id),
    amount          REAL NOT NULL,
    transfer_date   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fin_budgets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace   TEXT NOT NULL,
    category_id INTEGER NOT NULL REFERENCES fin_categories(id),
    amount      REAL NOT NULL,
    UNIQUE(workspace, category_id)
);

CREATE TABLE IF NOT EXISTS fin_goals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace     TEXT NOT NULL,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK(kind IN ('savings','debt')),
    account_id    INTEGER REFERENCES fin_accounts(id),
    target_amount REAL NOT NULL,
    target_date   TEXT
);

CREATE TABLE IF NOT EXISTS fin_category_rules (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace        TEXT NOT NULL,
    merchant_contains TEXT NOT NULL,
    category_id      INTEGER NOT NULL REFERENCES fin_categories(id)
);

CREATE TABLE IF NOT EXISTS fin_import_profiles (
    workspace  TEXT NOT NULL,
    account_id INTEGER NOT NULL REFERENCES fin_accounts(id),
    mapping    TEXT NOT NULL,
    PRIMARY KEY(workspace, account_id)
);

-- Per-workspace profile (currency, monthly_income). One row per workspace.
CREATE TABLE IF NOT EXISTS fin_profile (
    workspace      TEXT PRIMARY KEY,
    currency       TEXT NOT NULL DEFAULT 'USD',
    monthly_income REAL,
    setup          INTEGER NOT NULL DEFAULT 0
);

-- Migration marker: presence of a row means the one-time JSON→typed move ran for
-- this workspace, so it never runs twice.
CREATE TABLE IF NOT EXISTS fin_meta (
    workspace   TEXT PRIMARY KEY,
    migrated_at INTEGER NOT NULL
);
";

/// Create the `fin_*` schema (idempotent). Run from the finance app's
/// `ensure_store`, inside `DataStore::with_conn`.
pub fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)
}

/// Default expense categories seeded for a fresh workspace (matches the prior
/// JSON defaults). `income` is added as the lone income-type category so imported
/// salary rows can map somewhere.
pub const DEFAULT_EXPENSE_CATEGORIES: &[&str] =
    &["groceries", "dining", "transport", "utilities", "rent", "entertainment", "other"];

/// Seed default categories for a workspace if it has none yet. Idempotent via the
/// UNIQUE(workspace, name) constraint + INSERT OR IGNORE.
pub fn seed_default_categories(conn: &Connection, workspace: &str) -> rusqlite::Result<()> {
    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM fin_categories WHERE workspace = ?1",
        [workspace],
        |r| r.get(0),
    )?;
    if existing > 0 {
        return Ok(());
    }
    for name in DEFAULT_EXPENSE_CATEGORIES {
        conn.execute(
            "INSERT OR IGNORE INTO fin_categories (workspace, name, type) VALUES (?1, ?2, 'expense')",
            rusqlite::params![workspace, name],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO fin_categories (workspace, name, type) VALUES (?1, 'income', 'income')",
        [workspace],
    )?;
    Ok(())
}
