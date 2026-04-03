# Claude Code Config Adapter Implementation Plan

## 1. Overview

This plan outlines the implementation of ForgeConfigAdapter to read Claude Code's `settings.json` configuration and convert it to Forge's configuration format. This enables users to share configurations between Claude Code and Forge while maintaining Forge's own configuration format as the authoritative source.

## 2. Background Analysis

### 2.1 Claude Code Configuration Specification

| Claude Code Config | Type | Description |
|-------------------|------|-------------|
| `permissions.allow/deny/ask` | String Array | Permission rules with `ToolName(pattern)` syntax |
| `defaultMode` | String | Default permission mode (default/plan/acceptEdits/dontAsk/auto) |
| `env` | Object | Environment variables to inject |
| `mcpServers` | Object | MCP server configurations |
| `hooks.PostToolUse` | Object | Post-tool-use hooks |
| `hooks.SessionStart` | Object | Session start hooks |
| `sandbox.enabled` | Boolean | Enable sandbox |
| `sandbox.additionalDirectories` | String Array | Additional allowed directories |

### 2.2 Forge Current Implementation

| Feature | Status | Notes |
|---------|--------|-------|
| **PermissionOperation** | ✅ Implemented | Write/Read/Execute/Fetch operations |
| **PolicyService** | ✅ Implemented | Permission checking via policy engine |
| **ToolCatalog::contains** | ✅ Implemented | Only checks built-in tools |
| **MCP Server Config** | ✅ Aligned | Supports stdio/http with command/args/env |
| **Sandbox Config** | ✅ Aligned | FilesystemSandboxConfig/NetworkSandboxConfig |
| **Lifecycle Events** | ✅ Aligned | Hook system with Start/End/ToolcallStart/ToolcallEnd |
| **Environment Variables** | ⚠️ Via ENV only | No YAML config support (intentional - via adapter) |

### 2.3 Gaps and Issues Identified

1. **Permission Pattern Matching**: Claude Code uses `ToolName(pattern)` syntax (e.g., `Bash(npm *)`), Forge uses exact matching
2. **MCP Tool Permissions**: MCP tools (`mcp_*`) bypass permission checking - **SECURITY ISSUE**
3. **Agent Tool Permissions**: Agent-as-Tool (`agent_*`) bypass permission checking - **SECURITY ISSUE**
4. **Hook Configuration**: No user-configurable hooks (only code-based)

## 3. Implementation Phases

### Phase 1: Permission System Alignment

- [ ] **1.1** Implement Claude Code permission pattern parser
  - Create `PermissionPattern` struct to parse `ToolName(pattern)` syntax
  - Support glob patterns: `Bash(npm *)`, `Bash(git *)`
  - Support MCP tools: `mcp__serverName__toolName`
  - Add `PatternMatcher` trait for extensible matching

- [ ] **1.2** Extend PolicyService to check MCP tool permissions
  - Add `PermissionOperation::McpExecute` variant
  - Add MCP tool name to permission operation
  - Implement pattern matching in PolicyEngine

- [ ] **1.3** Extend PolicyService to check Agent-as-Tool permissions
  - Add `PermissionOperation::AgentExecute` variant
  - Add agent name to permission operation

- [ ] **1.4** Add sandbox `additionalDirectories` support
  - Extend `FilesystemSandboxConfig` with `additional_directories` field

### Phase 2: ForgeConfigAdapter Implementation

- [ ] **2.1** Create adapter module structure
  ```
  crates/forge_config/src/
    adapter/
      mod.rs
      claude_code.rs      # Claude Code settings.json parser
      adapter.rs          # ForgeConfigAdapter trait
      converter.rs        # Conversion logic
  ```

- [ ] **2.2** Implement ClaudeCodeSettings struct
  ```rust
  pub struct ClaudeCodeSettings {
      pub permissions: PermissionsConfig,
      pub env: HashMap<String, String>,
      pub mcp_servers: HashMap<String, McpServerConfig>,
      pub hooks: HooksConfig,
      pub sandbox: SandboxConfig,
  }
  ```

- [ ] **2.3** Implement ForgeConfigAdapter trait
  ```rust
  pub trait ForgeConfigAdapter {
      fn adapt(&self, settings: ClaudeCodeSettings) -> AdaptResult<SettingConfig>;
      fn supported_versions() -> Vec<&'static str>;
  }
  ```

- [ ] **2.4** Implement conversion logic
  - permissions.allow/deny/ask → Policy rules
  - mcpServers → McpConfig
  - env → Inject into Environment at runtime (not YAML)
  - hooks → LifecycleEvent handlers (future)

### Phase 3: Integration

- [ ] **3.1** Update ConfigReader to support adapter
  - Add `read_with_adapter()` method
  - Add adapter selection logic

- [ ] **3.2** Update Environment paths
  - Add `claude_settings_path()` for Claude Code config location

- [ ] **3.3** Add `forge config migrate` CLI command
  - Detect Claude Code settings.json
  - Convert and merge into Forge settings.yaml
  - Support `--dry-run` for preview

### Phase 4: Hook Protocol Extension (Future)

- [ ] **4.1** Extend LifecycleEvent to support user configuration
  - Add `matcher` field (regex pattern)
  - Add `type` field (command/shell)
  - Add `statusMessage` field
  - Add `once` field

- [ ] **4.2** Add HookConfig to SettingConfig
  - Define hook configurations in YAML
  - Load and register hooks at startup

## 4. Security Considerations

1. **MCP Tool Permission Bypass**: This is a critical security issue - MCP tools currently bypass all permission checks
2. **Agent Tool Permission Bypass**: Similarly, Agent-as-Tool bypasses permission checks
3. **Pattern Matching Complexity**: Glob patterns could be exploited - implement rate limiting
4. **Adapter Trust Model**: Adapter runs with user permissions - validate all inputs

## 5. Testing Strategy

- [ ] Unit tests for PermissionPattern parser
- [ ] Integration tests for MCP tool permission checking
- [ ] Integration tests for Agent-as-Tool permission checking
- [ ] Adapter conversion tests with sample Claude Code settings
- [ ] CLI migrate command tests

## 6. Acceptance Criteria

1. ✅ Claude Code `settings.json` can be parsed
2. ✅ Permission patterns (`Bash(npm *)`) are correctly matched
3. ✅ MCP tools are subject to permission checks
4. ✅ Agent-as-Tool are subject to permission checks
5. ✅ `forge config migrate` command works correctly
6. ✅ All existing tests pass
7. ✅ No security regressions

## 7. Related Issues

- [ ] #10: Implement Claude Code Config Adapter
- [ ] #11: Fix MCP Tool Permission Bypass (Security)
- [ ] #12: Fix Agent-as-Tool Permission Bypass (Security)
- [ ] #13: Implement Permission Pattern Matching
- [ ] #14: Extend Hook Protocol for User Configuration