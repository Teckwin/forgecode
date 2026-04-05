use std::path::Path;

use crate::config::SandboxConfig;

use super::Sandbox;

/// Windows sandbox — currently a passthrough stub.
///
/// Windows does not yet have a lightweight sandbox mechanism comparable to
/// macOS sandbox-exec or Linux bubblewrap, so commands are passed through
/// unchanged.
pub struct WindowsSandbox {
    #[allow(dead_code)]
    config: SandboxConfig,
}

impl WindowsSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }
}

impl Sandbox for WindowsSandbox {
    fn wrap_command(&self, command: &str, _working_dir: &Path) -> anyhow::Result<String> {
        tracing::warn!("Windows sandbox is a passthrough stub; command runs unsandboxed");
        Ok(command.to_string())
    }

    fn is_available(&self) -> bool {
        // Always report available so it's selected on Windows, even though it's
        // just a passthrough.
        true
    }
}
