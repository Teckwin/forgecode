use std::sync::Arc;

use anyhow::bail;
use bytes::Bytes;
use forge_app::{EnvironmentInfra, FileReaderInfra, FileWriterInfra};
use forge_domain::{AppConfig, AppConfigRepository, ModelId, ProviderId};
use merge::Merge;
use tokio::sync::Mutex;

/// Repository for managing application configuration with caching support.
///
/// This repository uses infrastructure traits for file I/O operations and
/// maintains an in-memory cache to reduce file system access. The configuration
/// file path is automatically inferred from the environment.
#[derive(derive_setters::Setters)]
#[setters(into)]
pub struct AppConfigRepositoryImpl<F> {
    infra: Arc<F>,
    cache: Arc<Mutex<Option<AppConfig>>>,
    override_model: Option<ModelId>,
    override_provider: Option<ProviderId>,
}

impl<F> AppConfigRepositoryImpl<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self {
            infra,
            cache: Arc::new(Mutex::new(None)),
            override_model: None,
            override_provider: None,
        }
    }
}

impl<F: EnvironmentInfra + FileReaderInfra + FileWriterInfra> AppConfigRepositoryImpl<F> {
    /// Returns the sample local config content from embedded resource
    #[cfg(feature = "include_sample_config")]
    fn sample_local_config() -> &'static str {
        include_str!("../resources/sample_config.yaml")
    }

    #[cfg(not(feature = "include_sample_config"))]
    fn sample_local_config() -> &'static str {
        ""
    }

    /// Writes sample config file to the local project directory
    pub async fn write_sample_local_config(&self) -> anyhow::Result<bool> {
        let path = self.infra.get_environment().local_config();

        // Check if file already exists
        if self.infra.read_utf8(&path).await.is_ok() {
            return Ok(false);
        }

        let content = Self::sample_local_config();
        if content.is_empty() {
            tracing::warn!("Sample config content is empty, skipping write");
            return Ok(false);
        }

        self.infra.write(&path, Bytes::from(content.to_string())).await?;
        tracing::info!(path = %path.display(), "Created sample local config file");
        Ok(true)
    }

    /// Reads configuration from the JSON file with fallback strategies:
    async fn read(&self) -> AppConfig {
        let path = self.infra.get_environment().app_config();
        let content = match self.infra.read_utf8(&path).await {
            Ok(content) => content,
            Err(e) => {
                tracing::error!(
                    path = %path.display(),
                    error = %e,
                    "Failed to read config file. Using default config."
                );
                return AppConfig::default();
            }
        };

        // Strategy 1: Try normal parsing
        serde_json::from_str::<AppConfig>(&content)
            .or_else(|_| {
                // Strategy 2: Try JSON repair for syntactically broken JSON
                tracing::warn!(path = %path.display(), "Failed to parse config file, attempting repair...");
                forge_json_repair::json_repair::<AppConfig>(&content).inspect(|_| {
                    tracing::info!(path = %path.display(), "Successfully repaired config file");
                })
            })
            .inspect_err(|e| {
                tracing::error!(
                    path = %path.display(),
                    error = %e,
                    "Failed to repair config file. Using default config."
                );
            })
            .unwrap_or_default()
    }

    /// Reads local project config (.forge.yaml) and writes sample if not exists
    async fn read_local(&self) -> AppConfig {
        let path = self.infra.get_environment().local_config();
        eprintln!("DEBUG: read_local path = {:?}", path);
        let content = match self.infra.read_utf8(&path).await {
            Ok(content) => {
                eprintln!("DEBUG: read_local content = {}", content);
                content
            }
            Err(e) => {
                eprintln!("DEBUG: read_local error = {:?}", e);
                tracing::debug!(
                    path = %path.display(),
                    error = %e,
                    "Local config file not found. Writing sample config..."
                );
                // Write sample config for first-time users
                if let Err(write_err) = self.write_sample_local_config().await {
                    tracing::warn!(error = %write_err, "Failed to write sample config file");
                }
                return AppConfig::default();
            }
        };

        // Try YAML first (new format), fallback to JSON
        let parsed: Result<AppConfig, _> = serde_yml::from_str(&content);
        eprintln!("DEBUG: YAML parse result = {:?}", parsed);
        
        if parsed.is_err() {
            // Try JSON fallback
            let parsed_json: Result<AppConfig, _> = serde_json::from_str(&content);
            eprintln!("DEBUG: JSON parse result = {:?}", parsed_json);
            return parsed_json.unwrap_or_default();
        }
        
        parsed.unwrap_or_default()
    }

    async fn write(&self, config: &AppConfig) -> anyhow::Result<()> {
        let path = self.infra.get_environment().app_config();
        let content = serde_json::to_string_pretty(config)?;
        self.infra.write(&path, Bytes::from(content)).await?;
        Ok(())
    }

    fn get_overrides(&self) -> (Option<ModelId>, Option<ProviderId>) {
        (self.override_model.clone(), self.override_provider.clone())
    }

    fn apply_overrides(&self, mut config: AppConfig) -> AppConfig {
        let (model, provider) = self.get_overrides();

        // Override the default provider first
        if let Some(ref provider_id) = provider {
            config.provider = Some(provider_id.clone());

            // If we have model override, set it directly
            if let Some(ref model_id) = model {
                config.model = Some(model_id.clone());
            }
        }

        // If only model override (no provider override)
        if provider.is_none()
            && let Some(model_id) = model
        {
            // Simply set the model
            config.model = Some(model_id);
        }

        config
    }
}

