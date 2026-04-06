//! Tests for new ForgeConfig fields (agents, mcp_servers, permissions, sandbox,
//! rules, memory) — validates JSON round-trip and settings.json compatibility.

use forge_config::{
    AgentParameterSettings, AgentProviderSettings, ForgeConfig, MemorySettings, PermissionSettings,
    RulesSettings, SandboxSettings,
};
use pretty_assertions::assert_eq;
use std::collections::HashMap;

/// Helper: create a base ForgeConfig from defaults, then overlay JSON fields.
fn config_with_json_overlay(json: &str) -> ForgeConfig {
    // Start from defaults
    let base = serde_json::to_value(ForgeConfig::default()).unwrap();
    let overlay: serde_json::Value = serde_json::from_str(json).unwrap();

    // Merge overlay into base
    let mut merged = base;
    if let (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) =
        (&mut merged, overlay)
    {
        for (k, v) in overlay_map {
            base_map.insert(k, v);
        }
    }

    serde_json::from_value(merged).expect("deserialize merged config")
}

/// Verify full round-trip with all new fields.
#[test]
fn test_forge_config_json_round_trip_with_new_fields() {
    let mut agents = HashMap::new();
    agents.insert(
        "forge".to_string(),
        AgentProviderSettings {
            provider: Some("anthropic".to_string()),
            model: Some("claude-sonnet-4-20250514".to_string()),
            api_key: Some("${ANTHROPIC_API_KEY}".to_string()),
            base_url: None,
            parameters: Some(AgentParameterSettings {
                temperature: Some(0.7),
                max_tokens: Some(20480),
                ..Default::default()
            }),
        },
    );

    let config = ForgeConfig {
        agents: Some(agents),
        mcp_servers: Some(HashMap::from([(
            "ctx7".to_string(),
            serde_json::json!({"type": "stdio", "command": "npx"}),
        )])),
        permissions: Some(PermissionSettings {
            allow: vec!["git *".to_string()],
            deny: vec!["rm -rf *".to_string()],
            ..Default::default()
        }),
        sandbox: Some(SandboxSettings {
            enabled: true,
            allow_network: false,
            ..Default::default()
        }),
        rules: Some(RulesSettings {
            auto_load: true,
            enforce_mode: forge_config::EnforceMode::Strict,
        }),
        memory: Some(MemorySettings { auto_memory_enabled: true }),
        ..Default::default()
    };

    let json_str = serde_json::to_string_pretty(&config).expect("serialize");
    let restored: ForgeConfig = serde_json::from_str(&json_str).expect("deserialize");

    assert_eq!(config.agents, restored.agents);
    assert_eq!(config.permissions, restored.permissions);
    assert_eq!(config.sandbox, restored.sandbox);
    assert_eq!(config.rules, restored.rules);
    assert_eq!(config.memory, restored.memory);
    assert_eq!(config.mcp_servers, restored.mcp_servers);
}

/// Minimal settings: only session field.
#[test]
fn test_minimal_settings_json_parses() {
    let config = config_with_json_overlay(
        r#"{"session": {"provider_id": "anthropic", "model_id": "claude-sonnet-4-20250514"}}"#,
    );
    let session = config.session.unwrap();
    assert_eq!(session.provider_id.as_deref(), Some("anthropic"));
    assert_eq!(
        session.model_id.as_deref(),
        Some("claude-sonnet-4-20250514")
    );
    assert!(config.agents.is_none());
    assert!(config.permissions.is_none());
}

