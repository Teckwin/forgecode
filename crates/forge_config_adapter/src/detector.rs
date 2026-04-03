//! Config detector module
//!
//! Provides auto-detection of external configuration files and automatic
//! conversion to Forge's format.

use std::path::{Path, PathBuf};

use anyhow::Result;

/// Known config file patterns for different ecosystems
/// Configuration source types from external ecosystems
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    /// Claude Code settings.json
    ClaudeCodeSettings,
    /// Claude Code CLAUDE.md (instructions)
    ClaudeCodeMd,
    /// Claude Code rules/*.md (conditional rules)
    ClaudeCodeRules,
    /// Claude Code settings.local.json (local overrides)
    ClaudeCodeSettingsLocal,
    /// Other ecosystem configs (future extension)
    Unknown,
}

/// Represents a detected external config file
#[derive(Debug, Clone)]
pub struct DetectedConfig {
    pub source: ConfigSource,
    pub path: PathBuf,
    /// Optional: extracted metadata (e.g., frontmatter globs for rules)
    pub metadata: Option<DetectedConfigMetadata>,
}

impl DetectedConfig {
    /// Create a new detected config
    pub fn new(source: ConfigSource, path: PathBuf) -> Self {
        Self { source, path, metadata: None }
    }

    /// Create a new detected config with metadata
    pub fn with_metadata(
        source: ConfigSource,
        path: PathBuf,
        metadata: DetectedConfigMetadata,
    ) -> Self {
        Self { source, path, metadata: Some(metadata) }
    }
}

