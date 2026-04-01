# Sandbox 集成实现方案

## 一、需求概述

在工具调用时引入 `ai-sandbox` 包进行安全管控，同时确保对 Agent 的调用（如 `sage`, `forge` 等工具调用）不受影响。

## 二、当前架构分析

### 2.1 工具调用链路

```
┌─────────────────────────────────────────────────────────────────────┐
│                       工具调用执行链路                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Agent (LLM)                                                        │
│      │                                                              │
│      ▼                                                              │
│  ToolCall (sem_search, fs_search, shell, etc.)                     │
│      │                                                              │
│      ▼                                                              │
│  ToolExecutor::execute()                                           │
│      │                                                              │
│      ├──► ToolCatalog::Shell  ──► ShellService                     │
│      │                              │                               │
│      │                              ▼                               │
│      │                       ForgeCommandExecutorService           │
│      │                              │                               │
│      │                              ▼                               │
│      │                       tokio::process::Command               │
│      │                              │                               │
│      ├──► ToolCatalog::Read   ──► FsReadService                   │
│      ├──► ToolCatalog::Write  ──► FsWriteService                  │
│      ├──► ToolCatalog::Patch  ──► FsPatchService                  │
│      └──► ...                                                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 关键发现

1. **Shell 执行入口**: `ForgeCommandExecutorService` (forge_infra/src/executor.rs)
   - 使用 `tokio::process::Command` 执行系统命令
   - 这是最需要沙箱化的部分

2. **文件操作**: 通过 `FsReadService`, `FsWriteService`, `FsPatchService` 等
   - 当前没有访问控制
   - 可以通过沙箱策略限制

3. **Agent 调用**: 通过 `AgentRegistry` 服务
   - 不经过 ShellService
   - 需要在工具执行层面区分

## 三、ai-sandbox 能力分析

### 3.1 核心 API

```rust
// 创建沙箱管理器
let manager = SandboxManager::new();

// 定义命令
let command = SandboxCommand {
    program: OsString::from("ls"),
    args: vec!["-la".to_string()],
    cwd: PathBuf::from("/tmp"),
    env: HashMap::new(),
};

// 定义策略
let policy = SandboxPolicy::default();

// 创建沙箱化执行请求
let request = manager.create_exec_request(command, policy)?;
```

### 3.2 策略类型

| 策略类型 | 说明 |
|----------|------|
| `SandboxPolicy::Default()` | 默认策略，限制文件系统和网络访问 |
| `FileSystemSandboxPolicy` | 文件系统访问策略 |
| `NetworkSandboxPolicy` | 网络访问策略 |
| `SandboxType` | 平台特定：Landlock (Linux), Seatbelt (macOS), Restricted Token (Windows) |

### 3.3 执行模式

- **沙箱化执行**: `SandboxManager::execute(request)`
- **普通执行**: 直接使用 `tokio::process::Command`

## 四、实施方案

### 4.1 设计原则

1. **配置驱动**: 通过 YAML 配置控制是否启用沙箱
2. **工具分类**:
   - **需要沙箱**: Shell 执行、文件操作、删除等高风险操作
   - **不需要沙箱**: Agent 调用 (sage, forge)、读取配置、获取工具列表等
3. **优雅降级**: 沙箱不可用时回退到普通执行

### 4.2 配置结构

在 `forge.default.yaml` 中添加：

```yaml
# Sandbox 安全配置
sandbox:
  enabled: true  # 全局开关
  
  # Shell 命令执行配置
  shell:
    enabled: true
    # 允许的命令白名单（可选）
    allowed_commands:
      - "cargo"
      - "git"
      - "npm"
      - "pnpm"
      - "python"
      - "python3"
      - "node"
      - "rustc"
      - "go"
    # 禁止的命令列表
    blocked_commands:
      - "rm"
      - "del"
      - "format"
  
  # 文件系统访问配置
  filesystem:
    enabled: true
    # 允许的工作目录（限制在项目目录内）
    allowed_paths:
      - "${WORKSPACE}"
    # 禁止的路径模式
    blocked_paths:
      - "~/.ssh"
      - "~/.aws"
      - "**/id_rsa"
  
  # 网络访问配置
  network:
    enabled: false  # 默认禁止网络访问（fetch 工具可单独配置）
    allowed_domains:
      - "api.github.com"
