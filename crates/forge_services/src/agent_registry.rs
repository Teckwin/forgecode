use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use forge_app::domain::AgentId;
use forge_app::{AgentRepository, EnvironmentInfra};
use forge_domain::{
    Agent, AgentDefinition, AgentProviderConfig, ApiKey, ModelId, ModelParameters, ProviderConfig,
    ProviderId, ProviderRepository, Temperature, TopK, TopP,
};
use tokio::sync::RwLock;

/// AgentRegistryService manages the active-agent ID and a registry of runtime
/// Agents in-memory. It lazily loads agents from AgentRepository on first
/// access.
pub struct ForgeAgentRegistryService<R> {
    // Infrastructure dependency for loading agent definitions
    repository: Arc<R>,

    // In-memory storage for agents keyed by AgentId string
    // Lazily initialized on first access
    // Wrapped in RwLock to allow invalidation
    agents: RwLock<Option<DashMap<String, Agent>>>,

    // In-memory storage for the active agent ID
    active_agent_id: RwLock<Option<AgentId>>,
}

impl<R> ForgeAgentRegistryService<R> {
    /// Creates a new AgentRegistryService with the given repository
    pub fn new(repository: Arc<R>) -> Self {
        Self {
            repository,
            agents: RwLock::new(None),
            active_agent_id: RwLock::new(None),
        }
    }
}

impl<R: AgentRepository + EnvironmentInfra + ProviderRepository> ForgeAgentRegistryService<R> {
    /// Lazily initializes and returns the agents map
    /// Loads agents from repository on first call, subsequent calls return
    /// cached value
    async fn ensure_agents_loaded(&self) -> anyhow::Result<DashMap<String, Agent>> {
        // Check if already loaded
        {
            let agents_read = self.agents.read().await;
            if let Some(agents) = agents_read.as_ref() {
                return Ok(agents.clone());
            }
        }

        // Not loaded yet, acquire write lock and load
        let mut agents_write = self.agents.write().await;

        // Double-check in case another task loaded while we were waiting for write
        // lock
        if let Some(agents) = agents_write.as_ref() {
            return Ok(agents.clone());
        }

        // Load agents
        let agents_map = self.load_agents().await?;

        // Store and return
        *agents_write = Some(agents_map.clone());
        Ok(agents_map)
    }

    fn merged_provider_config(
        default_provider_id: &ProviderId,
        default_model_id: &ModelId,
        legacy_provider_id: Option<&ProviderId>,
        legacy_model_id: Option<&ModelId>,
        override_config: Option<&AgentProviderConfig>,
        definition_config: Option<ProviderConfig>,
    ) -> ProviderConfig {
        let override_provider_id = override_config
            .and_then(|config| config.provider_id.clone())
            .map(ProviderId::from);
        let override_model_id = override_config
            .and_then(|config| config.model_id.clone())
            .map(ModelId::new);

        let provider = definition_config
            .as_ref()
            .map(|config| config.provider.clone())
            .or(override_provider_id)
            .or_else(|| legacy_provider_id.cloned())
            .unwrap_or_else(|| default_provider_id.clone());
        let model = definition_config
            .as_ref()
            .and_then(|config| config.model.clone())
            .or(override_model_id)
            .or_else(|| legacy_model_id.cloned())
            .or_else(|| Some(default_model_id.clone()));
        let api_key = definition_config
            .as_ref()
            .and_then(|config| config.api_key.clone())
            .or_else(|| {
                override_config
                    .and_then(|config| config.api_key.clone())
                    .map(ApiKey::from)
            });
        let base_url = definition_config
            .as_ref()
            .and_then(|config| config.base_url.clone())
            .or_else(|| override_config.and_then(|config| config.base_url.clone()));
        let parameters = definition_config
            .as_ref()
            .and_then(|config| config.parameters.clone())
            .or_else(|| {
                override_config.and_then(|config| {
                    let temperature = config
                        .temperature
                        .and_then(|value| Temperature::new(value as f32).ok());
                    let top_p = config.top_p.and_then(|value| TopP::new(value as f32).ok());
                    let top_k = config.top_k.and_then(|value| TopK::new(value).ok());
                    let max_tokens = config
                        .max_tokens
                        .and_then(|value| forge_domain::MaxTokens::new(value).ok());

                    if temperature.is_some()
                        || top_p.is_some()
                        || top_k.is_some()
                        || max_tokens.is_some()
                    {
                        Some(ModelParameters {
                            temperature,
                            top_p,
                            top_k,
                            max_tokens,
                            reasoning: None,
                            max_tool_failure_per_turn: None,
                            max_requests_per_turn: None,
                        })
                    } else {
                        None
                    }
                })
            });

        ProviderConfig { provider, model, api_key, base_url, parameters }
    }

