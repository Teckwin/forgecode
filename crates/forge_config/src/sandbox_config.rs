//! Sandbox configuration for tool execution security

use derive_setters::Setters;
use fake::Dummy;
use fake::rand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const fn default_true() -> bool {
    true
}

const fn default_false() -> bool {
    false
}

/// Default permission mode for sandbox
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    /// Allow all commands by default, block dangerous ones
    #[default]
    Blacklist,
    /// Deny all commands by default, allow explicitly listed ones
    Whitelist,
    /// Allow commands but prompt for confirmation on sensitive ones
    Greylist,
}

impl<F: fake::Fake> fake::Dummy<F> for PermissionMode {
    fn dummy(_: &F) -> Self {
        // Randomly select a permission mode
        use fake::Fake;
        let idx: usize = (0..3).fake();
        match idx {
            0 => PermissionMode::Blacklist,
            1 => PermissionMode::Whitelist,
            _ => PermissionMode::Greylist,
        }
    }

    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &F, _: &mut R) -> Self {
        Self::dummy(&fake::Faker)
    }
}

/// Sandbox security configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Setters, PartialEq, Dummy, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SandboxConfig {
    /// Whether sandbox is enabled globally
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Permission mode for command execution
    /// - Blacklist: Allow all by default, block dangerous commands
    /// - Whitelist: Deny all by default, only allow explicitly listed commands
    /// - Greylist: Allow by default, prompt for sensitive commands
    #[serde(default)]
    pub permission_mode: PermissionMode,

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
