use derive_setters::Setters;
use merge::Merge;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ModelId, ProviderId};

/// Configuration for shell command suggestion generation.
///
/// Allows specifying a dedicated provider and model for shell command
/// suggestion generation, instead of using the active agent's provider and
/// model. This is useful when you want to use a cheaper or faster model for
/// simple command suggestions. Both provider and model must be specified
/// together.
#[derive(Debug, Clone, Serialize, Deserialize, Setters, JsonSchema, PartialEq, Merge)]
#[setters(into)]
pub struct SuggestConfig {
    /// Provider ID to use for command suggestion generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub provider: Option<ProviderId>,

    /// Model ID to use for command suggestion generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[merge(strategy = crate::merge::option)]
    pub model: Option<ModelId>,
}
