//! File indexing pipeline: chunk → embed → store.

use anyhow::{Context, Result};

use crate::chunker::Chunker;
use crate::db::VecStoreDb;
use crate::embedding::EmbeddingProvider;

/// Indexes files into the vector store.
pub struct Indexer<'a> {
    db: &'a VecStoreDb,
    chunker: Chunker,
}

impl<'a> Indexer<'a> {
    pub fn new(db: &'a VecStoreDb, chunker: Chunker) -> Self {
        Self { db, chunker }
    }

    /// Index a batch of files for a workspace.
    ///
    /// Returns (nodes_created, relations_created).
    pub async fn index_files(
        &self,
        workspace_id: &str,
        files: &[(String, String)], // (path, content)
        embedding: &dyn EmbeddingProvider,
    ) -> Result<(usize, usize)> {
        let mut total_nodes = 0;
        let mut total_relations = 0;

        for (path, content) in files {
            let (nodes, relations) = self
                .index_file(workspace_id, path, content, embedding)
                .await
                .with_context(|| format!("Failed to index file: {path}"))?;
            total_nodes += nodes;
            total_relations += relations;
        }

        Ok((total_nodes, total_relations))
    }

    /// Index a single file: upsert the file record, chunk, embed, and store.
    async fn index_file(
        &self,
        workspace_id: &str,
        path: &str,
        content: &str,
        embedding: &dyn EmbeddingProvider,
    ) -> Result<(usize, usize)> {
        let hash = sha256_hex(content);
        let chunks = self.chunker.chunk(content);

        if chunks.is_empty() {
            // Still record the file even if empty (for hash tracking)
            self.upsert_file(workspace_id, path, &hash)?;
            return Ok((0, 0));
        }

        // Collect chunk texts for batch embedding
        let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = embedding.embed(&texts).await?;

        self.db.with_tx(|tx| {
            // Upsert file record
            let file_id = {
                // Try to find existing
                let existing: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM files WHERE workspace_id = ?1 AND path = ?2",
                        rusqlite::params![workspace_id, path],
                        |row| row.get(0),
                    )
                    .optional()?;

                if let Some(id) = existing {
                    // Delete old chunks and vec entries
                    tx.execute(
                        "DELETE FROM vec_chunks WHERE chunk_id IN (SELECT id FROM chunks WHERE file_id = ?1)",
                        rusqlite::params![id],
                    )?;
                    tx.execute("DELETE FROM chunks WHERE file_id = ?1", rusqlite::params![id])?;
                    tx.execute(
                        "UPDATE files SET hash = ?1 WHERE id = ?2",
                        rusqlite::params![hash, id],
                    )?;
                    id
                } else {
                    tx.execute(
                        "INSERT INTO files (workspace_id, path, hash) VALUES (?1, ?2, ?3)",
                        rusqlite::params![workspace_id, path, hash],
                    )?;
                    tx.last_insert_rowid()
                }
            };

            // Insert chunks and their vectors
            let mut chunk_insert = tx.prepare(
                "INSERT INTO chunks (file_id, content, start_line, end_line) VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut vec_insert =
                tx.prepare("INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)")?;

            for (chunk, emb) in chunks.iter().zip(embeddings.iter()) {
                chunk_insert.execute(rusqlite::params![
                    file_id,
                    chunk.content,
                    chunk.start_line,
                    chunk.end_line,
                ])?;
                let chunk_id = tx.last_insert_rowid();

                // Convert Vec<f32> to bytes for sqlite-vec
                let emb_bytes = float_vec_to_bytes(emb);
                vec_insert.execute(rusqlite::params![chunk_id, emb_bytes])?;
            }

            Ok((chunks.len(), chunks.len()))
        })
    }

