use std::path::Path;

use super::Sandbox;

/// A no-op sandbox that passes commands through without any isolation.
pub struct NoopSandbox;

impl Sandbox for NoopSandbox {
    fn wrap_command(&self, command: &str, _working_dir: &Path) -> anyhow::Result<String> {
        Ok(command.to_string())
    }

    fn is_available(&self) -> bool {
        true
    }
}
