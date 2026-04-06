use std::path::Path;

use crate::error::AdapterError;
use crate::normalized::NormalizedConfig;

/// Stub adapter for Cursor IDE configuration.
///
/// Currently not implemented — `detect` always returns false.
pub struct CursorAdapter;

impl crate::ConfigAdapter for CursorAdapter {
    fn tool_name(&self) -> &str {
        "cursor"
    }

    fn detect(&self, _project_dir: &Path) -> bool {
        false
    }

    fn read(&self, _project_dir: &Path) -> Result<NormalizedConfig, AdapterError> {
        Ok(NormalizedConfig::default())
    }

    fn write(&self, _project_dir: &Path, _config: &NormalizedConfig) -> Result<(), AdapterError> {
        Err(AdapterError::UnsupportedFormat(
            "Cursor adapter is not yet implemented".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigAdapter;
    use tempfile::TempDir;

    #[test]
    fn detect_always_returns_false() {
        let tmp = TempDir::new().unwrap();
        assert!(!CursorAdapter.detect(tmp.path()));
    }

    #[test]
    fn read_returns_default_config() {
        let tmp = TempDir::new().unwrap();
        let config = CursorAdapter.read(tmp.path()).unwrap();
        assert!(config.model.is_none());
        assert!(config.provider.is_none());
        assert!(config.agents.is_empty());
        assert!(config.mcp_servers.is_empty());
    }

    #[test]
    fn write_returns_unsupported_format_error() {
        let tmp = TempDir::new().unwrap();
        let config = NormalizedConfig::default();
        let result = CursorAdapter.write(tmp.path(), &config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AdapterError::UnsupportedFormat(_)),
            "Expected UnsupportedFormat error, got: {err:?}"
        );
    }

    #[test]
    fn tool_name_returns_cursor() {
        assert_eq!(CursorAdapter.tool_name(), "cursor");
    }
}
