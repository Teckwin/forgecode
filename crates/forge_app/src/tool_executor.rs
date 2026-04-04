use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use forge_domain::{CodebaseQueryResult, ToolCallContext, ToolCatalog, ToolKind, ToolOutput};
use tokio::task::JoinSet;

use crate::fmt::content::FormatContent;
use crate::operation::{TempContentFiles, ToolOperation};
use crate::services::{Services, ShellService};
use crate::{
    AgentRegistry, ConversationService, EnvironmentInfra, FollowUpService, FsPatchService,
    FsReadService, FsRemoveService, FsSearchService, FsUndoService, FsWriteService,
    ImageReadService, NetFetchService, PlanCreateService, ProviderService, SkillFetchService,
    WorkspaceService,
};

/// Result of path safety check
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PathSafety {
    /// Path is safe to access within working directory
    Safe,
    /// Path is outside working directory - requires user confirmation
    RequiresConfirmation,
    /// Path is outside working directory and operation is write/delete - requires double confirmation
    RequiresDoubleConfirmation,
    /// Operation is denied
    Denied(String),
}

pub struct ToolExecutor<S> {
    services: Arc<S>,
}

impl<
    S: FsReadService
        + ImageReadService
        + FsWriteService
        + FsSearchService
        + WorkspaceService
        + NetFetchService
        + FsRemoveService
        + FsPatchService
        + FsUndoService
        + ShellService
        + FollowUpService
        + ConversationService
        + EnvironmentInfra
        + PlanCreateService
        + SkillFetchService
        + AgentRegistry
        + ProviderService
        + Services,
