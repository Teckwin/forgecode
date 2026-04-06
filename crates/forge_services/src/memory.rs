use std::path::{Path, PathBuf};
use std::sync::Arc;

use forge_app::{DirectoryReaderInfra, EnvironmentInfra, FileReaderInfra, FileWriterInfra};

/// Auto-memory service that manages cross-session AI notes.
///
/// Memory files live in `<project>/.forge/memory/` and `~/.forge/memory/`.
/// `MEMORY.md` is the entry-point file always loaded at session start.
/// Additional topic files (e.g. `debugging.md`, `patterns.md`) are discovered
/// automatically.
///
/// The agent writes to memory as it works; contents persist across
/// conversations. Organisation is semantic (by topic), not chronological.
#[derive(Clone)]
pub struct ForgeMemoryService<F> {
    infra: Arc<F>,
}

impl<F: EnvironmentInfra + FileReaderInfra + FileWriterInfra + DirectoryReaderInfra>
    ForgeMemoryService<F>
{
    pub fn new(infra: Arc<F>) -> Self {
        Self { infra }
    }

    /// Load all memory files, returning their contents.
    ///
    /// Loading order: global memory first, then project memory. Within each
    /// directory, `MEMORY.md` is loaded first (if it exists), followed by
    /// remaining `*.md` files in alphabetical order.
    pub async fn load_memories(&self) -> Vec<MemoryEntry> {
        let env = self.infra.get_environment();
        let mut entries = Vec::new();

        // Global memory
        let global_dir = env.memory_global_path();
        entries.extend(self.load_dir(&global_dir, "global").await);

        // Project memory
        let project_dir = env.memory_project_path();
        entries.extend(self.load_dir(&project_dir, "project").await);

        entries
    }

    /// Write or update a memory file in the project memory directory.
    ///
    /// `filename` must be a simple filename (e.g. `"MEMORY.md"`, `"patterns.md"`).
    /// Path separators and `..` components are rejected to prevent path traversal.
    pub async fn write_memory(&self, filename: &str, content: &str) -> anyhow::Result<PathBuf> {
        // Validate filename — reject path traversal attempts
        if filename.contains('/')
            || filename.contains('\\')
            || filename.contains("..")
            || filename.is_empty()
        {
            anyhow::bail!(
                "Invalid memory filename: must be a simple filename without path separators or '..'"
            );
        }

        let env = self.infra.get_environment();
        let dir = env.memory_project_path();
        let path = dir.join(filename);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        self.infra
            .write(&path, bytes::Bytes::from(content.to_owned()))
            .await?;

        Ok(path)
    }

    async fn load_dir(&self, dir: &Path, scope: &str) -> Vec<MemoryEntry> {
        let mut entries = Vec::new();

        // Try MEMORY.md first (always loaded)
        let memory_md = dir.join("MEMORY.md");
        if let Ok(content) = self.infra.read_utf8(&memory_md).await
            && !content.trim().is_empty()
        {
            entries.push(MemoryEntry {
                scope: scope.to_string(),
                filename: "MEMORY.md".to_string(),
                content,
            });
        }

        // Load remaining *.md files
        match self.infra.read_directory_files(dir, Some("*.md")).await {
            Ok(files) => {
                for (path, content) in files {
                    let filename = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    // Skip MEMORY.md (already loaded above)
                    if filename == "MEMORY.md" {
                        continue;
                    }

                    if !content.trim().is_empty() {
                        entries.push(MemoryEntry { scope: scope.to_string(), filename, content });
                    }
                }
            }
            Err(e) => {
                // Log at debug — directory not existing is expected for fresh projects
                tracing::debug!(dir = %dir.display(), error = %e, "Memory directory not readable (may not exist)");
            }
        }

        entries
    }
}

