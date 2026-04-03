//! Claude Code settings.json parser and converter

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Claude Code settings.json root structure
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ClaudeCodeSettings {
    /// Permission rules (allow/deny/ask arrays)
    #[serde(default)]
    pub permissions: Option<PermissionsConfig>,

    /// Environment variables
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    /// MCP server configurations
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,

    /// Hooks configuration
    #[serde(default)]
    pub hooks: Option<HooksConfig>,

    /// Sandbox settings
    #[serde(default)]
    pub sandbox: Option<SandboxConfig>,
}

/// Claude Code permissions configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub allow: Option<Vec<String>>,
    #[serde(default)]
    pub deny: Option<Vec<String>>,
    #[serde(default)]
    pub ask: Option<Vec<String>>,
    #[serde(default, rename = "defaultMode")]
    pub default_mode: Option<String>,
}

/// Claude Code MCP server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    /// Command to run (for stdio servers)
    #[serde(default)]
    pub command: Option<String>,

    /// Arguments for the command
    #[serde(default)]
    pub args: Option<Vec<String>>,

    /// Environment variables
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    /// HTTP URL (for SSE servers)
    #[serde(default)]
    pub url: Option<String>,

    /// Timeout in seconds
    #[serde(default)]
    pub timeout: Option<u64>,
}

/// Claude Code hooks configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct HooksConfig {
    #[serde(default, rename = "PostToolUse")]
    pub post_tool_use: Option<Vec<HookAction>>,

    #[serde(default, rename = "SessionStart")]
    pub session_start: Option<Vec<HookAction>>,

    #[serde(default, rename = "Notification")]
    pub notification: Option<Vec<HookAction>>,
}

/// Claude Code hook action
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookAction {
    #[serde(default)]
    pub matcher: Option<String>,

    #[serde(default)]
    #[serde(rename = "type")]
    pub hook_type: Option<String>,

    #[serde(default)]
    pub command: Option<String>,

    #[serde(default)]
    pub status_message: Option<String>,

    #[serde(default)]
    pub once: Option<bool>,
}

/// Claude Code sandbox configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SandboxConfig {
    #[serde(default)]
    pub enabled: Option<bool>,

    #[serde(default, rename = "allowedDirectories")]
    pub allowed_directories: Option<Vec<String>>,

    #[serde(default, rename = "deniedDirectories")]
    pub denied_directories: Option<Vec<String>>,

    #[serde(default)]
    pub network: Option<String>,
}

/// Claude Code settings parser
pub struct ClaudeCodeParser;

impl ClaudeCodeParser {
    /// Parse a Claude Code settings.json file
    pub fn parse(path: &Path) -> Result<ClaudeCodeSettings> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read settings.json from {:?}", path))?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse settings.json from {:?}", path))
    }

    /// Check if a file is a valid Claude Code settings.json
    pub fn is_valid(path: &Path) -> bool {
        Self::parse(path).is_ok()
    }
}

/// Converter from Claude Code settings to Forge settings
pub struct ClaudeCodeToForgeConverter;

impl ClaudeCodeToForgeConverter {
    /// Convert Claude Code settings to Forge SettingConfig
    ///
    /// This converts:
    /// - permissions -> Forge policy.yaml format (using PermissionPattern)
    /// - mcpServers -> Forge MCP configuration
    /// - env -> Forge environment variables (merged at runtime)
    /// - hooks -> Forge lifecycle hooks (if supported)
    /// - sandbox -> Forge sandbox configuration
    pub fn convert(settings: ClaudeCodeSettings) -> Result<ConvertedConfig> {
        let mut converted = ConvertedConfig::default();

        // Convert permissions
        if let Some(permissions) = settings.permissions {
            converted.permissions.allow = permissions.allow.unwrap_or_default();
            converted.permissions.deny = permissions.deny.unwrap_or_default();
            converted.permissions.ask = permissions.ask.unwrap_or_default();
            converted.permissions.default_mode = permissions.default_mode;
        }

        // Convert MCP servers
        if let Some(mcp_servers) = settings.mcp_servers {
            for (name, config) in mcp_servers {
                let converted_server = ConvertedMcpServer {
                    name,
                    command: config.command,
                    args: config.args.unwrap_or_default(),
                    env: config.env.unwrap_or_default(),
                    url: config.url,
                    timeout: config.timeout,
                };
                converted.mcp_servers.push(converted_server);
            }
        }

        // Convert environment variables (store for later merging)
        converted.env = settings.env.unwrap_or_default();

        // Convert sandbox config
        if let Some(sandbox) = settings.sandbox {
            converted.sandbox.enabled = sandbox.enabled.unwrap_or(true);
            converted.sandbox.allowed_directories = sandbox.allowed_directories.unwrap_or_default();
            converted.sandbox.denied_directories = sandbox.denied_directories.unwrap_or_default();
            converted.sandbox.network = sandbox.network;
        }

        // Convert hooks (store for later processing)
        converted.hooks = settings.hooks;

        Ok(converted)
    }
}

/// Converted configuration structure
#[derive(Debug, Clone, Default)]
pub struct ConvertedConfig {
    pub permissions: ConvertedPermissions,
    pub mcp_servers: Vec<ConvertedMcpServer>,
    pub env: HashMap<String, String>,
    pub sandbox: ConvertedSandbox,
    pub hooks: Option<HooksConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct ConvertedPermissions {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
    pub default_mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConvertedMcpServer {
    pub name: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub url: Option<String>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct ConvertedSandbox {
    pub enabled: bool,
    pub allowed_directories: Vec<String>,
    pub denied_directories: Vec<String>,
    pub network: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_settings() {
        let json = r#"{}"#;
        let settings: ClaudeCodeSettings = serde_json::from_str(json).unwrap();
        assert!(settings.permissions.is_none());
        assert!(settings.mcp_servers.is_none());
    }

    #[test]
    fn test_parse_permissions() {
        let json = r#"{
            "permissions": {
                "allow": ["Bash(npm *)", "Read(*.rs)"],
                "deny": ["Bash(rm -rf *)"],
                "ask": ["Write(*)"]
            }
        }"#;
        let settings: ClaudeCodeSettings = serde_json::from_str(json).unwrap();
        let permissions = settings.permissions.unwrap();
        assert_eq!(permissions.allow.unwrap().len(), 2);
        assert_eq!(permissions.deny.unwrap().len(), 1);
    }

    #[test]
    fn test_parse_mcp_servers() {
        let json = r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
                }
            }
        }"#;
        let settings: ClaudeCodeSettings = serde_json::from_str(json).unwrap();
        let mcp = settings.mcp_servers.unwrap();
        assert!(mcp.contains_key("filesystem"));
    }

    #[test]
    fn test_converter_permissions() {
        let settings = ClaudeCodeSettings {
            permissions: Some(PermissionsConfig {
                allow: Some(vec!["Bash(npm *)".to_string()]),
                deny: Some(vec!["Bash(rm -rf *)".to_string()]),
                ask: None,
                default_mode: Some("ask".to_string()),
            }),
            ..Default::default()
        };

        let converted = ClaudeCodeToForgeConverter::convert(settings).unwrap();
        assert_eq!(converted.permissions.allow.len(), 1);
        assert_eq!(converted.permissions.deny.len(), 1);
        assert_eq!(converted.permissions.default_mode, Some("ask".to_string()));
    }
}
