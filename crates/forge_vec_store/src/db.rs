//! SQLite connection management and schema migration.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::Connection;
use sqlite_vec::sqlite3_vec_init;

const SCHEMA_VERSION: u32 = 1;

/// SQLite database wrapper with sqlite-vec extension loaded.
pub struct VecStoreDb {
    conn: Mutex<Connection>,
    dimensions: usize,
}

impl VecStoreDb {
    /// Open (or create) a database at the given path with the specified
    /// embedding dimensions.
    pub fn open(path: impl AsRef<Path>, dimensions: usize) -> Result<Self> {
        // Register sqlite-vec as auto extension before opening
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("Failed to open database at {}", path.as_ref().display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let db = Self { conn: Mutex::new(conn), dimensions };
        db.migrate()?;
        Ok(db)
    }

    /// Open an in-memory database (useful for testing).
    pub fn open_in_memory(dimensions: usize) -> Result<Self> {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;

        let db = Self { conn: Mutex::new(conn), dimensions };
        db.migrate()?;
        Ok(db)
    }

    /// Returns the configured embedding dimensions.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Execute a closure with exclusive access to the connection.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        f(&conn)
    }

    /// Execute a closure within a transaction.
    pub fn with_tx<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<T>,
    {
        let mut conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        let tx = conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    fn migrate(&self) -> Result<()> {
        self.with_conn(|conn| {
            let version: u32 = conn
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap_or(0);

            if version < 1 {
                Self::migrate_v1(conn)?;
            }

            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            Ok(())
        })
    }

    fn migrate_v1(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                working_dir TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT
            );

            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                hash TEXT NOT NULL,
                UNIQUE(workspace_id, path)
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                content TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_files_workspace ON files(workspace_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id);
            ",
        )?;

        // Create the sqlite-vec virtual table for vector search.
        // We use a separate statement because virtual table DDL can't be
        // inside execute_batch on some builds.
        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[{}])",
                // Placeholder dimensions — caller must ensure consistency
                384
            ),
            [],
        )?;

        Ok(())
    }

    /// Recreate the vec_chunks virtual table with the correct dimensions.
    /// Call this after opening if the provider's dimensions differ from default.
    pub fn ensure_vec_table(&self, dimensions: usize) -> Result<()> {
        self.with_conn(|conn| {
            // Drop and recreate — vec0 tables don't support ALTER
            conn.execute_batch("DROP TABLE IF EXISTS vec_chunks;")?;
            conn.execute(
                &format!(
                    "CREATE VIRTUAL TABLE vec_chunks USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[{dimensions}])"
                ),
                [],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory() {
        let db = VecStoreDb::open_in_memory(384).unwrap();
        // Verify sqlite-vec is loaded
        db.with_conn(|conn| {
            let version: String =
                conn.query_row("SELECT vec_version()", [], |row| row.get(0))?;
            assert!(!version.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_schema_created() {
        let db = VecStoreDb::open_in_memory(384).unwrap();
        db.with_conn(|conn| {
            // Check tables exist
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('workspaces', 'files', 'chunks')",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count, 3);
            Ok(())
        })
        .unwrap();
    }
}
