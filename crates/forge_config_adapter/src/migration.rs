use std::path::{Path, PathBuf};

use crate::ConfigAdapter;
use crate::error::AdapterError;

/// A single action in a migration plan.
#[derive(Debug, Clone)]
pub enum MigrationAction {
    /// Create a new file with the given content.
    CreateFile { path: PathBuf, content: String },

    /// Copy a single file from source to destination.
    CopyFile { src: PathBuf, dest: PathBuf },

    /// Copy an entire directory tree.
    CopyDirectory { src: PathBuf, dest: PathBuf },
}

/// A plan describing how to migrate configuration from one tool format to another.
#[derive(Debug, Clone)]
pub struct MigrationPlan {
    /// Human-readable description of the migration.
    pub description: String,

    /// The source tool name.
    pub source_tool: String,

    /// The destination tool name.
    pub dest_tool: String,

    /// Ordered list of actions to perform.
    pub actions: Vec<MigrationAction>,
}

/// Reads configuration from `source` adapter and produces a migration plan
/// that, when executed, writes the equivalent configuration via the `dest` adapter format.
///
/// The plan contains serialised file-creation actions so that it can be reviewed
/// before execution.
pub fn plan_migration(
    source: &dyn ConfigAdapter,
    dest: &dyn ConfigAdapter,
    project_dir: &Path,
) -> Result<MigrationPlan, AdapterError> {
    let config = source.read(project_dir)?;

    let mut actions = Vec::new();

    // Settings / main config
    let settings_json = serde_json::to_string_pretty(&serde_json::json!({
        "model": config.model,
        "provider": config.provider,
    }))
    .map_err(|e| AdapterError::Other(e.to_string()))?;

    let dest_dir = match dest.tool_name() {
        "claude" => project_dir.join(".claude"),
        other => project_dir.join(format!(".{other}")),
    };

    actions.push(MigrationAction::CreateFile {
        path: dest_dir.join("settings.json"),
        content: settings_json,
    });

    // Custom instructions
    if let Some(ref instructions) = config.custom_instructions {
        let filename = match dest.tool_name() {
            "claude" => "CLAUDE.md",
            _ => "instructions.md",
        };
        actions.push(MigrationAction::CreateFile {
            path: project_dir.join(filename),
            content: instructions.clone(),
        });
    }

    // MCP servers
    if !config.mcp_servers.is_empty() {
        let mcp_json = serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": config.mcp_servers
        }))
        .map_err(|e| AdapterError::Other(e.to_string()))?;
        actions.push(MigrationAction::CreateFile {
            path: dest_dir.join(".mcp.json"),
            content: mcp_json,
        });
    }

    // Rules
    if !config.rules.is_empty() {
        let rules_dir = dest_dir.join("rules");
        for rule in &config.rules {
            actions.push(MigrationAction::CreateFile {
                path: rules_dir.join(&rule.path),
                content: rule.content.clone(),
            });
        }
    }

    Ok(MigrationPlan {
        description: format!(
            "Migrate configuration from {} to {}",
            source.tool_name(),
            dest.tool_name()
        ),
        source_tool: source.tool_name().to_string(),
        dest_tool: dest.tool_name().to_string(),
        actions,
    })
}

