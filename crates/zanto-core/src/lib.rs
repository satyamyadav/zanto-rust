// Re-exported so downstream crates can build typed SQL against the connection
// `DataStore::with_conn` lends, without taking their own rusqlite dependency
// (the version must match this crate's).
pub use rusqlite;

pub mod artifacts;
pub mod chat;
pub mod config;
pub mod context;
pub mod data;
pub mod permissions;
pub mod session;
pub mod summarize;
pub mod tools;