#[async_trait::async_trait]
impl<F: EnvironmentInfra + FileReaderInfra + FileWriterInfra + Send + Sync> AppConfigRepository
    for AppConfigRepositoryImpl<F>
{
    async fn get_app_config(&self) -> anyhow::Result<AppConfig> {
        // Check cache first
        let cache = self.cache.lock().await;
        if let Some(ref cached_config) = *cache {
            // Apply overrides even to cached config since overrides can change via env vars
            return Ok(self.apply_overrides(cached_config.clone()));
        }
        drop(cache);

        // Cache miss, read global config
        let global_config = self.read().await;

        // Read local project config (with sample config generation for first-time users)
        let local_config = self.read_local().await;

        // Merge configs: local > global > defaults
        let mut merged = global_config;
        merged.merge(local_config);

        // Update cache with the newly read config (without overrides)
        let mut cache = self.cache.lock().await;
        *cache = Some(merged.clone());

        // Apply overrides to the config before returning
        Ok(self.apply_overrides(merged))
    }

    async fn set_app_config(&self, config: &AppConfig) -> anyhow::Result<()> {
        let (model, provider) = self.get_overrides();

        if model.is_some() || provider.is_some() {
            bail!("Could not save configuration: Model or Provider was overridden")
        }

        self.write(config).await?;

        // Bust the cache after successful write
        let mut cache = self.cache.lock().await;
        *cache = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::collections::{BTreeMap, HashMap};
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use std::sync::Mutex;

    use bytes::Bytes;
    use forge_app::{EnvironmentInfra, FileReaderInfra, FileWriterInfra};
    use forge_domain::{AppConfig, Environment, ModelId, ProviderId};
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    /// Mock infrastructure for testing that stores files in memory
    #[derive(Clone)]
    struct MockInfra {
        files: Arc<Mutex<HashMap<PathBuf, String>>>,
        config_path: PathBuf,
    }

    impl MockInfra {
        fn new(config_path: PathBuf) -> Self {
            Self { files: Arc::new(Mutex::new(HashMap::new())), config_path }
        }
    }

    impl EnvironmentInfra for MockInfra {
        fn get_environment(&self) -> Environment {
            use fake::{Fake, Faker};
            let mut env: Environment = Faker.fake();
            env = env.base_path(self.config_path.parent().unwrap().to_path_buf());
            env.cwd = self.config_path.parent().unwrap().to_path_buf();
            env
        }

        fn get_env_var(&self, _key: &str) -> Option<String> {
            None
        }

        fn get_env_vars(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }

        fn is_restricted(&self) -> bool {
            false
        }
    }

    #[async_trait::async_trait]
    impl FileReaderInfra for MockInfra {
        async fn read_utf8(&self, path: &Path) -> anyhow::Result<String> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found"))
        }

        fn read_batch_utf8(
            &self,
            _batch_size: usize,
            _paths: Vec<PathBuf>,
        ) -> impl futures::Stream<Item = (PathBuf, anyhow::Result<String>)> + Send {
            futures::stream::empty()
        }

        async fn read(&self, _path: &Path) -> anyhow::Result<Vec<u8>> {
            unimplemented!()
        }

        async fn range_read_utf8(
            &self,
            _path: &Path,
            _start_line: u64,
            _end_line: u64,
        ) -> anyhow::Result<(String, forge_domain::FileInfo)> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl FileWriterInfra for MockInfra {
        async fn write(&self, path: &Path, contents: Bytes) -> anyhow::Result<()> {
            let content = String::from_utf8(contents.to_vec())?;
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), content);
            Ok(())
        }

        async fn write_temp(&self, _: &str, _: &str, _: &str) -> anyhow::Result<PathBuf> {
            unimplemented!()
        }
    }

    fn repository_fixture() -> (AppConfigRepositoryImpl<MockInfra>, TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");
        let infra = Arc::new(MockInfra::new(config_path));
        (AppConfigRepositoryImpl::new(infra), temp_dir)
    }

    fn repository_with_config_fixture() -> (AppConfigRepositoryImpl<MockInfra>, TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");

        // Create a config file with default config
        let config = AppConfig::default();
        let content = serde_json::to_string_pretty(&config).unwrap();

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, content);

        (AppConfigRepositoryImpl::new(infra), temp_dir)
    }

    #[tokio::test]
    async fn test_get_app_config_exists() {
        let expected = AppConfig::default();
        let (repo, _temp_dir) = repository_with_config_fixture();

        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_get_app_config_not_exists() {
        let (repo, _temp_dir) = repository_fixture();

        let actual = repo.get_app_config().await.unwrap();

        // Should return default config when file doesn't exist
        let expected = AppConfig::default();
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_set_app_config() {
        let fixture = AppConfig::default();
        let (repo, _temp_dir) = repository_fixture();

        let actual = repo.set_app_config(&fixture).await;

        assert!(actual.is_ok());

        // Verify the config was actually written by reading it back
        let read_config = repo.get_app_config().await.unwrap();
        assert_eq!(read_config, fixture);
    }

    #[tokio::test]
    async fn test_cache_behavior() {
        let (repo, _temp_dir) = repository_with_config_fixture();

        // First read should populate cache
        let first_read = repo.get_app_config().await.unwrap();

        // Second read should use cache (no file system access)
        let second_read = repo.get_app_config().await.unwrap();
        assert_eq!(first_read, second_read);

        // Write new config should bust cache
        let new_config = AppConfig::default();
        repo.set_app_config(&new_config).await.unwrap();

        // Next read should get fresh data
        let third_read = repo.get_app_config().await.unwrap();
        assert_eq!(third_read, new_config);
    }

    #[tokio::test]
    async fn test_read_handles_custom_provider() {
        let fixture = r#"{
            "provider": "xyz"
        }"#;
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra
            .files
            .lock()
            .unwrap()
            .insert(config_path, fixture.to_string());

        let repo = AppConfigRepositoryImpl::new(infra);

        let actual = repo.get_app_config().await.unwrap();

        let expected = AppConfig {
            provider: Some(ProviderId::from_str("xyz").unwrap()),
            ..Default::default()
        };
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_read_returns_default_if_not_exists() {
        let (repo, _temp_dir) = repository_fixture();

        let config = repo.get_app_config().await.unwrap();

        // Config should be the default
        assert_eq!(config, AppConfig::default());
    }

    #[tokio::test]
    async fn test_override_model() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");

        // Set up a config with a specific model
        let config = AppConfig {
            model: Some(ModelId::new("claude-3-5-sonnet-20241022")),
            ..Default::default()
        };
        let content = serde_json::to_string_pretty(&config).unwrap();

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, content);

        let repo =
            AppConfigRepositoryImpl::new(infra).override_model(ModelId::new("override-model"));
        let actual = repo.get_app_config().await.unwrap();

        // The override model should be applied
        assert_eq!(actual.model, Some(ModelId::new("override-model")));
    }

    #[tokio::test]
    async fn test_override_provider() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");

        // Set up a config with a specific provider
        let config = AppConfig { provider: Some(ProviderId::ANTHROPIC), ..Default::default() };
        let content = serde_json::to_string_pretty(&config).unwrap();

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, content);

        let repo = AppConfigRepositoryImpl::new(infra).override_provider(ProviderId::OPENAI);
        let actual = repo.get_app_config().await.unwrap();

        // The override provider should be applied
        assert_eq!(actual.provider, Some(ProviderId::OPENAI));
    }

    #[tokio::test]
    async fn test_override_prevents_config_write() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");

        let infra = Arc::new(MockInfra::new(config_path));
        let repo =
            AppConfigRepositoryImpl::new(infra).override_model(ModelId::new("override-model"));

        // Attempting to write config when override is set should fail
        let config = AppConfig::default();
        let actual = repo.set_app_config(&config).await;

        assert!(actual.is_err());
        assert!(
            actual
                .unwrap_err()
                .to_string()
                .contains("Model or Provider was overridden")
        );
    }

    #[tokio::test]
    async fn test_provider_override_applied_with_no_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");
        let expected = ProviderId::from_str("open_router").unwrap();

        let infra = Arc::new(MockInfra::new(config_path));
        let repo = AppConfigRepositoryImpl::new(infra)
            .override_provider(expected.clone())
            .override_model(ModelId::new("test-model"));

        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual.provider, Some(expected));
    }

    #[tokio::test]
    async fn test_model_override_applied_with_no_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");
        let expected = ModelId::new("gpt-4-test");

        let infra = Arc::new(MockInfra::new(config_path));
        let repo = AppConfigRepositoryImpl::new(infra)
            .override_provider(ProviderId::OPENAI)
            .override_model(expected.clone());

        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual.model, Some(expected));
    }

    #[tokio::test]
    async fn test_provider_override_on_cached_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");
        let expected = ProviderId::ANTHROPIC;

        let infra = Arc::new(MockInfra::new(config_path));
        let repo = AppConfigRepositoryImpl::new(infra)
            .override_provider(expected.clone())
            .override_model(ModelId::new("test-model"));

        // First call populates cache
        repo.get_app_config().await.unwrap();

        // Second call should still apply override to cached config
        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual.provider, Some(expected));
    }

    #[tokio::test]
    async fn test_model_override_on_cached_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");
        let expected = ModelId::new("gpt-4-cached");

        let infra = Arc::new(MockInfra::new(config_path));
        let repo = AppConfigRepositoryImpl::new(infra)
            .override_provider(ProviderId::OPENAI)
            .override_model(expected.clone());

        // First call populates cache
        repo.get_app_config().await.unwrap();

        // Second call should still apply override to cached config
        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual.model, Some(expected));
    }

    #[tokio::test]
    async fn test_model_override_with_existing_provider() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");
        let expected = ModelId::new("override-model");

        // Set up config with provider but no model
        let config = AppConfig { provider: Some(ProviderId::ANTHROPIC), ..Default::default() };
        let content = serde_json::to_string_pretty(&config).unwrap();

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, content);

        let repo = AppConfigRepositoryImpl::new(infra).override_model(expected.clone());
        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual.model, Some(expected));
    }

    #[tokio::test]
    async fn test_read_repairs_invalid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");

        // Invalid JSON with trailing comma
        let json = r#"{"provider": "openai",}"#;

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra
            .files
            .lock()
            .unwrap()
            .insert(config_path, json.to_string());

        let repo = AppConfigRepositoryImpl::new(infra);
        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual.provider, Some(ProviderId::OPENAI));
    }

    #[tokio::test]
    async fn test_read_returns_default_on_unrepairable_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".config.json");

        // JSON that can't be repaired to AppConfig
        let json = r#"["this", "is", "an", "array"]"#;

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra
            .files
            .lock()
            .unwrap()
            .insert(config_path, json.to_string());

        let repo = AppConfigRepositoryImpl::new(infra);
        let actual = repo.get_app_config().await.unwrap();

        assert_eq!(actual, AppConfig::default());
    }

    #[tokio::test]
    async fn test_multi_provider_per_agent_yaml() {
        // Test: Different agents use different providers in YAML format
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".forge.yaml");

        // YAML config with simple model format (string instead of map)
        let yaml = r#"