/// A single memory entry loaded from disk.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    /// `"global"` or `"project"`.
    pub scope: String,
    /// Filename (e.g. `"MEMORY.md"`, `"patterns.md"`).
    pub filename: String,
    /// File content.
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use bytes::Bytes;
    use fake::{Fake, Faker};
    use forge_app::{DirectoryReaderInfra, EnvironmentInfra, FileReaderInfra, FileWriterInfra};
    use forge_domain::{ConfigOperation, Environment};
    use tempfile::TempDir;

    /// Mock infra backed by real temp directories on the filesystem.
    struct MockMemoryInfra {
        /// Stands in for `~/.forge` (base_path).
        _global_tmp: TempDir,
        /// Stands in for the project `cwd`.
        _project_tmp: TempDir,
        env: Environment,
    }

    impl MockMemoryInfra {
        fn new() -> Self {
            let global_tmp = TempDir::new().unwrap();
            let project_tmp = TempDir::new().unwrap();

            let mut env: Environment = Faker.fake();
            env.base_path = global_tmp.path().to_path_buf();
            env.cwd = project_tmp.path().to_path_buf();

            Self { _global_tmp: global_tmp, _project_tmp: project_tmp, env }
        }

        fn global_memory_dir(&self) -> PathBuf {
            self.env.memory_global_path()
        }

        fn project_memory_dir(&self) -> PathBuf {
            self.env.memory_project_path()
        }
    }

    impl EnvironmentInfra for MockMemoryInfra {
        fn get_environment(&self) -> Environment {
            self.env.clone()
        }

        fn get_env_var(&self, _key: &str) -> Option<String> {
            None
        }

        fn get_env_vars(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }

        async fn update_environment(&self, _ops: Vec<ConfigOperation>) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl FileReaderInfra for MockMemoryInfra {
        async fn read_utf8(&self, path: &Path) -> anyhow::Result<String> {
            Ok(tokio::fs::read_to_string(path).await?)
        }

        fn read_batch_utf8(
            &self,
            _batch_size: usize,
            _paths: Vec<PathBuf>,
        ) -> impl futures::Stream<Item = (PathBuf, anyhow::Result<String>)> + Send {
            futures::stream::empty()
        }

        async fn read(&self, _path: &Path) -> anyhow::Result<Vec<u8>> {
            unimplemented!()
        }

        async fn range_read_utf8(
            &self,
            _path: &Path,
            _start_line: u64,
            _end_line: u64,
        ) -> anyhow::Result<(String, forge_domain::FileInfo)> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl FileWriterInfra for MockMemoryInfra {
        async fn write(&self, path: &Path, contents: Bytes) -> anyhow::Result<()> {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(path, &contents).await?;
            Ok(())
        }

        async fn write_temp(
            &self,
            _prefix: &str,
            _ext: &str,
            _content: &str,
        ) -> anyhow::Result<PathBuf> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl DirectoryReaderInfra for MockMemoryInfra {
        async fn list_directory_entries(
            &self,
            _directory: &Path,
        ) -> anyhow::Result<Vec<(PathBuf, bool)>> {
            unimplemented!()
        }

        async fn read_directory_files(
            &self,
            directory: &Path,
            pattern: Option<&str>,
        ) -> anyhow::Result<Vec<(PathBuf, String)>> {
            let mut results = Vec::new();
            let mut entries = tokio::fs::read_dir(directory).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(pat) = pattern {
                    // Simple *.md matching
                    let ext = pat.trim_start_matches("*.");
                    match path.extension() {
                        Some(e) if e == ext => {}
                        _ => continue,
                    }
                }
                let content = tokio::fs::read_to_string(&path).await?;
                results.push((path, content));
            }
            results.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(results)
        }
    }

    /// Helper: create a file inside the given directory, creating parents as needed.
    async fn create_file(dir: &Path, name: &str, content: &str) {
        tokio::fs::create_dir_all(dir).await.unwrap();
        tokio::fs::write(dir.join(name), content).await.unwrap();
    }

    fn service(infra: MockMemoryInfra) -> ForgeMemoryService<MockMemoryInfra> {
        ForgeMemoryService::new(Arc::new(infra))
    }

    // ---------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_load_memories_empty() {
        let mock = MockMemoryInfra::new();
        let svc = service(mock);

        let entries = svc.load_memories().await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_load_memories_global_only() {
        let mock = MockMemoryInfra::new();
        create_file(&mock.global_memory_dir(), "MEMORY.md", "global notes").await;

        let svc = service(mock);
        let entries = svc.load_memories().await;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].scope, "global");
        assert_eq!(entries[0].filename, "MEMORY.md");
        assert_eq!(entries[0].content, "global notes");
    }

    #[tokio::test]
    async fn test_load_memories_project_only() {
        let mock = MockMemoryInfra::new();
        create_file(&mock.project_memory_dir(), "MEMORY.md", "project notes").await;

        let svc = service(mock);
        let entries = svc.load_memories().await;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].scope, "project");
        assert_eq!(entries[0].filename, "MEMORY.md");
        assert_eq!(entries[0].content, "project notes");
    }

    #[tokio::test]
    async fn test_load_memories_both_scopes() {
        let mock = MockMemoryInfra::new();
        create_file(&mock.global_memory_dir(), "MEMORY.md", "global").await;
        create_file(&mock.project_memory_dir(), "MEMORY.md", "project").await;

        let svc = service(mock);
        let entries = svc.load_memories().await;

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].scope, "global");
        assert_eq!(entries[1].scope, "project");
    }

    #[tokio::test]
    async fn test_load_memories_memory_md_first() {
        let mock = MockMemoryInfra::new();
        let dir = mock.global_memory_dir();
        // Create alphabetically-earlier file first
        create_file(&dir, "aaa.md", "aaa content").await;
        create_file(&dir, "MEMORY.md", "main memory").await;
        create_file(&dir, "zzz.md", "zzz content").await;

        let svc = service(mock);
        let entries = svc.load_memories().await;

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].filename, "MEMORY.md");
        // Remaining files should be alphabetical
        assert_eq!(entries[1].filename, "aaa.md");
        assert_eq!(entries[2].filename, "zzz.md");
    }

    #[tokio::test]
    async fn test_load_memories_skips_empty_files() {
        let mock = MockMemoryInfra::new();
        let dir = mock.global_memory_dir();
        create_file(&dir, "MEMORY.md", "has content").await;
        create_file(&dir, "empty.md", "").await;
        create_file(&dir, "whitespace.md", "   \n  ").await;

        let svc = service(mock);
        let entries = svc.load_memories().await;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].filename, "MEMORY.md");
    }

    #[tokio::test]
    async fn test_load_memories_skips_memory_md_duplicate() {
        let mock = MockMemoryInfra::new();
        let dir = mock.project_memory_dir();
        create_file(&dir, "MEMORY.md", "main").await;
        create_file(&dir, "extra.md", "extra").await;

        let svc = service(mock);
        let entries = svc.load_memories().await;

        // MEMORY.md should appear exactly once, not twice
        let memory_count = entries.iter().filter(|e| e.filename == "MEMORY.md").count();
        assert_eq!(memory_count, 1);
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_write_memory_creates_file() {
        let mock = MockMemoryInfra::new();
        let expected_dir = mock.project_memory_dir();
        let svc = service(mock);

        let path = svc.write_memory("notes.md", "my notes").await.unwrap();

        assert_eq!(path, expected_dir.join("notes.md"));
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "my notes");
    }

    #[tokio::test]
    async fn test_write_memory_creates_parent_dirs() {
        let mock = MockMemoryInfra::new();
        let expected_dir = mock.project_memory_dir();
        let svc = service(mock);

        // The project memory directory does not exist yet
        assert!(!expected_dir.exists());

        let path = svc.write_memory("MEMORY.md", "hello").await.unwrap();

        assert!(path.exists());
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hello");
    }

    #[tokio::test]
    async fn test_write_memory_overwrites_existing() {
        let mock = MockMemoryInfra::new();
        let dir = mock.project_memory_dir();
        create_file(&dir, "notes.md", "old content").await;

        let svc = service(mock);
        let path = svc.write_memory("notes.md", "new content").await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_write_memory_rejects_path_traversal() {
        let mock = MockMemoryInfra::new();
        let svc = service(mock);

        let result = svc.write_memory("../escape.md", "bad").await;
        assert!(
            result.is_err(),
            "Path traversal with '..' should be rejected"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid memory filename"),
            "Error should mention invalid filename, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_write_memory_rejects_slashes() {
        let mock = MockMemoryInfra::new();
        let svc = service(mock);

        let result = svc.write_memory("sub/file.md", "bad").await;
        assert!(result.is_err(), "Path with '/' should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid memory filename"),
            "Error should mention invalid filename, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_write_memory_rejects_empty() {
        let mock = MockMemoryInfra::new();
        let svc = service(mock);

        let result = svc.write_memory("", "bad").await;
        assert!(result.is_err(), "Empty filename should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid memory filename"),
            "Error should mention invalid filename, got: {err_msg}"
        );
    }
}
