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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn create_sandbox_returns_sandbox_instance() {
        let config = SandboxConfig::default();
        let sandbox = create_sandbox(config);
        // Should be able to call trait methods on the returned box
        let _ = sandbox.is_available();
    }

    #[test]
    fn disabled_config_returns_noop_sandbox() {
        let config = SandboxConfig {
            cwd: PathBuf::from("/project"),
            readonly_paths: vec![],
            writable_paths: vec![],
            allow_network: false,
            enabled: false,
        };
        let sandbox = create_sandbox(config);

        // A noop sandbox returns the command unchanged
        let cmd = "echo test";
        let result = sandbox
            .wrap_command(cmd, Path::new("/project"))
            .expect("wrap_command should succeed");
        assert_eq!(
            result, cmd,
            "disabled sandbox should return the command unchanged (noop behavior)"
        );
    }

    #[test]
    fn enabled_sandbox_is_available() {
        let config = SandboxConfig::default();
        let sandbox = create_sandbox(config);
        // On any platform, create_sandbox either returns the platform sandbox
        // or falls back to noop, both of which report is_available() == true
        assert!(sandbox.is_available());
    }

    #[test]
    fn wrap_command_produces_valid_shell_syntax() {
        let config = SandboxConfig {
            cwd: PathBuf::from("/workspace"),
            readonly_paths: vec![PathBuf::from("/usr/share")],
            writable_paths: vec![PathBuf::from("/tmp/out")],
            allow_network: false,
            enabled: true,
        };
        let sandbox = create_sandbox(config);
        let result = sandbox
            .wrap_command("echo hello && ls", Path::new("/workspace"))
            .expect("wrap_command should succeed");

        // The result should be a non-empty string that is valid for passing to a shell
        assert!(!result.is_empty());
        // It should contain the original command somewhere
        assert!(
            result.contains("echo hello"),
            "wrapped command should contain the original command text"
        );
    }
}
