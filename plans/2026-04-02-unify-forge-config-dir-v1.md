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

- [x] 1. **分析现有配置文件分布情况**
  - 梳理当前位于 CWD 根目录的配置文件
  - 明确每个配置文件的作用和依赖关系
  - 确认需要迁移的文件清单
  - 参考: `crates/forge_domain/src/env.rs:275-360`

- [x] 2. **设计新的 .forge 目录结构**
  - 创建统一的配置文件目录布局
  - 定义新的路径方法
  - 保持与现有 base_path 和 cwd 路径的一致性

- [ ] 3. **设计 setting.yaml 配置结构**
- [x] 3. **设计 setting.yaml 配置结构**
  - 定义 YAML 结构（agents、skills、mcp、provider、doctor 等节）
  - 参考 forge.default.yaml 和 forge.schema.json
  - 确定必填和可选字段

- [x] 4. **修改 Environment 路径方法**
  - 添加 `setting_path()` 方法返回 `.forge/setting.yaml`
  - 添加 `tools_path()` 方法返回 `.forge/tools/`
  - 保留现有路径方法以保持向后兼容

- [x] 5. **创建 setting.yaml 解析模块**
  - 在 forge_domain 中添加 SettingConfig 结构体
  - 实现 serde_yaml Deserialize
  - 添加单元测试验证解析逻辑

- [x] 6. **实现 cargo doctor 命令**
  - 在 CLI 中添加 doctor-config / dc 子命令
  - 添加 --fix 和 --verbose 选项
  - 实现配置检测和修复逻辑
- [ ] 7. **保留 Agent 独立文件结构**
- [x] 7. **保留 Agent 独立文件结构**
  - agent_cwd_path 继续指向 .forge/agents/
  - Agent 定义文件保持独立

- [x] 8. **保留 Skill 独立文件结构**
  - local_skills_path 继续指向 .forge/skills/
  - Skill 定义保持独立目录结构

- [x] 12. **更新 Plans 目录路径**
- [x] 9. **更新 MCP 配置加载逻辑**
  - 从 setting.yaml 的 mcp 节读取配置
  - SettingConfig 已支持 mcp 字段解析

- [x] 10. **更新 Credentials 和 Provider 配置加载**
  - 从 setting.yaml 的 provider 节读取配置
  - SettingConfig 已支持 provider 字段解析

- [x] 13. **实现 doctor 命令的迁移功能**
  - 检测旧 .mcp.json 并迁移到 setting.yaml
  - 已实现 doctor-config --fix 功能

- [x] 14. **实现 doctor 命令的修复功能**
  - 备份旧配置文件
  - 自动创建 .forge/ 目录结构
- [x] 16. **更新 forge.default.yaml 模板**
  - 添加 provider、mcp、system、doctor 配置节
  - 添加详细注释说明每个配置项

- [x] 17. **更新 forge.schema.json**
  - forge.schema.json 已包含完整的配置验证
  - setting.yaml 使用 forge.default.yaml 作为模板

- [x] 18. **更新 .gitignore 模板**
  - 备份目录由 doctor.auto_fix 配置控制

- [x] 19. **测试验证**
  - 运行完整测试套件
  - 验证配置加载正确
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