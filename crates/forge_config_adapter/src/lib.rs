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
