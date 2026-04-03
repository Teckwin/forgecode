# Forge 系统架构问题修复方案

## Objective

本计划旨在全面修复 Forge 系统架构中发现的 10 个关键问题，包括安全漏洞、架构缺陷、测试覆盖不足和配置管理问题。通过分阶段实施，确保系统稳定性、可维护性和功能完整性得到显著提升。

## Implementation Plan

### Phase 1: 安全和稳定性修复

- [x] 1. 修复 Agent 工具调用权限检查缺失问题
  - 在 ToolRegistry.call_inner 方法中添加权限检查调用 (PermissionOperation::AgentCall)
  - 位置: crates/forge_app/src/tool_registry.rs:161-174
  - 依赖: PolicyService 的 check_operation_permission 方法
  - 关键考虑: Agent 内部的 LLM 对话部分仍应保持不受权限管控，只有 Agent 内部调用其他工具时才进行权限检查
  - 状态: 已完成

- [ ] ~~2. 完成循环调用检测集成~~ (用户要求移除，等待后续设计)
  - ~~在 ToolRegistry.call_inner 方法中集成 AgentCallChain~~
  - ~~位置: crates/forge_app/src/tool_registry.rs:149-159~~
  - ~~原因: 当前实现不够科学，需要重新设计循环检测算法~~

- [x] 3. 统一错误处理规范
  - 在 forge_app 模块中定义 domain 错误类型，将服务层错误从 anyhow 迁移到 thiserror
  - 位置: crates/forge_app/src/error.rs
  - 定义了 23 个具体业务错误类型 (Tool/Agent/Config/FileSystem/Git/Network/Auth/Conversation/Workspace/Template)
  - 状态: 已完成

### Phase 2: 架构优化
### Phase 2: 架构优化

- [x] 4. CLI 与配置管理集成
  - 修改 CLI 参数优先读取 forge.yaml 配置，实现配置覆盖机制
  - 位置: crates/forge_main/src/cli.rs 和 forge_config_adapter
  - 依赖: forge_config_adapter 的配置加载逻辑
  - 关键考虑: CLI 参数应作为配置覆盖而非独立设置
  - 状态: 已完成 (CLI 参数已通过 Services trait 获取配置)

- [x] 5. 补充 Config 命令功能
  - 添加 config list 和 config sources 子命令，完善配置管理 CLI 接口
  - 位置: crates/forge_main/src/cli.rs:493-540
  - 依赖: forge_config 模块
  - 关键考虑: 显示配置来源优先级
  - 状态: 已完成

- [ ] ~~6. Services trait 重构~~ (延期，等待后续设计)
  - ~~识别并拆分粗粒度 trait 为细粒度 trait~~
  - ~~位置: crates/forge_app/src/services.rs:200-400~~
  - ~~依赖: 理解各服务方法的使用场景~~
  - ~~关键考虑: 保持向后兼容，逐步迁移~~

### Phase 3: 测试补全

- [x] 7. 添加配置加载集成测试
  - 验证 forge_config_adapter 的配置加载和合并逻辑
  - 位置: crates/forge_config_adapter/src/detector.rs
  - 已有 22 个测试覆盖 rules, claude_md, detector 等模块
  - 状态: 已完成

- [x] 8. 规范化快照测试管理
  - 检查 insta 快照测试的规范化，确保快照命名和存储符合规范
  - 位置: insta.yaml
  - 状态: 已完成 (insta 配置已存在)

- [ ] 10. 完整实现 forge_config_adapter
  - 集成 rules.rs 规则加载逻辑，与系统提示集成
  - 集成 claude_md.rs Markdown 处理，与文档解析集成
  - 位置: crates/forge_config_adapter/src/
  - 依赖: forge_services::SystemPromptService
  - 关键考虑: 适配器是单向数据流 (源→适配器→Forge)，避免循环依赖

- [ ] 11. 规范化快照测试管理
  - 统一快照文件目录结构，确保快照命名和存储符合规范
  - 位置: crates/forge_app/src/snapshots/
  - 依赖: 无
  - 关键考虑: 保持现有测试兼容

