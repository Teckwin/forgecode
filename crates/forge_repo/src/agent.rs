use std::sync::Arc;

use anyhow::{Context, Result};
use forge_app::domain::{AgentDefinition, Template};
use forge_app::{
    AgentRepository, DirectoryReaderInfra, EnvironmentInfra, FileDirectoryInfra, FileInfoInfra,
    FileRemoverInfra, FileWriterInfra,
};
use gray_matter::Matter;
use gray_matter::engine::YAML;

/// Infrastructure implementation for loading agent definitions from multiple
/// sources:
/// 1. Built-in agents (embedded in the application)
/// 2. Global custom agents (from ~/.forge/agents/ directory)
/// 3. Project-local agents (from .forge/agents/ directory in current working
///    directory)
///
/// ## Agent Precedence
/// When agents have duplicate IDs across different sources, the precedence
/// order is: **CWD (project-local) > Global custom > Built-in**
///
/// This means project-local agents can override global agents, and both can
/// override built-in agents.
///
/// ## Directory Resolution
/// - **Built-in agents**: Embedded in application binary
/// - **Global agents**: `{HOME}/.forge/agents/*.md`
/// - **CWD agents**: `./.forge/agents/*.md` (relative to current working
///   directory)
///
/// Missing directories are handled gracefully and don't prevent loading from
/// other sources.
pub struct ForgeAgentRepository<I> {
    infra: Arc<I>,
}

impl<I> ForgeAgentRepository<I> {
    pub fn new(infra: Arc<I>) -> Self {
        Self { infra }
    }
}

#[async_trait::async_trait]
impl<I: FileInfoInfra + EnvironmentInfra + DirectoryReaderInfra> AgentRepository
    for ForgeAgentRepository<I>
{
    /// Load all agent definitions from all available sources with conflict
    /// resolution.
    async fn get_agents(&self) -> anyhow::Result<Vec<forge_app::domain::AgentDefinition>> {
        self.load_agents().await
    }
}

/// Extended implementation for dynamic agent creation/deletion
/// Requires additional infrastructure traits for file operations
#[async_trait::async_trait]
impl<
    I: FileInfoInfra
        + EnvironmentInfra
        + DirectoryReaderInfra
        + FileWriterInfra
        + FileRemoverInfra
        + FileDirectoryInfra,
> forge_app::AgentRepositoryExt for ForgeAgentRepository<I>
{
    /// Create a new agent definition and persist it to the agents directory.
    async fn create_agent(&self, agent: forge_domain::Agent) -> anyhow::Result<()> {
        // Determine the target directory: prefer CWD agents directory
        let env = self.infra.get_environment();
        let target_dir = env.agent_cwd_path();

        // Ensure the directory exists using FileDirectoryInfra
        self.infra.create_dirs(&target_dir).await.with_context(|| {
            format!(
                "Failed to create agents directory: {}",
                target_dir.display()
            )
        })?;

        // Generate the file path
        let file_name = format!("{}.md", agent.id.as_str());
        let file_path = target_dir.join(&file_name);

        // Serialize the agent definition to markdown format
        let content = serialize_agent_to_markdown(&agent);

        // Write the file using FileWriterInfra (takes Bytes)
        self.infra
            .write(&file_path, content.into())
            .await
            .with_context(|| format!("Failed to write agent file: {}", file_path.display()))?;

        Ok(())
    }

    /// Delete an agent by ID from the agents directory.
    async fn delete_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        // Check in CWD agents directory first
        let env = self.infra.get_environment();
        let cwd_path = env.agent_cwd_path();
        let cwd_file = cwd_path.join(format!("{}.md", agent_id));

        if self.infra.exists(&cwd_file).await? {
            self.infra.remove(&cwd_file).await?;
            return Ok(());
        }

        // Then check global agents directory
        let global_path = env.agent_path();
        let global_file = global_path.join(format!("{}.md", agent_id));

        if self.infra.exists(&global_file).await? {
            self.infra.remove(&global_file).await?;
            return Ok(());
        }

        // Agent not found
        anyhow::bail!("Agent '{}' not found in agents directory", agent_id)
    }
}

