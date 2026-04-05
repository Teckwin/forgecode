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