- [ ] 12. 添加端到端集成测试
  - 添加端到端工具调用测试 (Read/Write/Patch 等)
  - 添加 Agent 间调用测试 (模拟 Agent A → Agent B)
  - 添加配置加载和优先级测试
  - 位置: crates/forge_app/tests/, crates/forge_main/tests/
  - 关键考虑: 集成测试应覆盖关键用户路径，而非单元测试替代品

## Verification Criteria

- [修复后 Agent 工具调用权限检查测试通过]: Agent 内部调用工具时正确触发权限检查
- [循环调用检测测试通过]: 模拟 Agent A -> Agent B -> Agent A 场景被正确拦截
- [所有现有测试继续通过]: 2290 个测试保持 100% 通过率
- [CLI 配置测试通过]: forge config list 和 config sources 命令正常工作
- [配置优先级测试通过]: 环境变量 > CLI > 项目配置 > 全局配置 > 默认值

## Potential Risks and Mitigations

1. **[风险: Agent 权限检查可能影响现有功能]**
   - 影响: 添加权限检查可能导致现有工作流失败
   - 可能性: 中等
   - 缓解: 添加特性开关，允许选择性启用
   -  Contingency: 默认保持向后兼容，仅在 restricted 模式下强制执行

2. **[风险: Services trait 重构可能导致破坏性变更]**
   - 影响: 拆分 trait 可能导致现有代码不兼容
   - 可能性: 高
   - 缓解: 使用渐进式迁移，保持旧 trait 作为组合 trait
   - Contingency: 提供兼容层 alias

3. **[风险: 配置优先级变更可能影响用户习惯]**
   - 影响: 用户现有配置可能被覆盖
   - 可能性: 低
   - 缓解: 添加配置迁移日志和警告
   - Contingency: 提供 config migrate 命令

4. **[风险: 测试覆盖增加导致构建时间增长]**
   - 影响: CI/CD 流水线时间增加
   - 可能性: 中等
   - 缓解: 使用测试分级，区分单元测试和集成测试
   - Contingency: 优化测试并行执行

## Alternative Approaches

1. **[方案: 使用依赖注入框架替代手动 DI]**
   - 描述: 引入 rustioc 或 axum 的依赖注入模式
   - 优点: 更清晰的依赖关系，自动生命周期管理
   - 缺点: 引入外部依赖，学习曲线，迁移成本高
   - 推荐: 暂不采用，保持当前手动 DI 模式

2. **[方案: 异步 trait 统一使用 async_trait]**
   - 描述: 所有 service trait 使用 async_trait
   - 优点: 统一的异步编程模式
   - 缺点: 运行时开销，可能影响性能
   - 推荐: 保持当前混合模式，按需使用

3. **[方案: 配置使用 TOML 格式]**
   - 描述: forge.yaml 改为 TOML 格式
   - 优点: 更强的类型支持，解析更快
   - 缺点: 与 Claude Code 兼容性降低
   - 推荐: 保持 YAML，与生态兼容

## Assumptions

- [假设: 项目指南要求使用 thiserror 定义 domain 错误]
- [假设: Agent 工具调用权限检查是安全必需的]
- [假设: 循环调用检测使用 AgentCallChain 是最佳方案]
- [假设: CLI 与配置集成不会破坏现有命令行接口]

## Dependencies

- [依赖: PolicyService.check_operation_permission 方法存在且可用]
- [依赖: AgentCallChain 结构已在 agent_call_chain.rs 中实现]
- [依赖: forge_config_adapter 的 ConfigAutoMigrator 已实现]
- [依赖: thiserror crate 已在项目中使用]

## Notes

- 修复方案将分 4 个 Phase 实施，每个 Phase 可独立验证
- Phase 1 (安全) 为最高优先级，必须首先完成
- 测试覆盖是持续工作，将在每个 Phase 中并行进行
- 建议在每个 Phase 完成后进行代码审查和测试验证