impl<I: FileInfoInfra + EnvironmentInfra + DirectoryReaderInfra> ForgeAgentRepository<I> {
    /// Load all agent definitions from all available sources
    async fn load_agents(&self) -> anyhow::Result<Vec<AgentDefinition>> {
        // Load built-in agents (no path - will display as "BUILT IN")
        let mut agents = self.init_default().await?;

        // Load custom agents from global directory
        let dir = self.infra.get_environment().agent_path();
        let custom_agents = self.init_agent_dir(&dir).await?;
        agents.extend(custom_agents);

        // Load custom agents from CWD
        let dir = self.infra.get_environment().agent_cwd_path();
        let cwd_agents = self.init_agent_dir(&dir).await?;
        agents.extend(cwd_agents);

        // Handle agent ID conflicts by keeping the last occurrence
        // This gives precedence order: CWD > Global Custom > Built-in
        Ok(resolve_agent_conflicts(agents))
    }

    async fn init_default(&self) -> anyhow::Result<Vec<AgentDefinition>> {
        parse_agent_iter(
            [
                ("forge", include_str!("agents/forge.md")),
                ("muse", include_str!("agents/muse.md")),
                ("sage", include_str!("agents/sage.md")),
            ]
            .into_iter()
            .map(|(name, content)| (name.to_string(), content.to_string())),
        )
    }

    async fn init_agent_dir(&self, dir: &std::path::Path) -> anyhow::Result<Vec<AgentDefinition>> {
        if !self.infra.exists(dir).await? {
            return Ok(vec![]);
        }

        // Use DirectoryReaderInfra to read all .md files in parallel
        let files = self
            .infra
            .read_directory_files(dir, Some("*.md"))
            .await
            .with_context(|| format!("Failed to read agents from: {}", dir.display()))?;

        let mut agents = Vec::new();
        for (path, content) in files {
            let mut agent = parse_agent_file(&content)
                .with_context(|| format!("Failed to parse agent: {}", path.display()))?;

            // Store the file path
            agent.path = Some(path.display().to_string());
            agents.push(agent);
        }

        Ok(agents)
    }
}

/// Implementation function for resolving agent ID conflicts by keeping the last
/// occurrence. This implements the precedence order: CWD Custom > Global Custom
/// > Built-in
fn resolve_agent_conflicts(agents: Vec<AgentDefinition>) -> Vec<AgentDefinition> {
    use std::collections::HashMap;

    // Use HashMap to deduplicate by agent ID, keeping the last occurrence
    let mut agent_map: HashMap<String, AgentDefinition> = HashMap::new();

    for agent in agents {
        agent_map.insert(agent.id.to_string(), agent);
    }

    // Convert back to vector (order is not guaranteed but doesn't matter for the
    // service)
    agent_map.into_values().collect()
}

fn parse_agent_iter<I, Path: AsRef<str>, Content: AsRef<str>>(
    contents: I,
) -> anyhow::Result<Vec<AgentDefinition>>
where
    I: Iterator<Item = (Path, Content)>,
{
    let mut agents = vec![];

    for (name, content) in contents {
        let agent = parse_agent_file(content.as_ref())
            .with_context(|| format!("Failed to parse agent: {}", name.as_ref()))?;

        agents.push(agent);
    }

    Ok(agents)
}

/// Parse raw content into an AgentDefinition with YAML frontmatter
fn parse_agent_file(content: &str) -> Result<AgentDefinition> {
    // Parse the frontmatter using gray_matter with type-safe deserialization
    let gray_matter = Matter::<YAML>::new();
    let result = gray_matter.parse::<AgentDefinition>(content)?;

    // Extract the frontmatter
    let agent = result
        .data
        .context("Empty system prompt content")?
        .system_prompt(Template::new(result.content));

    Ok(agent)
}