provider: anthropic
model: claude-sonnet-4-20250514

agents:
  forge:
    provider: anthropic
    model: claude-sonnet-4-20250514
    temperature: 0.7
  commit:
    provider: openai
    model: gpt-4.1
    temperature: 0.3
    maxTokens: 1024
  suggest:
    provider: deepseek
    model: deepseek-chat
    url: "https://api.deepseek.com/v1"
"#;

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, yaml.to_string());

        let repo = AppConfigRepositoryImpl::new(infra);
        let actual = repo.get_app_config().await.unwrap();

        // Verify global settings
        assert_eq!(actual.provider, Some(ProviderId::ANTHROPIC));
        assert_eq!(actual.model, Some(ModelId::new("claude-sonnet-4-20250514")));

        // Verify agent-specific settings
        let default_agent = actual.agents.get("forge").expect("default agent should exist");
        assert_eq!(default_agent.provider, Some(ProviderId::ANTHROPIC));
        assert_eq!(default_agent.model.as_ref().map(|m| m.as_str()), Some("claude-sonnet-4-20250514"));
        assert_eq!(default_agent.temperature, Some(0.7));

        let commit_agent = actual.agents.get("commit").expect("commit agent should exist");
        assert_eq!(commit_agent.provider, Some(ProviderId::OPENAI));
        assert_eq!(commit_agent.model.as_ref().map(|m| m.as_str()), Some("gpt-4.1"));
        assert_eq!(commit_agent.temperature, Some(0.3));
        assert_eq!(commit_agent.max_tokens, Some(1024));

        let suggest_agent = actual.agents.get("suggest").expect("suggest agent should exist");
        assert_eq!(suggest_agent.provider, Some(ProviderId::from_str("deepseek").unwrap()));
        assert_eq!(suggest_agent.model.as_ref().map(|m| m.as_str()), Some("deepseek-chat"));
        assert_eq!(suggest_agent.url.as_deref(), Some("https://api.deepseek.com/v1"));
    }

    #[tokio::test]
    async fn test_multi_provider_per_agent_json() {
        // Test: Different agents use different providers in JSON format
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".forge.yaml");

        // JSON config with simple model format
        let json = r#"{
            "provider": "anthropic",
            "model": "claude-sonnet-4-20250514",
            "agents": {
                "forge": {
                    "provider": "anthropic",
                    "model": "claude-sonnet-4-20250514",
                    "temperature": 0.7
                },
                "commit": {
                    "provider": "openai",
                    "model": "gpt-4.1",
                    "temperature": 0.3,
                    "maxTokens": 1024
                },
                "suggest": {
                    "provider": "deepseek",
                    "model": "deepseek-chat",
                    "url": "https://api.deepseek.com/v1"
                }
            }
        }"#;

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, json.to_string());

        let repo = AppConfigRepositoryImpl::new(infra);
        let actual = repo.get_app_config().await.unwrap();

        // Verify global settings
        assert_eq!(actual.provider, Some(ProviderId::ANTHROPIC));

        // Verify agent-specific settings
        let default_agent = actual.agents.get("forge").expect("default agent should exist");
        assert_eq!(default_agent.provider, Some(ProviderId::ANTHROPIC));

        let commit_agent = actual.agents.get("commit").expect("commit agent should exist");
        assert_eq!(commit_agent.provider, Some(ProviderId::OPENAI));
        assert_eq!(commit_agent.max_tokens, Some(1024));

        let suggest_agent = actual.agents.get("suggest").expect("suggest agent should exist");
        assert_eq!(suggest_agent.provider, Some(ProviderId::from_str("deepseek").unwrap()));
        assert_eq!(suggest_agent.url.as_deref(), Some("https://api.deepseek.com/v1"));
    }

    #[tokio::test]
    async fn test_agent_config_url_override() {
        // Test: Agent can override provider URL for custom endpoints
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".forge.yaml");

        let yaml = r#"
