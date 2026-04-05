use fake::Dummy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Permission rules controlling what operations the agent may perform.
///
/// Modelled after Claude Code's `permissions` section in `settings.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Dummy)]
pub struct PermissionSettings {
    /// Command patterns that are automatically allowed (e.g. `"git *"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,

    /// Command patterns that require user confirmation before execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ask: Vec<String>,

    /// Command patterns that are always denied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,

    /// Path patterns where writes are allowed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_write: Vec<String>,

    /// Path patterns where writes are denied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_write: Vec<String>,

    /// Path patterns where reads are allowed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_read: Vec<String>,

    /// Path patterns where reads are denied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_read: Vec<String>,
}
