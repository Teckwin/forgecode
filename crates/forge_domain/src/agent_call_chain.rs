use std::sync::Arc;

use dashmap::DashSet;

use crate::AgentId;

/// AgentCallChain tracks the call stack of nested Agent tool invocations
/// to detect and prevent circular calls.
#[derive(Debug, Clone)]
pub struct AgentCallChain {
    /// Set of AgentIds currently in the call chain
    /// Using DashSet for thread-safe O(1) operations
    agents: Arc<DashSet<String>>,
}

impl AgentCallChain {
    /// Creates a new empty AgentCallChain
    pub fn new() -> Self {
        Self { agents: Arc::new(DashSet::new()) }
    }

    /// Returns true if the call chain is empty
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Returns the number of agents in the call chain
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Checks if an agent is currently in the call chain
    pub fn contains(&self, agent_id: &AgentId) -> bool {
        self.agents.contains(agent_id.as_str())
    }

    /// Adds an agent to the call chain
    /// Returns true if the agent was not already in the chain
    pub fn push(&self, agent_id: &AgentId) -> bool {
        self.agents.insert(agent_id.as_str().to_string())
    }

    /// Removes an agent from the call chain
    /// Returns true if the agent was in the chain and was removed
    pub fn pop(&self, agent_id: &AgentId) -> bool {
        self.agents.remove(agent_id.as_str()).is_some()
    }

    /// Creates a scope that automatically manages agent lifecycle
    /// The agent is added on creation and removed when the scope is dropped
    pub fn scope(&self, agent_id: AgentId) -> AgentCallScope<'_> {
        self.push(&agent_id);
        AgentCallScope { chain: self, agent_id }
    }

    /// Returns a string representation of the current call chain
    pub fn get_chain_str(&self) -> String {
        self.agents.iter().map(|s| s.clone()).collect::<Vec<_>>().join(" -> ")
    }
}

impl Default for AgentCallChain {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII scope for automatic agent lifecycle management
pub struct AgentCallScope<'a> {
    chain: &'a AgentCallChain,
    agent_id: AgentId,
}

impl Drop for AgentCallScope<'_> {
    fn drop(&mut self) {
        self.chain.pop(&self.agent_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_new_chain_is_empty() {
        let chain = AgentCallChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn test_push_adds_agent_to_chain() {
        let chain = AgentCallChain::new();
        let agent_id = AgentId::new("test-agent");

        let added = chain.push(&agent_id);

        assert!(added);
        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_push_returns_false_for_duplicate_agent() {
        let chain = AgentCallChain::new();
        let agent_id = AgentId::new("test-agent");

        chain.push(&agent_id);
        let added_again = chain.push(&agent_id);

        assert!(!added_again);
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_contains_returns_true_for_existing_agent() {
        let chain = AgentCallChain::new();
        let agent_id = AgentId::new("test-agent");

        chain.push(&agent_id);

        assert!(chain.contains(&agent_id));
    }

    #[test]
    fn test_contains_returns_false_for_nonexistent_agent() {
        let chain = AgentCallChain::new();
        let agent_id = AgentId::new("test-agent");

        assert!(!chain.contains(&agent_id));
    }

    #[test]
    fn test_pop_removes_agent_from_chain() {
        let chain = AgentCallChain::new();
        let agent_id = AgentId::new("test-agent");

        chain.push(&agent_id);
        let removed = chain.pop(&agent_id);

        assert!(removed);
        assert!(chain.is_empty());
    }

    #[test]
    fn test_pop_returns_false_for_nonexistent_agent() {
        let chain = AgentCallChain::new();
        let agent_id = AgentId::new("test-agent");

        let removed = chain.pop(&agent_id);

        assert!(!removed);
    }

    #[test]
    fn test_scope_automatically_manages_agent_lifecycle() {
        let chain = AgentCallChain::new();
        let agent_id = AgentId::new("test-agent");

        {
            let _scope = chain.scope(agent_id.clone());
            assert!(chain.contains(&agent_id));
        }

        // After scope is dropped, agent should be removed
        assert!(!chain.contains(&agent_id));
    }

    #[test]
    fn test_multiple_agents_in_chain() {
        let chain = AgentCallChain::new();
        let agent_a = AgentId::new("agent-a");
        let agent_b = AgentId::new("agent-b");
        let agent_c = AgentId::new("agent-c");

        chain.push(&agent_a);
        chain.push(&agent_b);
        chain.push(&agent_c);

        assert_eq!(chain.len(), 3);
        assert!(chain.contains(&agent_a));
        assert!(chain.contains(&agent_b));
        assert!(chain.contains(&agent_c));
    }

    #[test]
    fn test_nested_scope_tracking() {
        let chain = AgentCallChain::new();
        let agent_a = AgentId::new("agent-a");
        let agent_b = AgentId::new("agent-b");

        {
            let _scope_a = chain.scope(agent_a.clone());
            assert!(chain.contains(&agent_a));

            {
                let _scope_b = chain.scope(agent_b.clone());
                assert!(chain.contains(&agent_a));
                assert!(chain.contains(&agent_b));
            }

            // agent_b should be removed, but agent_a remains
            assert!(chain.contains(&agent_a));
            assert!(!chain.contains(&agent_b));
        }

        // Both should be removed
        assert!(!chain.contains(&agent_a));
        assert!(!chain.contains(&agent_b));
    }

    #[test]
    fn test_detect_circular_call() {
        let chain = AgentCallChain::new();
        let agent_a = AgentId::new("agent-a");
        let agent_b = AgentId::new("agent-b");

        // Simulate: Agent A calls Agent B
        chain.push(&agent_a);
        chain.push(&agent_b);

        // Check if adding agent_a again would create circular call
        // (agent_a is already in chain, calling agent_a again = circular)
        let would_be_circular = chain.contains(&agent_a);

        assert!(would_be_circular);
    }
}