/// Empty permissions deserializes correctly.
#[test]
fn test_permission_settings_empty() {
    let config = config_with_json_overlay(r#"{"permissions": {}}"#);
    let perms = config.permissions.unwrap();
    assert!(perms.allow.is_empty());
    assert!(perms.deny.is_empty());
}

/// Sandbox defaults: enabled=false, allow_network=true.
#[test]
fn test_sandbox_settings_defaults() {
    let config = config_with_json_overlay(r#"{"sandbox": {}}"#);
    let sandbox = config.sandbox.unwrap();
    assert!(!sandbox.enabled);
    assert!(sandbox.allow_network);
}

/// Agents with full parameters.
#[test]
fn test_agents_config_with_parameters() {
    let config = config_with_json_overlay(
        r#"{"agents": {"sage": {"provider": "anthropic", "model": "claude-sonnet-4-20250514", "parameters": {"temperature": 0.5, "top_p": 0.9, "top_k": 40, "max_tokens": 8192}}}}"#,
    );
    let agents = config.agents.unwrap();
    let sage = agents.get("sage").unwrap();
    let params = sage.parameters.as_ref().unwrap();
    assert_eq!(params.temperature, Some(0.5));
    assert_eq!(params.top_p, Some(0.9));
    assert_eq!(params.top_k, Some(40));
    assert_eq!(params.max_tokens, Some(8192));
}

/// MCP servers parse as opaque JSON.
#[test]
fn test_mcp_servers_opaque_json() {
    let config = config_with_json_overlay(
        r#"{"mcp_servers": {"my-server": {"type": "stdio", "command": "node", "args": ["server.js"]}}}"#,
    );
    let servers = config.mcp_servers.unwrap();
    assert!(servers.contains_key("my-server"));
    assert_eq!(servers["my-server"]["type"], "stdio");
    assert_eq!(servers["my-server"]["command"], "node");
}

/// Rules enforce_mode strict.
#[test]
fn test_rules_enforce_mode_strict() {
    let config =
        config_with_json_overlay(r#"{"rules": {"auto_load": true, "enforce_mode": "strict"}}"#);
    let rules = config.rules.unwrap();
    assert!(rules.auto_load);
    assert_eq!(rules.enforce_mode, forge_config::EnforceMode::Strict);
}

/// Rules enforce_mode normal.
#[test]
fn test_rules_enforce_mode_normal() {
    let config = config_with_json_overlay(r#"{"rules": {"enforce_mode": "normal"}}"#);
    let rules = config.rules.unwrap();
    assert_eq!(rules.enforce_mode, forge_config::EnforceMode::Normal);
}

/// Multiple agents with different providers.
#[test]
fn test_multiple_agents_different_providers() {
    let config = config_with_json_overlay(
        r#"{"agents": {
            "forge": {"provider": "anthropic", "model": "claude-sonnet-4-20250514"},
            "muse": {"provider": "openai", "model": "gpt-4o", "api_key": "${OPENAI_API_KEY}"},
            "sage": {"provider": "google", "model": "gemini-pro"}
        }}"#,
    );
    let agents = config.agents.unwrap();
    assert_eq!(agents.len(), 3);
    assert_eq!(agents["forge"].provider.as_deref(), Some("anthropic"));
    assert_eq!(agents["muse"].provider.as_deref(), Some("openai"));
    assert_eq!(agents["sage"].provider.as_deref(), Some("google"));
    assert_eq!(agents["muse"].api_key.as_deref(), Some("${OPENAI_API_KEY}"));
}

/// Permission rules with all fields populated.
#[test]
fn test_permissions_all_fields() {
    let config = config_with_json_overlay(
        r#"{"permissions": {
            "allow": ["git *", "cargo *"],
            "ask": ["npm *"],
            "deny": ["rm -rf *"],
            "allow_write": ["."],
            "deny_write": [".git/", ".env"],
            "allow_read": ["/"],
            "deny_read": ["/etc/shadow"]
        }}"#,
    );
    let perms = config.permissions.unwrap();
    assert_eq!(perms.allow, vec!["git *", "cargo *"]);
    assert_eq!(perms.ask, vec!["npm *"]);
    assert_eq!(perms.deny, vec!["rm -rf *"]);
    assert_eq!(perms.allow_write, vec!["."]);
    assert_eq!(perms.deny_write, vec![".git/", ".env"]);
    assert_eq!(perms.allow_read, vec!["/"]);
    assert_eq!(perms.deny_read, vec!["/etc/shadow"]);
}