```

### 4.3 修改点

| 文件 | 修改内容 |
|------|----------|
| `forge.default.yaml` | 添加 sandbox 配置 |
| `forge.schema.json` | 添加 sandbox 配置的 JSON Schema |
| `crates/forge_domain/src/config.rs` | 添加 SandboxConfig 结构体 |
| `crates/forge_infra/src/executor.rs` | 集成 ai-sandbox 执行 Shell |
| `crates/forge_app/src/tool_executor.rs` | 根据配置决定是否沙箱化 |

### 4.4 实现步骤

#### 步骤 1: 添加依赖

在 `Cargo.toml` (workspace) 中添加：

```toml
[dependencies.ai-sandbox]
version = "0.1.0"
features = ["process-hardening"]
```

#### 步骤 2: 创建 SandboxConfig

在 `forge_domain` 中创建沙箱配置结构：

```rust
// crates/forge_domain/src/sandbox_config.rs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SandboxConfig {
    pub enabled: bool,
    pub shell: Option<ShellSandboxConfig>,
    pub filesystem: Option<FilesystemSandboxConfig>,
    pub network: Option<NetworkSandboxConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShellSandboxConfig {
    pub enabled: bool,
    pub allowed_commands: Option<Vec<String>>,
    pub blocked_commands: Option<Vec<String>>,
}
```

#### 步骤 3: 集成到 Shell 执行器

修改 `ForgeCommandExecutorService`:

```rust
impl ForgeCommandExecutorService {
    pub fn new(env: Environment, ...) -> Self {
        // 初始化沙箱管理器
        let sandbox_manager = if env.sandbox_config().enabled {
            Some(SandboxManager::new())
        } else {
            None
        };
        Self { env, sandbox_manager, ... }
    }

    async fn execute_with_sandbox(&self, command: Command, ...) -> Result<CommandOutput> {
        if let Some(manager) = &self.sandbox_manager {
            // 使用沙箱执行
            let policy = self.build_sandbox_policy(&command);
            let request = manager.create_exec_request(command, policy)?;
            manager.execute(request).await?
        } else {
            // 回退到普通执行
            self.execute_command_internal(command, ...).await
        }
    }
}
```

#### 步骤 4: 区分 Agent 调用

在 `ToolExecutor` 中，需要区分：

1. **Shell 工具调用** - 需要沙箱化
2. **Agent 调用 (sage, forge)** - 不需要沙箱化

```rust
async fn call_internal(&self, tool_input: ToolCatalog, context: &ToolCallContext) -> ... {
    match tool_input {
        ToolCatalog::Shell(input) => {
            // Shell 执行需要沙箱化
            let output = self.services.shell(input).await?;
        }
        ToolCatalog::Sage(input) | ToolCatalog::Forge(input) => {
            // Agent 调用不需要沙箱化，直接执行
            let output = self.services.execute_agent(input).await?;
        }
        // 其他工具...
    }
}
```

### 4.5 风险控制

| 风险 | 缓解措施 |
|------|----------|
| 沙箱不可用 | 优雅降级到普通执行，记录警告日志 |
| 误拦截 Agent 调用 | Agent 调用路径与 Shell 分离，单独处理 |
| 性能影响 | 沙箱初始化只执行一次，命令执行路径优化 |
| 配置错误 | 添加配置验证，启动时检查策略有效性 |

### 4.6 Agent 调用保护机制

确保 `sage`, `forge` 等工具调用不受沙箱影响：

```rust
// 在 tool_executor.rs 中
async fn call_internal(&self, tool_input: ToolCatalog, ...) -> ... {
    match tool_input {
        // 这些工具直接调用 Agent，不经过 Shell
        ToolCatalog::Sage(input) => {
            let output = self.services.get_agent_response(input).await?;
        }
        ToolCatalog::Forge(input) => {
            let output = self.services.get_agent_response(input).await?;
        }
        // 其他工具使用沙箱
        _ => {
            if self.should_use_sandbox(&tool_input) {
                self.execute_with_sandbox(tool_input, ...).await
            } else {
                self.execute_normal(tool_input, ...).await
            }
        }
    }
}

fn should_use_sandbox(&self, tool: &ToolCatalog) -> bool {
    matches!(
        tool,
        ToolCatalog::Shell(_) |
        ToolCatalog::Write(_) |
        ToolCatalog::Remove(_) |
        ToolCatalog::Patch(_)
    )
}
```

## 五、验证计划

1. **单元测试**:
   - 测试沙箱配置解析
   - 测试命令白名单/黑名单
   - 测试沙箱不可用时的降级

2. **集成测试**:
   - 测试 Shell 命令沙箱化执行
   - 测试 Agent 调用不受影响
   - 测试文件系统限制

3. **手动测试**:
   - 使用真实 API 测试 `echo "test" | cargo run --`
   - 测试 `sage` 工具调用
   - 验证沙箱日志输出

## 六、总结

| 特性 | 实现方式 |
|------|----------|
| 配置驱动 | YAML 配置控制开关和策略 |
| 工具分类 | Shell/文件操作沙箱化，Agent 调用保持原样 |
| 优雅降级 | 沙箱不可用时回退普通执行 |
| Agent 保护 | 独立执行路径，不经过沙箱 |