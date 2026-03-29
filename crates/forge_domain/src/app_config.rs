use std::collections::HashMap;

use derive_more::From;
use serde::{Deserialize, Serialize};

use crate::{AgentConfig, CommitConfig, ModelId, ProviderId, SuggestConfig};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitAuth {
    pub session_id: String,
    pub auth_url: String,
    pub token: String,
}

#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(merge::Merge)]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub key_info: Option<LoginInfo>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub provider: Option<ProviderId>,

    /// Custom provider URL for non-official implementations.
    /// Useful for self-hosted models, proxy endpoints, or vendor-specific endpoints.
    /// Example: https://api.anthropic.com, https://api.openai.com/v1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub url: Option<String>,

    /// Model configuration (simple string format).
    /// Format: "claude-sonnet-4-20250514"
    /// This is used as the default model for the provider specified in `provider` field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub model: Option<ModelId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub commit: Option<CommitConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub suggest: Option<SuggestConfig>,

    /// Agent-specific configurations.
    /// Allows setting dedicated provider/model/url/key for each core agent.
    /// Keys: "forge", "sage", "muse", "commit", "suggest" or any custom agent id
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[merge(strategy = crate::merge::hashmap)]
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Clone, Serialize, Deserialize, From, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoginInfo {
    pub api_key: String,
    pub api_key_name: String,
    pub api_key_masked: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_provider_id: Option<String>,
}
