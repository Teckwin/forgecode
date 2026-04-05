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
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                path
            ));
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

        // Escape single quotes in profile and command for shell embedding
        let escaped_profile = profile.replace('\'', "'\\''");
        let escaped_command = command.replace('\'', "'\\''");

        Ok(format!(
            "sandbox-exec -p '{}' /bin/sh -c '{}'",
            escaped_profile, escaped_command
        ))
    }

    fn is_available(&self) -> bool {
        // sandbox-exec is available on macOS by default
        Path::new("/usr/bin/sandbox-exec").exists()
    }
}