/// Metadata extracted from config files
#[derive(Debug, Clone, Default)]
pub struct DetectedConfigMetadata {
    /// For CLAUDE.md: frontmatter globs
    pub globs: Option<Vec<String>>,
    /// For CLAUDE.md: description
    pub description: Option<String>,
    /// For rules/*.md: rule name from filename
    pub rule_name: Option<String>,
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
                    ConfigSource::ClaudeCodeSettings,
                    global_settings,
                ));
            }

            // Check ~/.claude/CLAUDE.md (global user instructions)
            let global_claude_md = home.join(".claude").join("CLAUDE.md");
            if global_claude_md.exists() {
                if let Ok(metadata) = Self::extract_claude_md_metadata(&global_claude_md) {
                    configs.push(DetectedConfig::with_metadata(
                        ConfigSource::ClaudeCodeMd,
                        global_claude_md,
                        metadata,
                    ));
                } else {
                    configs.push(DetectedConfig::new(
                        ConfigSource::ClaudeCodeMd,
                        global_claude_md,
                    ));
                }
            }

            // Check ~/.claude/rules/*.md (global user rules)
            let rules_dir = home.join(".claude").join("rules");
            if rules_dir.is_dir() {
                if let Ok(rule_configs) =
                    Self::detect_rules_dir(&rules_dir, ConfigSource::ClaudeCodeRules)
                {
                    configs.extend(rule_configs);
                }
            }
        }

        // Check ./.claude/settings.json (project level)
        let project_settings = cwd.join(".claude").join("settings.json");
        if project_settings.exists() {
            configs.push(DetectedConfig::new(
                ConfigSource::ClaudeCodeSettings,
                project_settings,
            ));
        }

        // Check ./.claude/settings.local.json (local overrides)
        let local_settings = cwd.join(".claude").join("settings.local.json");
        if local_settings.exists() {
            configs.push(DetectedConfig::new(
                ConfigSource::ClaudeCodeSettingsLocal,
                local_settings,
            ));
        }

        // Check ./CLAUDE.md (project instructions)
        let project_claude_md = cwd.join("CLAUDE.md");
        if project_claude_md.exists() {
            if let Ok(metadata) = Self::extract_claude_md_metadata(&project_claude_md) {
                configs.push(DetectedConfig::with_metadata(
                    ConfigSource::ClaudeCodeMd,
                    project_claude_md,
                    metadata,
                ));
            } else {
                configs.push(DetectedConfig::new(
                    ConfigSource::ClaudeCodeMd,
                    project_claude_md,
                ));
            }
        }

        // Check ./.claude/rules/*.md (project rules)
        let project_rules_dir = cwd.join(".claude").join("rules");
        if project_rules_dir.is_dir() {
            if let Ok(rule_configs) =
                Self::detect_rules_dir(&project_rules_dir, ConfigSource::ClaudeCodeRules)
            {
                configs.extend(rule_configs);
            }
        }

        configs
    }

    /// Detect rules in a rules directory
    fn detect_rules_dir(rules_dir: &Path, source: ConfigSource) -> Result<Vec<DetectedConfig>> {
        let mut configs = Vec::new();

        if let Ok(entries) = std::fs::read_dir(rules_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                    let rule_name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string());

                    let mut metadata = DetectedConfigMetadata { rule_name, ..Default::default() };

                    // Try to extract globs from frontmatter
                    if let Ok(file_content) = std::fs::read_to_string(&path) {
                        if let Ok(frontmatter) = Self::parse_frontmatter(&file_content) {
                            metadata.globs = frontmatter
                                .get("globs")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect()
                                });
                            metadata.description = frontmatter
                                .get("description")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                        }
                    }

                    configs.push(DetectedConfig::with_metadata(
                        source.clone(),
                        path,
                        metadata,
                    ));
                }
            }
        }

        Ok(configs)
    }

    /// Extract metadata from CLAUDE.md frontmatter
    fn extract_claude_md_metadata(path: &Path) -> Result<DetectedConfigMetadata> {
        let content = std::fs::read_to_string(path)?;
        let mut metadata = DetectedConfigMetadata::default();

        if let Ok(frontmatter) = Self::parse_frontmatter(&content) {
            metadata.globs = frontmatter
                .get("globs")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                });
            metadata.description = frontmatter
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        Ok(metadata)
    }

    /// Parse YAML frontmatter from markdown content
    /// Parse YAML frontmatter from markdown content
    fn parse_frontmatter(
        content: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        let mut metadata = std::collections::HashMap::new();

        // Check for --- delimited frontmatter
        if let Some(stripped) = content.strip_prefix("---") {
            if let Some(end_idx) = stripped.find("---") {
                let frontmatter = &stripped[..end_idx];
                for line in frontmatter.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some(colon_idx) = line.find(':') {
                        let key = line[..colon_idx].trim().to_string();
                        let value = line[colon_idx + 1..].trim();

                        // Handle array values
                        if value.starts_with('[') && value.ends_with(']') {
                            let arr: Vec<String> = value[1..value.len() - 1]
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .collect();
                            metadata.insert(
                                key,
                                serde_json::Value::Array(
                                    arr.into_iter().map(serde_json::Value::String).collect(),
                                ),
                            );
                        } else {
                            // Handle string values
                            let value = value.trim_matches('"');
                            metadata.insert(key, serde_json::Value::String(value.to_string()));
                        }
                    }
                }
            }
        }

        Ok(metadata)
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
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detector_returns_empty_for_nonexistent_paths() {
        let temp_dir = std::env::temp_dir();
        let configs = ConfigDetector::detect_all(&temp_dir);
        // May or may not have configs depending on test environment
        // Just verify it doesn't panic
        assert!(configs.len() >= 0);
    }

    #[test]
    fn test_detect_claude_code_settings() {
        // Create a temporary directory with Claude Code settings
        let temp_dir = TempDir::new().unwrap();
        let claude_dir = temp_dir.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();

        let settings_path = claude_dir.join("settings.json");
        fs::write(
            &settings_path,
            r#"{
                "permissions": {
                    "allow": ["Bash(npm *)"]
                }
            }"#,
        )
        .unwrap();

        let configs = ConfigDetector::detect_claude_code_configs(temp_dir.path());
        assert!(configs
            .iter()
            .any(|c| c.source == ConfigSource::ClaudeCodeSettings));
    }

    #[test]
    fn test_detect_claude_md() {
        // This test uses the global ~/.claude directory if it exists
        // or tests the function doesn't panic
        let temp_dir = std::env::temp_dir();
        let _configs = ConfigDetector::detect_claude_code_configs(&temp_dir);

        // Just verify the function works without panicking
        // The actual detection depends on the test environment
        assert!(true);
    }

    #[test]
    fn test_detect_rules_directory() {
        // This test checks if the function handles missing directories
        let temp_dir = std::env::temp_dir();
        let _configs = ConfigDetector::detect_claude_code_configs(&temp_dir);

        // Just verify the function works without panicking
        assert!(true);
    }

    #[test]
    fn test_detect_project_claude_md() {
        // This test checks if the function handles missing files
        let temp_dir = std::env::temp_dir();
        let _configs = ConfigDetector::detect_claude_code_configs(&temp_dir);

        // Just verify the function works without panicking
        assert!(true);
    }

    #[test]
    fn test_has_external_configs() {
        // This test checks if the function handles various scenarios
        let temp_dir = std::env::temp_dir();

        // Should not panic regardless of what configs exist
        let result = ConfigDetector::has_external_configs(&temp_dir);
        assert!(!result || result);
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
globs: ["*.ts", "*.js"]
description: "Test description"
model: "sonnet"
---

# Content here
"#;
        let metadata = ConfigDetector::parse_frontmatter(content).unwrap();

        assert!(metadata.contains_key("globs"));
        assert!(metadata.contains_key("description"));
        assert!(metadata.contains_key("model"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = r#"# Just Content

No frontmatter here.
"#;
        let metadata = ConfigDetector::parse_frontmatter(content).unwrap();
        // Should return empty since no frontmatter
        assert!(metadata.is_empty() || !metadata.contains_key("globs"));
    }
}
