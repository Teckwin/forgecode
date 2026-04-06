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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_config() -> SandboxConfig {
        SandboxConfig {
            cwd: PathBuf::from("/project"),
            readonly_paths: vec![],
            writable_paths: vec![],
            allow_network: false,
            enabled: true,
        }
    }

    #[test]
    fn wrap_command_starts_with_bwrap() {
        let sandbox = LinuxSandbox::new(make_config());
        let result = sandbox
            .wrap_command("echo hello", Path::new("/project"))
            .expect("wrap_command should succeed");

        assert!(
            result.starts_with("bwrap "),
            "wrapped command should start with bwrap"
        );
    }

    #[test]
    fn wrap_command_contains_unshare_all() {
        let sandbox = LinuxSandbox::new(make_config());
        let result = sandbox
            .wrap_command("ls", Path::new("/project"))
            .expect("wrap_command should succeed");

        assert!(
            result.contains("--unshare-all"),
            "should include --unshare-all for namespace isolation"
        );
    }

    #[test]
    fn share_net_only_when_allow_network_is_true() {
        // Without network
        let sandbox = LinuxSandbox::new(make_config());
        let result = sandbox
            .wrap_command("ls", Path::new("/project"))
            .expect("wrap_command should succeed");
        assert!(
            !result.contains("--share-net"),
            "should not have --share-net when network is disabled"
        );

        // With network
        let mut config = make_config();
        config.allow_network = true;
        let sandbox = LinuxSandbox::new(config);
        let result = sandbox
            .wrap_command("ls", Path::new("/project"))
            .expect("wrap_command should succeed");
        assert!(
            result.contains("--share-net"),
            "should have --share-net when network is enabled"
        );
    }

    #[test]
    fn readonly_paths_get_ro_bind_flags() {
        let mut config = make_config();
        config.readonly_paths = vec![
            PathBuf::from("/data/shared"),
            PathBuf::from("/opt/tools"),
        ];
        let sandbox = LinuxSandbox::new(config);
        let args = sandbox.build_bwrap_args("ls", Path::new("/project"));

        // Find all --ro-bind entries and check our paths are included
        let ro_bind_pairs: Vec<_> = args
            .windows(3)
            .filter(|w| w[0] == "--ro-bind")
            .map(|w| (w[1].clone(), w[2].clone()))
            .collect();

        assert!(
            ro_bind_pairs
                .iter()
                .any(|(src, dst)| src == "/data/shared" && dst == "/data/shared"),
            "should have --ro-bind for /data/shared"
        );
        assert!(
            ro_bind_pairs
                .iter()
                .any(|(src, dst)| src == "/opt/tools" && dst == "/opt/tools"),
            "should have --ro-bind for /opt/tools"
        );
    }

    #[test]
    fn writable_paths_get_bind_flags() {
        let mut config = make_config();
        config.writable_paths = vec![PathBuf::from("/tmp/output")];
        let sandbox = LinuxSandbox::new(config);
        let args = sandbox.build_bwrap_args("ls", Path::new("/project"));

        // Find --bind entries (but not --ro-bind)
        let bind_pairs: Vec<_> = args
            .windows(3)
            .filter(|w| w[0] == "--bind")
            .map(|w| (w[1].clone(), w[2].clone()))
            .collect();

        assert!(
            bind_pairs
                .iter()
                .any(|(src, dst)| src == "/tmp/output" && dst == "/tmp/output"),
            "should have --bind for /tmp/output"
        );
    }

    #[test]
    fn working_dir_gets_bind_mount_and_chdir() {
        let sandbox = LinuxSandbox::new(make_config());
        let args = sandbox.build_bwrap_args("ls", Path::new("/my/workdir"));

        // Should have --bind for working dir
        let bind_pairs: Vec<_> = args
            .windows(3)
            .filter(|w| w[0] == "--bind")
            .map(|w| (w[1].clone(), w[2].clone()))
            .collect();
        assert!(
            bind_pairs
                .iter()
                .any(|(src, dst)| src == "/my/workdir" && dst == "/my/workdir"),
            "should have --bind for working directory"
        );

        // Should have --chdir
        let chdir_idx = args.iter().position(|a| a == "--chdir");
        assert!(chdir_idx.is_some(), "should have --chdir flag");
        assert_eq!(args[chdir_idx.unwrap() + 1], "/my/workdir");
    }

    #[test]
    fn command_is_passed_via_bin_sh() {
        let sandbox = LinuxSandbox::new(make_config());
        let args = sandbox.build_bwrap_args("echo test", Path::new("/project"));

        let len = args.len();
        assert!(len >= 3);
        assert_eq!(args[len - 3], "/bin/sh");
        assert_eq!(args[len - 2], "-c");
        assert_eq!(args[len - 1], "echo test");
    }
}
