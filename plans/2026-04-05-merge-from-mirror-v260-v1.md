# 分支融合计划：基于 mirror/v2.6.0-antinomy 融合当前修改

## Objective

将当前 fork 的修改与官方 mirror/v2.6.0-antinomy 分支进行融合：
- **CI配置**：完全采用 mirror/v2.6.0-antinomy 的实现（不进行任何修改）
- **代码特性**：进行融合分析，由用户确认最终使用哪个版本

目标：创建一个可用于执行的详细融合计划，同时记录所有需要用户确认的事项。

## Implementation Plan

- [ ] 1. 分析当前分支与 mirror/v2.6.0-antinomy 的差异
  - 列出所有变更的文件和目录
  - 统计代码行数差异
  - 确定差异类型：CI配置、代码、配置文件、文档等

- [ ] 2. 对比 CI 配置文件差异
  - 使用 git diff 对比 .github/workflows/ 目录
  - 记录每处差异的具体内容
  - 确认当前分支的 protoc 修复在官方版本中的状态

- [ ] 3. 对比 Skills 目录差异
  - 对比 .forge/skills/ 目录
  - 列出官方新增的 Skills
  - 列出官方删除的 Skills
  - 列出当前分支修改的 Skills

- [ ] 4. 对比 Rules 目录差异
  - 对比 .forge/rules/ 目录
  - 确认规则文件的变更情况

- [ ] 5. 对比 Plans 目录差异
  - 对比 plans/ 目录
  - 列出官方和当前分支各自的计划文件

- [ ] 6. 对比配置文件差异
  - 对比 .rustfmt.toml
  - 对比 Cargo.toml 版本号
  - 对比其他配置文件

- [ ] 7. 对比代码目录差异
  - 对比 crates/forge_app/ 目录
  - 对比 crates/forge_services/ 目录
  - 列出主要的代码变更

- [ ] 8. 创建详细的差异分析报告
  - 按类别整理所有差异
  - 为每项差异标注处理建议
  - 明确需要用户确认的事项

- [ ] 9. 制定 CI 配置融合步骤
  - 确认采用官方 CI 配置
  - 记录当前分支 CI 修改的处理方式

- [ ] 10. 制定代码特性融合步骤
  - 根据用户确认的决定，列出需要融合的模块
  - 确定融合方式（保留当前/采用官方/手动合并）

- [ ] 11. 验证融合结果
  - 运行 cargo check 验证编译
  - 确认 CI 配置与官方一致

## Verification Criteria

- [差异分析完成]: 所有变更文件已列出并分类
- [CI配置确认]: CI 配置与 mirror/v2.6.0-antinomy 完全一致
- [用户确认完成]: 所有需要确认的事项已获用户明确回复
- [代码编译通过]: cargo check --workspace --all-features 执行成功
- [融合计划完成]: 详细的执行步骤已制定并通过验证

## Potential Risks and Mitigations

1. **CI 配置差异导致功能丢失**
   - Impact: 当前分支的 protoc 修复可能被覆盖
   - Likelihood: High
   - Mitigation: 在采用官方配置前，先确认官方版本是否已包含相关修复，或创建补丁保留修复
   - Contingency: 如果官方版本缺少关键修复，保留当前分支的 CI 修改作为补丁

2. **大量代码合并冲突**
   - Impact: 215 个文件差异可能导致大量冲突
   - Likelihood: Medium
   - Mitigation: 使用 git merge 或 rebase 进行融合，逐个模块处理冲突
   - Contingency: 保留当前分支的完整代码，仅手动应用必要的官方更新

3. **版本号不兼容**
   - Impact: 官方 0.1.0 vs 当前 0.1.2 可能导致依赖问题
   - Likelihood: Low
   - Mitigation: 与用户确认版本策略，保留当前版本或升级到官方版本
   - Contingency: 手动调整版本号或锁定依赖版本

## Alternative Approaches

1. **完全采用官方版本**
   - Description: 直接 reset 或 checkout 到 mirror/v2.6.0-antinomy，丢弃所有当前分支修改
   - Pros: 简单直接，与官方保持完全一致
   - Cons: 丢失所有当前分支的修改，包括自定义的 Skills、Plans 等
   - Recommendation: 不推荐，除非确认当前分支修改无需保留

2. **选择性融合（推荐）**
   - Description: 仅融合 CI 配置（官方版本），其他模块由用户确认后选择性融合
   - Pros: 保留用户需要的修改，同时与官方 CI 保持一致
   - Cons: 需要用户逐项确认，工作量较大
   - Recommendation: 推荐使用此方式，给予用户最大控制权

3. **双分支策略**
   - Description: 创建新分支专门用于融合实验，保留当前分支不变
   - Pros: 风险低，可在实验分支反复尝试
   - Cons: 需要管理多个分支，增加复杂性
   - Recommendation: 适用于风险较高的融合场景

## Assumptions

- [假设1]: 用户希望 CI 配置与官方完全一致
- [假设2]: 用户可能希望保留部分当前分支的代码特性
- [假设3]: 用户愿意参与融合决策过程，提供必要的确认

## Dependencies

- [依赖1]: mirror/v2.6.0-antinomy 分支可访问且包含完整的官方配置
- [依赖2]: 用户能够提供关于代码特性保留的决定
- [依赖3]: git 和相关工具正常工作

## Notes

- 当前分支与 mirror/v2.6.0-antinomy 的差异较大（215个文件），需要仔细分析
- CI 配置差异较小（仅4处），融合相对简单
- 代码特性差异较大，需要用户逐项确认
- 建议先完成差异分析，再制定详细的融合执行步骤