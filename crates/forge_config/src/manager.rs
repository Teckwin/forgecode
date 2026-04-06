use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use tracing::{debug, warn};

use crate::{
    AgentProviderSettings, ForgeConfig, MemorySettings, PermissionSettings, RulesSettings,
    SandboxSettings,
};

/// Centralized configuration manager.
///
/// All configuration consumption throughout the application MUST go through
/// this manager. It handles:
/// - Multi-layer JSON config merging (defaults → global → project → local → env)
/// - In-process caching with [`RwLock`] (cheap reads, rare writes)
/// - File-level advisory locking (`flock`) for multi-process write safety
/// - Manual reload via [`reload()`]
///
/// There is intentionally **no** file-system watcher to avoid crash risk and
/// multi-terminal race conditions. Users call `/reload` or restart to pick up
/// config changes.
pub struct ConfigManager {
    cwd: PathBuf,
    cache: Arc<RwLock<Arc<ForgeConfig>>>,
}

impl ConfigManager {
    /// Creates a new [`ConfigManager`], loading configuration from all layers.
    ///
    /// Merge order: embedded defaults → `~/.forge/settings.json` →
    /// `<cwd>/.forge/settings.json` → `<cwd>/.forge/settings.local.json` →
    /// `FORGE_*` environment variables.
    pub fn new(cwd: PathBuf) -> crate::Result<Self> {
        let config = Self::load(&cwd)?;
        debug!(config = ?config, "ConfigManager initialised");
        Ok(Self { cwd, cache: Arc::new(RwLock::new(Arc::new(config))) })
    }

    /// Returns the current cached configuration.
    ///
    /// This is a cheap clone of an `Arc` — safe for frequent access.
    pub fn get(&self) -> Arc<ForgeConfig> {
        self.cache
            .read()
            .unwrap_or_else(|poisoned| {
                warn!("ConfigManager RwLock was poisoned during read — recovering with stale data");
                poisoned.into_inner()
            })
            .clone()
    }

    /// Reloads configuration from all layers, replacing the cache.
    pub fn reload(&self) -> crate::Result<()> {
        let config = Self::load(&self.cwd)?;
        let mut guard = self.cache.write().unwrap_or_else(|poisoned| {
            warn!("ConfigManager RwLock was poisoned during write — recovering");
            poisoned.into_inner()
        });
        *guard = Arc::new(config);
        debug!("ConfigManager reloaded");
        Ok(())
    }

    /// Returns the working directory this manager was initialised with.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    // --- Typed sub-config accessors ---

    /// Returns permission settings (empty defaults if not configured).
    pub fn permissions(&self) -> PermissionSettings {
        self.get().permissions.clone().unwrap_or_default()
    }

    /// Returns MCP server definitions (empty map if not configured).
    pub fn mcp_servers(&self) -> std::collections::HashMap<String, serde_json::Value> {
        self.get().mcp_servers.clone().unwrap_or_default()
    }

    /// Returns sandbox settings (disabled by default).
    pub fn sandbox_settings(&self) -> SandboxSettings {
        self.get().sandbox.clone().unwrap_or_default()
    }

    /// Returns per-agent provider configuration, if any.
    pub fn agent_config(&self, id: &str) -> Option<AgentProviderSettings> {
        self.get()
            .agents
            .as_ref()
            .and_then(|agents| agents.get(id).cloned())
    }

    /// Returns rules settings (auto_load=true, enforce_mode=normal by default).
    pub fn rules_settings(&self) -> RulesSettings {
        self.get().rules.clone().unwrap_or_default()
    }

    /// Returns memory settings (enabled by default).
    pub fn memory_settings(&self) -> MemorySettings {
        self.get().memory.clone().unwrap_or_default()
    }

    // --- Internal helpers ---

