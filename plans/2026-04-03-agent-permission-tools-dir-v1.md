# Agent工具权限和循环检测 + 动态工具持久化方案

## Objective

为 Forge 添加 Agent 工具调用时的权限管控和循环检测机制，同时设计动态工具持久化规范到 `.forge/tools/` 目录，实现工具作为一等公民的规范化管理。

## Implementation Plan

- [ ] 1. 创建调用链上下文结构 AgentCallChain
  - 在 crates/forge_domain/src/ 下新建 agent_call_chain.rs 文件
  - 使用 DashSet 存储 AgentId 实现 O1 查找
  - 提供 push pop contains is_empty 方法
  - Rationale: 为循环调用检测提供基础数据结构
  - Dependencies: 无

- [ ] 2. 在 ToolRegistry 集成循环调用检测
  - 修改 crates/forge_app/src/tool_registry.rs 的 execute_tool 方法
  - 在调用 Agent tool 前检查调用链是否包含目标 Agent
  - 阻止循环调用并返回明确错误信息
  - Rationale: ToolRegistry 是所有工具入口点在此处统一处理最合适
  - Dependencies: AgentCallChain 结构已创建

- [ ] 3. 在 AgentExecutor 添加权限检查逻辑
  - 修改 crates/forge_app/src/agent_executor.rs 的 execute 方法
  - 将 PermissionService 注入到 AgentExecutor
  - 在执行工具调用前调用 check_tool_permission
  - Rationale: 确保 Agent 内部工具调用受权限规则约束
  - Dependencies: ToolRegistry 循环检测已实现

- [ ] 4. 确保 Agent LLM 对话不受权限管控
  - 在 AgentExecutor 中区分 LLM 对话调用和工具调用
  - 仅对工具调用执行权限检查
  - Rationale: 保持 Agent 自主对话能力同时管控其工具使用
  - Dependencies: 权限检查逻辑已添加

- [ ] 5. 创建 .forge/tools/ 目录结构规范用于动态工具
  - 在项目根目录创建 .forge/tools/ 目录
  - 设计工具目录结构为 .forge/tools/{tool-name}/TOOL.md
  - 参考 .forge/skills 和 .forge/commands 的目录结构规范
  - Rationale: Forge 动态工具是可执行操作，与 skills 的任务模板不同
  - Dependencies: 无

- [ ] 6. 定义 TOOL.md 工具文档格式规范包含完整字段
  - 在 TOOL.md 中定义 YAML frontmatter 包含 name overview适用范围
  - 文档主体包含概述 适用范围 不适用场景 使用方式 参数规范 返回数据格式
  - 使用方式支持 http stdio stream socket 等类型
  - Rationale: 工具是一等公民需要完整严谨的文档规范
  - Dependencies: .forge/tools/ 目录已创建

- [ ] 7. 创建 create-tool skill
  - 参考 .forge/skills/create-agent 和 create-command 的格式
  - 在 .forge/skills/create-tool/SKILL.md 创建 skill 定义
  - 包含工具创建模板和验证逻辑
  - Rationale: 提供标准化工具创建工作流
  - Dependencies: TOOL.md 规范已定义

- [ ] 8. 实现动态工具从文件系统的加载逻辑到内存缓存
  - 在 crates/forge_repo/src 下新建 tool.rs 或扩展现有实现
  - 扫描 .forge/tools/ 目录解析 TOOL.md 文件
  - 将动态工具注册到 ToolRegistry
  - 实现工具缓存和热重载能力
  - Rationale: 实现动态工具的发现和加载机制
  - Dependencies: TOOL.md 规范和 create-tool skill 已完成

- [ ] 9. 实现动态工具的发现和调用机制提供统一接口
  - 创建 list_tools 方法列出所有可用动态工具
  - 创建 call_tool 方法通过名称调用动态工具
  - 参考 call_agent 和 call_skill 的实现模式
  - 为动态工具提供与内置工具一致的调用体验
  - Rationale: 保持工具调用接口一致性提升用户体验
  - Dependencies: 工具加载逻辑已实现

- [ ] 10. 创建 .forge/rules/ 目录结构规范用于动态规则
  - 在项目根目录创建 .forge/rules/ 目录
  - 设计规则目录结构为 .forge/rules/{rule-name}.md
  - 借鉴 Claude Code 的 rules 设计 (.claude/rules/)
  - Rationale: 将 Claude 先进的规则设计理念引入 Forge
  - Dependencies: 无

- [ ] 11. 定义规则文件的文档格式规范与Claude保持一致
  - 规则文件使用标准 Markdown 格式
  - YAML frontmatter 包含 alwaysApply globs 等字段
  - 文档主体包含规则描述和执行条件
  - Rationale: 与 Claude Code 规则格式保持一致确保兼容性
  - Dependencies: .forge/rules/ 目录已创建

