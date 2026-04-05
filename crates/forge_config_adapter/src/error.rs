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
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn json(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.into(),
            source,
        }
    }

    pub fn toml(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Toml {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn yaml(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Yaml {
            path: path.into(),
            message: message.into(),
        }
    }
}
