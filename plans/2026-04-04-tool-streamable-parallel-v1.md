# Tool 定义增强：Streamable 能力声明与并行调用优化

## Objective

为 Forge 系统增强 Tool 定义的能力声明机制，添加 `streamable`、`parallel_calls` 等字段支持，并实现工具级别的并行调用优化。

## Implementation Plan

- [ ] 1. 增强 ToolDefinition 结构，添加 capabilities 字段
   - 在 `crates/forge_domain/src/tools/definition/tool_definition.rs` 中添加 `ToolCapabilities` 结构体
   - 包含 `streamable`、`parallel_calls`、`estimated_duration_ms`、`idempotent` 等字段
   - 使用 serde 和 schemars 支持序列化和 JSON Schema 生成
   - Rationale: 为工具提供显式的能力声明，便于调度器做出优化决策
   - Dependencies: 无

- [ ] 2. 更新现有工具定义，添加默认 capabilities
   - 修改 `crates/forge_domain/src/tools/catalog.rs` 中的内置工具定义
   - 为 Read/Write/Patch/Remove/Shell 等工具设置适当的 capabilities
   - Rationale: 保持向后兼容，为现有工具提供合理的默认能力
   - Dependencies: Task 1 完成

- [ ] 3. 在 ToolExecutor 中实现并行调度方法
   - 添加 `execute_parallel` 方法支持批量工具并行执行
   - 使用 `tokio::task::join_all` 实现并发执行
   - 添加超时控制和错误处理
   - Rationale: 允许上游 Agent 并行调用多个独立工具快速收集信息
   - Dependencies: Task 1 完成

- [ ] 4. 在 ToolRegistry 中暴露并行执行接口
   - 修改 `crates/forge_app/src/tool_registry.rs` 添加并行执行入口
   - 支持根据 ToolCapabilities 自动选择串行或并行执行
   - Rationale: 提供统一的工具调度入口，隐藏底层实现细节
   - Dependencies: Task 3 完成

- [ ] 5. 添加单元测试验证新功能
   - 为 ToolCapabilities 添加序列化/反序列化测试
   - 为并行执行方法添加并发测试
   - 验证 ToolDefinition 能力声明正确工作
   - Rationale: 确保新功能的正确性和稳定性
   - Dependencies: Task 1-4 完成

- [ ] 6. 更新文档和示例
   - 更新 `.forge/skills/create-tool/SKILL.md` 中的 TOOL.md 格式规范
   - 添加 capabilities 字段说明和示例
   - Rationale: 保持文档与实现同步
   - Dependencies: Task 1 完成

## Verification Criteria

- [ToolDefinition 包含 capabilities 字段]: 序列化/反序列化正常工作
- [现有工具保持兼容]: 不破坏现有功能，所有测试通过
- [并行执行正确工作]: 多个独立工具可并行执行，结果正确收集
- [流式输出能力声明]: streamable 字段可正确设置和读取
- [文档更新完整]: TOOL.md 规范包含新字段说明

## Potential Risks and Mitigations

1. **[风险: 向后兼容性问题]**
   - Impact: 添加新字段可能影响现有 JSON 序列化
   - Likelihood: 低
   - Mitigation: capabilities 字段使用 Option 类型，默认 None 保持兼容
   - Contingency: 使用 #[serde(default)] 确保缺失字段时使用默认值

2. **[风险: 并行执行导致资源竞争]**
   - Impact: 多个工具并行访问共享资源可能导致竞态条件
   - Likelihood: 中
   - Mitigation: 使用 tokio 的 Mutex 保护共享状态，添加并发限制
   - Contingency: 提供配置项控制最大并行度

3. **[风险: 破坏现有工具调用流程]**
   - Impact: 修改 ToolExecutor 可能影响现有工具调用
   - Likelihood: 低
   - Mitigation: 保持原有 execute 方法不变，仅添加新的并行方法
   - Contingency: 使用特性开关允许禁用并行功能

## Alternative Approaches

1. **[方案A: 仅添加能力声明，不实现并行执行]**
   - Description: 只增强 ToolDefinition 添加 capabilities，不改变执行逻辑
   - Pros: 最小化变更，降低风险
   - Cons: 无法实现用户期望的并行工具调用优化
   - Recommendation: 不采用，用户明确需要并行能力

2. **[方案B: 使用 trait 实现流式工具]**
   - Description: 定义 StreamableTool trait，让工具自行实现流式输出
   - Pros: 更灵活的扩展性
   - Cons: 需要大量重构，影响范围大
   - Recommendation: 考虑作为后续优化方向，当前保持简单实现

3. **[方案C: 在 Agent 层面实现并行工具调用]**
   - Description: 不修改 ToolExecutor，在 AgentExecutor 中实现工具批量并行调用
   - Pros: 不影响底层工具执行逻辑
   - Cons: 只能在 Agent 场景使用，不够通用
   - Recommendation: 采用此方案作为核心实现，ToolExecutor 并行作为补充

## Assumptions

- [假设: tokio 并发原语足够满足并行执行需求]
- [假设: 现有工具都是线程安全的，可以并行执行]
- [假设: ToolDefinition 的变更可以通过 serde 兼容处理]
- [假设: 并行执行不会引入新的安全性问题]

## Dependencies

- crates/forge_domain/src/tools/definition/tool_definition.rs
- crates/forge_domain/src/tools/catalog.rs
- crates/forge_app/src/tool_executor.rs
- crates/forge_app/src/tool_registry.rs
- .forge/skills/create-tool/SKILL.md