/// Serialize an AgentDefinition to markdown format with YAML frontmatter
fn serialize_agent_to_markdown(agent: &forge_domain::Agent) -> String {
    // Build YAML frontmatter
    let mut yaml_parts = Vec::new();

    // Core fields
    yaml_parts.push(format!("id: \"{}\"", agent.id));
    if let Some(title) = &agent.title {
        yaml_parts.push(format!("title: \"{}\"", title));
    }
    if let Some(description) = &agent.description {
        yaml_parts.push(format!("description: \"{}\"", description));
    }

    // Reasoning - only serialize if explicitly enabled
    if let Some(reasoning) = &agent.reasoning
        && reasoning.enabled.unwrap_or(false)
    {
        yaml_parts.push("reasoning:".to_string());
        yaml_parts.push(format!("  enabled: {}", reasoning.enabled.unwrap_or(false)));
    }

    // Tools
    if let Some(tools) = &agent.tools
        && !tools.is_empty()
    {
        yaml_parts.push("tools:".to_string());
        for tool in tools {
            yaml_parts.push(format!("  - {}", tool));
        }
    }

    // Provider config (agent has provider and model directly)
    yaml_parts.push("provider_config:".to_string());
    yaml_parts.push(format!("  provider: {}", agent.provider));
    yaml_parts.push(format!("  model: {}", agent.model));

    // Compact
    if agent.compact.retention_window > 0
        || agent.compact.eviction_window > 0.0
        || agent.compact.max_tokens.is_some()
    {
        yaml_parts.push("compact:".to_string());
        if agent.compact.retention_window > 0 {
            yaml_parts.push(format!(
                "  retention_window: {}",
                agent.compact.retention_window
            ));
        }
        if agent.compact.eviction_window > 0.0 {
            yaml_parts.push(format!(
                "  eviction_window: {}",
                agent.compact.eviction_window
            ));
        }
        if let Some(max_tokens) = agent.compact.max_tokens {
            yaml_parts.push(format!("  max_tokens: {}", max_tokens));
        }
    }

    // Max turns
    if let Some(max_turns) = agent.max_turns {
        yaml_parts.push(format!("max_turns: {}", max_turns));
    }

    // Max tool failure per turn
    if let Some(max_tool_failure_per_turn) = agent.max_tool_failure_per_turn {
        yaml_parts.push(format!(
            "max_tool_failure_per_turn: {}",
            max_tool_failure_per_turn
        ));
    }

    // Build the markdown
    let yaml = yaml_parts.join("\n");
    let system_prompt = agent
        .system_prompt
        .as_ref()
        .map(|s| s.template.as_str())
        .unwrap_or("");

    format!("---\n{}\n---\n{}", yaml, system_prompt)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn test_parse_basic_agent() {
        let content = forge_test_kit::fixture!("/src/fixtures/agents/basic.md").await;

        let actual = parse_agent_file(&content).unwrap();

        assert_eq!(actual.id.as_str(), "test-basic");
        assert_eq!(actual.title.as_ref().unwrap(), "Basic Test Agent");
        assert_eq!(
            actual.description.as_ref().unwrap(),
            "A simple test agent for basic functionality"
        );
        assert_eq!(
            actual.system_prompt.as_ref().unwrap().template,
            "This is a basic test agent used for testing fundamental functionality."
        );
    }

    #[tokio::test]
    async fn test_parse_advanced_agent() {
        let content = forge_test_kit::fixture!("/src/fixtures/agents/advanced.md").await;

        let actual = parse_agent_file(&content).unwrap();

        assert_eq!(actual.id.as_str(), "test-advanced");
        assert_eq!(actual.title.as_ref().unwrap(), "Advanced Test Agent");
        assert_eq!(
            actual.description.as_ref().unwrap(),
            "An advanced test agent with full configuration"
        );
    }
}