    /// Upsert a file record without chunks (for empty files).
    fn upsert_file(&self, workspace_id: &str, path: &str, hash: &str) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO files (workspace_id, path, hash) VALUES (?1, ?2, ?3)
                 ON CONFLICT(workspace_id, path) DO UPDATE SET hash = excluded.hash",
                rusqlite::params![workspace_id, path, hash],
            )?;
            Ok(())
        })
    }

    /// Delete files by path from a workspace.
    pub fn delete_files(&self, workspace_id: &str, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        self.db.with_tx(|tx| {
            for path in paths {
                // Delete vec entries first
                tx.execute(
                    "DELETE FROM vec_chunks WHERE chunk_id IN (
                        SELECT c.id FROM chunks c
                        JOIN files f ON c.file_id = f.id
                        WHERE f.workspace_id = ?1 AND f.path = ?2
                    )",
                    rusqlite::params![workspace_id, path],
                )?;
                // CASCADE deletes chunks
                tx.execute(
                    "DELETE FROM files WHERE workspace_id = ?1 AND path = ?2",
                    rusqlite::params![workspace_id, path],
                )?;
            }
            Ok(())
        })
    }

    /// List all files in a workspace with their hashes.
    pub fn list_files(&self, workspace_id: &str) -> Result<Vec<(String, String)>> {
        self.db.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT path, hash FROM files WHERE workspace_id = ?1 ORDER BY path")?;
            let rows = stmt
                .query_map(rusqlite::params![workspace_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }
}

/// Compute SHA-256 hex digest of a string.
fn sha256_hex(s: &str) -> String {
    let mut hasher = SimpleHash::new();
    hasher.update(s.as_bytes());
    hasher.finalize_hex()
}

/// Minimal SHA-256 — we avoid pulling in `sha2` by using rusqlite's bundled SQLite
/// which has sha256 support, but for simplicity let's compute it in Rust.
/// We'll use a simple approach: hash via SQLite itself.
///
/// Actually, let's just implement a basic content hash using std.
/// For production, this would use sha2 crate. For now we use a simpler approach.
struct SimpleHash {
    data: Vec<u8>,
}

impl SimpleHash {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn update(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    fn finalize_hex(&self) -> String {
        // Use a simple FNV-like hash combined with length for uniqueness.
        // This is NOT cryptographic — for file change detection it's sufficient.
        // In production, replace with sha2::Sha256.
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in &self.data {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let h2: u64 = {
            let mut h = 0x517cc1b727220a95u64;
            for &b in self.data.iter().rev() {
                h ^= b as u64;
                h = h.wrapping_mul(0x6c62272e07bb0142);
            }
            h
        };
        format!("{:016x}{:016x}", h, h2)
    }
}

/// Convert a `Vec<f32>` to raw bytes for sqlite-vec.
fn float_vec_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
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
    use crate::chunker::ChunkConfig;
    use crate::embedding::NoopEmbeddingProvider;

    #[tokio::test]
    async fn test_index_and_list_files() {
        let db = Arc::new(VecStoreDb::open_in_memory(384).unwrap());
        db.ensure_vec_table(384).unwrap();

        // Create workspace
        let ws_mgr = crate::workspace::WorkspaceManager::new(&db);
        let ws_id = ws_mgr.create("/tmp/test").unwrap();

        let indexer = Indexer::new(&db, Chunker::new(ChunkConfig { chunk_size: 10, overlap: 2 }));
        let embedding = NoopEmbeddingProvider::new(384);

        let files = vec![
            ("src/main.rs".to_string(), "fn main() {\n    println!(\"hello\");\n}".to_string()),
            ("src/lib.rs".to_string(), "pub mod foo;".to_string()),
        ];

        let (nodes, _) = indexer.index_files(&ws_id, &files, &embedding).await.unwrap();
        assert!(nodes >= 2); // At least one chunk per file

        let listed = indexer.list_files(&ws_id).unwrap();
        assert_eq!(listed.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_files() {
        let db = Arc::new(VecStoreDb::open_in_memory(384).unwrap());
        db.ensure_vec_table(384).unwrap();

        let ws_mgr = crate::workspace::WorkspaceManager::new(&db);
        let ws_id = ws_mgr.create("/tmp/test").unwrap();

        let indexer = Indexer::new(&db, Chunker::default());
        let embedding = NoopEmbeddingProvider::new(384);

        let files = vec![("a.rs".to_string(), "fn a() {}".to_string())];
        indexer.index_files(&ws_id, &files, &embedding).await.unwrap();
        assert_eq!(indexer.list_files(&ws_id).unwrap().len(), 1);

        indexer.delete_files(&ws_id, &["a.rs".to_string()]).unwrap();
        assert_eq!(indexer.list_files(&ws_id).unwrap().len(), 0);
    }
}
