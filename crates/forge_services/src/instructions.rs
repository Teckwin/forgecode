use std::path::PathBuf;
use std::sync::Arc;

use forge_app::{
    CommandInfra, CustomInstructionsService, DirectoryReaderInfra, EnvironmentInfra,
    FileReaderInfra,
};

/// Discovers and loads custom instructions from multiple sources:
///
/// 1. AGENTS.md files (legacy, kept for backward compatibility)
///    - `~/.forge/AGENTS.md`
///    - Git root `AGENTS.md`
///    - `<cwd>/AGENTS.md`
///
/// 2. FORGE.md files (new, aligned with Claude's CLAUDE.md)
///    - `~/.forge/FORGE.md`
///    - `<cwd>/.forge/FORGE.md`
///    - `<cwd>/FORGE.md`
///
/// 3. Rules directories (*.md files, aligned with Claude's .claude/rules/)
///    - `~/.forge/rules/*.md`
///    - `<cwd>/.forge/rules/*.md`
#[derive(Clone)]
pub struct ForgeCustomInstructionsService<F> {
    infra: Arc<F>,
    cache: tokio::sync::OnceCell<Vec<String>>,
}

impl<F: EnvironmentInfra + FileReaderInfra + CommandInfra + DirectoryReaderInfra>
    ForgeCustomInstructionsService<F>
{
    pub fn new(infra: Arc<F>) -> Self {
        Self { infra, cache: Default::default() }
    }

    /// Discover all instruction file paths (AGENTS.md + FORGE.md).
    async fn discover_instruction_files(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let environment = self.infra.get_environment();

        // --- AGENTS.md (legacy) ---
        let base_agent_md = environment.global_agentsmd_path();
        if !paths.contains(&base_agent_md) {
            paths.push(base_agent_md);
        }

        if let Some(git_root_path) = self.get_git_root().await {
            let git_agent_md = git_root_path.join("AGENTS.md");
            if !paths.contains(&git_agent_md) {
                paths.push(git_agent_md);
            }
        }

        let cwd_agent_md = environment.local_agentsmd_path();
        if !paths.contains(&cwd_agent_md) {
            paths.push(cwd_agent_md);
        }

        // --- FORGE.md (new) ---
        let global_forge_md = environment.forge_md_global_path();
        if !paths.contains(&global_forge_md) {
            paths.push(global_forge_md);
        }

        // .forge/FORGE.md and cwd/FORGE.md (forge_md_project_path picks whichever exists)
        let project_forge_md = environment.forge_md_project_path();
        if !paths.contains(&project_forge_md) {
            paths.push(project_forge_md);
        }

        paths
    }

    /// Load all *.md files from a rules/ directory.
    async fn load_rules_dir(&self, dir: &PathBuf) -> Vec<String> {
        let mut rules = Vec::new();
        match self.infra.read_directory_files(dir, Some("*.md")).await {
            Ok(files) => {
                for (_path, content) in files {
                    if !content.trim().is_empty() {
                        rules.push(content);
                    }
                }
            }
            Err(_) => {
                // Directory doesn't exist or isn't readable — that's fine.
            }
        }
        rules
    }

    async fn get_git_root(&self) -> Option<PathBuf> {
        let output = self
            .infra
            .execute_command(
                "git rev-parse --show-toplevel".to_owned(),
                self.infra.get_environment().cwd,
                true,
                None,
            )
            .await
            .ok()?;

        if output.success() {
            Some(PathBuf::from(output.stdout.trim()))
        } else {
            None
        }
    }

    async fn init(&self) -> Vec<String> {
        let mut all_instructions = Vec::new();

        // 1. Load AGENTS.md + FORGE.md files
        let paths = self.discover_instruction_files().await;
        for path in paths {
            if let Ok(content) = self.infra.read_utf8(&path).await {
                all_instructions.push(content);
            }
        }

        // 2. Load rules/ directories
        let environment = self.infra.get_environment();

        let global_rules = self.load_rules_dir(&environment.rules_global_path()).await;
        all_instructions.extend(global_rules);

        let project_rules = self.load_rules_dir(&environment.rules_project_path()).await;
        all_instructions.extend(project_rules);

        all_instructions
    }
}

#[async_trait::async_trait]
impl<F: EnvironmentInfra + FileReaderInfra + CommandInfra + DirectoryReaderInfra>
    CustomInstructionsService for ForgeCustomInstructionsService<F>
{
    async fn get_custom_instructions(&self) -> Vec<String> {
        self.cache.get_or_init(|| self.init()).await.clone()
    }
}
