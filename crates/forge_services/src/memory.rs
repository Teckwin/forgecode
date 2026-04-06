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
    pub async fn write_memory(&self, filename: &str, content: &str) -> anyhow::Result<PathBuf> {
        let env = self.infra.get_environment();
        let dir = env.memory_project_path();

        // Ensure directory exists
        let path = dir.join(filename);
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
            Err(_) => {
                // Directory doesn't exist — that's fine.
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
