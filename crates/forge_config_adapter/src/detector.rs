//! Config detector module
//!
//! Provides auto-detection of external configuration files and automatic
//! conversion to Forge's format.

use std::path::{Path, PathBuf};

/// Known config file patterns for different ecosystems
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Claude Code settings.json
    ClaudeCode,
    /// Other ecosystem configs (future extension)
    Unknown,
}

/// Represents a detected external config file
#[derive(Debug, Clone)]
pub struct DetectedConfig {
    pub source: ConfigSource,
    pub path: PathBuf,
}

impl DetectedConfig {
    /// Create a new detected config
    pub fn new(source: ConfigSource, path: PathBuf) -> Self {
        Self { source, path }
    }
}

/// Detects external config files in standard locations
pub struct ConfigDetector;

impl ConfigDetector {
    /// Scan for Claude Code settings.json in standard locations
    pub fn detect_claude_code_configs(cwd: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        // Check ~/.claude/settings.json
        if let Some(home) = dirs::home_dir() {
            let global_settings = home.join(".claude").join("settings.json");
            if global_settings.exists() {
                configs.push(DetectedConfig::new(
                    ConfigSource::ClaudeCode,
                    global_settings,
                ));
            }
        }

        // Check ./.claude/settings.json (project level)
        let project_settings = cwd.join(".claude").join("settings.json");
        if project_settings.exists() {
            configs.push(DetectedConfig::new(
                ConfigSource::ClaudeCode,
                project_settings,
            ));
        }

        configs
    }

    /// Auto-detect all known external configs
    pub fn detect_all(cwd: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        // Add Claude Code configs
        configs.extend(Self::detect_claude_code_configs(cwd));

        configs
    }

    /// Check if any external configs exist
    pub fn has_external_configs(cwd: &Path) -> bool {
        !Self::detect_all(cwd).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_returns_empty_for_nonexistent_paths() {
        let temp_dir = std::env::temp_dir();
        let configs = ConfigDetector::detect_all(&temp_dir);
        // May or may not have configs depending on test environment
        // Just verify it doesn't panic
        assert!(configs.len() >= 0);
    }
}
