use std::collections::HashMap;
use std::path::Path;

use crate::error::AdapterError;
use crate::normalized::{McpServerConfig, NormalizedConfig, NormalizedPermissions, RuleFile};
use crate::normalized_config_to_settings_json;

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

            if let Some(session) = parsed.get("session") {
                if let Some(model) = session.get("model_id").and_then(|v| v.as_str()) {
                    config.model = Some(model.to_string());
                }
                if let Some(provider) = session.get("provider_id").and_then(|v| v.as_str()) {
                    config.provider = Some(provider.to_string());
                }
            }
            if config.model.is_none()
                && let Some(model) = parsed.get("model").and_then(|v| v.as_str())
            {
                config.model = Some(model.to_string());
            }
            if config.provider.is_none()
                && let Some(provider) = parsed.get("provider").and_then(|v| v.as_str())
            {
                config.provider = Some(provider.to_string());
            }
            if let Some(agents) = parsed.get("agents").and_then(|v| v.as_object()) {
                for (name, agent_value) in agents {
                    if let Some(agent_obj) = agent_value.as_object() {
                        let mut agent = crate::normalized::AgentProviderConfig::default();
                        if let Some(provider) = agent_obj.get("provider").and_then(|v| v.as_str()) {
                            agent.provider = Some(provider.to_string());
                        }
                        if let Some(model) = agent_obj.get("model").and_then(|v| v.as_str()) {
                            agent.model = Some(model.to_string());
                        }
                        if let Some(api_key) = agent_obj.get("api_key").and_then(|v| v.as_str()) {
                            agent.api_key_env = Some(api_key.to_string());
                        }
                        if let Some(base_url) = agent_obj.get("base_url").and_then(|v| v.as_str()) {
                            agent.base_url = Some(base_url.to_string());
                        }
                        if let Some(max_tokens) = agent_obj
                            .get("parameters")
                            .and_then(|v| v.get("max_tokens"))
                            .and_then(|v| v.as_u64())
                        {
                            agent.max_tokens = Some(max_tokens as u32);
                        }
                        config.agents.insert(name.clone(), agent);
                    }
                }
            }

            // permissions
            if let Some(perms) = parsed.get("permissions") {
                config.permissions = parse_claude_permissions(perms);
            }
        }

        // 2. Read CLAUDE.md for custom instructions
        let claude_md = project_dir.join("CLAUDE.md");
        if claude_md.is_file() {
            let content =
                std::fs::read_to_string(&claude_md).map_err(|e| AdapterError::io(&claude_md, e))?;
            config.custom_instructions = Some(content);
        }

        // 3. MCP servers — try .claude/.mcp.json first, fall back to .mcp.json
        let mcp_path = {
            let inner = project_dir.join(".claude").join(".mcp.json");
            if inner.is_file() {
                Some(inner)
            } else {
                let outer = project_dir.join(".mcp.json");
                if outer.is_file() { Some(outer) } else { None }
            }
        };
        if let Some(mcp_path) = mcp_path {
            let content =
                std::fs::read_to_string(&mcp_path).map_err(|e| AdapterError::io(&mcp_path, e))?;
            let parsed: serde_json::Value =
                serde_json::from_str(&content).map_err(|e| AdapterError::json(&mcp_path, e))?;
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
        std::fs::create_dir_all(&claude_dir).map_err(|e| AdapterError::io(&claude_dir, e))?;

        // Write settings.json
        let settings_path = claude_dir.join("settings.json");
        let settings_json =
            serde_json::to_string_pretty(&normalized_config_to_settings_json(config))
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
            std::fs::write(&mcp_path, mcp_json).map_err(|e| AdapterError::io(&mcp_path, e))?;
        }

        // Write rules (validate paths don't escape rules directory)
        if !config.rules.is_empty() {
            let rules_dir = claude_dir.join("rules");
            std::fs::create_dir_all(&rules_dir).map_err(|e| AdapterError::io(&rules_dir, e))?;
            let canonical_rules = rules_dir
                .canonicalize()
                .map_err(|e| AdapterError::io(&rules_dir, e))?;
            for rule in &config.rules {
                // Reject paths with `..` components to prevent directory escape
                if rule
                    .path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    return Err(AdapterError::Other(format!(
                        "Rule path contains '..': {}",
                        rule.path.display()
                    )));
                }
                let rule_path = rules_dir.join(&rule.path);
                // Double-check: after join, the resolved path must stay under rules_dir
                if let Some(parent) = rule_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| AdapterError::io(parent, e))?;
                }
                let canonical_rule = rule_path
                    .parent()
                    .and_then(|p| p.canonicalize().ok())
                    .map(|p| p.join(rule_path.file_name().unwrap_or_default()));
                if let Some(ref cr) = canonical_rule
                    && !cr.starts_with(&canonical_rules)
                {
                    return Err(AdapterError::Other(format!(
                        "Rule path escapes rules directory: {}",
                        rule.path.display()
                    )));
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
        // Skip servers with empty command — they'll fail at runtime
        if command.is_empty() {
            tracing::warn!(server = %name, "MCP server has empty command, skipping");
            continue;
        }
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
        map.insert(name.clone(), McpServerConfig { command, args, env });
    }
    map
}

