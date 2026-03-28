use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use forge_app::{
    agent_config_service::{AgentConfigService as Trait, AgentConfigMerger},
    AgentRepository, EnvironmentInfra,
};
use forge_domain::{Agent, AgentId, ProviderRepository};

/// ForgeAgentConfigService - Agent配置服务实现
/// 负责使用AgentConfigMerger合并配置
pub struct ForgeAgentConfigService<F> {
    infra: Arc<F>,
    cache: RwLock<Option<Vec<Agent>>>,
}

impl<F> ForgeAgentConfigService<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self {
            infra,
            cache: RwLock::new(None),
        }
    }
}

#[async_trait]
impl<F: AgentRepository + EnvironmentInfra + ProviderRepository + Send + Sync> Trait
    for ForgeAgentConfigService<F>
{
    async fn get_agent_config(&self, agent_id: &AgentId) -> anyhow::Result<Option<Agent>> {
        // Use the agent repository to get the agent definition and merge with config
        let agent_defs = self.infra.get_agents().await?;
        
        // Find the agent definition
        let agent_def = agent_defs
            .into_iter()
            .find(|def| def.id == *agent_id);
        
        if let Some(def) = agent_def {
            // Get environment for config merging
            let env = self.infra.get_environment();
            let merger = AgentConfigMerger::new(env);
            Ok(Some(merger.merge(def)))
        } else {
            Ok(None)
        }
    }

    async fn get_all_agent_configs(&self) -> anyhow::Result<Vec<Agent>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref agents) = *cache {
                return Ok(agents.clone());
            }
        }

        // Get agent definitions and merge with config
        let agent_defs = self.infra.get_agents().await?;
        
        // Get environment for config merging
        let env = self.infra.get_environment();
        let merger = AgentConfigMerger::new(env);
        
        let agents: Vec<Agent> = agent_defs
            .into_iter()
            .map(|def| merger.merge(def))
            .collect();

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(agents.clone());
        }

        Ok(agents)
    }

    async fn reload_configs(&self) -> anyhow::Result<()> {
        // Clear cache
        {
            let mut cache = self.cache.write().await;
            *cache = None;
        }

        Ok(())
    }
}