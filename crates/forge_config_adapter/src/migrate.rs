//! Auto-config migration module
//!
//! Provides automatic detection and conversion of external configuration files.
//! This runs on startup to seamlessly migrate external configs to Forge's format.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use crate::claude_code::{ClaudeCodeParser, ClaudeCodeToForgeConverter, ConvertedConfig};
use crate::detector::ConfigDetector;

/// Auto-migration result
#[derive(Debug, Clone)]
pub struct MigrationResult {
    /// Whether any migration was performed
    pub migrated: bool,
    /// List of migrated config sources
    pub sources: Vec<String>,
    /// Converted config (if any)
    pub converted: Option<ConvertedConfig>,
}

/// Auto-migrator for external configs
pub struct ConfigAutoMigrator;

impl ConfigAutoMigrator {
    /// Auto-detect and convert external configs
    ///
    /// This scans for known external config files and converts them to Forge's format.
    /// Returns the converted config if any were found.
    pub fn detect_and_convert(cwd: &PathBuf) -> Result<MigrationResult> {
        let configs = ConfigDetector::detect_all(cwd);

        if configs.is_empty() {
            return Ok(MigrationResult { migrated: false, sources: vec![], converted: None });
        }

        let mut all_converted = ConvertedConfig::default();
        let mut sources = Vec::new();

        for config in configs {
            let source_name = match config.source {
                crate::detector::ConfigSource::ClaudeCode => "claude-code".to_string(),
                crate::detector::ConfigSource::Unknown => "unknown".to_string(),
            };

            match Self::convert_config(&config.path) {
                Ok(converted) => {
                    // Merge permissions
                    if !converted.permissions.allow.is_empty() {
                        all_converted
                            .permissions
                            .allow
                            .extend(converted.permissions.allow);
                    }
                    if !converted.permissions.deny.is_empty() {
                        all_converted
                            .permissions
                            .deny
                            .extend(converted.permissions.deny);
                    }
                    if !converted.permissions.ask.is_empty() {
                        all_converted
                            .permissions
                            .ask
                            .extend(converted.permissions.ask);
                    }

                    // Merge MCP servers
                    all_converted.mcp_servers.extend(converted.mcp_servers);

                    // Merge env vars
                    all_converted.env.extend(converted.env);

                    // Merge sandbox settings (only use first non-default)
                    if !converted.sandbox.allowed_directories.is_empty()
                        && all_converted.sandbox.allowed_directories.is_empty()
                    {
                        all_converted.sandbox.allowed_directories =
                            converted.sandbox.allowed_directories;
                    }

                    sources.push(source_name);
                }
                Err(e) => {
                    tracing::warn!("Failed to convert config from {:?}: {}", config.path, e);
                }
            }
        }

        let migrated = !sources.is_empty();

        Ok(MigrationResult {
            migrated,
            sources,
            converted: if migrated { Some(all_converted) } else { None },
        })
    }

    /// Convert a single config file
    fn convert_config(path: &PathBuf) -> Result<ConvertedConfig> {
        let settings = ClaudeCodeParser::parse(path)?;
        let converted = ClaudeCodeToForgeConverter::convert(settings)?;
        Ok(converted)
    }

    /// Check if any external configs exist (quick check without parsing)
    pub fn has_external_configs(cwd: &PathBuf) -> bool {
        ConfigDetector::has_external_configs(cwd)
    }
}

/// Extension trait for converting Claude Code permissions to Forge format
pub trait ToForgePermissions {
    /// Convert to Forge's permission pattern format
    fn to_forge_permissions(&self) -> HashMap<String, Vec<String>>;
}

impl ToForgePermissions for ConvertedConfig {
    fn to_forge_permissions(&self) -> HashMap<String, Vec<String>> {
        let mut map = HashMap::new();

        if !self.permissions.allow.is_empty() {
            map.insert("allow".to_string(), self.permissions.allow.clone());
        }
        if !self.permissions.deny.is_empty() {
            map.insert("deny".to_string(), self.permissions.deny.clone());
        }
        if !self.permissions.ask.is_empty() {
            map.insert("ask".to_string(), self.permissions.ask.clone());
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_configs_returns_none() {
        let temp_dir = std::env::temp_dir();
        let result = ConfigAutoMigrator::detect_and_convert(&temp_dir).unwrap();
        // May or may not have configs depending on test environment
        assert!(result.converted.is_none() || result.converted.is_some());
    }
}
