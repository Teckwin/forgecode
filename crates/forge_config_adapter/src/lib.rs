//! Forge Config Adapter
//!
//! This module provides automatic detection and conversion of external configuration
//! formats (like Claude Code's settings.json) into Forge's configuration format.
//!
//! ## Auto-Detection Flow
//!
//! 1. On startup, the adapter scans for known config files:
//!    - Claude Code: `~/.claude/settings.json`, `./.claude/settings.json`
//!    - Claude Code: `~/.claude/CLAUDE.md`, `./CLAUDE.md`
//!    - Claude Code: `~/.claude/rules/*.md`, `./.claude/rules/*.md`
//!    - Other ecosystem configs (future)
//!
//! 2. If detected, it automatically converts and merges into Forge's settings.yaml
//!
//! 3. Users don't need to run any additional commands - it just works!

mod claude_code;
mod claude_md;
mod detector;
mod migrate;
mod rules;

pub use claude_code::*;
pub use claude_md::*;
pub use detector::*;
pub use migrate::*;
pub use rules::*;
