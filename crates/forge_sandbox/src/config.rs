use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What to do when sandbox wrapping fails or sandbox is unavailable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxFallback {
    /// Block the command (fail-closed).
    #[default]
    Deny,
    /// Run the command without sandbox (fail-open).
    Allow,
}

/// Configuration for the sandbox environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Working directory inside the sandbox.
    pub cwd: PathBuf,

    /// Paths that should be mounted read-only inside the sandbox.
    pub readonly_paths: Vec<PathBuf>,

    /// Paths that should be mounted read-write inside the sandbox.
    pub writable_paths: Vec<PathBuf>,

    /// Whether network access is allowed inside the sandbox.
    pub allow_network: bool,

    /// Whether sandboxing is enabled at all. When false, commands run unsandboxed.
    pub enabled: bool,

    /// Behavior when sandbox wrapping fails or sandbox is unavailable.
    #[serde(default)]
    pub sandbox_fallback: SandboxFallback,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            cwd: PathBuf::from("."),
            readonly_paths: Vec::new(),
            writable_paths: Vec::new(),
            allow_network: false,
            enabled: true,
            sandbox_fallback: SandboxFallback::Deny,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = SandboxConfig::default();
        assert_eq!(config.cwd, PathBuf::from("."));
        assert!(config.readonly_paths.is_empty());
        assert!(config.writable_paths.is_empty());
        assert!(!config.allow_network);
        assert!(config.enabled);
        assert_eq!(config.sandbox_fallback, SandboxFallback::Deny);
    }

    #[test]
    fn config_with_custom_paths_round_trips_through_serde_json() {
        let config = SandboxConfig {
            cwd: PathBuf::from("/home/user/project"),
            readonly_paths: vec![PathBuf::from("/usr/lib"), PathBuf::from("/opt/tools")],
            writable_paths: vec![PathBuf::from("/tmp/output")],
            allow_network: true,
            enabled: false,
            sandbox_fallback: SandboxFallback::Allow,
        };

        let json = serde_json::to_string(&config).expect("serialize should succeed");
        let deserialized: SandboxConfig =
            serde_json::from_str(&json).expect("deserialize should succeed");

        assert_eq!(deserialized.cwd, config.cwd);
        assert_eq!(deserialized.readonly_paths, config.readonly_paths);
        assert_eq!(deserialized.writable_paths, config.writable_paths);
        assert_eq!(deserialized.allow_network, config.allow_network);
        assert_eq!(deserialized.enabled, config.enabled);
    }

    #[test]
    fn config_deserializes_from_json_string() {
        let json = r#"{
            "cwd": "/workspace",
            "readonly_paths": ["/data"],
            "writable_paths": [],
            "allow_network": false,
            "enabled": true
        }"#;

        let config: SandboxConfig = serde_json::from_str(json).expect("should parse");
        assert_eq!(config.cwd, PathBuf::from("/workspace"));
        assert_eq!(config.readonly_paths, vec![PathBuf::from("/data")]);
        assert!(config.writable_paths.is_empty());
        assert!(!config.allow_network);
        assert!(config.enabled);
    }
}