provider: openai

agents:
  forge:
    provider: openai_compatible
    model: custom-model
    url: "https://my-custom-endpoint.com/v1"
    apiKey: "sk-custom-key"
"#;

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, yaml.to_string());

        let repo = AppConfigRepositoryImpl::new(infra);
        let actual = repo.get_app_config().await.unwrap();

        let default_agent = actual.agents.get("forge").expect("default agent should exist");
        assert_eq!(default_agent.provider, Some(ProviderId::OPENAI_COMPATIBLE));
        assert_eq!(default_agent.model.as_ref().map(|m| m.as_str()), Some("custom-model"));
        assert_eq!(default_agent.url.as_deref(), Some("https://my-custom-endpoint.com/v1"));
        assert_eq!(default_agent.api_key.as_deref(), Some("sk-custom-key"));
    }

    #[tokio::test]
    async fn test_agent_fallback_to_global() {
        // Test: Agent without explicit config falls back to global settings
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".forge.yaml");

        let yaml = r#"
provider: anthropic
model: claude-sonnet-4-20250514

agents:
  commit:
    provider: anthropic
    model: gpt-4.1
"#;

        let infra = Arc::new(MockInfra::new(config_path.clone()));
        infra.files.lock().unwrap().insert(config_path, yaml.to_string());

        let repo = AppConfigRepositoryImpl::new(infra);
        let actual = repo.get_app_config().await.unwrap();

        // Global provider should be used for default agent
        let default_agent = actual.agents.get("forge");
        assert!(default_agent.is_none());

        // Commit agent has model override with explicit provider
        let commit_agent = actual.agents.get("commit").expect("commit agent should exist");
        assert_eq!(commit_agent.provider, Some(ProviderId::ANTHROPIC));
        assert_eq!(commit_agent.model.as_ref().map(|m| m.as_str()), Some("gpt-4.1"));
    }
}
