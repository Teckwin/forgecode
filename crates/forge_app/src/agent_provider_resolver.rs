use std::sync::Arc;

use anyhow::Result;
use forge_domain::{ApiKey, AuthCredential, AgentId, ModelId, Provider};

use crate::{AgentRegistry, AppConfigService, ProviderAuthService, ProviderService};

/// Resolver for agent providers and models.
/// Handles provider resolution, credential refresh, and model lookup.
pub struct AgentProviderResolver<S>(Arc<S>);

impl<S> AgentProviderResolver<S> {
    /// Creates a new AgentProviderResolver instance
    pub fn new(services: Arc<S>) -> Self {
        Self(services)
    }
}

impl<S> AgentProviderResolver<S>
where
    S: AgentRegistry + ProviderService + AppConfigService + ProviderAuthService,
{
    /// Gets the provider for the specified agent, or the default provider if no
    /// agent is provided. Automatically refreshes OAuth credentials if they're
    /// about to expire.
    ///
    /// If the agent has custom_api_key or custom_url configured, these will be
    /// applied to the provider to override the default credentials.
    pub async fn get_provider(&self, agent_id: Option<AgentId>) -> Result<Provider<url::Url>> {
        let (provider_id, custom_url, custom_api_key) = if let Some(agent_id) = agent_id {
            // Load all agent definitions and find the one we need

            if let Some(agent) = self.0.get_agent(&agent_id).await? {
                // If the agent definition has a provider, use it; otherwise use default
                // Also extract custom_url and custom_api_key from agent config
                (agent.provider.clone(), agent.custom_url.clone(), agent.custom_api_key.clone())
            } else {
                // TODO: Needs review, should we throw an err here?
                // we can throw crate::Error::AgentNotFound
                (self.0.get_default_provider().await?, None, None)
            }
        } else {
            (self.0.get_default_provider().await?, None, None)
        };

        let mut provider = self.0.get_provider(provider_id).await?;

        // Apply custom URL if specified in agent config
        if let Some(url_str) = custom_url {
            if let Ok(url) = url_str.parse() {
                provider.url = url;
            }
        }

        // Apply custom API key if specified in agent config
        // This overrides any stored credential for this agent
        if let Some(api_key_str) = custom_api_key {
            // For API key auth, directly create a credential and set it on the provider
            // This bypasses the credential store check
            let api_key = ApiKey::from(api_key_str);
            provider.credential = Some(AuthCredential::new_api_key(provider.id.clone(), api_key));
        }

        Ok(provider)
    }

    /// Gets the model for the specified agent, or the default model if no agent
    /// is provided
    pub async fn get_model(&self, agent_id: Option<AgentId>) -> Result<ModelId> {
        if let Some(agent_id) = agent_id {
            if let Some(agent) = self.0.get_agent(&agent_id).await? {
                Ok(agent.model)
            } else {
                // TODO: Needs review, should we throw an err here?
                // we can throw crate::Error::AgentNotFound
                let provider_id = self.get_provider(Some(agent_id)).await?.id;
                Ok(self.0.get_provider_model(Some(&provider_id)).await?)
            }
        } else {
            let provider_id = self.get_provider(None).await?.id;
            Ok(self.0.get_provider_model(Some(&provider_id)).await?)
        }
    }
}