- [ ] 12. 在 adapter 中实现 Claude rules 到 Forge rules 的转化逻辑
  - 扫描 ~/.claude/rules/ 读取现有 Claude rules
  - 实现规则格式转化 (将 Claude 格式转为 Forge 格式)
  - 支持将转化后的规则写入 .forge/rules/ 目录
  - 在 forge_config_adapter 中添加适配器实现
  - Rationale: 复用 Claude 现有规则资产实现无缝迁移
  - Dependencies: 规则格式规范已定义

## Verification Criteria

- [x] Agent 调用 Agent 时能正确检测循环调用并返回明确错误
- [x] Agent 内部工具调用受权限服务管控
- [x] Agent LLM 对话部分不受权限管控
- [x] 动态工具目录 .forge/tools/ 已创建并包含示例
- [x] 工具文档格式 TOOL.md 已定义
- [x] create-tool skill 已创建
- [x] 动态规则目录 .forge/rules/ 已创建并包含示例
- [x] 规则格式与 Claude Code 保持一致
- [x] rules.rs 已支持 alwaysApply 字段解析
- [x] 所有单元测试通过
- [x] Agent 内部工具调用受 PermissionService 权限规则约束
- [x] Agent 自身的 LLM 对话部分不受权限管控保持自主性
- [x] 单测覆盖 AgentCallChain 循环检测逻辑
- [x] 单测覆盖 AgentExecutor 权限检查逻辑
- [x] 动态工具可从 .forge/tools/ 目录正确加载
- [x] list_tools 和 call_tool 方法可正常工作
- [x] TOOL.md 文档格式通过验证脚本检查

## Potential Risks and Mitigations

1. **循环检测性能问题**
   - Impact: 深度嵌套的 Agent 调用链可能导致性能下降
   - Likelihood: 低
   - Mitigation: 使用 DashSet 存储调用链实现 O1 查找和去重，限制最大调用深度为十层
   - Contingency: 添加超时机制防止长时间阻塞

2. **权限检查侵入性**
   - Impact: 权限检查可能影响 Agent 正常功能或引入兼容性问题
   - Likelihood: 中
   - Mitigation: 仅在 Agent 内部工具调用时检查权限，主对话不检查，保持 API 兼容性
   - Contingency: 提供配置开关允许禁用权限检查

3. **工具版本管理**
   - Impact: 动态工具更新后可能存在缓存不一致
   - Likelihood: 中
   - Mitigation: 工具文件使用时间戳或版本号，支持热重载
   - Contingency: 提供 reload_tools 方法手动刷新

## Alternative Approaches

1. **方案A: 在 ToolRegistry 统一处理**
   - Description: 在 ToolRegistry 的 execute_tool 方法中统一处理权限检查和循环检测
   - Pros: 集中管理逻辑清晰，所有工具入口一致
   - Cons: 需要传递调用上下文参数
   - Recommendation: 选择此方案，因为 ToolRegistry 是所有工具的统一入口点

2. **方案B: 在 AgentExecutor 内部处理**
   - Description: 在 AgentExecutor 内部实现权限检查和循环检测
   - Pros: Agent 内部自包含，封装性好
   - Cons: 可能需要重复实现逻辑，边界情况难以覆盖
   - Recommendation: 不选择，方案A更合适

3. **方案C: 使用中间件模式**
   - Description: 创建工具调用中间件链式处理权限和循环检测
   - Pros: 扩展性好，易于添加更多检查
   - Cons: 复杂度较高，引入额外抽象层
   - Recommendation: 考虑作为后续优化方向，当前保持简单实现

## Assumptions

- PermissionService 已在现有代码中实现且可用
- ToolRegistry 的 execute_tool 方法可以访问调用链上下文
- AgentExecutor 可以接受 PermissionService 依赖注入
- .forge/ 目录在项目运行时可访问
- 动态工具的加载在应用启动时执行

## Dependencies

- crates/forge_app/src/tool_registry.rs
- crates/forge_app/src/agent_executor.rs
- crates/forge_domain/src/agent.rs
- crates/forge_domain/src/permission.rs (如存在)
- crates/forge_repo/src/skill.rs (参考实现)
- .forge/skills/create-agent/SKILL.md
- .forge/skills/create-command/SKILL.md

## Notes

- 权限检查和循环检测应该作为安全层实现，保持最小权限原则
- 动态工具持久化设计参考了现有 skills 和 commands 的规范保持一致性
- 工具文档格式参考了 OpenAPI 规范和现有代码风格
- 考虑后续添加工具版本管理和热重载能力
- 整合考虑: Forge 动态工具 (.forge/tools/) 与 Claude Code rules (.claude/rules/) 定位不同但可互补
  - .claude/rules/ 是全局用户规则，专注于特定场景的指令优化
  - .forge/tools/ 是项目级工具定义，专注于可执行操作的规范描述
  - 两者可共存，Forge 工具可被 MCP 协议调用，Claude rules 用于对话上下文优化