> ToolExecutor<S>
{
    pub fn new(services: Arc<S>) -> Self {
        Self { services }
    }

    /// Checks if a path is safe to access based on sandbox rules:
    /// - Within working directory: Safe
    /// - Outside working directory:
    ///   - Read: Requires confirmation
    ///   - Write/Delete: Requires double confirmation
    fn check_path_safety(&self, path: &str, tool_kind: &ToolKind) -> PathSafety {
        let env = self.services.get_environment();
        let cwd = &env.cwd;
        let path_buf = PathBuf::from(path);

        // Normalize the path to absolute
        let absolute_path = if path_buf.is_absolute() {
            path_buf.clone()
        } else {
            cwd.join(path_buf)
        };

        // Check if path is within working directory
        if absolute_path.starts_with(cwd) {
            return PathSafety::Safe;
        }

        // Path is outside working directory
        // Determine the required confirmation level based on tool type
        match tool_kind {
            // Read operations require single confirmation
            ToolKind::Read | ToolKind::FsSearch | ToolKind::SemSearch | ToolKind::TodoRead => {
                PathSafety::RequiresConfirmation
            }
            // Write, delete, patch, undo require double confirmation
            ToolKind::Write | ToolKind::Remove | ToolKind::Patch | ToolKind::Undo => {
                PathSafety::RequiresDoubleConfirmation
            }
            // Shell commands with external cwd also require double confirmation
            ToolKind::Shell => {
                // Only require confirmation if the cwd is outside working directory
                if absolute_path.starts_with(cwd) {
                    PathSafety::Safe
                } else {
                    PathSafety::RequiresDoubleConfirmation
                }
            }
            // Other tools (Fetch, Followup, Plan, Skill, TodoWrite) are not file-based
            _ => PathSafety::Safe,
        }
    }

    fn require_prior_read(
        &self,
        context: &ToolCallContext,
        raw_path: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        let target_path = self.normalize_path(raw_path.to_string());
        let has_read = context.with_metrics(|metrics| {
            metrics.files_accessed.contains(&target_path)
                || metrics.files_accessed.contains(raw_path)
        })?;

        if has_read {
            Ok(())
        } else {
            Err(anyhow!(
                "You must read the file with the read tool before attempting to {action}.",
                action = action
            ))
        }
    }

    async fn dump_operation(&self, operation: &ToolOperation) -> anyhow::Result<TempContentFiles> {
        match operation {
            ToolOperation::NetFetch { input: _, output } => {
                let original_length = output.content.len();
                let is_truncated =
                    original_length > self.services.get_environment().fetch_truncation_limit;
                let mut files = TempContentFiles::default();

                if is_truncated {
                    files = files.stdout(
                        self.create_temp_file("forge_fetch_", ".txt", &output.content)
                            .await?,
                    );
                }

                Ok(files)
            }
            ToolOperation::Shell { output } => {
                let env = self.services.get_environment();
                let stdout_lines = output.output.stdout.lines().count();
                let stderr_lines = output.output.stderr.lines().count();
                let stdout_truncated =
                    stdout_lines > env.stdout_max_prefix_length + env.stdout_max_suffix_length;
                let stderr_truncated =
                    stderr_lines > env.stdout_max_prefix_length + env.stdout_max_suffix_length;

                let mut files = TempContentFiles::default();

                if stdout_truncated {
                    files = files.stdout(
                        self.create_temp_file("forge_shell_stdout_", ".txt", &output.output.stdout)
                            .await?,
                    );
                }
                if stderr_truncated {
                    files = files.stderr(
                        self.create_temp_file("forge_shell_stderr_", ".txt", &output.output.stderr)
                            .await?,
                    );
                }

                Ok(files)
            }
            _ => Ok(TempContentFiles::default()),
        }
    }

    /// Converts a path to absolute by joining it with the current working
    /// directory if it's relative
    fn normalize_path(&self, path: String) -> String {
        let env = self.services.get_environment();
        let path_buf = PathBuf::from(&path);

        if path_buf.is_absolute() {
            path
        } else {
            PathBuf::from(&env.cwd).join(path_buf).display().to_string()
        }
    }

    async fn create_temp_file(
        &self,
        prefix: &str,
        ext: &str,
        content: &str,
    ) -> anyhow::Result<std::path::PathBuf> {
        let path = tempfile::Builder::new()
            .disable_cleanup(true)
            .prefix(prefix)
            .suffix(ext)
            .tempfile()?
            .into_temp_path()
            .to_path_buf();
        self.services
            .write(
                path.to_string_lossy().to_string(),
                content.to_string(),
                true,
            )
            .await?;
        Ok(path)
    }

    async fn call_internal(
        &self,
        input: ToolCatalog,
        context: &ToolCallContext,
    ) -> anyhow::Result<ToolOperation> {
        Ok(match input {
            ToolCatalog::Read(input) => {
                let normalized_path = self.normalize_path(input.file_path.clone());
                let output = self
                    .services
                    .read(
                        normalized_path,
                        input.start_line.map(|i| i as u64),
                        input.end_line.map(|i| i as u64),
                    )
                    .await?;

                (input, output).into()
            }
            ToolCatalog::Write(input) => {
                let normalized_path = self.normalize_path(input.file_path.clone());
                let output = self
                    .services
                    .write(normalized_path, input.content.clone(), input.overwrite)
                    .await?;
                (input, output).into()
            }
            ToolCatalog::FsSearch(input) => {
                let mut params = input.clone();
                // Normalize path if provided
                if let Some(ref path) = params.path {
                    params.path = Some(self.normalize_path(path.clone()));
                }
                let output = self.services.search(params).await?;
                (input, output).into()
            }
            ToolCatalog::SemSearch(input) => {
                let env = self.services.get_environment();
                let services = self.services.clone();
                let cwd = env.cwd.clone();
                let limit = env.sem_search_limit;
                let top_k = env.sem_search_top_k as u32;
                let params: Vec<_> = input
                    .queries
                    .iter()
                    .map(|search_query| {
                        forge_domain::SearchParams::new(&search_query.query, &search_query.use_case)
                            .limit(limit)
                            .top_k(top_k)
                    })
                    .collect();

                // Execute all queries in parallel
                let futures: Vec<_> = params
                    .into_iter()
                    .map(|param| services.query_workspace(cwd.clone(), param))
                    .collect();

                let mut results = futures::future::try_join_all(futures).await?;

                // Deduplicate results across queries
                crate::search_dedup::deduplicate_results(&mut results);

                let output = input
                    .queries
                    .into_iter()
                    .zip(results)
                    .map(|(query, results)| CodebaseQueryResult {
                        query: query.query,
                        use_case: query.use_case,
                        results,
                    })
                    .collect::<Vec<_>>();

                let output = forge_domain::CodebaseSearchResults { queries: output };
                ToolOperation::CodebaseSearch { output }
            }
            ToolCatalog::Remove(input) => {
                let normalized_path = self.normalize_path(input.path.clone());
                let output = self.services.remove(normalized_path).await?;
                (input, output).into()
            }
            ToolCatalog::Patch(input) => {
                let normalized_path = self.normalize_path(input.file_path.clone());
                let output = self
                    .services
                    .patch(
                        normalized_path,
                        input.old_string.clone(),
                        input.new_string.clone(),
                        input.replace_all,
                    )
                    .await?;
                (input, output).into()
            }
            ToolCatalog::Undo(input) => {
                let normalized_path = self.normalize_path(input.path.clone());
                let output = self.services.undo(normalized_path).await?;
                (input, output).into()
            }
            ToolCatalog::Shell(input) => {
                let cwd = input
                    .cwd
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| self.services.get_environment().cwd.display().to_string());
                let normalized_cwd = self.normalize_path(cwd);
                let output = self
                    .services
                    .execute(
                        input.command.clone(),
                        PathBuf::from(normalized_cwd),
                        input.keep_ansi,
                        false,
                        input.env.clone(),
                        input.description.clone(),
                    )
                    .await?;
                output.into()
            }
            ToolCatalog::Fetch(input) => {
                let output = self.services.fetch(input.url.clone(), input.raw).await?;
                (input, output).into()
            }
            ToolCatalog::Followup(input) => {
                let output = self
                    .services
                    .follow_up(
                        input.question.clone(),
                        input
                            .option1
                            .clone()
                            .into_iter()
                            .chain(input.option2.clone())
                            .chain(input.option3.clone())
                            .chain(input.option4.clone())
                            .chain(input.option5.clone())
                            .collect(),
                        input.multiple,
                    )
                    .await?;
                output.into()
            }
            ToolCatalog::Plan(input) => {
                let output = self
                    .services
                    .create_plan(
                        input.plan_name.clone(),
                        input.version.clone(),
                        input.content.clone(),
                    )
                    .await?;
                (input, output).into()
            }
            ToolCatalog::Skill(input) => {
                let skill = self.services.fetch_skill(input.name.clone()).await?;
                ToolOperation::Skill { output: skill }
            }
            ToolCatalog::TodoWrite(input) => {
                let before = context.get_todos()?;
                context.update_todos(input.todos.clone())?;
                let after = context.get_todos()?;
                ToolOperation::TodoWrite { before, after }
            }
            ToolCatalog::TodoRead(_input) => {
                let todos = context.get_todos()?;
                ToolOperation::TodoRead { output: todos }
            }
        })
    }

    pub async fn execute(
        &self,
        tool_input: ToolCatalog,
        context: &ToolCallContext,
    ) -> anyhow::Result<ToolOutput> {
        let tool_kind = tool_input.kind();
        let env = self.services.get_environment();

        // Enforce path safety check for file operations outside working directory
        self.enforce_path_safety(&tool_input)?;

        // Enforce read-before-edit for patch
        if let ToolCatalog::Patch(input) = &tool_input {
            self.require_prior_read(context, &input.file_path, "edit it")?;
        }

        // Enforce read-before-edit for overwrite writes
        if let ToolCatalog::Write(input) = &tool_input
            && input.overwrite
        {
            self.require_prior_read(context, &input.file_path, "overwrite it")?;
        }

        let execution_result = self.call_internal(tool_input.clone(), context).await;

        if let Err(ref error) = execution_result {
            tracing::error!(error = ?error, "Tool execution failed");
        }

        let operation = execution_result?;

        // Send formatted output message
        if let Some(output) = operation.to_content(&env) {
            context.send(output).await?;
        }

        let truncation_path = self.dump_operation(&operation).await?;

        context.with_metrics(|metrics| {
            operation.into_tool_output(tool_kind, truncation_path, &env, metrics)
        })
    }

    /// Enforces path safety rules for file operations
    fn enforce_path_safety(&self, tool_input: &ToolCatalog) -> anyhow::Result<()> {
        let tool_kind = tool_input.kind();

        // Extract path from tool input
        let path = match tool_input {
            ToolCatalog::Read(input) => Some(input.file_path.as_str()),
            ToolCatalog::Write(input) => Some(input.file_path.as_str()),
            ToolCatalog::FsSearch(input) => input.path.as_deref(),
            ToolCatalog::SemSearch(_) => None,
            ToolCatalog::Remove(input) => Some(input.path.as_str()),
            ToolCatalog::Patch(input) => Some(input.file_path.as_str()),
            ToolCatalog::Undo(input) => Some(input.path.as_str()),
            ToolCatalog::Shell(input) => input.cwd.as_ref().map(|p| p.to_str().unwrap_or("")),
            _ => None,
        };

        // If no path to check, skip safety check
        let Some(path) = path else {
            return Ok(());
        };

        // Skip empty paths
        if path.is_empty() {
            return Ok(());
        }

        let safety = self.check_path_safety(path, &tool_kind);

        match safety {
            PathSafety::Safe => Ok(()),
            PathSafety::RequiresConfirmation => {
                // Log warning for read operations outside cwd
                tracing::warn!(
                    "Tool {} is accessing path outside working directory: {}. Allowing but logging warning.",
                    tool_kind,
                    path
                );
                Ok(())
            }
            PathSafety::RequiresDoubleConfirmation => {
                // Deny write/delete operations outside cwd - requires user confirmation which is not implemented
                Err(anyhow!(
                    "Tool {} is attempting to modify path outside working directory: {}. This operation is not allowed for security reasons. Please work within the current working directory.",
                    tool_kind,
                    path
                ))
            }
            PathSafety::Denied(reason) => Err(anyhow!(reason)),
        }
    }

    /// Executes multiple tools in parallel, returning results in order.
    ///
    /// # Arguments
    /// * `tools` - Vector of tool inputs to execute
    /// * `context` - Tool call context
    ///
    /// # Returns
    /// Vector of ToolOutput results in the same order as input tools.
    /// If any tool fails, the error is included in its position.
    #[allow(dead_code)]
    pub async fn execute_parallel(
        &self,
        tools: Vec<ToolCatalog>,
        context: &ToolCallContext,
    ) -> Vec<anyhow::Result<ToolOutput>> {
        let mut join_set = JoinSet::new();

        // Spawn all tasks
        for tool in tools {
            let executor = Self { services: Arc::clone(&self.services) };
            let ctx = context.clone();

            join_set.spawn(async move { executor.execute(tool, &ctx).await });
        }

        // Collect results in order
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(output)) => results.push(Ok(output)),
                Ok(Err(e)) => results.push(Err(e)),
                Err(join_error) => results.push(Err(anyhow!("Task join error: {}", join_error))),
            }
        }

        results
    }

    /// Executes multiple tools in parallel, returning results as soon as they complete.
    ///
    /// Unlike `execute_parallel`, this method returns results in completion order
    /// rather than input order.
    #[allow(dead_code)]
    pub async fn execute_parallel_unordered(
        &self,
        tools: Vec<ToolCatalog>,
        context: &ToolCallContext,
    ) -> Vec<anyhow::Result<ToolOutput>> {
        let mut handles = Vec::new();

        for tool in tools {
            let executor = Self { services: Arc::clone(&self.services) };
            let ctx = context.clone();

            handles.push(tokio::spawn(
                async move { executor.execute(tool, &ctx).await },
            ));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(output)) => results.push(Ok(output)),
                Ok(Err(e)) => results.push(Err(e)),
                Err(join_error) => results.push(Err(anyhow!("Task join error: {}", join_error))),
            }
        }

        results
    }
}
