use std::collections::HashMap;
use std::path::Path;

use crate::error::AdapterError;
use crate::normalized::{
    AgentProviderConfig, McpServerConfig, NormalizedConfig, NormalizedPermissions,
};

/// Adapter that reads the legacy Forge configuration.
///
/// This is a **read-only** adapter. It reads from:
///   - `~/.forge/.forge.toml`  — main TOML config (model, provider, agents)
///   - `~/.forge/.mcp.json`    — MCP server definitions (JSON)
///   - `~/.forge/permissions.yaml` — permission settings (YAML, simple key: [values])
///
/// The `project_dir` parameter is ignored; all paths are resolved relative
/// to the user's home directory.
pub struct ForgeLegacyAdapter;

impl ForgeLegacyAdapter {
    /// Returns the legacy forge config directory (`~/forge/` or `~/.forge/`).
    fn config_dir() -> Option<std::path::PathBuf> {
        let home = dirs::home_dir()?;
        let dot_forge = home.join(".forge");
        if dot_forge.is_dir() {
            return Some(dot_forge);
        }
        let forge = home.join("forge");
        if forge.is_dir() {
            return Some(forge);
        }
        None
    }
}

impl crate::ConfigAdapter for ForgeLegacyAdapter {
    fn tool_name(&self) -> &str {
        "forge_legacy"
    }

    fn detect(&self, _project_dir: &Path) -> bool {
        Self::config_dir()
            .map(|dir| dir.join(".forge.toml").is_file())
            .unwrap_or(false)
    }

    fn read(&self, _project_dir: &Path) -> Result<NormalizedConfig, AdapterError> {
        let config_dir = Self::config_dir().ok_or_else(|| {
            AdapterError::Other("Could not determine legacy forge config directory".into())
        })?;

        let mut config = NormalizedConfig::default();

        // 1. Parse .forge.toml
        let toml_path = config_dir.join(".forge.toml");
        if toml_path.is_file() {
            let content = std::fs::read_to_string(&toml_path)
                .map_err(|e| AdapterError::io(&toml_path, e))?;
            parse_forge_toml(&content, &toml_path, &mut config)?;
        }

        // 2. Parse .mcp.json
        let mcp_path = config_dir.join(".mcp.json");
        if mcp_path.is_file() {
            let content = std::fs::read_to_string(&mcp_path)
                .map_err(|e| AdapterError::io(&mcp_path, e))?;
            let parsed: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| AdapterError::json(&mcp_path, e))?;
            if let Some(servers) = parsed.get("mcpServers").and_then(|v| v.as_object()) {
                config.mcp_servers = parse_mcp_servers_json(servers);
            }
        }

        // 3. Parse permissions.yaml (simple line-based parser to avoid a yaml dep)
        let perms_path = config_dir.join("permissions.yaml");
        if perms_path.is_file() {
            let content = std::fs::read_to_string(&perms_path)
                .map_err(|e| AdapterError::io(&perms_path, e))?;
            config.permissions = parse_permissions_yaml(&content);
        }

        Ok(config)
    }

    fn write(&self, _project_dir: &Path, _config: &NormalizedConfig) -> Result<(), AdapterError> {
        Err(AdapterError::ReadOnly("forge_legacy".into()))
    }
}

