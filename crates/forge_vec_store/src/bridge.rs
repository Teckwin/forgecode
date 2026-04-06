//! Bridge layer: implement forge_domain repository traits for LocalVecStore.

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use forge_domain::{
    ApiKey, CodeSearchQuery, FileDeletion, FileHash, FileUpload, FileUploadInfo,
    FuzzySearchRepository, Node, NodeData, NodeId, SearchMatch, SyntaxError,
    ValidationRepository, WorkspaceAuth, WorkspaceFiles, WorkspaceId, WorkspaceIndexRepository,
    WorkspaceInfo,
};

use crate::chunker::Chunker;
use crate::indexer::Indexer;
use crate::search::SearchEngine;
use crate::workspace::WorkspaceManager;
use crate::LocalVecStore;

// ──────────────────── WorkspaceIndexRepository ────────────────────

#[async_trait]
impl WorkspaceIndexRepository for LocalVecStore {
    async fn authenticate(&self) -> Result<WorkspaceAuth> {
        // Local mode: generate a local-only auth token
        Ok(WorkspaceAuth::new(
            forge_domain::UserId::generate(),
            ApiKey::from("local-vec-store".to_string()),
        ))
    }

    async fn create_workspace(
        &self,
        working_dir: &Path,
        _auth_token: &ApiKey,
    ) -> Result<WorkspaceId> {
        let mgr = WorkspaceManager::new(&self.db);
        let dir_str = working_dir.to_string_lossy().to_string();
        let id = mgr.create(&dir_str)?;
        WorkspaceId::from_string(&id)
    }

    async fn upload_files(
        &self,
        upload: &FileUpload,
        _auth_token: &ApiKey,
    ) -> Result<FileUploadInfo> {
        let indexer = Indexer::new(&self.db, Chunker::default());
        let files: Vec<(String, String)> = upload
            .data
            .iter()
            .map(|f| (f.path.clone(), f.content.clone()))
            .collect();

        let ws_id = upload.workspace_id.to_string();
        let (nodes, relations) = indexer
            .index_files(&ws_id, &files, self.embedding.active())
            .await?;

        // Update workspace timestamp
        let mgr = WorkspaceManager::new(&self.db);
        mgr.touch(&ws_id)?;

        Ok(FileUploadInfo::new(nodes, relations))
    }

    async fn search(
        &self,
        query: &CodeSearchQuery<'_>,
        _auth_token: &ApiKey,
    ) -> Result<Vec<Node>> {
        let engine = SearchEngine::new(&self.db);
        let ws_id = query.workspace_id.to_string();

        let ends_with_ref = query.data.ends_with.as_deref();
        let mut results = engine
            .search(
                &ws_id,
                query.data.query,
                self.embedding.active(),
                query.data.limit,
                query.data.top_k,
                query.data.starts_with.as_deref(),
                ends_with_ref,
            )
            .await?;

        // Rerank by use_case if provided
        if !query.data.use_case.is_empty() {
            engine
                .rerank(&mut results, &query.data.use_case, self.embedding.active())
                .await?;
        }

        // Convert to domain Node type
        let nodes = results
            .into_iter()
            .map(|r| Node {
                node_id: NodeId::from(r.chunk_id.to_string()),
                node: NodeData::FileChunk(forge_domain::FileChunk {
                    file_path: r.file_path,
                    content: r.content,
                    start_line: r.start_line,
                    end_line: r.end_line,
                }),
                relevance: r.relevance,
                distance: Some(r.distance),
            })
            .collect();

        Ok(nodes)
    }

    async fn list_workspaces(&self, _auth_token: &ApiKey) -> Result<Vec<WorkspaceInfo>> {
        let mgr = WorkspaceManager::new(&self.db);
        let rows = mgr.list()?;

        rows.into_iter()
            .map(|row| {
                let workspace_id = WorkspaceId::from_string(&row.id)?;
                let (node_count, relation_count) = mgr.counts(&row.id).unwrap_or((0, 0));
                let last_updated = row.updated_at_dt();
                let created_at = row.created_at_dt().unwrap_or_else(chrono::Utc::now);
                Ok(WorkspaceInfo {
                    workspace_id,
                    working_dir: row.working_dir,
                    node_count: Some(node_count),
                    relation_count: Some(relation_count),
                    last_updated,
                    created_at,
                })
            })
            .collect()
    }

