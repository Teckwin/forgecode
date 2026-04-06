use std::path::PathBuf;

use fake::Dummy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What to do when sandbox wrapping fails or sandbox is unavailable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Dummy)]
#[serde(rename_all = "snake_case")]
pub enum SandboxFallback {
    /// Block the command (fail-closed). This is the secure default.
    #[default]
    Deny,
    /// Run the command without sandbox (fail-open). Use only when you
    /// understand the security implications.
    Allow,
}

/// Settings for OS-level sandbox enforcement on tool execution.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Dummy)]
pub struct SandboxSettings {
    /// Whether sandbox enforcement is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Whether network access is allowed inside the sandbox.
    #[serde(default = "default_true")]
    pub allow_network: bool,

    /// Additional paths the sandbox may write to (beyond `cwd`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_paths: Vec<PathBuf>,

    /// Additional paths the sandbox may read (beyond system defaults).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readonly_paths: Vec<PathBuf>,

    /// Behavior when sandbox wrapping fails or sandbox is unavailable.
    /// `deny` (default) blocks the command; `allow` runs it unsandboxed.
    #[serde(default)]
    pub sandbox_fallback: SandboxFallback,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sandbox_fallback_is_deny() {
        assert_eq!(
            SandboxSettings::default().sandbox_fallback,
            SandboxFallback::Deny
        );
    }

    #[test]
    fn sandbox_fallback_round_trips_through_json() {
        let settings = SandboxSettings {
            enabled: true,
            sandbox_fallback: SandboxFallback::Allow,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: SandboxSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sandbox_fallback, SandboxFallback::Allow);
    }

    #[test]
    fn sandbox_fallback_deny_from_json() {
        let json = r#"{"enabled": true, "sandbox_fallback": "deny"}"#;
        let parsed: SandboxSettings = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.sandbox_fallback, SandboxFallback::Deny);
    }
}