/// Parse the TOML configuration file into NormalizedConfig fields.
fn parse_forge_toml(
    content: &str,
    path: &Path,
    config: &mut NormalizedConfig,
) -> Result<(), AdapterError> {
    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| AdapterError::toml(path, e.to_string()))?;

    if let Some(model) = doc.get("model").and_then(|v| v.as_str()) {
        config.model = Some(model.to_string());
    }
    if let Some(provider) = doc.get("provider").and_then(|v| v.as_str()) {
        config.provider = Some(provider.to_string());
    }
    if let Some(instructions) = doc.get("custom_instructions").and_then(|v| v.as_str()) {
        config.custom_instructions = Some(instructions.to_string());
    }

    // agents table
    if let Some(agents_table) = doc.get("agents").and_then(|v| v.as_table()) {
        for (name, agent_val) in agents_table {
            let mut agent = AgentProviderConfig::default();
            if let Some(t) = agent_val.as_table() {
                if let Some(m) = t.get("model").and_then(|v| v.as_str()) {
                    agent.model = Some(m.to_string());
                }
                if let Some(p) = t.get("provider").and_then(|v| v.as_str()) {
                    agent.provider = Some(p.to_string());
                }
                if let Some(k) = t.get("api_key_env").and_then(|v| v.as_str()) {
                    agent.api_key_env = Some(k.to_string());
                }
                if let Some(u) = t.get("base_url").and_then(|v| v.as_str()) {
                    agent.base_url = Some(u.to_string());
                }
                if let Some(mt) = t.get("max_tokens").and_then(|v| v.as_integer()) {
                    agent.max_tokens = Some(mt as u32);
                }
            }
            config.agents.insert(name.to_string(), agent);
        }
    }

    Ok(())
}

fn parse_mcp_servers_json(
    servers: &serde_json::Map<String, serde_json::Value>,
) -> HashMap<String, McpServerConfig> {
    let mut map = HashMap::new();
    for (name, val) in servers {
        let command = val
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let args = val
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let env = val
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        map.insert(
            name.clone(),
            McpServerConfig {
                command,
                args,
                env,
            },
        );
    }
    map
}

