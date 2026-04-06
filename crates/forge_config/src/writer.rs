use std::path::Path;

use crate::ForgeConfig;

/// Writes a [`ForgeConfig`] to the user configuration file on disk.
pub struct ConfigWriter {
    config: ForgeConfig,
}

impl ConfigWriter {
    /// Creates a new `ConfigWriter` for the given configuration.
    pub fn new(config: ForgeConfig) -> Self {
        Self { config }
    }

    /// Serializes and writes the configuration to `path`, creating all parent
    /// directories recursively if they do not already exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be serialized or the file
    /// cannot be written.
    pub fn write(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml_edit::ser::to_string_pretty(&self.config)?;

        std::fs::write(path, contents)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writer_creates_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("forge.toml");

        let config = ForgeConfig::default();
        let writer = ConfigWriter::new(config);
        writer.write(&path).unwrap();

        assert!(path.is_file(), "Config file should be created");

        // Verify the file is valid TOML
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: toml_edit::DocumentMut = content.parse().expect("Should be valid TOML");
        // ForgeConfig::default() should produce a non-empty document
        assert!(!parsed.to_string().is_empty());
    }

    #[test]
    fn test_writer_creates_parent_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("deep").join("forge.toml");

        // Parent directories do not exist yet
        assert!(!path.parent().unwrap().exists());

        let config = ForgeConfig::default();
        let writer = ConfigWriter::new(config);
        writer.write(&path).unwrap();

        assert!(
            path.is_file(),
            "Config file should be created in nested dirs"
        );
        assert!(
            path.parent().unwrap().exists(),
            "Parent dirs should be created"
        );
    }
}