    fn apply_agent_settings(
        def: AgentDefinition,
        default_provider_id: &ProviderId,
        default_model_id: &ModelId,
        agent_configs: &HashMap<String, AgentProviderConfig>,
    ) -> AgentDefinition {
        let override_config = agent_configs.get(def.id.as_str());
        if override_config.is_none() {
            return def;
        }

        let mut def = def;
        let legacy_provider_id = def.provider.clone();
        let legacy_model_id = def.model.clone();
        def.provider = None;
        def.model = None;
        def.provider_config = Some(Self::merged_provider_config(
            default_provider_id,
            default_model_id,
            legacy_provider_id.as_ref(),
            legacy_model_id.as_ref(),
            override_config,
            def.provider_config.clone(),
        ));
        def
    }

    /// Load agents from repository
    async fn load_agents(&self) -> anyhow::Result<DashMap<String, Agent>> {
        let agent_defs = self.repository.get_agents().await?;
        let env = self.repository.get_environment();
        let session = env
            .session
            .as_ref()
            .ok_or(forge_domain::Error::NoDefaultProvider)?;
        let default_provider_id = session
            .provider_id
            .as_ref()
            .map(|id| ProviderId::from(id.clone()))
            .ok_or(forge_domain::Error::NoDefaultProvider)?;
        let default_model = session.model_id.as_ref().map(ModelId::new).ok_or_else(|| {
            anyhow::anyhow!(
                "No default model configured for provider {}",
                default_provider_id
            )
        })?;
        let agent_configs = env.agents.unwrap_or_default();

        let agents_map = DashMap::new();

        for def in agent_defs {
            let resolved_def = Self::apply_agent_settings(
                def,
                &default_provider_id,
                &default_model,
                &agent_configs,
            );
            let agent = Agent::from_agent_def(
                resolved_def,
                default_provider_id.clone(),
                default_model.clone(),
            );
            agents_map.insert(agent.id.as_str().to_string(), agent);
        }

        Ok(agents_map)
    }
}