/// Execute a previously planned migration by writing all files to disk.
pub fn execute_migration(plan: &MigrationPlan) -> Result<(), AdapterError> {
    for action in &plan.actions {
        match action {
            MigrationAction::CreateFile { path, content } => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| AdapterError::io(parent, e))?;
                }
                std::fs::write(path, content).map_err(|e| AdapterError::io(path, e))?;
                tracing::info!("Created {}", path.display());
            }
            MigrationAction::CopyFile { src, dest } => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| AdapterError::io(parent, e))?;
                }
                std::fs::copy(src, dest).map_err(|e| AdapterError::io(src, e))?;
                tracing::info!("Copied {} -> {}", src.display(), dest.display());
            }
            MigrationAction::CopyDirectory { src, dest } => {
                copy_dir_recursive(src, dest)?;
                tracing::info!("Copied directory {} -> {}", src.display(), dest.display());
            }
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), AdapterError> {
    std::fs::create_dir_all(dest).map_err(|e| AdapterError::io(dest, e))?;
    let entries = std::fs::read_dir(src).map_err(|e| AdapterError::io(src, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| AdapterError::io(src, e))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        // Skip symlinks to prevent symlink-following attacks
        if src_path.is_symlink() {
            tracing::warn!(
                "Skipping symlink during migration copy: {}",
                src_path.display()
            );
            continue;
        }

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path).map_err(|e| AdapterError::io(&src_path, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn execute_migration_create_file_action() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("subdir").join("config.json");
        let plan = MigrationPlan {
            description: "test".to_string(),
            source_tool: "a".to_string(),
            dest_tool: "b".to_string(),
            actions: vec![MigrationAction::CreateFile {
                path: file_path.clone(),
                content: r#"{"hello":"world"}"#.to_string(),
            }],
        };

        execute_migration(&plan).unwrap();

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, r#"{"hello":"world"}"#);
    }

    #[test]
    fn execute_migration_copy_file_action() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("source.txt");
        std::fs::write(&src, "original content").unwrap();

        let dest = tmp.path().join("dest_dir").join("copied.txt");
        let plan = MigrationPlan {
            description: "test".to_string(),
            source_tool: "a".to_string(),
            dest_tool: "b".to_string(),
            actions: vec![MigrationAction::CopyFile { src: src.clone(), dest: dest.clone() }],
        };

        execute_migration(&plan).unwrap();

        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn execute_migration_copy_directory_action() {
        let tmp = TempDir::new().unwrap();
        let src_dir = tmp.path().join("src_tree");
        std::fs::create_dir_all(src_dir.join("nested")).unwrap();
        std::fs::write(src_dir.join("a.txt"), "file a").unwrap();
        std::fs::write(src_dir.join("nested").join("b.txt"), "file b").unwrap();

        let dest_dir = tmp.path().join("dest_tree");
        let plan = MigrationPlan {
            description: "test".to_string(),
            source_tool: "a".to_string(),
            dest_tool: "b".to_string(),
            actions: vec![MigrationAction::CopyDirectory {
                src: src_dir.clone(),
                dest: dest_dir.clone(),
            }],
        };

        execute_migration(&plan).unwrap();

        assert_eq!(
            std::fs::read_to_string(dest_dir.join("a.txt")).unwrap(),
            "file a"
        );
        assert_eq!(
            std::fs::read_to_string(dest_dir.join("nested").join("b.txt")).unwrap(),
            "file b"
        );
    }

    #[test]
    fn execute_migration_empty_plan_succeeds() {
        let plan = MigrationPlan {
            description: "empty".to_string(),
            source_tool: "a".to_string(),
            dest_tool: "b".to_string(),
            actions: vec![],
        };

        let result = execute_migration(&plan);
        assert!(result.is_ok());
    }

    #[test]
    fn execute_migration_multiple_actions_in_order() {
        let tmp = TempDir::new().unwrap();
        let file1 = tmp.path().join("first.txt");
        let file2 = tmp.path().join("second.txt");
        let plan = MigrationPlan {
            description: "multi".to_string(),
            source_tool: "a".to_string(),
            dest_tool: "b".to_string(),
            actions: vec![
                MigrationAction::CreateFile { path: file1.clone(), content: "one".to_string() },
                MigrationAction::CreateFile { path: file2.clone(), content: "two".to_string() },
            ],
        };

        execute_migration(&plan).unwrap();

        assert_eq!(std::fs::read_to_string(&file1).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(&file2).unwrap(), "two");
    }

    #[test]
    fn test_plan_migration_creates_actions() {
        use crate::adapters::ClaudeAdapter;
        use crate::normalized::{McpServerConfig, NormalizedConfig};
        use std::collections::HashMap;

        // Set up a temp dir with forge legacy config files
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        // We can't easily mock the home dir for ForgeLegacyAdapter, so instead
        // build a stub source adapter that returns a known NormalizedConfig.
        struct StubSource(NormalizedConfig);
        impl crate::ConfigAdapter for StubSource {
            fn tool_name(&self) -> &str {
                "stub"
            }
            fn detect(&self, _project_dir: &Path) -> bool {
                true
            }
            fn read(
                &self,
                _project_dir: &Path,
            ) -> Result<NormalizedConfig, crate::error::AdapterError> {
                Ok(self.0.clone())
            }
            fn write(
                &self,
                _project_dir: &Path,
                _config: &NormalizedConfig,
            ) -> Result<(), crate::error::AdapterError> {
                Ok(())
            }
        }

        let mut config = NormalizedConfig::default();
        config.model = Some("claude-sonnet-4-20250514".to_string());
        config.provider = Some("anthropic".to_string());
        config.custom_instructions = Some("Be helpful.".to_string());
        config.mcp_servers.insert(
            "ctx7".to_string(),
            McpServerConfig {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@context7/mcp".to_string()],
                env: HashMap::new(),
            },
        );
        config.permissions.allowed_commands = vec!["git".to_string()];

        let source = StubSource(config);
        let dest = ClaudeAdapter;

        let plan = plan_migration(&source, &dest, home).unwrap();

        assert!(!plan.actions.is_empty());
        assert_eq!(plan.source_tool, "stub");
        assert_eq!(plan.dest_tool, "claude");

        // Should have at least: settings.json, CLAUDE.md, .mcp.json
        let paths: Vec<String> = plan
            .actions
            .iter()
            .filter_map(|a| match a {
                MigrationAction::CreateFile { path, .. } => {
                    Some(path.file_name().unwrap().to_string_lossy().to_string())
                }
                _ => None,
            })
            .collect();
        assert!(paths.contains(&"settings.json".to_string()));
        assert!(paths.contains(&"CLAUDE.md".to_string()));
        assert!(paths.contains(&".mcp.json".to_string()));
    }

    #[test]
    fn test_plan_migration_empty_config() {
        use crate::adapters::ClaudeAdapter;
        use crate::normalized::NormalizedConfig;

        struct StubSource;
        impl crate::ConfigAdapter for StubSource {
            fn tool_name(&self) -> &str {
                "stub"
            }
            fn detect(&self, _project_dir: &Path) -> bool {
                true
            }
            fn read(
                &self,
                _project_dir: &Path,
            ) -> Result<NormalizedConfig, crate::error::AdapterError> {
                Ok(NormalizedConfig::default())
            }
            fn write(
                &self,
                _project_dir: &Path,
                _config: &NormalizedConfig,
            ) -> Result<(), crate::error::AdapterError> {
                Ok(())
            }
        }

        let tmp = TempDir::new().unwrap();
        let plan = plan_migration(&StubSource, &ClaudeAdapter, tmp.path()).unwrap();

        // Even an empty config should create settings.json
        assert!(!plan.actions.is_empty());
        let has_settings = plan.actions.iter().any(|a| match a {
            MigrationAction::CreateFile { path, .. } => {
                path.file_name().unwrap().to_string_lossy() == "settings.json"
            }
            _ => false,
        });
        assert!(has_settings, "Expected settings.json in migration actions");

        // No CLAUDE.md since no custom_instructions
        let has_claude_md = plan.actions.iter().any(|a| match a {
            MigrationAction::CreateFile { path, .. } => {
                path.file_name().unwrap().to_string_lossy() == "CLAUDE.md"
            }
            _ => false,
        });
        assert!(
            !has_claude_md,
            "Should not create CLAUDE.md for empty config"
        );
    }
}
