//! Vector search engine using sqlite-vec KNN queries.

use anyhow::Result;

use crate::db::VecStoreDb;
use crate::embedding::EmbeddingProvider;

/// Search result from the vector store.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Chunk database ID.
    pub chunk_id: i64,
    /// File path relative to workspace root.
    pub file_path: String,
    /// Chunk content.
    pub content: String,
    /// 1-based start line.
    pub start_line: u32,
    /// 1-based end line.
    pub end_line: u32,
    /// Distance from the query vector (lower is better).
    pub distance: f32,
    /// Relevance score (higher is better), set after reranking.
    pub relevance: Option<f32>,
}

/// Performs vector similarity search over indexed code chunks.
pub struct SearchEngine<'a> {
    db: &'a VecStoreDb,
}

impl<'a> SearchEngine<'a> {
    pub fn new(db: &'a VecStoreDb) -> Self {
        Self { db }
    }

    /// Search for chunks similar to a query within a workspace.
    ///
    /// # Arguments
    /// * `workspace_id` - Workspace to search in
    /// * `query` - Natural language query
    /// * `embedding` - Provider to embed the query
    /// * `limit` - Max results to return
    /// * `top_k` - Top-k candidates from vector search
    /// * `starts_with` - Optional file path prefix filter
    /// * `ends_with` - Optional file extension filters
    pub async fn search(
        &self,
        workspace_id: &str,
        query: &str,
        embedding: &dyn EmbeddingProvider,
        limit: Option<usize>,
        top_k: Option<u32>,
        starts_with: Option<&str>,
        ends_with: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        let limit = limit.unwrap_or(20);
        let top_k = top_k.unwrap_or(100);

        // Embed the query
        let query_vecs = embedding.embed(&[query]).await?;
        let query_vec = &query_vecs[0];
        let query_bytes = float_vec_to_bytes(query_vec);

        // KNN search via sqlite-vec, joined with chunk and file metadata
        self.db.with_conn(|conn| {
            let mut results = Vec::new();

            // We need to join vec_chunks with chunks and files to filter by workspace
            let sql = format!(
                "SELECT v.chunk_id, v.distance, c.content, c.start_line, c.end_line, f.path
                 FROM vec_chunks v
                 JOIN chunks c ON c.id = v.chunk_id
                 JOIN files f ON f.id = c.file_id
                 WHERE f.workspace_id = ?1
                   AND v.embedding MATCH ?2
                   AND k = ?3
                 ORDER BY v.distance ASC",
            );

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params![workspace_id, query_bytes, top_k],
                |row| {
                    Ok(SearchResult {
                        chunk_id: row.get(0)?,
                        distance: row.get(1)?,
                        content: row.get(2)?,
                        start_line: row.get(3)?,
                        end_line: row.get(4)?,
                        file_path: row.get(5)?,
                        relevance: None,
                    })
                },
            )?;

            for row in rows {
                let result = row?;

                // Apply path filters
                if let Some(prefix) = starts_with {
                    if !result.file_path.starts_with(prefix) {
                        continue;
                    }
                }
                if let Some(suffixes) = ends_with {
                    if !suffixes.is_empty()
                        && !suffixes.iter().any(|s| result.file_path.ends_with(s.as_str()))
                    {
                        continue;
                    }
                }

                results.push(result);
                if results.len() >= limit {
                    break;
                }
            }

            // Compute relevance as inverse of distance (normalized to 0-1 range)
            if let Some(max_dist) = results.iter().map(|r| r.distance).reduce(f32::max) {
                if max_dist > 0.0 {
                    for r in &mut results {
                        r.relevance = Some(1.0 - (r.distance / max_dist));
                    }
                }
            }

            Ok(results)
        })
    }

    /// Rerank results using a second embedding (use_case / relevance_query).
    pub async fn rerank(
        &self,
        results: &mut [SearchResult],
        use_case: &str,
        embedding: &dyn EmbeddingProvider,
    ) -> Result<()> {
        if results.is_empty() || use_case.is_empty() {
            return Ok(());
        }

        let use_case_vecs = embedding.embed(&[use_case]).await?;
        let use_case_vec = &use_case_vecs[0];

        // Compute cosine similarity between each chunk and use_case
        let chunk_texts: Vec<&str> = results.iter().map(|r| r.content.as_str()).collect();
        let chunk_vecs = embedding.embed(&chunk_texts).await?;

        for (result, chunk_vec) in results.iter_mut().zip(chunk_vecs.iter()) {
            result.relevance = Some(cosine_similarity(use_case_vec, chunk_vec));
        }

        // Sort by relevance descending
        results.sort_by(|a, b| {
            b.relevance
                .unwrap_or(0.0)
                .partial_cmp(&a.relevance.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

fn float_vec_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
