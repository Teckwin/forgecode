# Claude Code 配置适配器实现计划

## Objective

实现一个 ForgeConfigAdapter 适配器层，使 Forge 能够读取和兼容 Claude Code 的 settings.json 配置文件格式，同时保持 Forge 自身配置规范的独立性。

Claude Code 配置规范见：`/Users/ryota/works/claude-code/reports/claude-code-user-configuration-spec.md`

## Implementation Plan

### Phase 1: 分析与设计 (Tasks 1-3)
- [ ] 1. 创建适配器模块结构，在 forge_config crate 中创建新的 adapter 子模块
- [ ] 2. 定义 ClaudeCodeSettings 结构体对应 Claude Code 的 settings.json 格式
- [ ] 3. 创建 ForgeConfigAdapter trait 定义转换接口

### Phase 2: 核心解析器实现 (Tasks 4-8)
- [ ] 4. 实现 Claude Code 配置解析，支持 permissions 环境变量和 mcpServers
- [ ] 5. 实现权限规则转换器，解析 ToolName(pattern) 格式并转换为 Forge 的权限模型
- [ ] 6. 实现环境变量适配器，映射 Claude Code env 到 Forge 配置
- [ ] 7. 实现 MCP 服务器适配器，映射 mcpServers 到 Forge MCP 配置
- [ ] 8. 实现 Hooks 适配器，映射 PostToolUse 和 SessionStart 等类型

### Phase 3: 集成与CLI (Tasks 9-12)
- [ ] 9. 集成到 ConfigReader，添加 read_claude_code_settings 方法
- [ ] 10. 添加路径支持到 Environment，添加 claude_code_settings_path 方法
- [ ] 11. 添加配置迁移工具，实现 forge config migrate 命令
- [ ] 12. 编写测试用例验证权限规则转换和配置合并流程

## Verification Criteria

- Claude Code settings.json 配置文件能够被正确解析为 Forge 配置结构
- 权限规则（allow/deny/ask）正确转换为 Forge SandboxConfig 格式
- MCP 服务器配置（command/args/env）正确映射到 Forge MCP 配置
- 环境变量配置正确合并到 Forge 配置中
- 配置加载优先级正确：Forge 默认配置优先级最低，Claude Code 用户配置优先级最高
- 所有现有单元测试和集成测试继续通过

## Potential Risks and Mitigations

1. **配置格式差异导致的解析失败**
   - 风险：Claude Code 使用 JSON 格式，Forge 使用 TOML 和 YAML 格式，可能存在字段映射不完整的情况
   - 缓解：在适配器中实现完整的字段映射，并添加详细的日志记录未映射的字段供后续扩展

2. **权限模型差异**
   - 风险：Claude Code 的权限规则语法（ToolName(pattern)）与 Forge 的权限模型存在较大差异
   - 缓解：实现专门的权限规则解析器，支持通配符模式匹配，并提供详细的转换日志

3. **向后兼容性影响**
   - 风险：添加适配器可能影响现有的配置加载流程，导致现有用户配置失效
   - 缓解：适配器作为可选的额外配置层，不影响现有的 ForgeConfig 加载逻辑，用户需要主动启用

## Alternative Approaches

1. **完全兼容模式**：直接支持 settings.json 作为 Forge 配置格式的之一
   - 优点：用户可以直接使用 Claude Code 的配置文件，无需任何转换
   - 缺点：需要放弃 Forge 自身的部分配置特色，增加维护复杂度

2. **只读适配器模式**：仅读取 Claude Code 配置但不写入，保持配置格式独立
   - 优点：实现简单安全，不改变 Forge 现有的配置存储格式
   - 缺点：用户无法在 Forge 中直接编辑 Claude Code 格式的配置

3. **双向同步模式**：实现配置的双向转换和同步
   - 优点：最大灵活性，用户可以选择使用任一格式
   - 缺点：实现复杂度最高，需要处理配置冲突和优先级问题