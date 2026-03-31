//! Sandbox configuration for tool execution security

use derive_setters::Setters;
use fake::Dummy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const fn default_true() -> bool {
    true
}

const fn default_false() -> bool {
    false
}

/// Sandbox security configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Setters, PartialEq, Dummy, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SandboxConfig {
    /// Whether sandbox is enabled globally
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Shell command execution configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<ShellSandboxConfig>,

    /// Filesystem access configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<FilesystemSandboxConfig>,

    /// Network access configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkSandboxConfig>,
}

/// Shell command sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Setters, PartialEq, Dummy, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ShellSandboxConfig {
    /// Whether shell sandbox is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// List of allowed commands (if empty, all commands are allowed)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_commands: Vec<String>,

    /// List of blocked commands
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_commands: Vec<String>,

    /// Maximum execution time in seconds
    #[serde(default)]
    pub timeout_secs: u64,

    /// Working directory for command execution (null = current dir)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

/// Filesystem sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Setters, PartialEq, Dummy, Default)]
#[serde(rename_all = "kebab-case")]
pub struct FilesystemSandboxConfig {
    /// Whether filesystem sandbox is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// List of allowed directories (if empty, all directories are allowed)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_directories: Vec<String>,

    /// List of blocked directories
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_directories: Vec<String>,

    /// Whether read operations are allowed
    #[serde(default = "default_true")]
    pub allow_read: bool,

    /// Whether write operations are allowed
    #[serde(default = "default_true")]
    pub allow_write: bool,

    /// Whether delete operations are allowed
    #[serde(default = "default_false")]
    pub allow_delete: bool,
}

/// Network sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Setters, PartialEq, Dummy, Default)]
#[serde(rename_all = "kebab-case")]
pub struct NetworkSandboxConfig {
    /// Whether network sandbox is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// List of allowed domains (if empty, all domains are allowed)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_domains: Vec<String>,

    /// List of blocked domains
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_domains: Vec<String>,

    /// Whether HTTP requests are allowed
    #[serde(default = "default_true")]
    pub allow_http: bool,

    /// Whether HTTPS requests are allowed
    #[serde(default = "default_true")]
    pub allow_https: bool,
}
