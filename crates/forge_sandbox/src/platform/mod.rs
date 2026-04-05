mod noop;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

use std::path::Path;

use crate::config::SandboxConfig;

/// Trait for platform-specific sandbox implementations.
pub trait Sandbox: Send + Sync {
    /// Wrap a shell command so it executes inside the sandbox.
    ///
    /// Returns the full command string that should be passed to the shell.
    fn wrap_command(&self, command: &str, working_dir: &Path) -> anyhow::Result<String>;

    /// Check whether the sandbox mechanism is available on this system.
    fn is_available(&self) -> bool;
}

/// Create the appropriate sandbox implementation for the current platform.
///
/// If sandboxing is disabled in the config, or the platform-specific sandbox is
/// not available, falls back to the [`NoopSandbox`] which passes commands through
/// unchanged.
pub fn create_sandbox(config: SandboxConfig) -> Box<dyn Sandbox> {
    if !config.enabled {
        tracing::debug!("sandboxing disabled by config, using noop");
        return Box::new(noop::NoopSandbox);
    }

    #[cfg(target_os = "macos")]
    {
        let sb = macos::MacOsSandbox::new(config);
        if sb.is_available() {
            tracing::info!("using macOS sandbox-exec sandbox");
            return Box::new(sb);
        }
        tracing::warn!("macOS sandbox-exec not available, falling back to noop");
    }

    #[cfg(target_os = "linux")]
    {
        let sb = linux::LinuxSandbox::new(config);
        if sb.is_available() {
            tracing::info!("using Linux bubblewrap sandbox");
            return Box::new(sb);
        }
        tracing::warn!("bubblewrap (bwrap) not available, falling back to noop");
    }

    #[cfg(target_os = "windows")]
    {
        let sb = windows::WindowsSandbox::new(config);
        if sb.is_available() {
            tracing::info!("using Windows sandbox (passthrough)");
            return Box::new(sb);
        }
    }

    Box::new(noop::NoopSandbox)
}
