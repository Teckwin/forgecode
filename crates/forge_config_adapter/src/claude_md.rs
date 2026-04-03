//! CLAUDE.md converter module
//!
//! Converts Claude Code's CLAUDE.md files to Forge's instruction format.

use std::path::Path;

use anyhow::Result;

use crate::claude_code::ConvertedConfig;

/// Represents a converted CLAUDE.md instruction
#[derive(Debug, Clone, Default)]
pub struct ConvertedInstruction {
    /// The instruction content (body after frontmatter)
    pub content: String,
    /// Frontmatter globs (file patterns this applies to)
    pub globs: Vec<String>,
    /// Frontmatter description
    pub description: Option<String>,
    /// Frontmatter model preference
    pub model: Option<String>,
    /// Frontmatter effort level
    pub effort: Option<String>,
    /// Frontmatter context
    pub context: Option<String>,
    /// Frontmatter agent type
    pub agent: Option<String>,
    /// Frontmatter skills
    pub skills: Vec<String>,
}

/// Parser for Claude Code CLAUDE.md files
pub struct ClaudeMdParser;

impl ClaudeMdParser {
    /// Parse a CLAUDE.md file and extract frontmatter and content
    pub fn parse(path: &Path) -> Result<ConvertedInstruction> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_content(&content)
    }

    /// Parse CLAUDE.md content string
    pub fn parse_content(content: &str) -> Result<ConvertedInstruction> {
        let mut instruction = ConvertedInstruction::default();

        // Check for --- delimited frontmatter
        if let Some(stripped) = content.strip_prefix("---") {
            if let Some(end_idx) = stripped.find("---") {
                let frontmatter = &stripped[..end_idx];
                let body = &stripped[end_idx + 3..];

                // Parse frontmatter
                Self::parse_frontmatter(frontmatter, &mut instruction);

                // Clean up body (remove leading/trailing whitespace)
                instruction.content = body.trim().to_string();
            } else {
                // No frontmatter, entire content is the body
                instruction.content = content.trim().to_string();
            }
        } else {
            // No frontmatter
            instruction.content = content.trim().to_string();
        }

        Ok(instruction)
    }

    /// Parse frontmatter and populate instruction fields
    fn parse_frontmatter(frontmatter: &str, instruction: &mut ConvertedInstruction) {
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
                            instruction.globs = value[1..value.len() - 1]
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .collect();
                        }
                    "description" => {
                        instruction.description = Some(value.trim_matches('"').to_string());
                    }
                    "model" => {
                        instruction.model = Some(value.trim_matches('"').to_string());
                    }
                    "effort" => {
                        instruction.effort = Some(value.trim_matches('"').to_string());
                    }
                    "context" => {
                        instruction.context = Some(value.trim_matches('"').to_string());
                    }
                    "agent" => {
                        instruction.agent = Some(value.trim_matches('"').to_string());
                    }
                    "skills"
                        if value.starts_with('[') && value.ends_with(']') => {
                            instruction.skills = value[1..value.len() - 1]
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .collect();
                        }
                    _ => {}
                }
            }
        }
    }
}

/// Converter from CLAUDE.md to Forge format
pub struct ClaudeMdToForgeConverter;

impl ClaudeMdToForgeConverter {
    /// Convert a CLAUDE.md instruction to Forge's format
    ///
    /// In Forge, CLAUDE.md content becomes part of the custom instructions
    /// that are prepended to the system prompt.
    pub fn convert(instruction: ConvertedInstruction) -> Result<ConvertedConfig> {
        let mut config = ConvertedConfig::default();

        // Add the instruction content as a special instruction
        // The content will be used as custom instructions in Forge
        config.instructions.push(InstructionConfig {
            content: instruction.content,
            globs: instruction.globs,
            description: instruction.description,
            model: instruction.model,
            effort: instruction.effort,
            context: instruction.context,
            agent: instruction.agent,
            skills: instruction.skills,
        });

        Ok(config)
    }
}

/// Re-export InstructionConfig from claude_code for convenience
pub use crate::claude_code::InstructionConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_content() {
        let content = r#"# Hello World

This is a simple instruction file.
"#;
        let result = ClaudeMdParser::parse_content(content).unwrap();
        assert_eq!(
            result.content,
            "# Hello World\n\nThis is a simple instruction file."
        );
        assert!(result.globs.is_empty());
    }

    #[test]
    fn test_parse_with_frontmatter() {
        let content = r#"---
globs: ["*.ts", "src/**/*.ts"]
description: "TypeScript instructions"
model: "sonnet"
effort: "high"
---

# TypeScript Project

Use TypeScript best practices.
"#;
        let result = ClaudeMdParser::parse_content(content).unwrap();
        assert!(result.content.contains("TypeScript Project"));
        assert_eq!(result.globs, vec!["*.ts", "src/**/*.ts"]);
        assert_eq!(
            result.description,
            Some("TypeScript instructions".to_string())
        );
        assert_eq!(result.model, Some("sonnet".to_string()));
        assert_eq!(result.effort, Some("high".to_string()));
    }

    #[test]
    fn test_parse_with_skills() {
        let content = r#"---
skills: ["react", "typescript"]
agent: "general-purpose"
---

Build React applications.
"#;
        let result = ClaudeMdParser::parse_content(content).unwrap();
        assert_eq!(result.skills, vec!["react", "typescript"]);
        assert_eq!(result.agent, Some("general-purpose".to_string()));
    }

    #[test]
    fn test_convert_to_forge() {
        let instruction = ConvertedInstruction {
            content: "Use TypeScript best practices.".to_string(),
            globs: vec!["*.ts".to_string()],
            description: Some("TypeScript instructions".to_string()),
            model: None,
            effort: None,
            context: None,
            agent: None,
            skills: vec![],
        };

        let config = ClaudeMdToForgeConverter::convert(instruction).unwrap();
        assert_eq!(config.instructions.len(), 1);
        assert_eq!(
            config.instructions[0].content,
            "Use TypeScript best practices."
        );
        assert_eq!(config.instructions[0].globs, vec!["*.ts"]);
    }
}