    async fn get_workspace(
        &self,
        workspace_id: &WorkspaceId,
        _auth_token: &ApiKey,
    ) -> Result<Option<WorkspaceInfo>> {
        let mgr = WorkspaceManager::new(&self.db);
        let row = mgr.get(&workspace_id.to_string())?;

        match row {
            Some(row) => {
                let (node_count, relation_count) = mgr.counts(&row.id).unwrap_or((0, 0));
                let last_updated = row.updated_at_dt();
                let created_at = row.created_at_dt().unwrap_or_else(chrono::Utc::now);
                Ok(Some(WorkspaceInfo {
                    workspace_id: workspace_id.clone(),
                    working_dir: row.working_dir,
                    node_count: Some(node_count),
                    relation_count: Some(relation_count),
                    last_updated,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_workspace_files(
        &self,
        workspace: &WorkspaceFiles,
        _auth_token: &ApiKey,
    ) -> Result<Vec<FileHash>> {
        let indexer = Indexer::new(&self.db, Chunker::default());
        let ws_id = workspace.workspace_id.to_string();
        let files = indexer.list_files(&ws_id)?;

        Ok(files
            .into_iter()
            .map(|(path, hash)| FileHash { path, hash })
            .collect())
    }

    async fn delete_files(&self, deletion: &FileDeletion, _auth_token: &ApiKey) -> Result<()> {
        let indexer = Indexer::new(&self.db, Chunker::default());
        let ws_id = deletion.workspace_id.to_string();
        indexer.delete_files(&ws_id, &deletion.data)?;
        Ok(())
    }

    async fn delete_workspace(
        &self,
        workspace_id: &WorkspaceId,
        _auth_token: &ApiKey,
    ) -> Result<()> {
        let mgr = WorkspaceManager::new(&self.db);
        mgr.delete(&workspace_id.to_string())
    }
}

// ──────────────────── FuzzySearchRepository ────────────────────

#[async_trait]
impl FuzzySearchRepository for LocalVecStore {
    async fn fuzzy_search(
        &self,
        needle: &str,
        haystack: &str,
        search_all: bool,
    ) -> Result<Vec<SearchMatch>> {
        let results = crate::fuzzy::fuzzy_search(needle, haystack, search_all);
        Ok(results
            .into_iter()
            .map(|(start, end)| SearchMatch { start_line: start, end_line: end })
            .collect())
    }
}

// ──────────────────── ValidationRepository ────────────────────

#[async_trait]
impl ValidationRepository for LocalVecStore {
    async fn validate_file(
        &self,
        path: impl AsRef<Path> + Send,
        content: &str,
    ) -> Result<Vec<SyntaxError>> {
        let errors = crate::validation::validate_file(path, content);
        Ok(errors
            .into_iter()
            .map(|e| SyntaxError { line: e.line, column: e.column, message: e.message })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::db::VecStoreDb;
    use crate::embedding::{EmbeddingRegistry, NoopEmbeddingProvider};

    fn test_store() -> LocalVecStore {
        let db = Arc::new(VecStoreDb::open_in_memory(384).unwrap());
        db.ensure_vec_table(384).unwrap();
        let embedding = Arc::new(EmbeddingRegistry::new(
            "noop",
            Arc::new(NoopEmbeddingProvider::new(384)),
        ));
        LocalVecStore::new(db, embedding)
    }

    #[tokio::test]
    async fn test_workspace_lifecycle() {
        let store = test_store();
        let token = ApiKey::from("test".to_string());

        // Authenticate
        let auth = store.authenticate().await.unwrap();
        assert!(!auth.token.is_empty());

        // Create workspace
        let ws_id = store
            .create_workspace(Path::new("/tmp/test-project"), &token)
            .await
            .unwrap();

        // List workspaces
        let workspaces = store.list_workspaces(&token).await.unwrap();
        assert_eq!(workspaces.len(), 1);

        // Get workspace
        let info = store.get_workspace(&ws_id, &token).await.unwrap();
        assert!(info.is_some());

        // Delete workspace
        store.delete_workspace(&ws_id, &token).await.unwrap();
        let workspaces = store.list_workspaces(&token).await.unwrap();
        assert!(workspaces.is_empty());
    }

    #[tokio::test]
    async fn test_fuzzy_search_bridge() {
        let store = test_store();
        let results = store
            .fuzzy_search("main", "fn main() {\n    println!(\"hello\");\n}", true)
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_validation_bridge() {
        let store = test_store();
        let errors = ValidationRepository::validate_file(
            &store,
            "test.rs",
            "fn main() {}",
        )
        .await
        .unwrap();
        assert!(errors.is_empty());
    }
}
