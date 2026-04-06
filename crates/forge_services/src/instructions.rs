use std::path::{Path, PathBuf};
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
    async fn load_rules_dir(&self, dir: &Path) -> Vec<String> {
        let mut rules = Vec::new();
        match self.infra.read_directory_files(dir, Some("*.md")).await {
            Ok(files) => {
                for (_path, content) in files {
                    if !content.trim().is_empty() {
                        rules.push(content);
                    }
                }
            }
            Err(e) => {
                tracing::debug!(dir = %dir.display(), error = %e, "Rules directory not readable (may not exist)");
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use forge_app::{
        CommandInfra, CustomInstructionsService, DirectoryReaderInfra, EnvironmentInfra,
        FileReaderInfra,
    };
    use forge_domain::{CommandOutput, ConfigOperation, Environment};

    use super::ForgeCustomInstructionsService;

    /// Mock infrastructure for testing custom instructions loading.
    ///
    /// Uses real filesystem via tempfile for file reading and directory listing,
    /// but mocks the git command and environment paths.
    struct MockInstructionsInfra {
        environment: Environment,
        git_root: Option<PathBuf>,
    }

    impl MockInstructionsInfra {
        fn new(base_path: PathBuf, cwd: PathBuf, git_root: Option<PathBuf>) -> Self {
            use fake::{Fake, Faker};
            let env: Environment = Faker.fake();
            let environment = env.base_path(base_path).cwd(cwd);
            Self { environment, git_root }
        }
    }

    impl EnvironmentInfra for MockInstructionsInfra {
        fn get_env_var(&self, _key: &str) -> Option<String> {
            None
        }

        fn get_env_vars(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }

        fn get_environment(&self) -> Environment {
            self.environment.clone()
        }

        async fn update_environment(&self, _ops: Vec<ConfigOperation>) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl FileReaderInfra for MockInstructionsInfra {
        async fn read_utf8(&self, path: &Path) -> anyhow::Result<String> {
            let content = tokio::fs::read_to_string(path).await?;
            Ok(content)
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
    impl CommandInfra for MockInstructionsInfra {
        async fn execute_command(
            &self,
            command: String,
            _working_dir: PathBuf,
            _silent: bool,
            _env_vars: Option<Vec<String>>,
        ) -> anyhow::Result<CommandOutput> {
            if command.contains("git rev-parse") {
                match &self.git_root {
                    Some(path) => Ok(CommandOutput {
                        stdout: path.to_string_lossy().to_string(),
                        stderr: String::new(),
                        command,
                        exit_code: Some(0),
                    }),
                    None => Err(anyhow::anyhow!("git not available")),
                }
            } else {
                Err(anyhow::anyhow!("unknown command: {command}"))
            }
        }

        async fn execute_command_raw(
            &self,
            _command: &str,
            _working_dir: PathBuf,
            _env_vars: Option<Vec<String>>,
        ) -> anyhow::Result<std::process::ExitStatus> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl DirectoryReaderInfra for MockInstructionsInfra {
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
            let glob_pattern = pattern.unwrap_or("*");
            let full_pattern = directory.join(glob_pattern);
            let pattern_str = full_pattern
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("invalid path"))?;

            let mut results = Vec::new();
            for entry in glob::glob(pattern_str)? {
                let path = entry?;
                if path.is_file() {
                    let content = tokio::fs::read_to_string(&path).await?;
                    results.push((path, content));
                }
            }
            // Sort for deterministic ordering in tests
            results.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(results)
        }
    }

    /// Helper: create a service with the given mock infra.
    fn make_service(
        base_path: PathBuf,
        cwd: PathBuf,
        git_root: Option<PathBuf>,
    ) -> ForgeCustomInstructionsService<MockInstructionsInfra> {
        let infra = Arc::new(MockInstructionsInfra::new(base_path, cwd, git_root));
        ForgeCustomInstructionsService::new(infra)
    }

    /// Helper: write a file, creating parent directories as needed.
    async fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.unwrap();
        }
        tokio::fs::write(path, content).await.unwrap();
    }

    #[tokio::test]
    async fn test_no_instruction_files() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        assert!(result.is_empty(), "expected empty vec, got: {result:?}");
    }

    #[tokio::test]
    async fn test_global_agents_md() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        let global_agents = base_path.join("AGENTS.md");
        write_file(&global_agents, "global agents instructions").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        assert_eq!(result, vec!["global agents instructions"]);
    }

    #[tokio::test]
    async fn test_cwd_agents_md() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        let cwd_agents = cwd.join("AGENTS.md");
        write_file(&cwd_agents, "cwd agents instructions").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        assert_eq!(result, vec!["cwd agents instructions"]);
    }

    #[tokio::test]
    async fn test_global_forge_md() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        let global_forge = base_path.join("FORGE.md");
        write_file(&global_forge, "global forge instructions").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        assert_eq!(result, vec!["global forge instructions"]);
    }

    #[tokio::test]
    async fn test_project_forge_md() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        // .forge/FORGE.md takes priority but since we don't create it,
        // forge_md_project_path falls back to cwd/FORGE.md
        let project_forge = cwd.join("FORGE.md");
        write_file(&project_forge, "project forge instructions").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        assert_eq!(result, vec!["project forge instructions"]);
    }

    #[tokio::test]
    async fn test_git_root_agents_md() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        let git_root = tmp.path().join("gitroot");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();
        tokio::fs::create_dir_all(&git_root).await.unwrap();

        let git_agents = git_root.join("AGENTS.md");
        write_file(&git_agents, "git root agents instructions").await;

        let service = make_service(base_path, cwd, Some(git_root));
        let result = service.get_custom_instructions().await;
        assert_eq!(result, vec!["git root agents instructions"]);
    }

    #[tokio::test]
    async fn test_rules_global_loaded() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        let rules_dir = base_path.join("rules");
        write_file(&rules_dir.join("rule1.md"), "global rule one").await;
        write_file(&rules_dir.join("rule2.md"), "global rule two").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        assert_eq!(result, vec!["global rule one", "global rule two"]);
    }

    #[tokio::test]
    async fn test_rules_project_loaded() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        let rules_dir = cwd.join(".forge/rules");
        write_file(&rules_dir.join("proj_rule.md"), "project rule content").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        assert_eq!(result, vec!["project rule content"]);
    }

    #[tokio::test]
    async fn test_all_sources_combined() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        let git_root = tmp.path().join("gitroot");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();
        tokio::fs::create_dir_all(&git_root).await.unwrap();

        // AGENTS.md sources
        write_file(&base_path.join("AGENTS.md"), "global agents").await;
        write_file(&git_root.join("AGENTS.md"), "git agents").await;
        write_file(&cwd.join("AGENTS.md"), "cwd agents").await;

        // FORGE.md sources
        write_file(&base_path.join("FORGE.md"), "global forge").await;
        write_file(&cwd.join("FORGE.md"), "project forge").await;

        // Rules
        write_file(
            &base_path.join("rules/global_rule.md"),
            "global rule content",
        )
        .await;
        write_file(
            &cwd.join(".forge/rules/project_rule.md"),
            "project rule content",
        )
        .await;

        let service = make_service(base_path, cwd, Some(git_root));
        let result = service.get_custom_instructions().await;

        // Order: global AGENTS.md, git root AGENTS.md, cwd AGENTS.md,
        //        global FORGE.md, project FORGE.md,
        //        global rules, project rules
        assert_eq!(
            result,
            vec![
                "global agents",
                "git agents",
                "cwd agents",
                "global forge",
                "project forge",
                "global rule content",
                "project rule content",
            ]
        );
    }

    #[tokio::test]
    async fn test_caching() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        write_file(&base_path.join("AGENTS.md"), "cached content").await;

        let service = make_service(base_path.clone(), cwd.clone(), None);

        let first = service.get_custom_instructions().await;
        assert_eq!(first, vec!["cached content"]);

        // Modify the file after first load - result should be cached
        write_file(&base_path.join("AGENTS.md"), "modified content").await;

        let second = service.get_custom_instructions().await;
        assert_eq!(
            second, first,
            "second call should return cached result, not re-read files"
        );
    }

    #[tokio::test]
    async fn test_empty_files_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        let rules_dir = base_path.join("rules");
        write_file(&rules_dir.join("empty.md"), "").await;
        write_file(&rules_dir.join("whitespace.md"), "   \n  ").await;
        write_file(&rules_dir.join("real.md"), "actual content").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;
        // Empty and whitespace-only rules files are skipped by load_rules_dir
        assert_eq!(result, vec!["actual content"]);
    }

    #[tokio::test]
    async fn test_git_root_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base");
        let cwd = tmp.path().join("cwd");
        tokio::fs::create_dir_all(&base_path).await.unwrap();
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        // Only cwd AGENTS.md exists; git_root is None (simulates git failure)
        write_file(&cwd.join("AGENTS.md"), "cwd agents only").await;

        let service = make_service(base_path, cwd, None);
        let result = service.get_custom_instructions().await;

        // Should still work, just without git root instructions
        assert_eq!(result, vec!["cwd agents only"]);
    }
}
