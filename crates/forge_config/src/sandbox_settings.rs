use std::path::PathBuf;

use fake::Dummy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
}

fn default_true() -> bool {
    true
}