#[async_trait::async_trait]
impl<R: AgentRepository + EnvironmentInfra + ProviderRepository> forge_app::AgentRegistry
    for ForgeAgentRegistryService<R>
{
    async fn get_active_agent_id(&self) -> anyhow::Result<Option<AgentId>> {
        let agent_id = self.active_agent_id.read().await;
        Ok(agent_id.clone())
    }

    async fn set_active_agent_id(&self, agent_id: AgentId) -> anyhow::Result<()> {
        let mut active_agent = self.active_agent_id.write().await;
        *active_agent = Some(agent_id);
        Ok(())
    }

    async fn get_agents(&self) -> anyhow::Result<Vec<Agent>> {
        let agents = self.ensure_agents_loaded().await?;
        Ok(agents.iter().map(|entry| entry.value().clone()).collect())
    }

    async fn get_agent(&self, agent_id: &AgentId) -> anyhow::Result<Option<Agent>> {
        let agents = self.ensure_agents_loaded().await?;
        Ok(agents.get(agent_id.as_str()).map(|v| v.value().clone()))
    }

    async fn reload_agents(&self) -> anyhow::Result<()> {
        *self.agents.write().await = None;

        self.ensure_agents_loaded().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;

    use forge_app::{AgentRegistry, EnvironmentInfra};
    use forge_domain::{
        AgentDefinition, AnyProvider, AuthCredential, Environment, InputModality, Model,
        ModelSource, ProviderResponse, ProviderTemplate,
    };

    use super::*;

    #[derive(Clone)]
    struct MockInfra {
        env: Environment,
        agents: Vec<AgentDefinition>,
        providers: HashMap<ProviderId, ProviderTemplate>,
    }

    impl EnvironmentInfra for MockInfra {
        fn get_env_var(&self, _key: &str) -> Option<String> {
            None
        }

        fn get_env_vars(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }

        fn get_environment(&self) -> Environment {
            self.env.clone()
        }

        async fn update_environment(
            &self,
            _ops: Vec<forge_domain::ConfigOperation>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl AgentRepository for MockInfra {
        async fn get_agents(&self) -> anyhow::Result<Vec<AgentDefinition>> {
            Ok(self.agents.clone())
        }
    }

    #[async_trait::async_trait]
    impl ProviderRepository for MockInfra {
        async fn get_all_providers(&self) -> anyhow::Result<Vec<AnyProvider>> {
            Ok(self
                .providers
                .values()
                .cloned()
                .map(AnyProvider::Template)
                .collect())
        }

        async fn get_provider(&self, id: ProviderId) -> anyhow::Result<ProviderTemplate> {
            self.providers
                .get(&id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("provider not found: {id}"))
        }

        async fn upsert_credential(&self, _credential: AuthCredential) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_credential(&self, _id: &ProviderId) -> anyhow::Result<Option<AuthCredential>> {
            Ok(None)
        }

        async fn remove_credential(&self, _id: &ProviderId) -> anyhow::Result<()> {
            Ok(())
        }

        async fn migrate_env_credentials(
            &self,
        ) -> anyhow::Result<Option<forge_domain::MigrationResult>> {
            Ok(None)
        }
    }

    fn provider_template(id: ProviderId) -> ProviderTemplate {
        ProviderTemplate {
            id,
            provider_type: Default::default(),
            response: Some(ProviderResponse::OpenAI),
            url: forge_domain::Template::new("https://example.com"),
            models: Some(ModelSource::Hardcoded(vec![Model {
                id: "test-model".into(),
                name: Some("Test Model".to_string()),
                description: None,
                context_length: Some(8192),
                tools_supported: Some(true),
                supports_parallel_tool_calls: Some(true),
                supports_reasoning: Some(false),
                input_modalities: vec![InputModality::Text],
            }])),
            auth_methods: vec![],
            url_params: vec![],
            credential: None,
            custom_headers: None,
        }
    }

    fn base_env() -> Environment {
        use fake::{Fake, Faker};

        let mut env: Environment = Faker.fake();
        env.session = Some(
            forge_domain::SessionConfig::default()
                .provider_id("openai".to_string())
                .model_id("gpt-4o".to_string()),
        );
        env.agents = None;
        env
    }

    #[tokio::test]
    async fn uses_session_defaults_when_agent_override_is_partial() {
        let mut env = base_env();
        env.agents = Some(HashMap::from([(
            "sage".to_string(),
            AgentProviderConfig {
                provider_id: Some("anthropic".to_string()),
                model_id: None,
                api_key: None,
                base_url: None,
                temperature: None,
                top_p: None,
                top_k: None,
                max_tokens: None,
            },
        )]));

        let infra = MockInfra {
            env,
            agents: vec![AgentDefinition::new("sage")],
            providers: HashMap::from([
                (ProviderId::OPENAI, provider_template(ProviderId::OPENAI)),
                (
                    ProviderId::ANTHROPIC,
                    provider_template(ProviderId::ANTHROPIC),
                ),
            ]),
        };
        let registry = ForgeAgentRegistryService::new(Arc::new(infra));

        let agent = registry
            .get_agent(&AgentId::new("sage"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(agent.provider, ProviderId::ANTHROPIC);
        assert_eq!(agent.model, ModelId::new("gpt-4o"));
    }

    #[tokio::test]
    async fn preserves_legacy_definition_fields_without_settings_override() {
        let env = base_env();

        let def = AgentDefinition::new("sage")
            .provider(ProviderId::ANTHROPIC)
            .model(ModelId::new("claude-sonnet-4-20250514"));

        let infra = MockInfra {
            env,
            agents: vec![def],
            providers: HashMap::from([
                (ProviderId::OPENAI, provider_template(ProviderId::OPENAI)),
                (
                    ProviderId::ANTHROPIC,
                    provider_template(ProviderId::ANTHROPIC),
                ),
            ]),
        };
        let registry = ForgeAgentRegistryService::new(Arc::new(infra));

        let agent = registry
            .get_agent(&AgentId::new("sage"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(agent.provider, ProviderId::ANTHROPIC);
        assert_eq!(agent.model, ModelId::new("claude-sonnet-4-20250514"));
    }

    #[tokio::test]
    async fn settings_override_legacy_definition_fields() {
        let mut env = base_env();
        env.agents = Some(HashMap::from([(
            "sage".to_string(),
            AgentProviderConfig {
                provider_id: Some("anthropic".to_string()),
                model_id: Some("claude-sonnet-4-20250514".to_string()),
                api_key: None,
                base_url: None,
                temperature: None,
                top_p: None,
                top_k: None,
                max_tokens: None,
            },
        )]));

        let def = AgentDefinition::new("sage")
            .provider(ProviderId::OPENAI)
            .model(ModelId::new("gpt-4o"));

        let infra = MockInfra {
            env,
            agents: vec![def],
            providers: HashMap::from([
                (ProviderId::OPENAI, provider_template(ProviderId::OPENAI)),
                (
                    ProviderId::ANTHROPIC,
                    provider_template(ProviderId::ANTHROPIC),
                ),
            ]),
        };
        let registry = ForgeAgentRegistryService::new(Arc::new(infra));

        let agent = registry
            .get_agent(&AgentId::new("sage"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(agent.provider, ProviderId::ANTHROPIC);
        assert_eq!(agent.model, ModelId::new("claude-sonnet-4-20250514"));
    }

    #[tokio::test]
    async fn prefers_definition_provider_config_over_settings_override() {
        let mut env = base_env();
        env.agents = Some(HashMap::from([(
            "sage".to_string(),
            AgentProviderConfig {
                provider_id: Some("anthropic".to_string()),
                model_id: Some("claude-sonnet-4-20250514".to_string()),
                api_key: None,
                base_url: None,
                temperature: None,
                top_p: None,
                top_k: None,
                max_tokens: None,
            },
        )]));

        let mut def = AgentDefinition::new("sage");
        def.provider_config = Some(ProviderConfig {
            provider: ProviderId::OPENAI,
            model: Some(ModelId::new("gpt-4.1")),
            api_key: None,
            base_url: None,
            parameters: None,
        });

        let infra = MockInfra {
            env,
            agents: vec![def],
            providers: HashMap::from([
                (ProviderId::OPENAI, provider_template(ProviderId::OPENAI)),
                (
                    ProviderId::ANTHROPIC,
                    provider_template(ProviderId::ANTHROPIC),
                ),
            ]),
        };
        let registry = ForgeAgentRegistryService::new(Arc::new(infra));

        let agent = registry
            .get_agent(&AgentId::new("sage"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(agent.provider, ProviderId::OPENAI);
        assert_eq!(agent.model, ModelId::new("gpt-4.1"));
    }
}
