use derive_setters::Setters;
use schemars::JsonSchema;
use schemars::Schema;
use serde::{Deserialize, Serialize};

use crate::ToolName;

///
/// /// Capabilities that describe what a tool can do
/// These help the system make intelligent decisions about scheduling and execution
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Setters)]
#[setters(into, strip_option)]
pub struct ToolCapabilities {
    /// Whether the tool supports streaming output
    /// When true, the tool can yield partial results before completion
    #[serde(default)]
    pub streamable: bool,
    /// Whether the tool can be called in parallel with other tools
    /// Parallel-safe tools don't have shared state dependencies
    #[serde(default = "default_true")]
    pub parallel_calls: bool,
    /// Estimated execution time in milliseconds (for scheduling decisions)
    /// None means unknown/variable duration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_duration_ms: Option<u64>,
    /// Whether the tool is idempotent (safe to retry)
    /// Idempotent tools produce the same result regardless of how many times they're called
    #[serde(default = "default_true")]
    pub idempotent: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ToolCapabilities {
    fn default() -> Self {
        Self {
            streamable: false,
            parallel_calls: true,
            estimated_duration_ms: None,
            idempotent: true,
        }
    }
}

///
/// /// Refer to the specification over here:
/// https://glama.ai/blog/2024-11-25-model-context-protocol-quickstart#server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Setters)]
#[setters(into, strip_option)]
pub struct ToolDefinition {
    pub name: ToolName,
    pub description: String,
    pub input_schema: Schema,
    /// Capabilities that describe what this tool can do
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ToolCapabilities>,
}

impl ToolDefinition {
    /// Create a new ToolDefinition with default capabilities
    pub fn new<N: ToString>(name: N) -> Self {
        ToolDefinition {
            name: ToolName::new(name),
            description: String::new(),
            input_schema: schemars::schema_for!(()), // Empty input schema
            capabilities: None,
        }
    }
}

pub trait ToolDescription {
    fn description(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_tool_capabilities_default() {
        let caps = ToolCapabilities::default();
        assert!(!caps.streamable);
        assert!(caps.parallel_calls); // default_true
        assert!(caps.idempotent); // default_true
        assert_eq!(caps.estimated_duration_ms, None);
    }

    #[test]
    fn test_tool_capabilities_with_setters() {
        let caps = ToolCapabilities {
            streamable: true,
            parallel_calls: false,
            estimated_duration_ms: Some(5000u64),
            idempotent: false,
        };

        assert!(caps.streamable);
        assert!(!caps.parallel_calls);
        assert!(!caps.idempotent);
        assert_eq!(caps.estimated_duration_ms, Some(5000u64));
    }

    #[test]
    fn test_tool_definition_with_capabilities() {
        let def = ToolDefinition {
            name: ToolName::new("test_tool"),
            description: "A test tool".to_string(),
            input_schema: schemars::schema_for!(()),
            capabilities: Some(
                ToolCapabilities::default()
                    .streamable(true)
                    .parallel_calls(true),
            ),
        };

        assert_eq!(def.name.to_string(), "test_tool");
        assert_eq!(def.description, "A test tool");
        assert!(def.capabilities.is_some());
    }
}
