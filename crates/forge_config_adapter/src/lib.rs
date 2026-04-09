//! # forge_config_adapter
//!
//! A tool-agnostic configuration adapter layer that reads, normalizes, and
//! writes configuration for multiple coding-assistant tools (Claude Code,
//! Cursor, legacy Forge, etc.).
//!
//! Each tool has a corresponding [`ConfigAdapter`] implementation that can
//! detect, read, and (where supported) write configuration in that tool's
//! native format. All adapters produce and consume a shared
//! [`NormalizedConfig`] type.

pub mod adapters;
pub mod error;
pub mod migration;
pub mod normalized;

pub use adapters::{ClaudeAdapter, CursorAdapter, ForgeLegacyAdapter};
pub use error::AdapterError;
pub use migration::{MigrationAction, MigrationPlan, execute_migration, plan_migration};
pub use normalized::{
    AgentProviderConfig, McpServerConfig, NormalizedConfig, NormalizedPermissions, RuleFile,
};

use std::path::Path;

fn normalized_agent_to_json(agent: &AgentProviderConfig) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    if let Some(ref provider) = agent.provider {
        obj.insert(
            "provider".into(),
            serde_json::Value::String(provider.clone()),
        );
    }
    if let Some(ref model) = agent.model {
        obj.insert("model".into(), serde_json::Value::String(model.clone()));
    }
    if let Some(ref api_key_env) = agent.api_key_env {
        obj.insert(
            "api_key".into(),
            serde_json::Value::String(api_key_env.clone()),
        );
    }
    if let Some(ref base_url) = agent.base_url {
        obj.insert(
            "base_url".into(),
            serde_json::Value::String(base_url.clone()),
        );
    }
    if let Some(max_tokens) = agent.max_tokens {
        obj.insert(
            "parameters".into(),
            serde_json::json!({"max_tokens": max_tokens}),
        );
    }
    serde_json::Value::Object(obj)
}

pub fn normalized_config_to_settings_json(config: &NormalizedConfig) -> serde_json::Value {
    let mut settings = serde_json::Map::new();
    if config.model.is_some() || config.provider.is_some() {
        let mut session = serde_json::Map::new();
        if let Some(ref provider) = config.provider {
            session.insert(
                "provider_id".into(),
                serde_json::Value::String(provider.clone()),
            );
        }
        if let Some(ref model) = config.model {
            session.insert("model_id".into(), serde_json::Value::String(model.clone()));
        }
        settings.insert("session".into(), serde_json::Value::Object(session));
    }
    if !config.agents.is_empty() {
        let agents = config
            .agents
            .iter()
            .map(|(name, agent)| (name.clone(), normalized_agent_to_json(agent)))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        settings.insert("agents".into(), serde_json::Value::Object(agents));
    }
    serde_json::Value::Object(settings)
}

/// Trait implemented by each tool-specific configuration adapter.
pub trait ConfigAdapter {
    /// Returns a human-readable tool name (e.g. `"claude"`, `"cursor"`).
    fn tool_name(&self) -> &str;

    /// Returns `true` if this adapter's configuration is detected in
    /// `project_dir`.
    fn detect(&self, project_dir: &Path) -> bool;

    /// Read the tool's native configuration from `project_dir` and normalize it.
    fn read(&self, project_dir: &Path) -> Result<NormalizedConfig, AdapterError>;

    /// Write a normalized configuration to `project_dir` in the tool's native
    /// format. Returns [`AdapterError::ReadOnly`] for read-only adapters.
    fn write(&self, project_dir: &Path, config: &NormalizedConfig) -> Result<(), AdapterError>;
}

/// Returns all built-in adapters.
pub fn all_adapters() -> Vec<Box<dyn ConfigAdapter>> {
    vec![
        Box::new(ClaudeAdapter),
        Box::new(CursorAdapter),
        Box::new(ForgeLegacyAdapter),
    ]
}

/// Detect which adapters match for the given project directory.
pub fn detect_adapters(project_dir: &Path) -> Vec<Box<dyn ConfigAdapter>> {
    all_adapters()
        .into_iter()
        .filter(|a| a.detect(project_dir))
        .collect()
}

