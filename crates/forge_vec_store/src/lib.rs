//! # forge_vec_store
//!
//! Local vector store for code search, powered by SQLite + sqlite-vec.
//!
//! Provides local implementations of workspace indexing, semantic search,
//! fuzzy search, and syntax validation without requiring any cloud services.

mod chunker;
mod db;
mod embedding;
mod fuzzy;
mod indexer;
mod search;
mod validation;
mod workspace;

pub mod bridge;

// Public API
pub use chunker::{ChunkConfig, Chunker};
pub use db::VecStoreDb;
pub use embedding::{EmbeddingProvider, EmbeddingRegistry, NoopEmbeddingProvider};
pub use search::SearchEngine;
pub use workspace::WorkspaceManager;

use std::sync::Arc;

/// Main entry point: a fully local vector store backed by SQLite.
///
/// Combines workspace management, file indexing, vector search,
/// fuzzy search, and syntax validation in a single struct.
pub struct LocalVecStore {
    db: Arc<VecStoreDb>,
    embedding: Arc<EmbeddingRegistry>,
}

impl LocalVecStore {
    /// Create a new local vector store at the given database path.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file (created if not exists)
    /// * `embedding` - Embedding registry with at least one provider registered
    pub fn new(db: Arc<VecStoreDb>, embedding: Arc<EmbeddingRegistry>) -> Self {
        Self { db, embedding }
    }

    /// Access the underlying database.
    pub fn db(&self) -> &VecStoreDb {
        &self.db
    }

    /// Access the embedding registry.
    pub fn embedding(&self) -> &EmbeddingRegistry {
        &self.embedding
    }
}
