use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use dashmap::DashMap;
use forge_app::AgentRepository;
use forge_app::domain::AgentId;
use forge_domain::{
    Agent, AppConfigRepository, MaxTokens, ProviderRepository, ReasoningConfig, Temperature, TopK,
    TopP,
};
use tokio::fs;
use tokio::sync::RwLock;

/// Writes agent creation debug info to logs directory (only in debug builds)
#[cfg(debug_assertions)]
async fn write_agent_debug_log(
    agent_name: &str,
    agent_id: &AgentId,
    provider_id: &str,
    model_id: &str,
    config_json: &str,
) {
    // Get current working directory from environment variable or use default
    let cwd = std::env::var("FORGE_CWD")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    let logs_dir = cwd.join("logs");

    // Create logs directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&logs_dir).await {
        tracing::warn!(error = %e, "Failed to create logs directory");
        return;
    }

    // Generate timestamp for filename
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("agent_{}_{}.log", agent_name, timestamp);
    let log_path = logs_dir.join(&filename);

    // Format the log content
    let log_content = format!(
        "=== Agent Creation Debug Log ===\n\
         Timestamp: {}\n\
         \n\
         === Agent Name ===\n\
         {}\n\
         \n\
         === Agent ID ===\n\
         {}\n\
         \n\
         === Provider ID ===\n\
         {}\n\
         \n\
         === Model ID ===\n\
         {}\n\
         \n\
         === Configuration ===\n\
         {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        agent_name,
        agent_id,
        provider_id,
        model_id,
        config_json
    );

    // Write to file
    match fs::write(&log_path, log_content).await {
        Ok(_) => {
            tracing::debug!(path = %log_path.display(), "Agent debug log written");
        }
        Err(e) => {
            tracing::warn!(error = %e, path = %log_path.display(), "Failed to write agent debug log");
        }
    }
}

/// No-op implementation for release builds
#[cfg(not(debug_assertions))]
async fn write_agent_debug_log(
    _agent_name: &str,
    _agent_id: &AgentId,
    _provider_id: &str,
    _model_id: &str,
    _config_json: &str,
) {
    // No-op in release builds
}

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

impl<R: AgentRepository + AppConfigRepository + ProviderRepository> ForgeAgentRegistryService<R> {
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

