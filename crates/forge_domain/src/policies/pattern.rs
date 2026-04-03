//! Permission pattern matching module
//!
//! This module implements Claude Code-style permission pattern matching syntax:
//! - `ToolName(pattern)` - matches tool with specific argument pattern
//! - Glob patterns: `*` matches any characters, `?` matches single character
//!
//! Examples:
//! - `Bash(npm *)` - matches Bash commands starting with "npm "
//! - `Read(*.rs)` - matches Read operations on .rs files
//! - `Write(src/*)` - matches Write operations in src/ directory

use glob::Pattern;
use serde::{Deserialize, Serialize};

/// A permission pattern that matches tool names with optional argument patterns
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionPattern {
    /// The tool name (e.g., "Bash", "Read", "Write")
    pub tool: String,
    /// Optional argument pattern (e.g., "npm *", "*.rs")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg_pattern: Option<String>,
}

impl PermissionPattern {
    /// Parse a permission pattern string like "Bash(npm *)" or "Read(*.rs)"
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use forge_domain::policies::PermissionPattern;
    /// let pattern = PermissionPattern::parse("Bash(npm *)").expect("valid pattern");
    /// assert_eq!(pattern.tool, "Bash");
    /// assert_eq!(pattern.arg_pattern, Some("npm *".to_string()));
    ///
    /// let pattern = PermissionPattern::parse("Read").expect("valid pattern");
    /// assert_eq!(pattern.tool, "Read");
    /// assert!(pattern.arg_pattern.is_none());
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        // Check for pattern syntax: ToolName(pattern)
        if let Some(end_paren) = s.rfind(')')
            && end_paren > 0
        {
            let tool = s[..end_paren].trim_end();
            if let Some(start_paren) = tool.rfind('(') {
                let tool_name = tool[..start_paren].trim();
                let arg_pattern = tool[start_paren + 1..].trim();

                if !tool_name.is_empty() {
                    return Some(PermissionPattern {
                        tool: tool_name.to_string(),
                        arg_pattern: Some(arg_pattern.to_string()),
                    });
                }
            }
        }

        // No pattern, just tool name
        Some(PermissionPattern { tool: s.to_string(), arg_pattern: None })
    }

    /// Check if this pattern matches a tool name (with optional argument)
    pub fn matches_tool(&self, tool_name: &str) -> bool {
        // Case-insensitive tool name matching
        self.tool.to_lowercase() == tool_name.to_lowercase()
    }

    /// Check if this pattern matches a tool name with its arguments
    pub fn matches(&self, tool_name: &str, arguments: Option<&str>) -> bool {
        if !self.matches_tool(tool_name) {
            return false;
        }

        // If no argument pattern is specified, any arguments are allowed
        let arg_pattern = match &self.arg_pattern {
            Some(p) => p,
            None => return true,
        };

        // If argument pattern exists, match against arguments
        let args = match arguments {
            Some(a) => a,
            None => return self.arg_pattern.is_none(), // No args but pattern requires them = no match
        };

        Self::match_glob_pattern(arg_pattern, args)
    }

    /// Match a glob pattern against a string
    fn match_glob_pattern(pattern: &str, text: &str) -> bool {
        match Pattern::new(pattern) {
            Ok(p) => p.matches(text),
            Err(_) => {
                // Fallback to exact match if pattern is invalid
                pattern == text
            }
        }
    }
}

/// A permission rule with pattern matching support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRule {
    /// The permission mode (allow/deny/ask)
    pub mode: PermissionMode,
    /// The permission pattern
    pub pattern: String,
}

impl PatternRule {
    /// Create a new pattern rule
    pub fn new(mode: PermissionMode, pattern: impl Into<String>) -> Self {
        Self { mode, pattern: pattern.into() }
    }

