pub mod migrations;
pub mod repo;
pub mod settings;

use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// The single owned handle to the database. Held in Tauri state; the webview
/// has no path to it (SPEC R5/R6).
pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    /// Opens (creating if needed) the database and brings the schema up to date.
    pub fn open(dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("timebox.db");
        let conn = Connection::open(&path)?;

        // WAL keeps a crash mid-write from tearing the file, which is what makes
        // "quit while awaiting a decision" survivable (acceptance test 7).
        // NORMAL is the right durability/throughput point under WAL.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;

        let db = Self { conn: Mutex::new(conn), path };
        db.migrate()?;
        Ok(db)
    }

    /// In-memory database for tests. Same migrations, no file.
    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self { conn: Mutex::new(conn), path: PathBuf::from(":memory:") };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> AppResult<()> {
        let mut guard = self.conn.lock();
        migrations::run(&mut guard)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> AppResult<T> {
        let guard = self.conn.lock();
        f(&guard).map_err(AppError::Db)
    }

    pub fn with_mut<T>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> AppResult<T> {
        let mut guard = self.conn.lock();
        f(&mut guard).map_err(AppError::Db)
    }

    pub fn schema_version(&self) -> AppResult<i64> {
        self.with(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
        })
    }

    pub fn journal_mode(&self) -> AppResult<String> {
        self.with(|c| c.query_row("PRAGMA journal_mode", [], |r| r.get(0)))
    }
}