fn read_rules_dir(rules_dir: &Path) -> Result<Vec<RuleFile>, AdapterError> {
    if !rules_dir.exists() {
        return Ok(Vec::new());
    }
    let mut rules = Vec::new();
    let canonical_base = rules_dir
        .canonicalize()
        .map_err(|e| AdapterError::io(rules_dir, e))?;
    let entries = std::fs::read_dir(rules_dir).map_err(|e| AdapterError::io(rules_dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| AdapterError::io(rules_dir, e))?;
        let path = entry.path();
        // Resolve symlinks and verify the file stays within rules_dir (prevent path traversal)
        if let Ok(canonical_path) = path.canonicalize() {
            if !canonical_path.starts_with(&canonical_base) {
                tracing::warn!(
                    "Skipping rules file outside rules directory: {}",
                    path.display()
                );
                continue;
            }
            if canonical_path.is_file() {
                let content = std::fs::read_to_string(&canonical_path)
                    .map_err(|e| AdapterError::io(&path, e))?;
                let relative = path.strip_prefix(rules_dir).unwrap_or(&path).to_path_buf();
                rules.push(RuleFile { path: relative, content });
            }
        }
    }
    // Sort for deterministic ordering.
    rules.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigAdapter;
    use tempfile::TempDir;

    #[test]
    fn detect_returns_true_when_claude_dir_exists() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".claude")).unwrap();
        assert!(ClaudeAdapter.detect(tmp.path()));
    }

    #[test]
    fn detect_returns_false_when_claude_dir_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(!ClaudeAdapter.detect(tmp.path()));
    }

    #[test]
    fn read_parses_session_and_agents_settings_json() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{
                "session": {
                    "provider_id": "anthropic",
                    "model_id": "claude-sonnet-4-20250514"
                },
                "agents": {
                    "sage": {
                        "provider": "openai",
                        "model": "gpt-4o",
                        "api_key": "${OPENAI_API_KEY}",
                        "base_url": "https://example.com/v1",
                        "parameters": { "max_tokens": 2048 }
                    }
                }
            }"#,
        )
        .unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(config.provider.as_deref(), Some("anthropic"));
        assert_eq!(config.model.as_deref(), Some("claude-sonnet-4-20250514"));
        let sage = config.agents.get("sage").unwrap();
        assert_eq!(sage.provider.as_deref(), Some("openai"));
        assert_eq!(sage.model.as_deref(), Some("gpt-4o"));
        assert_eq!(sage.api_key_env.as_deref(), Some("${OPENAI_API_KEY}"));
        assert_eq!(sage.base_url.as_deref(), Some("https://example.com/v1"));
        assert_eq!(sage.max_tokens, Some(2048));
    }

    #[test]
    fn read_parses_valid_settings_json() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{
                "model": "claude-sonnet-4-20250514",
                "provider": "anthropic",
                "permissions": {
                    "allowedReadPaths": ["/tmp"],
                    "allowedCommands": ["git"],
                    "deniedCommands": ["rm"]
                }
            }"#,
        )
        .unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(config.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(config.provider.as_deref(), Some("anthropic"));
        assert_eq!(config.permissions.allowed_read_paths, vec!["/tmp"]);
        assert_eq!(config.permissions.allowed_commands, vec!["git"]);
        assert_eq!(config.permissions.denied_commands, vec!["rm"]);
    }

    #[test]
    fn read_loads_claude_md_as_custom_instructions() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".claude")).unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "Be concise.").unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(config.custom_instructions.as_deref(), Some("Be concise."));
    }

    #[test]
    fn read_discovers_mcp_json_servers() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join(".mcp.json"),
            r#"{
                "mcpServers": {
                    "context7": {
                        "command": "npx",
                        "args": ["-y", "@context7/mcp"],
                        "env": { "TOKEN": "abc" }
                    }
                }
            }"#,
        )
        .unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        let srv = config.mcp_servers.get("context7").unwrap();
        assert_eq!(srv.command, "npx");
        assert_eq!(srv.args, vec!["-y", "@context7/mcp"]);
        assert_eq!(srv.env.get("TOKEN").map(|s| s.as_str()), Some("abc"));
    }

    #[test]
    fn read_falls_back_to_root_mcp_json() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".claude")).unwrap();
        std::fs::write(
            tmp.path().join(".mcp.json"),
            r#"{ "mcpServers": { "srv": { "command": "node" } } }"#,
        )
        .unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.mcp_servers.get("srv").unwrap().command, "node");
    }

    #[test]
    fn read_loads_rules_directory() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join(".claude").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("a_safety.md"), "No deletions.").unwrap();
        std::fs::write(rules_dir.join("b_style.md"), "Use Rust idioms.").unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(config.rules.len(), 2);
        // Rules should be sorted by path
        assert_eq!(config.rules[0].path.to_str().unwrap(), "a_safety.md");
        assert_eq!(config.rules[0].content, "No deletions.");
        assert_eq!(config.rules[1].path.to_str().unwrap(), "b_style.md");
        assert_eq!(config.rules[1].content, "Use Rust idioms.");
    }

    #[test]
    fn read_empty_project_returns_default_config() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".claude")).unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert!(config.model.is_none());
        assert!(config.provider.is_none());
        assert!(config.mcp_servers.is_empty());
        assert!(config.custom_instructions.is_none());
        assert!(config.rules.is_empty());
    }

    #[test]
    fn test_write_creates_files() {
        use crate::normalized::{McpServerConfig, NormalizedConfig, RuleFile};
        use std::collections::HashMap;

        let tmp = TempDir::new().unwrap();
        let config = NormalizedConfig {
            model: Some("claude-sonnet-4-20250514".to_string()),
            provider: Some("anthropic".to_string()),
            agents: HashMap::from([(
                "sage".to_string(),
                crate::normalized::AgentProviderConfig {
                    provider: Some("openai".to_string()),
                    model: Some("gpt-4o".to_string()),
                    api_key_env: Some("${OPENAI_API_KEY}".to_string()),
                    base_url: Some("https://example.com/v1".to_string()),
                    max_tokens: Some(4096),
                },
            )]),
            custom_instructions: Some("Be helpful and concise.".to_string()),
            mcp_servers: HashMap::from([(
                "test-server".to_string(),
                McpServerConfig {
                    command: "node".to_string(),
                    args: vec!["server.js".to_string()],
                    env: HashMap::new(),
                },
            )]),
            rules: vec![RuleFile {
                path: std::path::PathBuf::from("safety.md"),
                content: "No destructive operations.".to_string(),
            }],
            ..Default::default()
        };

        ClaudeAdapter.write(tmp.path(), &config).unwrap();

        // settings.json should exist
        let settings_path = tmp.path().join(".claude").join("settings.json");
        assert!(settings_path.is_file(), "settings.json should be created");
        let settings_content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&settings_content).unwrap();
        assert_eq!(settings["session"]["model_id"], "claude-sonnet-4-20250514");
        assert_eq!(settings["session"]["provider_id"], "anthropic");
        assert_eq!(settings["agents"]["sage"]["provider"], "openai");
        assert_eq!(settings["agents"]["sage"]["model"], "gpt-4o");
        assert_eq!(settings["agents"]["sage"]["api_key"], "${OPENAI_API_KEY}");
        assert_eq!(
            settings["agents"]["sage"]["base_url"],
            "https://example.com/v1"
        );
        assert_eq!(settings["agents"]["sage"]["parameters"]["max_tokens"], 4096);

        // CLAUDE.md should exist
        let claude_md = tmp.path().join("CLAUDE.md");
        assert!(claude_md.is_file(), "CLAUDE.md should be created");
        assert_eq!(
            std::fs::read_to_string(&claude_md).unwrap(),
            "Be helpful and concise."
        );

        // .mcp.json should exist
        let mcp_path = tmp.path().join(".claude").join(".mcp.json");
        assert!(mcp_path.is_file(), ".mcp.json should be created");

        // rules file should exist
        let rule_path = tmp.path().join(".claude").join("rules").join("safety.md");
        assert!(rule_path.is_file(), "rules/safety.md should be created");
        assert_eq!(
            std::fs::read_to_string(&rule_path).unwrap(),
            "No destructive operations."
        );
    }

    #[test]
    fn write_rejects_rule_path_with_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let mut config = NormalizedConfig::default();
        config.rules.push(RuleFile {
            path: std::path::PathBuf::from("../escape.md"),
            content: "malicious".to_string(),
        });

        let result = ClaudeAdapter.write(tmp.path(), &config);
        assert!(result.is_err(), "Rule path with '..' should be rejected");
        let err = result.unwrap_err().to_string();
        assert!(err.contains(".."), "Error should mention '..'");
    }

    #[test]
    fn read_mcp_skips_empty_command() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join(".mcp.json"),
            r#"{
                "mcpServers": {
                    "valid": { "command": "node", "args": [] },
                    "empty_cmd": { "command": "", "args": [] },
                    "no_cmd": { "args": ["--help"] }
                }
            }"#,
        )
        .unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(
            config.mcp_servers.len(),
            1,
            "Only valid server should be included"
        );
        assert!(config.mcp_servers.contains_key("valid"));
    }

    #[test]
    fn read_rules_dir_with_nonexistent_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".claude")).unwrap();
        // No rules/ dir created

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert!(config.rules.is_empty());
    }

    #[test]
    fn read_parses_write_paths() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{ "permissions": { "allowedWritePaths": ["/tmp/output", "/var/data"] } }"#,
        )
        .unwrap();

        let config = ClaudeAdapter.read(tmp.path()).unwrap();
        assert_eq!(
            config.permissions.allowed_write_paths,
            vec!["/tmp/output", "/var/data"]
        );
    }
}