    /// Load agents from repository
    async fn load_agents(&self) -> anyhow::Result<DashMap<String, Agent>> {
        // Load agent definitions from repository
        let agent_defs = self.repository.get_agents().await?;

        // Get default provider and model from app config
        let app_config = self.repository.get_app_config().await?;
        
        eprintln!("DEBUG: app_config.agents = {:?}", app_config.agents);
        
        tracing::debug!(app_config = ?app_config, "Loaded app config");

        let default_provider_id = app_config
            .provider
            .ok_or(forge_domain::Error::NoDefaultProvider)?;
        
        tracing::debug!(default_provider_id = %default_provider_id, "Default provider ID");
        
        let default_provider = self.repository.get_provider(default_provider_id).await?;
        let default_model = app_config
            .model
            .clone()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No default model configured. Please set `model` in your config file."
                )
            })?;

        tracing::debug!(default_model = %default_model, "Default model");

        // Create the agents map
        let agents_map = DashMap::new();

        // Convert definitions to runtime agents and populate map
        for def in agent_defs {
            let agent_id = def.id.as_str();
            eprintln!("DEBUG: Processing agent with id: {}", agent_id);
            // Use agent id directly as key to lookup config
            let agent_config = app_config.agents.get(agent_id);
            eprintln!("DEBUG: Agent id: {}, agents in config: {:?}", agent_id, app_config.agents.keys().collect::<Vec<_>>());

            // Determine which provider to use for this agent
            // If agent config specifies a provider, use that; otherwise use default
            let provider_id = agent_config
                .as_ref()
                .and_then(|c| c.provider.clone())
                .unwrap_or_else(|| default_provider.id.clone());

            // Extract url and api_key from agent config if specified
            // These will be used at runtime to override provider defaults
            let effective_url = agent_config
                .as_ref()
                .and_then(|c| c.url.clone());
            let effective_api_key = agent_config
                .as_ref()
                .and_then(|c| c.api_key.clone());

            // Use agent-specific config if available, otherwise use global defaults
            let (model_id, temperature, max_tokens, top_p, top_k, tools_disabled, reasoning) = if let Some(config) = agent_config {
                tracing::debug!(
                    agent_id = %agent_id,
                    agent_config = ?config,
                    "Using agent-specific config"
                );
                (
                    config.model.clone().unwrap_or_else(|| default_model.clone()),
                    config.temperature,
                    config.max_tokens,
                    config.top_p,
                    config.top_k,
                    config.tools_disabled,
                    config.reasoning,
                )
            } else {
                (
                    default_model.clone(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            };

            tracing::debug!(
                agent_id = %agent_id,
                provider_id = %provider_id,
                model_id = %model_id,
                url = ?effective_url,
                has_api_key = effective_api_key.is_some(),
                temperature = ?temperature,
                max_tokens = ?max_tokens,
                top_p = ?top_p,
                top_k = ?top_k,
                tools_disabled = ?tools_disabled,
                reasoning = ?reasoning,
                "Creating agent with provider and model"
            );

            let mut agent = Agent::from_agent_def(def, provider_id.clone(), model_id.clone());

            // Store custom URL and API key in agent if specified
            // Save whether they exist before consuming them
            let has_custom_url = effective_url.is_some();
            let has_custom_api_key = effective_api_key.is_some();

            if let Some(url) = effective_url {
                agent.custom_url = Some(url);
            }
            if let Some(api_key) = effective_api_key {
                agent.custom_api_key = Some(api_key);
            }

            // Apply additional agent config parameters
            if let Some(temperature) = temperature {
                // Convert f64 from config to Temperature (f32)
                agent.temperature = Temperature::new(temperature as f32).ok();
            }
            if let Some(max_tokens) = max_tokens {
                agent.max_tokens = MaxTokens::new(max_tokens).ok();
            }
            if let Some(top_p) = top_p {
                agent.top_p = TopP::new(top_p as f32).ok();
            }
            if let Some(top_k) = top_k {
                agent.top_k = TopK::new(top_k).ok();
            }
            if let Some(tools_disabled) = tools_disabled {
                // If tools_disabled is explicitly set to true, disable tools
                if tools_disabled {
                    agent.tools = Some(vec![]);
                }
            }
            if let Some(reasoning) = reasoning {
                // If reasoning is explicitly set to true, enable with default config
                if reasoning {
                    agent.reasoning = Some(ReasoningConfig::default());
                }
            }

            // Write agent creation debug log (only in debug builds)
            let agent_name = agent.title.clone().unwrap_or_else(|| agent.id.to_string());
            
            // Store has_url and has_api_key before consuming effective_url/effective_api_key
            let url_for_log = agent.custom_url.clone();
            let has_custom_url = agent.custom_url.is_some();
            let _has_custom_api_key = agent.custom_api_key.is_some();
            
            // Create config JSON
            let config_json = serde_json::json!({
                "provider_id": provider_id.to_string(),
                "model_id": model_id.to_string(),
                "temperature": temperature,
                "max_tokens": max_tokens,
                "top_p": top_p,
                "top_k": top_k,
                "tools_disabled": tools_disabled,
                "reasoning": reasoning,
                "has_custom_url": has_custom_url,
                "has_custom_api_key": _has_custom_api_key,
                "url": agent.custom_url.clone(),
            })
            .to_string();
            
            // Log detailed configuration info for debugging
            tracing::debug!(
                agent_name = %agent_name,
                agent_id = %agent.id,
                provider_id = %provider_id,
                model_id = %model_id,
                temperature = ?temperature,
                max_tokens = ?max_tokens,
                top_p = ?top_p,
                top_k = ?top_k,
                tools_disabled = ?tools_disabled,
                reasoning = ?reasoning,
                url = ?url_for_log,
                "Agent creation debug log details"
            );

            write_agent_debug_log(
                &agent_name,
                &agent.id,
                &provider_id.to_string(),
                &model_id.to_string(),
                &config_json,
            )
            .await;

            agents_map.insert(agent.id.as_str().to_string(), agent);
        }

        Ok(agents_map)
    }
}

#[async_trait::async_trait]
impl<R: AgentRepository + AppConfigRepository + ProviderRepository> forge_app::AgentRegistry
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
