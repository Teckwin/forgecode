//! Syntax validation using tree-sitter parsers.

use std::path::Path;

/// A syntax error found in a source file.
#[derive(Debug, Clone)]
pub struct LocalSyntaxError {
    /// Line number (1-based).
    pub line: u32,
    /// Column number (1-based).
    pub column: u32,
    /// Error description.
    pub message: String,
}

/// Validate the syntax of a source file using tree-sitter.
///
/// Returns an empty vec if:
/// - The file is syntactically valid
/// - The language is not supported
pub fn validate_file(path: impl AsRef<Path>, content: &str) -> Vec<LocalSyntaxError> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let language = match ext {
        "rs" => tree_sitter_rust::LANGUAGE,
        "js" | "jsx" | "mjs" | "cjs" => tree_sitter_javascript::LANGUAGE,
        "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX,
        "py" => tree_sitter_python::LANGUAGE,
        "go" => tree_sitter_go::LANGUAGE,
        "json" => tree_sitter_json::LANGUAGE,
        _ => return vec![], // Unsupported language
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&language.into()).is_err() {
        return vec![];
    }

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return vec![],
    };

    let mut errors = Vec::new();
    collect_errors(tree.root_node(), content, &mut errors);
    errors
}

/// Recursively collect ERROR and MISSING nodes from the syntax tree.
fn collect_errors(
    node: tree_sitter::Node<'_>,
    _content: &str,
    errors: &mut Vec<LocalSyntaxError>,
) {
    if node.is_error() {
        let start = node.start_position();
        errors.push(LocalSyntaxError {
            line: (start.row + 1) as u32,
            column: (start.column + 1) as u32,
            message: format!(
                "Syntax error at {}:{}", start.row + 1, start.column + 1
            ),
        });
    } else if node.is_missing() {
        let start = node.start_position();
        errors.push(LocalSyntaxError {
            line: (start.row + 1) as u32,
            column: (start.column + 1) as u32,
            message: format!(
                "Missing '{}' at {}:{}",
                node.kind(),
                start.row + 1,
                start.column + 1
            ),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_errors(child, _content, errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_rust() {
        let errors = validate_file("test.rs", "fn main() { println!(\"hello\"); }");
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_invalid_rust() {
        let errors = validate_file("test.rs", "fn main() {{{");
        assert!(!errors.is_empty(), "Expected syntax errors");
    }

    #[test]
    fn test_valid_json() {
        let errors = validate_file("test.json", r#"{"key": "value"}"#);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_invalid_json() {
        let errors = validate_file("test.json", r#"{"key": }"#);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_unsupported_language() {
        let errors = validate_file("test.xyz", "random content");
        assert!(errors.is_empty()); // Unsupported returns empty
    }

    #[test]
    fn test_valid_python() {
        let errors = validate_file("test.py", "def hello():\n    print('hi')");
        assert!(errors.is_empty());
    }
}
