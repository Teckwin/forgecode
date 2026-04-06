//! Pluggable embedding provider interface and registry.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, bail};

/// Trait for embedding providers that convert text to vectors.
///
/// Implement this trait to plug in your own embedding service
/// (local ONNX model, remote API, etc.).
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// The dimensionality of the output vectors.
    fn dimensions(&self) -> usize;

    /// Embed a batch of texts, returning one vector per input.
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

/// Registry that holds named embedding providers and tracks the active one.
pub struct EmbeddingRegistry {
    providers: HashMap<String, Arc<dyn EmbeddingProvider>>,
    active: String,
}

impl EmbeddingRegistry {
    /// Create a new registry with a single provider set as active.
    pub fn new(name: impl Into<String>, provider: Arc<dyn EmbeddingProvider>) -> Self {
        let name = name.into();
        let mut providers = HashMap::new();
        providers.insert(name.clone(), provider);
        Self { providers, active: name }
    }

    /// Register an additional provider.
    pub fn register(&mut self, name: impl Into<String>, provider: Arc<dyn EmbeddingProvider>) {
        self.providers.insert(name.into(), provider);
    }

    /// Switch the active provider by name.
    pub fn set_active(&mut self, name: &str) -> Result<()> {
        if !self.providers.contains_key(name) {
            bail!("Embedding provider '{name}' not registered");
        }
        self.active = name.to_string();
        Ok(())
    }

    /// Get the currently active provider.
    pub fn active(&self) -> &dyn EmbeddingProvider {
        self.providers[&self.active].as_ref()
    }

    /// Get the active provider as an Arc (for cloning).
    pub fn active_arc(&self) -> Arc<dyn EmbeddingProvider> {
        Arc::clone(&self.providers[&self.active])
    }

    /// Dimensions of the active provider.
    pub fn dimensions(&self) -> usize {
        self.active().dimensions()
    }
}

/// A no-op embedding provider that returns zero vectors.
/// Useful for testing or when embedding is not yet configured.
pub struct NoopEmbeddingProvider {
    dims: usize,
}

impl NoopEmbeddingProvider {
    pub fn new(dims: usize) -> Self {
        Self { dims }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for NoopEmbeddingProvider {
    fn dimensions(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.0f32; self.dims]).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_provider() {
        let provider = NoopEmbeddingProvider::new(128);
        assert_eq!(provider.dimensions(), 128);

        let vecs = provider.embed(&["hello", "world"]).await.unwrap();
        assert_eq!(vecs.len(), 2);
        assert_eq!(vecs[0].len(), 128);
        assert!(vecs[0].iter().all(|&v| v == 0.0));
    }

    #[tokio::test]
    async fn test_registry() {
        let provider = Arc::new(NoopEmbeddingProvider::new(384));
        let registry = EmbeddingRegistry::new("noop", provider);

        assert_eq!(registry.dimensions(), 384);
        assert_eq!(registry.active().dimensions(), 384);
    }
}
