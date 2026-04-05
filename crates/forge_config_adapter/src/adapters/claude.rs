use std::collections::HashMap;
use std::path::Path;

use crate::error::AdapterError;
use crate::normalized::{
    McpServerConfig, NormalizedConfig, NormalizedPermissions, RuleFile,
};

/// Adapter that reads Claude Code configuration from `.claude/` project directory.
///
/// Detects:
///   - `.claude/` directory existence
///
/// Reads:
///   - `.claude/settings.json` — model, provider, permissions
///   - `CLAUDE.md` — custom instructions
///   - `.claude/.mcp.json` or `.mcp.json` — MCP server definitions
///   - `.claude/rules/` — rule files
pub struct ClaudeAdapter;

impl crate::ConfigAdapter for ClaudeAdapter {
    fn tool_name(&self) -> &str {
        "claude"
    }

    fn detect(&self, project_dir: &Path) -> bool {
        project_dir.join(".claude").is_dir()
    }

    fn read(&self, project_dir: &Path) -> Result<NormalizedConfig, AdapterError> {
        let mut config = NormalizedConfig::default();

        // 1. Parse .claude/settings.json
        let settings_path = project_dir.join(".claude").join("settings.json");
        if settings_path.is_file() {
            let content = std::fs::read_to_string(&settings_path)
                .map_err(|e| AdapterError::io(&settings_path, e))?;
            let parsed: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| AdapterError::json(&settings_path, e))?;

            if let Some(model) = parsed.get("model").and_then(|v| v.as_str()) {
                config.model = Some(model.to_string());
            }
            if let Some(provider) = parsed.get("provider").and_then(|v| v.as_str()) {
                config.provider = Some(provider.to_string());
            }

            // permissions
            if let Some(perms) = parsed.get("permissions") {
                config.permissions = parse_claude_permissions(perms);
            }
        }

        // 2. Read CLAUDE.md for custom instructions
        let claude_md = project_dir.join("CLAUDE.md");
        if claude_md.is_file() {
            let content = std::fs::read_to_string(&claude_md)
                .map_err(|e| AdapterError::io(&claude_md, e))?;
            config.custom_instructions = Some(content);
        }

        // 3. MCP servers — try .claude/.mcp.json first, fall back to .mcp.json
        let mcp_path = {
            let inner = project_dir.join(".claude").join(".mcp.json");
            if inner.is_file() {
                Some(inner)
            } else {
                let outer = project_dir.join(".mcp.json");
                if outer.is_file() {
                    Some(outer)
                } else {
                    None
                }
            }
        };
        if let Some(mcp_path) = mcp_path {
            let content = std::fs::read_to_string(&mcp_path)
                .map_err(|e| AdapterError::io(&mcp_path, e))?;
            let parsed: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| AdapterError::json(&mcp_path, e))?;
            if let Some(servers) = parsed.get("mcpServers").and_then(|v| v.as_object()) {
                config.mcp_servers = parse_mcp_servers(servers);
            }
        }

        // 4. Rules directory
        let rules_dir = project_dir.join(".claude").join("rules");
        if rules_dir.is_dir() {
            config.rules = read_rules_dir(&rules_dir)?;
        }

        Ok(config)
    }

    fn write(&self, project_dir: &Path, config: &NormalizedConfig) -> Result<(), AdapterError> {
        let claude_dir = project_dir.join(".claude");
        std::fs::create_dir_all(&claude_dir)
            .map_err(|e| AdapterError::io(&claude_dir, e))?;

        // Write settings.json
        let settings_path = claude_dir.join("settings.json");
        let mut settings = serde_json::Map::new();
        if let Some(ref model) = config.model {
            settings.insert("model".into(), serde_json::Value::String(model.clone()));
        }
        if let Some(ref provider) = config.provider {
            settings.insert(
                "provider".into(),
                serde_json::Value::String(provider.clone()),
            );
        }
        let settings_json = serde_json::to_string_pretty(&settings)
            .map_err(|e| AdapterError::json(&settings_path, e))?;
        std::fs::write(&settings_path, settings_json)
            .map_err(|e| AdapterError::io(&settings_path, e))?;

        // Write CLAUDE.md if custom instructions present
        if let Some(ref instructions) = config.custom_instructions {
            let claude_md = project_dir.join("CLAUDE.md");
            std::fs::write(&claude_md, instructions)
                .map_err(|e| AdapterError::io(&claude_md, e))?;
        }

        // Write MCP servers
        if !config.mcp_servers.is_empty() {
            let mcp_path = claude_dir.join(".mcp.json");
            let mcp_obj = serde_json::json!({ "mcpServers": &config.mcp_servers });
            let mcp_json = serde_json::to_string_pretty(&mcp_obj)
                .map_err(|e| AdapterError::json(&mcp_path, e))?;
            std::fs::write(&mcp_path, mcp_json)
                .map_err(|e| AdapterError::io(&mcp_path, e))?;
        }

        // Write rules
        if !config.rules.is_empty() {
            let rules_dir = claude_dir.join("rules");
            std::fs::create_dir_all(&rules_dir)
                .map_err(|e| AdapterError::io(&rules_dir, e))?;
            for rule in &config.rules {
                let rule_path = rules_dir.join(&rule.path);
                if let Some(parent) = rule_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AdapterError::io(parent, e))?;
                }
                std::fs::write(&rule_path, &rule.content)
                    .map_err(|e| AdapterError::io(&rule_path, e))?;
            }
        }

        tracing::info!("Wrote Claude config to {}", claude_dir.display());
        Ok(())
    }
}

fn parse_claude_permissions(value: &serde_json::Value) -> NormalizedPermissions {
    let mut perms = NormalizedPermissions::default();
    if let Some(arr) = value.get("allowedReadPaths").and_then(|v| v.as_array()) {
        perms.allowed_read_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }
    if let Some(arr) = value.get("allowedWritePaths").and_then(|v| v.as_array()) {
        perms.allowed_write_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }
    if let Some(arr) = value.get("allowedCommands").and_then(|v| v.as_array()) {
        perms.allowed_commands = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }
    if let Some(arr) = value.get("deniedCommands").and_then(|v| v.as_array()) {
        perms.denied_commands = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }
    perms
}

fn parse_mcp_servers(
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

fn read_rules_dir(rules_dir: &Path) -> Result<Vec<RuleFile>, AdapterError> {
    let mut rules = Vec::new();
    let entries = std::fs::read_dir(rules_dir).map_err(|e| AdapterError::io(rules_dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| AdapterError::io(rules_dir, e))?;
        let path = entry.path();
        if path.is_file() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| AdapterError::io(&path, e))?;
            let relative = path
                .strip_prefix(rules_dir)
                .unwrap_or(&path)
                .to_path_buf();
            rules.push(RuleFile {
                path: relative,
                content,
            });
        }
    }
    // Sort for deterministic ordering.
    rules.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(rules)
}
