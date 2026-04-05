use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Tool-agnostic normalized configuration that all adapters produce.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NormalizedConfig {
    /// Default model identifier (e.g. "claude-sonnet-4-20250514").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider name (e.g. "anthropic", "openai", "openrouter").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// Named agent configurations keyed by agent name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub agents: HashMap<String, AgentProviderConfig>,

    /// MCP server definitions keyed by server name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Permissions settings.
    #[serde(default)]
    pub permissions: NormalizedPermissions,

    /// Custom instructions / system prompt additions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,

    /// Rule files loaded from a rules directory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleFile>,
}

/// Configuration for a named agent (provider + model pair).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// MCP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Command to launch the server.
    pub command: String,

    /// Arguments for the command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// Environment variables.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
}

/// Normalized permission settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NormalizedPermissions {
    /// Directories the tool is allowed to read from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_read_paths: Vec<String>,

    /// Directories the tool is allowed to write to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_write_paths: Vec<String>,

    /// Shell commands that are allowed to execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_commands: Vec<String>,

    /// Shell commands that are denied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_commands: Vec<String>,
}

/// A rule file loaded from a rules directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFile {
    /// Original file path (relative to the rules directory).
    pub path: PathBuf,

    /// Content of the rule file.
    pub content: String,
}
