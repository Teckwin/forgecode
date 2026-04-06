//! Workspace CRUD operations backed by SQLite.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::db::VecStoreDb;

/// Manages workspace lifecycle in the local database.
pub struct WorkspaceManager<'a> {
    db: &'a VecStoreDb,
}

impl<'a> WorkspaceManager<'a> {
    pub fn new(db: &'a VecStoreDb) -> Self {
        Self { db }
    }

    /// Create a new workspace for a working directory.
    /// Returns the workspace UUID.
    pub fn create(&self, working_dir: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO workspaces (id, working_dir, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, working_dir, now],
            )
            .context("Failed to create workspace")?;
            Ok(id)
        })
    }

    /// Find a workspace by its exact working directory.
    pub fn find_by_dir(&self, working_dir: &str) -> Result<Option<WorkspaceRow>> {
        self.db.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT id, working_dir, created_at, updated_at FROM workspaces WHERE working_dir = ?1")?;
            let row = stmt
                .query_row(rusqlite::params![working_dir], |row| {
                    Ok(WorkspaceRow {
                        id: row.get(0)?,
                        working_dir: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                })
                .optional()?;
            Ok(row)
        })
    }

    /// Get a workspace by ID.
    pub fn get(&self, id: &str) -> Result<Option<WorkspaceRow>> {
        self.db.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT id, working_dir, created_at, updated_at FROM workspaces WHERE id = ?1")?;
            let row = stmt
                .query_row(rusqlite::params![id], |row| {
                    Ok(WorkspaceRow {
                        id: row.get(0)?,
                        working_dir: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                })
                .optional()?;
            Ok(row)
        })
    }

    /// List all workspaces.
    pub fn list(&self) -> Result<Vec<WorkspaceRow>> {
        self.db.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT id, working_dir, created_at, updated_at FROM workspaces ORDER BY created_at")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(WorkspaceRow {
                        id: row.get(0)?,
                        working_dir: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Delete a workspace and all its data (cascade).
    pub fn delete(&self, id: &str) -> Result<()> {
        self.db.with_conn(|conn| {
            // Delete vec_chunks entries for this workspace
            conn.execute(
                "DELETE FROM vec_chunks WHERE chunk_id IN (
                    SELECT c.id FROM chunks c
                    JOIN files f ON c.file_id = f.id
                    WHERE f.workspace_id = ?1
                )",
                rusqlite::params![id],
            )?;
            // CASCADE will delete files and chunks
            let deleted = conn.execute("DELETE FROM workspaces WHERE id = ?1", rusqlite::params![id])?;
            if deleted == 0 {
                bail!("Workspace '{id}' not found");
            }
            Ok(())
        })
    }

    /// Touch the updated_at timestamp.
    pub fn touch(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.db.with_conn(|conn| {
            conn.execute(
                "UPDATE workspaces SET updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )?;
            Ok(())
        })
    }

    /// Get node and relation counts for a workspace.
    pub fn counts(&self, id: &str) -> Result<(u64, u64)> {
        self.db.with_conn(|conn| {
            let node_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM chunks c JOIN files f ON c.file_id = f.id WHERE f.workspace_id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )?;
            // Relations: each chunk belongs to a file (1 relation per chunk)
            Ok((node_count as u64, node_count as u64))
        })
    }
}

/// A row from the workspaces table.
#[derive(Debug, Clone)]
pub struct WorkspaceRow {
    pub id: String,
    pub working_dir: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

impl WorkspaceRow {
    pub fn created_at_dt(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.created_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    pub fn updated_at_dt(&self) -> Option<DateTime<Utc>> {
        self.updated_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }
}

/// Extension trait for optional query results.
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn test_db() -> Arc<VecStoreDb> {
        Arc::new(VecStoreDb::open_in_memory(384).unwrap())
    }

    #[test]
    fn test_create_and_find() {
        let db = test_db();
        let mgr = WorkspaceManager::new(&db);
        let id = mgr.create("/tmp/project").unwrap();
        let ws = mgr.find_by_dir("/tmp/project").unwrap().unwrap();
        assert_eq!(ws.id, id);
        assert_eq!(ws.working_dir, "/tmp/project");
    }

    #[test]
    fn test_list_and_delete() {
        let db = test_db();
        let mgr = WorkspaceManager::new(&db);
        let id = mgr.create("/tmp/a").unwrap();
        mgr.create("/tmp/b").unwrap();
        assert_eq!(mgr.list().unwrap().len(), 2);

        mgr.delete(&id).unwrap();
        assert_eq!(mgr.list().unwrap().len(), 1);
    }
}
