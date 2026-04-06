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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_command_returns_original_command_unchanged() {
        let sandbox = NoopSandbox;
        let cmd = "echo hello && ls -la /tmp";
        let result = sandbox
            .wrap_command(cmd, Path::new("/some/dir"))
            .expect("wrap_command should succeed");
        assert_eq!(result, cmd);
    }

    #[test]
    fn wrap_command_preserves_special_characters() {
        let sandbox = NoopSandbox;
        let cmd = "echo 'single quotes' \"double quotes\" $VAR";
        let result = sandbox
            .wrap_command(cmd, Path::new("/"))
            .expect("wrap_command should succeed");
        assert_eq!(result, cmd);
    }

    #[test]
    fn is_available_returns_true() {
        let sandbox = NoopSandbox;
        assert!(sandbox.is_available());
    }
}
