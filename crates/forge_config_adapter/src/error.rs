use std::path::PathBuf;

/// Errors that can occur during config adaptation.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("JSON parse error in {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("TOML parse error in {path}: {message}")]
    Toml { path: PathBuf, message: String },

    #[error("YAML parse error in {path}: {message}")]
    Yaml { path: PathBuf, message: String },

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("write not supported for read-only adapter: {0}")]
    ReadOnly(String),

    #[error("{0}")]
    Other(String),
}

impl AdapterError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }

    pub fn json(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::Json { path: path.into(), source }
    }

    pub fn toml(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Toml { path: path.into(), message: message.into() }
    }

    pub fn yaml(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Yaml { path: path.into(), message: message.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_display_contains_path() {
        let err = AdapterError::io(
            "/some/path.json",
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        );
        let msg = err.to_string();
        assert!(msg.contains("/some/path.json"), "msg was: {msg}");
        assert!(msg.contains("not found"), "msg was: {msg}");
    }

    #[test]
    fn json_error_display_contains_path() {
        // Create a real serde_json error by parsing invalid JSON
        let json_err = serde_json::from_str::<serde_json::Value>("{{bad}}").unwrap_err();
        let err = AdapterError::json("/config.json", json_err);
        let msg = err.to_string();
        assert!(msg.contains("/config.json"), "msg was: {msg}");
    }

    #[test]
    fn toml_error_display_contains_path_and_message() {
        let err = AdapterError::toml("/config.toml", "unexpected token");
        let msg = err.to_string();
        assert!(msg.contains("/config.toml"), "msg was: {msg}");
        assert!(msg.contains("unexpected token"), "msg was: {msg}");
    }

    #[test]
    fn yaml_error_display_contains_path_and_message() {
        let err = AdapterError::yaml("/perms.yaml", "invalid key");
        let msg = err.to_string();
        assert!(msg.contains("/perms.yaml"), "msg was: {msg}");
        assert!(msg.contains("invalid key"), "msg was: {msg}");
    }

    #[test]
    fn unsupported_format_display() {
        let err = AdapterError::UnsupportedFormat("cursor not supported".into());
        let msg = err.to_string();
        assert!(msg.contains("cursor not supported"), "msg was: {msg}");
    }

    #[test]
    fn read_only_display() {
        let err = AdapterError::ReadOnly("forge_legacy".into());
        let msg = err.to_string();
        assert!(msg.contains("forge_legacy"), "msg was: {msg}");
        assert!(msg.contains("read-only"), "msg was: {msg}");
    }

    #[test]
    fn other_error_display() {
        let err = AdapterError::Other("something went wrong".into());
        let msg = err.to_string();
        assert_eq!(msg, "something went wrong");
    }
}
