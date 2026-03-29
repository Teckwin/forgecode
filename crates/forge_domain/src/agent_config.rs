use derive_setters::Setters;
use merge::Merge;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ModelId, ProviderId};

/// Configuration for a specific agent.
/// Allows specifying provider, model, and optional URL/key for dedicated agent usage.
#[derive(Default, Debug, Clone, Serialize, Deserialize, Merge, Setters, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[setters(strip_option, into)]
pub struct AgentConfig {
    /// Provider ID to use for this agent.
    /// If not specified, falls back to global default provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub provider: Option<ProviderId>,

    /// Model ID to use for this agent.
    /// If not specified, falls back to provider's default model or global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub model: Option<ModelId>,

    /// Custom API URL for this agent (overrides provider's default URL).
    /// Useful for self-hosted models or proxy endpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub url: Option<String>,

    /// Custom API key for this agent (overrides provider's default auth).
    /// Use with caution - prefer using provider authentication instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub api_key: Option<String>,

    /// Temperature setting for this agent.
    /// Valid range: 0.0 - 2.0
    /// Lower values (e.g., 0.1) make responses more focused and deterministic.
    /// Higher values (e.g., 0.8) make responses more creative and diverse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub temperature: Option<f64>,

    /// Maximum tokens to generate for this agent.
    /// Valid range: 1 - 100000
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub max_tokens: Option<u32>,

    /// Top-p (nucleus sampling) for this agent.
    /// Valid range: 0.0 - 1.0
    /// Lower values make responses more focused, higher values make them more diverse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub top_p: Option<f64>,

    /// Top-k for this agent.
    /// Valid range: 1 - 1000
    /// Controls the number of highest probability tokens to consider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub top_k: Option<u32>,

    /// Explicitly disable tools for this agent.
    /// When true, tools are explicitly disabled (empty list).
    /// When false, falls back to agent definition or workflow default.
    /// Use this instead of `tools` field which represents available tools list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub tools_disabled: Option<bool>,

    /// Reasoning configuration for this agent.
    /// Enable reasoning for models that support it (e.g., o1, o3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub reasoning: Option<bool>,
}

///
/// Agent type identifiers for configuration.
/// These correspond to the three core agents in the system.
///
/// Note: The serde rename allows using "forge" in config files while
/// mapping to AgentType::Default internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    /// Default agent (forge) - used for general conversation and tasks
    /// Can be referenced as "default" or "forge" in config files
    #[serde(rename = "forge")]
    Default,
    /// Commit agent - used for generating commit messages
    Commit,
    /// Suggest agent - used for shell command suggestions
    Suggest,
}
impl std::str::FromStr for AgentType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "default" | "forge" => Ok(AgentType::Default),
            "commit" => Ok(AgentType::Commit),
            "suggest" => Ok(AgentType::Suggest),
            "sage" => Ok(AgentType::Default),  // sage uses default config
            "muse" => Ok(AgentType::Default),  // muse uses default config
            _ => anyhow::bail!("Unknown agent type: {}", s),
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Default => write!(f, "default"),
            AgentType::Commit => write!(f, "commit"),
            AgentType::Suggest => write!(f, "suggest"),
        }
    }
}