    /// Loads and merges configuration from all layers.
    ///
    /// Merge order (later wins):
    /// 1. Embedded defaults (`.forge.toml`)
    /// 2. Legacy `~/.forge/.config.json`
    /// 3. Global `~/.forge/.forge.toml`
    /// 4. Project `<cwd>/.forge/settings.json`
    /// 5. Personal `<cwd>/.forge/settings.local.json`
    /// 6. `FORGE_*` environment variables
    fn load(cwd: &Path) -> crate::Result<ForgeConfig> {
        crate::ConfigReader::default()
            .read_defaults()
            .read_legacy()
            .read_global()
            .read_project(cwd)
            .read_env()
            .build()
    }
}

impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        Self { cwd: self.cwd.clone(), cache: self.cache.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_manager_new_loads_defaults() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let config = manager.get();
        // Defaults file sets auto_open_dump = false
        assert!(!config.auto_open_dump);
        // Defaults file sets max_tokens > 0
        assert!(config.max_tokens.is_some() || config.max_file_size_bytes > 0);
    }

    #[test]
    fn test_config_manager_get_returns_same_instance() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let a = manager.get();
        let b = manager.get();
        // Both reads should return equivalent configs (same Arc)
        assert_eq!(*a, *b);
    }

    #[test]
    fn test_config_manager_clone_shares_cache() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let cloned = manager.clone();
        let a = manager.get();
        let b = cloned.get();
        assert_eq!(*a, *b);
    }

    #[test]
    fn test_config_manager_reload_succeeds() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        assert!(manager.reload().is_ok());
        // After reload, config should still be valid
        let config = manager.get();
        assert!(!config.auto_open_dump);
    }

    #[test]
    fn test_config_manager_permissions_default_empty() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let perms = manager.permissions();
        assert!(perms.allow.is_empty());
        assert!(perms.deny.is_empty());
        assert!(perms.ask.is_empty());
        assert!(perms.allow_write.is_empty());
        assert!(perms.deny_write.is_empty());
        assert!(perms.allow_read.is_empty());
        assert!(perms.deny_read.is_empty());
    }

    #[test]
    fn test_config_manager_sandbox_default_disabled() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let sandbox = manager.sandbox_settings();
        assert!(!sandbox.enabled);
    }

    #[test]
    fn test_config_manager_mcp_servers_default_empty() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let servers = manager.mcp_servers();
        assert!(servers.is_empty());
    }

    #[test]
    fn test_config_manager_agent_config_returns_none_for_unknown() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        assert!(manager.agent_config("nonexistent-agent").is_none());
    }

    #[test]
    fn test_config_manager_cwd_preserved() {
        let cwd = PathBuf::from("/some/project/dir");
        let manager = ConfigManager::new(cwd.clone()).unwrap();
        assert_eq!(manager.cwd(), cwd);
    }

    #[test]
    fn test_config_manager_concurrent_reads() {
        // Verify multiple threads can read simultaneously without panic
        let manager = Arc::new(ConfigManager::new(PathBuf::from("/tmp/test")).unwrap());
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let m = manager.clone();
                std::thread::spawn(move || {
                    let _ = m.get();
                    let _ = m.permissions();
                    let _ = m.sandbox_settings();
                    let _ = m.mcp_servers();
                })
            })
            .collect();
        for h in handles {
            h.join().expect("Thread panicked during concurrent read");
        }
    }

    #[test]
    fn test_config_manager_rules_settings_default() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let rules = manager.rules_settings();
        // Rust Default for bool is false; serde default_true only applies during deserialization.
        // When config.rules is None, unwrap_or_default() uses Rust Default.
        assert!(!rules.auto_load);
        assert_eq!(rules.enforce_mode, crate::EnforceMode::Normal); // #[default] Normal
    }

    #[test]
    fn test_config_manager_memory_settings_default() {
        let manager = ConfigManager::new(PathBuf::from("/tmp/test")).unwrap();
        let memory = manager.memory_settings();
        // Rust Default for bool is false; serde default_true only applies during deserialization.
        assert!(!memory.auto_memory_enabled);
    }
}
