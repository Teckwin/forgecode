#![cfg(test)]
//! Tests for ForgeAgentRegistryService and Agent tool conversion

use std::sync::Arc;

use crate::ForgeAgentRegistryService;
use forge_domain::{Agent, ModelId, ProviderId};
use pretty_assertions::assert_eq;

/// Test that ForgeAgentRegistryService can be instantiated
#[test]
fn test_agent_registry_service_exists() {
    // Verify the struct can be instantiated (just check type exists)
    let _: ForgeAgentRegistryService<()> = ForgeAgentRegistryService::new(Arc::new(()));
}

/// Test creating agent with minimal required fields
#[test]
fn test_agent_creation_minimal() {
    // Arrange & Act: Create agent with required fields
    let agent = Agent::new(
        "test_agent",
        ProviderId::ANTHROPIC,
        ModelId::new("claude-3-5-sonnet"),
    );

    // Assert
    assert_eq!(agent.id.as_str(), "test_agent");
    assert_eq!(agent.provider, ProviderId::ANTHROPIC);
    assert_eq!(agent.model.as_str(), "claude-3-5-sonnet");
}

/// Test agent tool_definition conversion when description is provided
#[test]
fn test_agent_tool_definition_with_description() {
    // Arrange
    let mut agent = Agent::new(
        "test_agent",
        ProviderId::ANTHROPIC,
        ModelId::new("claude-3-5-sonnet"),
    );
    agent.description = Some("A test agent for testing".to_string());

    // Act
    let tool_def = agent.tool_definition();

    // Assert
    assert!(tool_def.is_ok());
    let tool_def = tool_def.unwrap();
    assert_eq!(tool_def.name.as_str(), "test_agent");
    assert_eq!(tool_def.description, "A test agent for testing");
}

/// Test agent tool_definition conversion fails when no description
#[test]
fn test_agent_tool_definition_without_description_fails() {
    // Arrange
    let agent = Agent::new(
        "test_agent",
        ProviderId::ANTHROPIC,
        ModelId::new("claude-3-5-sonnet"),
    );

    // Act
    let tool_def = agent.tool_definition();

    // Assert
    assert!(tool_def.is_err());
}

/// Test agent tool_definition conversion fails when description is empty
#[test]
fn test_agent_tool_definition_with_empty_description_fails() {
    // Arrange
    let mut agent = Agent::new(
        "test_agent",
        ProviderId::ANTHROPIC,
        ModelId::new("claude-3-5-sonnet"),
    );
    agent.description = Some("".to_string());

    // Act
    let tool_def = agent.tool_definition();

    // Assert
    assert!(tool_def.is_err());
}
