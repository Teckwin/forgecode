use fake::Dummy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Settings for the auto-memory system.
///
/// Auto memory lets the agent write cross-session notes into
/// `<project>/.forge/memory/` and `~/.forge/memory/`. The `MEMORY.md` file
/// is always loaded at session start; additional topic files are discovered
/// automatically.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Dummy)]
pub struct MemorySettings {
    /// Whether the agent may automatically create and update memory files.
    #[serde(default = "default_true")]
    pub auto_memory_enabled: bool,
}

fn default_true() -> bool {
    true
}
