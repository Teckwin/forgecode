use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            cwd: PathBuf::from("."),
            readonly_paths: Vec::new(),
            writable_paths: Vec::new(),
            allow_network: false,
            enabled: true,
        }
    }
}
