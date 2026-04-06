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

            // Write settings.json if model/provider present
            if config.model.is_some() || config.provider.is_some() {
                let settings = serde_json::json!({
                    "model": config.model,
                    "provider": config.provider,
                });
                if let Ok(json) = serde_json::to_string_pretty(&settings) {
                    let _ = std::fs::write(forge_dir.join("settings.json"), json);
                }
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
