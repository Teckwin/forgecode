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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_normalized_config_has_empty_collections() {
        let config = NormalizedConfig::default();
        assert!(config.model.is_none());
        assert!(config.provider.is_none());
        assert!(config.agents.is_empty());
        assert!(config.mcp_servers.is_empty());
        assert!(config.custom_instructions.is_none());
        assert!(config.rules.is_empty());
        assert!(config.permissions.allowed_read_paths.is_empty());
        assert!(config.permissions.allowed_write_paths.is_empty());
        assert!(config.permissions.allowed_commands.is_empty());
        assert!(config.permissions.denied_commands.is_empty());
    }

    #[test]
    fn serialize_deserialize_round_trip_preserves_all_fields() {
        let mut config = NormalizedConfig::default();
        config.model = Some("claude-sonnet-4-20250514".to_string());
        config.provider = Some("anthropic".to_string());
        config.custom_instructions = Some("Be helpful.".to_string());
        config.permissions.allowed_read_paths = vec!["/tmp".to_string()];
        config.permissions.allowed_commands = vec!["git".to_string()];
        config.permissions.denied_commands = vec!["rm".to_string()];
        config.rules.push(RuleFile {
            path: PathBuf::from("safety.md"),
            content: "Do not delete files.".to_string(),
        });

        let mut agent = AgentProviderConfig::default();
        agent.model = Some("gpt-4".to_string());
        agent.provider = Some("openai".to_string());
        config.agents.insert("coder".to_string(), agent);

        config.mcp_servers.insert(
            "my-server".to_string(),
            McpServerConfig {
                command: "npx".to_string(),
                args: vec!["server".to_string()],
                env: HashMap::from([("KEY".to_string(), "val".to_string())]),
            },
        );

        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: NormalizedConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            deserialized.model.as_deref(),
            Some("claude-sonnet-4-20250514")
        );
        assert_eq!(deserialized.provider.as_deref(), Some("anthropic"));
        assert_eq!(
            deserialized.custom_instructions.as_deref(),
            Some("Be helpful.")
        );
        assert_eq!(deserialized.permissions.allowed_read_paths, vec!["/tmp"]);
        assert_eq!(deserialized.permissions.allowed_commands, vec!["git"]);
        assert_eq!(deserialized.permissions.denied_commands, vec!["rm"]);
        assert_eq!(deserialized.rules.len(), 1);
        assert_eq!(deserialized.rules[0].content, "Do not delete files.");
        assert_eq!(deserialized.agents.len(), 1);
        let coder = deserialized.agents.get("coder").unwrap();
        assert_eq!(coder.model.as_deref(), Some("gpt-4"));
        assert_eq!(coder.provider.as_deref(), Some("openai"));
        assert_eq!(deserialized.mcp_servers.len(), 1);
        let srv = deserialized.mcp_servers.get("my-server").unwrap();
        assert_eq!(srv.command, "npx");
        assert_eq!(srv.args, vec!["server"]);
        assert_eq!(srv.env.get("KEY").map(|s| s.as_str()), Some("val"));
    }

    #[test]
    fn agent_provider_config_full_round_trip() {
        let agent = AgentProviderConfig {
            model: Some("claude-opus-4-20250514".to_string()),
            provider: Some("anthropic".to_string()),
            api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
            base_url: Some("https://api.anthropic.com".to_string()),
            max_tokens: Some(4096),
        };

        let json = serde_json::to_string(&agent).expect("serialize");
        let deserialized: AgentProviderConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            deserialized.model.as_deref(),
            Some("claude-opus-4-20250514")
        );
        assert_eq!(deserialized.provider.as_deref(), Some("anthropic"));
        assert_eq!(
            deserialized.api_key_env.as_deref(),
            Some("ANTHROPIC_API_KEY")
        );
        assert_eq!(
            deserialized.base_url.as_deref(),
            Some("https://api.anthropic.com")
        );
        assert_eq!(deserialized.max_tokens, Some(4096));
    }
}