/// Simple YAML-like parser for permissions.yaml.
///
/// Expected format:
/// ```yaml
/// allowed_read_paths:
///   - /some/path
///   - /another
/// allowed_commands:
///   - git
/// ```
fn parse_permissions_yaml(content: &str) -> NormalizedPermissions {
    let mut perms = NormalizedPermissions::default();
    let mut current_key: Option<&str> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Key line: "some_key:"
        if !trimmed.starts_with('-') && trimmed.ends_with(':') {
            current_key = Some(trimmed.trim_end_matches(':'));
            continue;
        }

        // List item: "  - value"
        if let Some(value) = trimmed.strip_prefix("- ") {
            let value = value.trim().to_string();
            match current_key {
                Some("allowed_read_paths") => perms.allowed_read_paths.push(value),
                Some("allowed_write_paths") => perms.allowed_write_paths.push(value),
                Some("allowed_commands") => perms.allowed_commands.push(value),
                Some("denied_commands") => perms.denied_commands.push(value),
                _ => {}
            }
        }
    }

    perms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigAdapter;

    #[test]
    fn test_parse_permissions_yaml() {
        let yaml = r#"
allowed_read_paths:
  - /home/user/project
  - /tmp
allowed_commands:
  - git
  - cargo
denied_commands:
  - rm
"#;
        let perms = parse_permissions_yaml(yaml);
        assert_eq!(perms.allowed_read_paths, vec!["/home/user/project", "/tmp"]);
        assert_eq!(perms.allowed_commands, vec!["git", "cargo"]);
        assert_eq!(perms.denied_commands, vec!["rm"]);
        assert!(perms.allowed_write_paths.is_empty());
    }

    #[test]
    fn test_parse_permissions_yaml_with_all_fields() {
        let yaml = r#"
allowed_read_paths:
  - /read
allowed_write_paths:
  - /write
allowed_commands:
  - git
denied_commands:
  - rm
"#;
        let perms = parse_permissions_yaml(yaml);
        assert_eq!(perms.allowed_read_paths, vec!["/read"]);
        assert_eq!(perms.allowed_write_paths, vec!["/write"]);
        assert_eq!(perms.allowed_commands, vec!["git"]);
        assert_eq!(perms.denied_commands, vec!["rm"]);
    }

    #[test]
    fn test_parse_permissions_yaml_ignores_comments_and_blanks() {
        let yaml = r#"
# This is a comment
allowed_commands:

  - git
  # inline comment is not stripped but this line starts with #
  - cargo
"#;
        let perms = parse_permissions_yaml(yaml);
        assert_eq!(perms.allowed_commands, vec!["git", "cargo"]);
    }

    #[test]
    fn test_parse_forge_toml_basic() {
        let toml = r#"
model = "claude-sonnet-4-20250514"
provider = "anthropic"
custom_instructions = "Be helpful."
"#;
        let mut config = NormalizedConfig::default();
        parse_forge_toml(toml, Path::new("test.toml"), &mut config).unwrap();
        assert_eq!(config.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(config.provider.as_deref(), Some("anthropic"));
        assert_eq!(config.custom_instructions.as_deref(), Some("Be helpful."));
    }

    #[test]
    fn test_parse_forge_toml_with_agents() {
        let toml = r#"
model = "default-model"

[agents.coder]
model = "gpt-4"
provider = "openai"
api_key_env = "OPENAI_KEY"
base_url = "https://api.openai.com"
max_tokens = 8192

[agents.reviewer]
model = "claude-opus-4-20250514"
provider = "anthropic"
"#;
        let mut config = NormalizedConfig::default();
        parse_forge_toml(toml, Path::new("test.toml"), &mut config).unwrap();
        assert_eq!(config.model.as_deref(), Some("default-model"));
        assert_eq!(config.agents.len(), 2);

        let coder = config.agents.get("coder").unwrap();
        assert_eq!(coder.model.as_deref(), Some("gpt-4"));
        assert_eq!(coder.provider.as_deref(), Some("openai"));
        assert_eq!(coder.api_key_env.as_deref(), Some("OPENAI_KEY"));
        assert_eq!(coder.base_url.as_deref(), Some("https://api.openai.com"));
        assert_eq!(coder.max_tokens, Some(8192));

        let reviewer = config.agents.get("reviewer").unwrap();
        assert_eq!(reviewer.model.as_deref(), Some("claude-opus-4-20250514"));
        assert_eq!(reviewer.provider.as_deref(), Some("anthropic"));
        assert!(reviewer.api_key_env.is_none());
    }

    #[test]
    fn test_parse_forge_toml_invalid_returns_error() {
        let toml = "this is not valid toml [[[";
        let mut config = NormalizedConfig::default();
        let result = parse_forge_toml(toml, Path::new("bad.toml"), &mut config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AdapterError::Toml { .. }),
            "Expected Toml error, got: {err:?}"
        );
    }

    #[test]
    fn test_parse_mcp_servers_json() {
        let json_str = r#"{
            "context7": {
                "command": "npx",
                "args": ["-y", "@context7/mcp"],
                "env": { "TOKEN": "abc" }
            },
            "bare": {
                "command": "node"
            }
        }"#;
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        let servers = parse_mcp_servers_json(parsed.as_object().unwrap());

        assert_eq!(servers.len(), 2);
        let ctx = servers.get("context7").unwrap();
        assert_eq!(ctx.command, "npx");
        assert_eq!(ctx.args, vec!["-y", "@context7/mcp"]);
        assert_eq!(ctx.env.get("TOKEN").map(|s| s.as_str()), Some("abc"));

        let bare = servers.get("bare").unwrap();
        assert_eq!(bare.command, "node");
        assert!(bare.args.is_empty());
        assert!(bare.env.is_empty());
    }

    #[test]
    fn write_returns_read_only_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = NormalizedConfig::default();
        let result = ForgeLegacyAdapter.write(tmp.path(), &config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AdapterError::ReadOnly(ref s) if s == "forge_legacy"),
            "Expected ReadOnly error, got: {err:?}"
        );
    }

    #[test]
    fn tool_name_returns_forge_legacy() {
        assert_eq!(ForgeLegacyAdapter.tool_name(), "forge_legacy");
    }
}
