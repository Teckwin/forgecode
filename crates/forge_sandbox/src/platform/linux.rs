use std::path::Path;

use crate::config::SandboxConfig;

use super::Sandbox;

/// Linux sandbox using bubblewrap (`bwrap`).
pub struct LinuxSandbox {
    config: SandboxConfig,
}

impl LinuxSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// Build the full bwrap command line arguments.
    fn build_bwrap_args(&self, command: &str, working_dir: &Path) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        // Namespace isolation
        args.push("--unshare-all".to_string());

        // Optionally re-share network
        if self.config.allow_network {
            args.push("--share-net".to_string());
        }

        // Mount /dev and /proc
        args.push("--dev".to_string());
        args.push("/dev".to_string());
        args.push("--proc".to_string());
        args.push("/proc".to_string());

        // Read-only bind mounts for system paths
        let system_paths = [
            "/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc",
        ];
        for path in &system_paths {
            if Path::new(path).exists() {
                args.push("--ro-bind".to_string());
                args.push(path.to_string());
                args.push(path.to_string());
            }
        }

        // Read-only bind mounts from config
        for path in &self.config.readonly_paths {
            let p = path.to_string_lossy().to_string();
            args.push("--ro-bind".to_string());
            args.push(p.clone());
            args.push(p);
        }

        // Read-write bind for working directory
        let cwd_str = working_dir.to_string_lossy().to_string();
        args.push("--bind".to_string());
        args.push(cwd_str.clone());
        args.push(cwd_str.clone());

        // Read-write bind mounts from config
        for path in &self.config.writable_paths {
            let p = path.to_string_lossy().to_string();
            args.push("--bind".to_string());
            args.push(p.clone());
            args.push(p);
        }

        // Tmpfs for /tmp
        args.push("--tmpfs".to_string());
        args.push("/tmp".to_string());

        // Set working directory
        args.push("--chdir".to_string());
        args.push(cwd_str);

        // The command to run
        args.push("/bin/sh".to_string());
        args.push("-c".to_string());
        args.push(command.to_string());

        args
    }
}

impl Sandbox for LinuxSandbox {
    fn wrap_command(&self, command: &str, working_dir: &Path) -> anyhow::Result<String> {
        let args = self.build_bwrap_args(command, working_dir);

        // Build the full command string with proper quoting
        let quoted_args: Vec<String> = args
            .iter()
            .map(|arg| {
                if arg.contains(' ') || arg.contains('\'') || arg.contains('"') {
                    format!("'{}'", arg.replace('\'', "'\\''"))
                } else {
                    arg.clone()
                }
            })
            .collect();

        Ok(format!("bwrap {}", quoted_args.join(" ")))
    }

    fn is_available(&self) -> bool {
        Path::new("/usr/bin/bwrap").exists() || Path::new("/usr/local/bin/bwrap").exists()
    }
}
