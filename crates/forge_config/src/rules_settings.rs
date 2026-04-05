use fake::Dummy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Settings for the rules/ directory system.
///
/// Rules are markdown files in `~/.forge/rules/` and `<project>/.forge/rules/`
/// that provide mandatory instructions for the agent. Files support YAML
/// frontmatter with `globs` for path-scoped rules.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Dummy)]
pub struct RulesSettings {
    /// Whether to automatically load rules from `rules/` directories.
    #[serde(default = "default_true")]
    pub auto_load: bool,

    /// Enforcement mode: `"strict"` wraps rules in XML tags for higher
    /// compliance; `"normal"` injects rules as plain text.
    #[serde(default = "default_enforce_mode")]
    pub enforce_mode: EnforceMode,
}

/// How strongly rules are enforced in the system prompt.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Dummy)]
#[serde(rename_all = "snake_case")]
pub enum EnforceMode {
    /// Rules are injected as plain text in the system prompt.
    #[default]
    Normal,
    /// Rules are wrapped in `<forge-rules priority="mandatory">` XML tags
    /// for stronger adherence.
    Strict,
}

fn default_true() -> bool {
    true
}

fn default_enforce_mode() -> EnforceMode {
    EnforceMode::Normal
}
