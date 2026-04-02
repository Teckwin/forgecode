# Forge 配置文件统一 + Tool Use 蜂群模式改造计划

## Objective

本方案包含两个核心目标：

1. **配置文件统一**：将散落在项目根目录的配置文件统一迁移到 `.forge/` 目录，合并为 `setting.yaml`
2. **Tool Use 蜂群模式**：实现 Agent 可作为 Tool 被调用的能力，支持串行/并行调用，为上层业务提供 Swarm 工作模式基础

本方案采用**最小侵入**设计，确保对现有系统破坏性最小，所有改动均为增量添加。

---

## 目录结构设计

项目目录结构如下：

- `.forge/agents/` - Agent 定义文件独立存放
- `.forge/skills/` - Skill 定义目录独立存放
- `.forge/commands/` - 命令定义文件独立存放
- `.forge/plans/` - 计划文件独立存放
- `.forge/setting.yaml` - 系统配置文件（Provider、MCP、全局设置）
- `.forge/tools/` - 自定义 Tool 定义目录（新增）
- `.forge/backup/` - 备份目录（doctor 命令使用）

旧配置文件位置（需要通过 doctor 命令迁移）：
- `forge.yaml` - 项目配置
- `.mcp.json` - 旧 MCP 配置

---

## setting.yaml 配置内容

仅包含系统配置相关的内容，分为以下章节：

- **provider** - LLM Provider 全局配置和 Agent 级别覆盖
- **mcp** - MCP 服务器配置
- **tools** - 自定义工具注册配置
- **system** - 全局系统配置（上下文压缩、日志、安全等）
- **doctor** - Doctor 命令配置

---

## Part 1: 配置文件统一改造

### Implementation Plan
## Implementation Plan

### Part 1: 配置文件统一改造

- [ ] 1. **分析现有配置文件分布情况**
  - 梳理当前位于 CWD 根目录的配置文件
  - 明确每个配置文件的作用和依赖关系
  - 确认需要迁移的文件清单
  - 参考: `crates/forge_domain/src/env.rs:275-360`

- [ ] 2. **设计新的 .forge 目录结构**
  - 创建统一的配置文件目录布局
  - 定义新的路径方法
  - 保持与现有 base_path 和 cwd 路径的一致性

- [ ] 3. **设计 setting.yaml 配置结构**
  - 定义 YAML 配置结构，包含 provider、mcp、tools、system、doctor 等章节
  - 使用 YAML 格式并添加友好注解

- [ ] 4. **修改 Environment 路径方法**
  - 在 env.rs 中新增/修改路径方法
  - 将 mcp_local_config 从 .mcp.json 迁移到 .forge/setting.yaml
  - 将 plans_path 从 plans/ 迁移到 .forge/plans/
  - 添加 setting_path 指向 .forge/setting.yaml
  - 添加 tools_path 指向 .forge/tools/

- [ ] 5. **创建 setting.yaml 解析模块**
  - 在 forge_domain 中创建 SettingConfig 结构
  - 定义 YAML 解析逻辑，支持所有现有配置项

- [ ] 6. **实现 cargo doctor 命令**
  - 在 CLI 中添加 doctor 子命令
  - 检测旧格式配置文件并提供迁移功能
  - 实现服务健康检查和自动修复功能

- [ ] 7. **保留 Agent 独立文件结构**
  - agent_cwd_path 继续指向 .forge/agents/
  - Agent 定义文件保持独立
  - 从 setting.yaml 读取 Agent 级别的 Provider 覆盖配置

- [ ] 8. **保留 Skill 独立文件结构**
  - local_skills_path 继续指向 .forge/skills/
  - Skill 定义保持独立目录结构

- [ ] 9. **更新 MCP 配置加载逻辑**
  - 从 setting.yaml 的 mcp 节读取配置
  - 移除独立的 .mcp.json 加载逻辑

- [ ] 10. **更新 Credentials 和 Provider 配置加载**
  - 从 setting.yaml 的 provider 节读取配置

- [ ] 11. **更新 Commands 加载路径**
  - 确保 command_path_local 指向 .forge/commands/

- [ ] 12. **更新 Plans 目录路径**
  - 将 plans 从 cwd/plans/ 迁移到 .forge/plans/

- [ ] 13. **实现 doctor 命令的迁移功能**
  - 检测旧格式配置文件并自动转换为 setting.yaml 格式
  - 备份旧文件到 .forge/backup/ 目录

- [ ] 14. **实现 doctor 命令的修复功能**
  - 检测损坏的配置文件并提供修复建议
  - 验证所有依赖服务的可用性

- [ ] 15. **删除旧配置文件加载逻辑**
  - 移除 env.rs 中所有旧路径的兼容检查方法
  - 简化配置加载流程

- [ ] 16. **更新 forge.default.yaml 模板**
  - 创建新的 setting.yaml 模板

- [ ] 17. **更新 forge.schema.json**
  - 更新 JSON Schema 以匹配新的 setting.yaml 结构

- [ ] 18. **更新 .gitignore 模板**
  - 添加新的配置路径到 .gitignore

- [ ] 19. **测试验证**
  - 运行 cargo check 确保编译通过
  - 运行 cargo insta test --accept 确保测试通过

### Part 2: Tool Use 蜂群模式实现

- [ ] 20. **实现 ToolScheduler 调度器**
  - 创建 tool_scheduler.rs
  - 实现串行执行策略
  - 实现并行执行策略

