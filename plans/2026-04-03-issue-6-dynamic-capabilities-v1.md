# Issue #6 动态能力扩展 - 实现计划

## Objective

根据 Issue #6 讨论结论，实现以下核心功能：

1. **Agent-as-Tool** - Agent 可作为 Tool 被其他 Agent 调用
2. **动态 Agent 注册** - 运行时动态创建/删除 Agent (基于已有 DynamicAgentRegistry)
3. **动态 Skill 注册** - 运行时动态创建/删除 Skill (基于已有 DynamicSkillRegistry)
4. **自定义 Tool 注册** - 支持 StdIO/Rust Script/HTTP 三种类型
5. **ToolScheduler** - 支持串行/并行 Tool 调用

## Implementation Plan

- [ ] 1. 扩展 AgentDefinition 支持作为 Tool 调用
  - 在 AgentDefinition 中添加 AsTool 相关的配置字段
  - 实现 Agent::to_tool_definition() 方法用于转换为 Tool 定义

- [ ] 2. 实现 ToolScheduler 服务
  - 支持串行执行模式：结果传递给下一个 Agent
  - 支持并行执行模式：多 Agent 同时执行
  - 定义 ToolScheduler trait 和实现

- [ ] 3. 完善 DynamicAgentRegistry
  - 添加运行时创建/删除 Agent 的方法
  - 支持持久化到文件系统
  - 添加单元测试

- [ ] 4. 完善 DynamicSkillRegistry  
  - 添加运行时创建/删除 Skill 的方法
  - 支持持久化到文件系统
  - 添加单元测试

- [ ] 5. 实现自定义 Tool 注册功能
  - 定义 ToolDefinition 支持 StdIO/Rust Script/HTTP 三种类型
  - 实现 ToolRegistry 扩展支持动态注册
  - 添加安全验证机制

- [ ] 6. 集成到 ForgeAPI
  - 在 ForgeAPI 初始化时注册各种服务
  - 暴露 API 接口用于动态管理

- [ ] 7. 添加单元测试和集成测试
  - Agent-as-Tool 测试
  - ToolScheduler 串行/并行测试
  - 动态注册测试

## Verification Criteria

- [ ] cargo fmt 通过
- [ ] cargo clippy 无警告
- [ ] cargo test 全部通过
- [ ] Agent 可作为 Tool 被其他 Agent 调用
- [ ] 支持串行和并行执行模式
- [ ] 支持动态创建/删除 Agent 和 Skill

## Potential Risks and Mitigations

1. **与现有 AgentExecutor 集成复杂度**
   Mitigation: 复用现有 AgentExecutor，添加新方法而非修改现有逻辑

2. **安全风险 - 自定义 Tool**
   Mitigation: 添加沙箱限制和权限验证，使用 Skill-as-Tool 替代动态 Tool 注册

3. **状态持久化一致性**
   Mitigation: 使用事务性写入，确保操作原子性

## Alternative Approaches

1. **完整 Swarm Mode 实现**: 需要重构 Orchestrator 架构
   - 优点: 功能完整
   - 缺点: 破坏性大，开发周期长

2. **Agent-as-Tool 组合模式** (选择方案): 通过 Agent 定义配置实现协作
   - 优点: 最小侵入，增量开发
   - 缺点: 需要顶层封装协调

选择方案2，遵循最小侵入原则。