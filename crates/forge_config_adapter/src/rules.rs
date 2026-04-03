//! Rules converter module
//!
//! Converts Claude Code's rules/*.md files to Forge's instruction format.

use std::path::Path;

use anyhow::Result;

use crate::claude_code::{ConvertedConfig, InstructionConfig};

/// Represents a converted rule from rules/*.md
#[derive(Debug, Clone)]
pub struct ConvertedRule {
    /// Rule name (from filename or frontmatter)
    pub name: String,
    /// Rule content (body after frontmatter)
    pub content: String,
    /// Frontmatter globs (file patterns this applies to)
    pub globs: Vec<String>,
    /// Frontmatter description
    pub description: Option<String>,
    /// Priority (higher = more important)
    pub priority: i32,
    /// Whether this rule is enabled (default: true)
    pub enabled: bool,
}

impl Default for ConvertedRule {
    fn default() -> Self {
        Self {
            name: String::new(),
            content: String::new(),
            globs: Vec::new(),
            description: None,
            priority: 0,
            enabled: true, // Default to enabled
        }
    }
}

/// Parser for Claude Code rules/*.md files
pub struct RulesParser;

impl RulesParser {
    /// Parse a rule file and extract frontmatter and content
    pub fn parse(path: &Path) -> Result<ConvertedRule> {
        let content = std::fs::read_to_string(path)?;

        // Extract filename as rule name
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();

        Self::parse_content(&name, &content)
    }

    /// Parse rule content string
    pub fn parse_content(name: &str, content: &str) -> Result<ConvertedRule> {
        let mut rule = ConvertedRule { name: name.to_string(), ..Default::default() };

        // Check for --- delimited frontmatter
        if let Some(stripped) = content.strip_prefix("---") {
            if let Some(end_idx) = stripped.find("---") {
                let frontmatter = &stripped[..end_idx];
                let body = &stripped[end_idx + 3..];

                // Parse frontmatter
                Self::parse_frontmatter(frontmatter, &mut rule);

                // Clean up body (remove leading/trailing whitespace)
                rule.content = body.trim().to_string();
            } else {
                // No frontmatter, entire content is the body
                rule.content = content.trim().to_string();
            }
        } else {
            // No frontmatter
            rule.content = content.trim().to_string();
        }

        Ok(rule)
    }

    /// Parse frontmatter and populate rule fields
    fn parse_frontmatter(frontmatter: &str, rule: &mut ConvertedRule) {
        for line in frontmatter.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(colon_idx) = line.find(':') {
                let key = line[..colon_idx].trim();
                let value = line[colon_idx + 1..].trim();

                match key {
                    "globs"
                        if value.starts_with('[') && value.ends_with(']') => {
                            rule.globs = value[1..value.len() - 1]
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .collect();
                        }
                    "description" => {
                        rule.description = Some(value.trim_matches('"').to_string());
                    }
                    "priority" => {
                        if let Ok(p) = value.parse::<i32>() {
                            rule.priority = p;
                        }
                    }
                    "enabled" => {
                        rule.enabled = value == "true" || value.is_empty();
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Converter from rules/*.md to Forge format
pub struct RulesToForgeConverter;

impl RulesToForgeConverter {
    /// Convert rules to Forge's instruction format
    ///
    /// In Forge, rules become conditional instructions based on globs.
    pub fn convert(rules: Vec<ConvertedRule>) -> Result<ConvertedConfig> {
        let mut config = ConvertedConfig::default();

        // Sort rules by priority (highest first)
        let mut sorted_rules = rules;
        sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        for rule in sorted_rules {
            if !rule.enabled {
                continue;
            }

            // Skip empty rules
            if rule.content.is_empty() {
                continue;
            }

            config.instructions.push(InstructionConfig {
                content: rule.content,
                globs: rule.globs,
                description: rule.description,
                model: None,
                effort: None,
                context: None,
                agent: None,
                skills: vec![],
            });
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let name = "test-rule";
        let content = r#"# Test Rule

This is a test rule content.
"#;
        let result = RulesParser::parse_content(name, content).unwrap();
        assert_eq!(result.name, "test-rule");
        assert!(result.content.contains("Test Rule"));
        assert!(result.enabled);
        assert_eq!(result.priority, 0);
    }

    #[test]
    fn test_parse_rule_with_frontmatter() {
        let name = "typescript-best-practices";
        let content = r#"---
globs: ["*.ts", "*.tsx"]
description: "TypeScript best practices rule"
priority: 10
enabled: true
---

# TypeScript Best Practices

- Always use strict mode
- Prefer interfaces over types for object shapes
"#;
        let result = RulesParser::parse_content(name, content).unwrap();
        assert_eq!(result.name, "typescript-best-practices");
        assert_eq!(result.globs, vec!["*.ts", "*.tsx"]);
        assert_eq!(
            result.description,
            Some("TypeScript best practices rule".to_string())
        );
        assert_eq!(result.priority, 10);
        assert!(result.enabled);
    }

    #[test]
    fn test_parse_disabled_rule() {
        let name = "disabled-rule";
        let content = r#"---
enabled: false
---

This rule is disabled.
"#;
        let result = RulesParser::parse_content(name, content).unwrap();
        assert!(!result.enabled);
    }

    #[test]
    fn test_convert_to_forge() {
        let rules = vec![
            ConvertedRule {
                name: "rule1".to_string(),
                content: "Rule 1 content".to_string(),
                globs: vec!["*.ts".to_string()],
                description: Some("Rule 1".to_string()),
                priority: 5,
                enabled: true,
            },
            ConvertedRule {
                name: "rule2".to_string(),
                content: "Rule 2 content".to_string(),
                globs: vec!["*.rs".to_string()],
                description: Some("Rule 2".to_string()),
                priority: 10,
                enabled: true,
            },
        ];

        let config = RulesToForgeConverter::convert(rules).unwrap();
        // Should be sorted by priority (10 > 5)
        assert_eq!(config.instructions.len(), 2);
        assert!(config.instructions[0].content.contains("Rule 2"));
    }

    #[test]
    fn test_convert_skips_disabled() {
        let rules = vec![
            ConvertedRule {
                name: "enabled".to_string(),
                content: "Enabled rule".to_string(),
                globs: vec![],
                description: None,
                priority: 0,
                enabled: true,
            },
            ConvertedRule {
                name: "disabled".to_string(),
                content: "Disabled rule".to_string(),
                globs: vec![],
                description: None,
                priority: 0,
                enabled: false,
            },
        ];

        let config = RulesToForgeConverter::convert(rules).unwrap();
        assert_eq!(config.instructions.len(), 1);
        assert!(config.instructions[0].content.contains("Enabled"));
    }
}
