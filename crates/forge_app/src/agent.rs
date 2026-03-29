use std::sync::Arc;

use forge_domain::{
    Agent, ChatCompletionMessage, Context, Conversation, ModelId, ProviderId, ResultStream,
    ToolCallContext, ToolCallFull, ToolResult,
};

use crate::services::{AgentRegistry, AppConfigService};
use crate::tool_registry::ToolRegistry;
use crate::{ConversationService, ProviderService, Services};

/// Agent service trait that provides core chat and tool call functionality.
/// This trait abstracts the essential operations needed by the Orchestrator.
#[async_trait::async_trait]
pub trait AgentService: Send + Sync + 'static {
    /// Execute a chat completion request
    async fn chat_agent(
        &self,
        id: &ModelId,
        context: Context,
        provider_id: Option<ProviderId>,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error>;

    /// Execute a tool call
    async fn call(
        &self,
        agent: &Agent,
        context: &ToolCallContext,
        call: ToolCallFull,
    ) -> ToolResult;

    /// Synchronize the on-going conversation
    async fn update(&self, conversation: Conversation) -> anyhow::Result<()>;
}

/// Blanket implementation of AgentService for any type that implements Services
#[async_trait::async_trait]
impl<T: Services> AgentService for T {
    async fn chat_agent(
        &self,
        id: &ModelId,
        context: Context,
        provider_id: Option<ProviderId>,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
        // Use the provided provider_id, or get from agent config by matching model_id, or fallback to default
        let provider_id = if let Some(provider_id) = provider_id {
            provider_id
        } else if let Ok(agents) = self.get_agents().await {
            // Find the agent that uses this model and use its provider
            if let Some(agent) = agents.into_iter().find(|a| &a.model == id) {
                tracing::debug!(
                    agent_id = %agent.id,
                    model_id = %id,
                    provider_id = %agent.provider,
                    "Resolved provider from agent config"
                );
                agent.provider
            } else {
                self.get_default_provider().await?
            }
        } else {
            self.get_default_provider().await?
        };
        let provider = self.get_provider(provider_id).await?;

        self.chat(id, context, provider).await
    }

    async fn call(
        &self,
        agent: &Agent,
        context: &ToolCallContext,
        call: ToolCallFull,
    ) -> ToolResult {
        let registry = ToolRegistry::new(Arc::new(self.clone()));
        registry.call(agent, context, call).await
    }

    async fn update(&self, conversation: Conversation) -> anyhow::Result<()> {
        self.upsert_conversation(conversation).await
    }
}
