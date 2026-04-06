use std::path::Path;

use crate::config::SandboxConfig;

use super::Sandbox;

/// macOS sandbox using `sandbox-exec` with a generated seatbelt (.sb) profile.
pub struct MacOsSandbox {
    config: SandboxConfig,
}

impl MacOsSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// Generate a seatbelt profile string based on the sandbox configuration.
    fn generate_profile(&self, working_dir: &Path) -> String {
        let mut profile = String::new();
        profile.push_str("(version 1)\n");
        profile.push_str("(deny default)\n");

        // Allow basic process operations
        profile.push_str("(allow process-exec)\n");
        profile.push_str("(allow process-fork)\n");
        profile.push_str("(allow signal)\n");
        profile.push_str("(allow sysctl-read)\n");

        // Allow reading system libraries and common paths
        let system_readonly = [
            "/usr/lib/",
            "/usr/share/",
            "/usr/bin/",
            "/usr/sbin/",
            "/bin/",
            "/sbin/",
            "/dev/",
            "/Library/",
            "/System/",
            "/private/var/",
            "/private/etc/",
            "/etc/",
            "/var/",
            "/tmp/",
        ];

        for path in &system_readonly {
            profile.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", path));
        }

        // Allow reading from configured readonly paths
        for path in &self.config.readonly_paths {
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                path.display()
            ));
        }

        // Allow read+write to working directory
        let cwd = working_dir.to_string_lossy();
        profile.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", cwd));
        profile.push_str(&format!("(allow file-write* (subpath \"{}\"))\n", cwd));

        // Allow read+write to configured writable paths
        for path in &self.config.writable_paths {
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                path.display()
            ));
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                path.display()
            ));
        }

        // Network access
        if self.config.allow_network {
            profile.push_str("(allow network*)\n");
        } else {
            // Allow local IPC sockets but deny internet
            profile.push_str("(allow network-bind (local ip))\n");
            profile.push_str("(allow network-inbound (local ip))\n");
        }

        // Allow mach lookups (needed for many macOS operations)
        profile.push_str("(allow mach-lookup)\n");

        profile
    }
}

impl Sandbox for MacOsSandbox {
    fn wrap_command(&self, command: &str, working_dir: &Path) -> anyhow::Result<String> {
        let profile = self.generate_profile(working_dir);

        // Write profile to a temp file to avoid shell injection via inline embedding.
        // Using a file reference is safer than embedding the profile string in a shell command.
        let tmp = tempfile::NamedTempFile::new()?;
        let profile_path = tmp.into_temp_path();
        // Persist the temp file (removes auto-delete on drop) so it survives
        // until sandbox-exec reads it. The file remains on disk; /tmp cleanup
        // will eventually reclaim it.
        let persisted = profile_path.keep()?;
        std::fs::write(&persisted, profile.as_bytes())?;
        let profile_path_str = persisted.to_string_lossy().to_string();

        // Escape single quotes in command for shell embedding
        let escaped_command = command.replace('\'', "'\\''");

        Ok(format!(
            "sandbox-exec -f '{}' /bin/sh -c '{}'",
            profile_path_str, escaped_command
        ))
    }

    fn is_available(&self) -> bool {
        // sandbox-exec is available on macOS by default
        Path::new("/usr/bin/sandbox-exec").exists()
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
    fn profile_includes_cwd_as_read_write() {
        let sandbox = MacOsSandbox::new(make_config());
        let working_dir = Path::new("/my/working/dir");
        let profile = sandbox.generate_profile(working_dir);

        assert!(
            profile.contains("(allow file-read* (subpath \"/my/working/dir\"))"),
            "profile should allow file-read on working dir"
        );
        assert!(
            profile.contains("(allow file-write* (subpath \"/my/working/dir\"))"),
            "profile should allow file-write on working dir"
        );
    }

    #[test]
    fn profile_includes_readonly_paths_as_read_only() {
        let mut config = make_config();
        config.readonly_paths = vec![PathBuf::from("/data/shared"), PathBuf::from("/opt/tools")];
        let sandbox = MacOsSandbox::new(config);
        let profile = sandbox.generate_profile(Path::new("/project"));

        assert!(profile.contains("(allow file-read* (subpath \"/data/shared\"))"));
        assert!(profile.contains("(allow file-read* (subpath \"/opt/tools\"))"));
        // readonly_paths should NOT have file-write
        // Count occurrences of file-write for these paths (should be zero)
        assert!(
            !profile.contains("(allow file-write* (subpath \"/data/shared\"))"),
            "readonly paths should not get write access"
        );
        assert!(
            !profile.contains("(allow file-write* (subpath \"/opt/tools\"))"),
            "readonly paths should not get write access"
        );
    }

    #[test]
    fn profile_denies_network_when_allow_network_is_false() {
        let sandbox = MacOsSandbox::new(make_config());
        let profile = sandbox.generate_profile(Path::new("/project"));

        assert!(
            !profile.contains("(allow network*)"),
            "profile should not allow full network access"
        );
        assert!(
            profile.contains("(deny default)"),
            "profile should deny by default"
        );
    }

    #[test]
    fn profile_allows_network_when_allow_network_is_true() {
        let mut config = make_config();
        config.allow_network = true;
        let sandbox = MacOsSandbox::new(config);
        let profile = sandbox.generate_profile(Path::new("/project"));

        assert!(
            profile.contains("(allow network*)"),
            "profile should allow full network access"
        );
    }

    #[test]
    fn profile_includes_writable_paths_as_read_write() {
        let mut config = make_config();
        config.writable_paths = vec![PathBuf::from("/tmp/output")];
        let sandbox = MacOsSandbox::new(config);
        let profile = sandbox.generate_profile(Path::new("/project"));

        assert!(profile.contains("(allow file-read* (subpath \"/tmp/output\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/tmp/output\"))"));
    }

    #[test]
    fn wrap_command_produces_sandbox_exec_format() {
        let sandbox = MacOsSandbox::new(make_config());
        let result = sandbox
            .wrap_command("echo hello", Path::new("/project"))
            .expect("wrap_command should succeed");

        assert!(
            result.starts_with("sandbox-exec -f "),
            "should start with sandbox-exec -f (file-based profile): {}",
            result
        );
        assert!(result.contains("/bin/sh -c '"), "should invoke /bin/sh -c");
        assert!(
            result.contains("echo hello"),
            "should contain the original command"
        );
    }

    #[test]
    fn wrap_command_escapes_single_quotes_in_command() {
        let sandbox = MacOsSandbox::new(make_config());
        let result = sandbox
            .wrap_command("echo 'hello world'", Path::new("/project"))
            .expect("wrap_command should succeed");

        // Single quotes within the command should be escaped as '\''
        assert!(
            result.contains("echo '\\''hello world'\\''"),
            "single quotes in command should be escaped: {}",
            result
        );
    }
}