    /// Parse and check if this rule matches a tool call
    pub fn matches(&self, tool_name: &str, arguments: Option<&str>) -> Option<PermissionMode> {
        let parsed = PermissionPattern::parse(&self.pattern)?;
        if parsed.matches(tool_name, arguments) {
            Some(self.mode)
        } else {
            None
        }
    }
}

/// Permission mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    /// Always allow the operation
    Allow,
    /// Always deny the operation
    Deny,
    /// Ask user for confirmation
    Ask,
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionMode::Allow => write!(f, "allow"),
            PermissionMode::Deny => write!(f, "deny"),
            PermissionMode::Ask => write!(f, "ask"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_name_only() {
        let pattern = PermissionPattern::parse("Read").unwrap();
        assert_eq!(pattern.tool, "Read");
        assert!(pattern.arg_pattern.is_none());
    }

    #[test]
    fn test_parse_with_argument_pattern() {
        let pattern = PermissionPattern::parse("Bash(npm *)").unwrap();
        assert_eq!(pattern.tool, "Bash");
        assert_eq!(pattern.arg_pattern, Some("npm *".to_string()));
    }

    #[test]
    fn test_parse_with_file_pattern() {
        let pattern = PermissionPattern::parse("Read(*.rs)").unwrap();
        assert_eq!(pattern.tool, "Read");
        assert_eq!(pattern.arg_pattern, Some("*.rs".to_string()));
    }

    #[test]
    fn test_parse_complex_mcp_pattern() {
        let pattern = PermissionPattern::parse("mcp__filesystem__read").unwrap();
        assert_eq!(pattern.tool, "mcp__filesystem__read");
        assert!(pattern.arg_pattern.is_none());
    }

    #[test]
    fn test_matches_tool_name() {
        let pattern = PermissionPattern::parse("Read").unwrap();
        assert!(pattern.matches_tool("read"));
        assert!(pattern.matches_tool("Read"));
        assert!(pattern.matches_tool("READ"));
        assert!(!pattern.matches_tool("Write"));
    }

    #[test]
    fn test_matches_with_argument_pattern() {
        let pattern = PermissionPattern::parse("Bash(npm *)").unwrap();
        assert!(pattern.matches("Bash", Some("npm install")));
        assert!(pattern.matches("Bash", Some("npm run dev")));
        assert!(!pattern.matches("Bash", Some("git status")));
        assert!(!pattern.matches("Read", Some("npm install")));
    }

    #[test]
    fn test_matches_without_arguments() {
        let pattern = PermissionPattern::parse("Read(*.rs)").unwrap();
        // No arguments provided, should not match if pattern requires one
        assert!(!pattern.matches("Read", None));
    }

    #[test]
    fn test_pattern_rule_matches() {
        let rule = PatternRule::new(PermissionMode::Allow, "Bash(npm *)");
        assert_eq!(
            rule.matches("Bash", Some("npm install")),
            Some(PermissionMode::Allow)
        );
        assert_eq!(rule.matches("Bash", Some("git status")), None);
    }

    #[test]
    fn test_glob_pattern_matching() {
        // Test glob patterns
        assert!(PermissionPattern::match_glob_pattern("*.rs", "main.rs"));
        assert!(PermissionPattern::match_glob_pattern("*.rs", "lib.rs"));
        assert!(!PermissionPattern::match_glob_pattern("*.rs", "main.txt"));

        // Test wildcards
        assert!(PermissionPattern::match_glob_pattern(
            "npm *",
            "npm install"
        ));
        assert!(PermissionPattern::match_glob_pattern(
            "npm *",
            "npm run build"
        ));
        assert!(!PermissionPattern::match_glob_pattern(
            "npm *",
            "yarn install"
        ));
    }

    #[test]
    fn test_permission_mode_display() {
        assert_eq!(PermissionMode::Allow.to_string(), "allow");
        assert_eq!(PermissionMode::Deny.to_string(), "deny");
        assert_eq!(PermissionMode::Ask.to_string(), "ask");
    }
}