- [ ] 21. **扩展 AgentExecutor 支持调度策略**
  - 添加 execution_strategy 参数
  - 支持串行/并行调用子 Agent
  - 实现上下文传递机制

- [ ] 22. **实现 DynamicAgentRegistry**
  - 创建 dynamic_agent.rs
  - 实现运行时 Agent 创建/删除/更新
  - 实现持久化到 .forge/agents/ 目录

- [ ] 23. **实现 DynamicSkillRegistry**
  - 创建 dynamic_skill.rs
  - 实现运行时 Skill 创建/删除
  - 实现持久化到 .forge/skills/ 目录

- [ ] 24. **实现自定义 Tool 注册机制**
  - 创建 tool_registry_ext.rs
  - 实现 StdIO Tool 加载和执行
  - 实现 Rust Script Tool 执行器
  - 实现 HTTP Tool 执行器

- [ ] 25. **注册动态 Tool 为可用工具**
  - 在 ToolRegistry 初始化时加载自定义 Tool
  - 将动态创建的 Agent 注册为 Tool
  - 将动态创建的 Skill 注册为 Tool

- [ ] 26. **更新 setting.yaml 解析支持 tools 节**
  - 扩展 SettingConfig 结构
  - 解析 tools 配置并注册到 ToolRegistry

- [ ] 27. **实现 Swarm 工作流 Agent 模板**
  - 创建 swarm-orchestrator.yaml 模板
  - 定义 Swarm 配置结构

- [ ] 28. **集成测试**
  - 测试串行/并行 Tool 调用
  - 测试动态创建 Agent
  - 测试动态创建 Skill
  - 测试自定义 Tool 注册
  - 测试 Swarm 工作流
---

## 破坏性评估

### 最小侵入原则

本方案对现有系统的破坏性评估为**低**：

| 改动类型 | 影响范围 | 破坏性 |
|----------|----------|--------|
| 路径配置 | env.rs | 低（仅修改路径常量） |
| setting.yaml 解析 | forge_domain | 中（新增模块） |
| ToolRegistry 扩展 | forge_app | 低（增量添加） |
| AgentExecutor 扩展 | forge_app | 低（新增方法） |
| DynamicAgentRegistry | forge_app | 低（新增模块） |

### 不修改的内容

- 不修改现有的 Agent 定义加载逻辑
- 不修改现有的 Skill 定义加载逻辑
- 不修改现有的 MCP 执行逻辑
- 不修改现有的 ToolExecutor 内置工具

### 兼容性策略

- 旧配置文件通过 doctor 命令迁移
- 现有 Agent/Skill 文件保持不变
- 渐进式迁移，不强制删除旧逻辑

---

## Verification Criteria

### Part 1: 配置文件统一

- [ ] 配置文件统一位于 `.forge/setting.yaml`
- [ ] 所有 JSON 配置升级为 YAML 格式
- [ ] `cargo doctor` 命令可正常执行迁移
- [ ] `cargo doctor` 命令可检测并修复损坏配置

### Part 2: Tool Use 蜂群模式

- [ ] ToolScheduler 支持串行调用
- [ ] ToolScheduler 支持并行调用
- [ ] DynamicAgentRegistry 可动态创建 Agent
- [ ] DynamicSkillRegistry 可动态创建 Skill
- [ ] 自定义 Tool 可通过 setting.yaml 注册
- [ ] Agent 可作为 Tool 被调用
- [ ] 多 Agent 可串行/并行协作

### 通用

- [ ] 所有现有功能正常工作（Agent、Skill、MCP、Provider 等）
- [ ] 代码编译通过，无警告
- [ ] 测试用例全部通过

---

## Potential Risks and Mitigations

1. **用户现有配置丢失风险**
   - Mitigation: `cargo doctor` 提供完整的迁移功能，自动转换旧配置

2. **配置格式变更影响现有用户**
   - Mitigation: 提供清晰的迁移指南和 doctor 命令辅助

3. **YAML 解析兼容性问题**
   - Mitigation: 使用成熟的 serde_yaml 库，确保解析稳定

4. **动态 Agent 创建的安全性**
   - Mitigation: 添加权限检查，防止恶意 Agent 定义

5. **并行 Tool 调用的资源竞争**
   - Mitigation: 实现资源限制和超时控制

---

## Alternative Approaches

1. **保持多文件格式**: 不合并配置，保持 forge.yaml、mcp.json 等独立文件
   - Pros: 改动小，用户可选迁移
   - Cons: 配置仍然分散，注解无法统一

2. **强制迁移**: 直接迁移所有配置，删除旧格式支持
   - Pros: 目录结构清晰，统一管理
   - Cons: 破坏性变更大，需要完善迁移工具

3. **渐进式迁移（推荐）**: 新格式为主，提供 doctor 命令辅助迁移
   - Pros: 平滑过渡，逐步统一
   - Cons: 需要维护 doctor 命令

---

## Assumptions

- 用户接受 `.forge/setting.yaml` 作为标准配置文件
- 用户愿意使用 `cargo doctor` 进行迁移
- YAML 格式的配置文件更易于人工阅读和维护
- 上层业务需要通过 Agent-as-Tool 实现 Swarm 工作模式
- 自定义 Tool 需要支持 StdIO 和 Rust Script 两种形式