/// Auto-detect and migrate external tool configs (e.g. `.claude/`) to forge format.
///
/// Called at startup. If `.forge/` already exists (forge config is present),
/// this is a no-op. Otherwise, if a supported tool config is detected
/// (e.g. `.claude/`), it auto-migrates to `.forge/` layout.
///
/// Returns `true` if a migration was performed.
pub fn try_auto_migrate(project_dir: &Path) -> bool {
    let forge_dir = project_dir.join(".forge");

    // If .forge/ already exists, no migration needed
    if forge_dir.is_dir() {
        return false;
    }

    // Detect external tool configs (skip forge_legacy — that's for ~/forge → ~/.forge)
    let adapters = detect_adapters(project_dir);
    let source = adapters
        .iter()
        .find(|a| a.tool_name() != "forge_legacy" && a.tool_name() != "cursor");

    let Some(source) = source else {
        return false;
    };

    tracing::info!(
        tool = source.tool_name(),
        "Detected {} config — auto-migrating to .forge/",
        source.tool_name()
    );

    // Create a "forge" destination adapter that writes to .forge/ layout
    // For now, directly read the source and write key files
    match source.read(project_dir) {
        Ok(config) => {
            // Create .forge/ directory
            if let Err(e) = std::fs::create_dir_all(&forge_dir) {
                tracing::warn!(error = ?e, "Failed to create .forge/ directory");
                return false;
            }

            // Write settings.json with session + agents shape
            if (config.model.is_some() || config.provider.is_some() || !config.agents.is_empty())
                && let Ok(json) =
                    serde_json::to_string_pretty(&normalized_config_to_settings_json(&config))
            {
                let _ = std::fs::write(forge_dir.join("settings.json"), json);
            }

            // Write FORGE.md from custom instructions
            if let Some(ref instructions) = config.custom_instructions {
                let _ = std::fs::write(project_dir.join("FORGE.md"), instructions);
            }

            // Copy MCP servers
            if !config.mcp_servers.is_empty() {
                let mcp = serde_json::json!({ "mcpServers": &config.mcp_servers });
                if let Ok(json) = serde_json::to_string_pretty(&mcp) {
                    let _ = std::fs::write(forge_dir.join(".mcp.json"), json);
                }
            }

            // Copy rules
            if !config.rules.is_empty() {
                let rules_dir = forge_dir.join("rules");
                let _ = std::fs::create_dir_all(&rules_dir);
                for rule in &config.rules {
                    let _ = std::fs::write(rules_dir.join(&rule.path), &rule.content);
                }
            }

            tracing::info!(
                "Migrated {} config to .forge/ — review and commit as needed",
                source.tool_name()
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                error = ?e,
                "Failed to read {} config for migration",
                source.tool_name()
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_adapters_returns_expected_count() {
        assert_eq!(all_adapters().len(), 3);
    }

    #[test]
    fn test_try_auto_migrate_writes_session_and_agents_settings() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
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

        assert!(try_auto_migrate(tmp.path()));

        let settings_path = tmp.path().join(".forge/settings.json");
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(settings_path).unwrap()).unwrap();
        assert_eq!(parsed["session"]["provider_id"], "anthropic");
        assert_eq!(parsed["session"]["model_id"], "claude-sonnet-4-20250514");
        assert_eq!(parsed["agents"]["sage"]["provider"], "openai");
        assert_eq!(parsed["agents"]["sage"]["model"], "gpt-4o");
        assert_eq!(parsed["agents"]["sage"]["api_key"], "${OPENAI_API_KEY}");
        assert_eq!(
            parsed["agents"]["sage"]["base_url"],
            "https://example.com/v1"
        );
        assert_eq!(parsed["agents"]["sage"]["parameters"]["max_tokens"], 2048);
    }

    #[test]
    fn test_detect_adapters_with_claude_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".claude")).unwrap();

        let detected = detect_adapters(tmp.path());

        // ClaudeAdapter should detect the .claude directory.
        // ForgeLegacyAdapter may also detect if ~/.forge exists on the host.
        let claude_adapters: Vec<_> = detected
            .iter()
            .filter(|a| a.tool_name() == "claude")
            .collect();
        assert_eq!(
            claude_adapters.len(),
            1,
            "Expected exactly one claude adapter"
        );
    }

    #[test]
    fn test_detect_adapters_empty_project() {
        let tmp = tempfile::TempDir::new().unwrap();

        let detected = detect_adapters(tmp.path());

        // ForgeLegacyAdapter may or may not detect depending on the host,
        // but an empty temp dir should not trigger ClaudeAdapter or CursorAdapter.
        // Filter out forge_legacy since it depends on the host's ~/.forge.
        let non_legacy: Vec<_> = detected
            .iter()
            .filter(|a| a.tool_name() != "forge_legacy")
            .collect();
        assert!(
            non_legacy.is_empty(),
            "Expected no non-legacy adapters for empty project, got: {:?}",
            non_legacy.iter().map(|a| a.tool_name()).collect::<Vec<_>>()
        );
    }
}
