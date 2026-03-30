# 修复闪退和重复输出问题

## Objective

修复 Forge 项目中存在的两个核心问题：
1. **闪退问题**：多窗口并发或长时间运行时程序崩溃（尤其在同时开启多个 UI 窗口时）
2. **重复输出问题**：执行时间久之后会出现重复输出 "I should think step by step..." 字符

通过改进并发控制、资源管理、消息去重机制和任务生命周期管理，提升系统稳定性。

## Implementation Plan

- [ ] 1. 修复 SharedSpinner 锁中毒问题
  - 修改 `crates/forge_main/src/stream_renderer.rs:30-64` 中的锁处理逻辑
  - 将 `unwrap_or_else(|e| e.into_inner())` 改为使用 `match` 进行安全的错误处理
  - 添加超时锁获取机制，防止长时间阻塞
  - Rationale: 当前使用 `unwrap_or_else` 处理 Mutex 锁中毒时会导致 panic，多窗口并发时更容易触发
  - Dependencies: 无
  - Key considerations: 需要保持现有 API 兼容性，确保锁正确释放

- [ ] 2. 改进 MpscStream 优雅关闭机制
  - 修改 `crates/forge_stream/src/mpsc_stream.rs:34-39` 中的 Drop 实现
  - 添加超时等待机制，等待后台任务完成后再中止
  - 实现 graceful shutdown 流程
  - Rationale: 当前 `abort()` 强制立即终止任务，可能导致数据竞争和资源泄漏
  - Dependencies: 任务 1
  - Key considerations: 需要平衡关闭延迟和资源释放，避免长时间阻塞

- [ ] 3. 为 ChatResponse 添加唯一标识符
  - 修改 `crates/forge_domain/src/chat_response.rs:54-75`
  - 为 `ChatResponse` 枚举添加 `message_id: Uuid` 字段
  - 实现消息去重缓存机制
  - Rationale: 当前消息没有唯一 ID，无法追踪已发送的消息，导致重复输出
  - Dependencies: 任务 2
  - Key considerations: 需要考虑向后兼容性，可能需要使用 `#[serde(default)]`

- [ ] 4. 实现消息去重和幂等性保证
  - 在消息发送端维护已发送消息的 ID 集合
  - 在接收端添加去重过滤逻辑
  - 实现消息序列号机制
  - Rationale: 长时间运行后可能出现消息重复发送，导致重复输出
  - Dependencies: 任务 3
  - Key considerations: 需要控制内存使用，设置合理的缓存过期策略

- [ ] 5. 添加工具调用超时机制
  - 修改 `crates/forge_app/src/orch.rs:70-84` 中的 `notifier.notified().await`
  - 为等待 UI 响应添加超时时间
  - 实现任务取消令牌（CancellationToken）
  - Rationale: 如果 UI 未正确响应 `notified().await`，会永远等待导致状态锁定
  - Dependencies: 任务 4
  - Key considerations: 超时时间需要合理设置，既不能太短导致正常流程失败，也不能太长影响体验

- [ ] 6. 改进 UI 循环的优雅退出机制
  - 修改 `crates/forge_main/src/ui.rs:316-320` 中的 Ctrl+C 信号处理
  - 实现任务取消令牌传播
  - 添加资源清理等待机制
  - Rationale: 当前 Ctrl+C 处理只是重置 spinner，正在执行的任务继续运行导致资源泄漏
  - Dependencies: 任务 5
  - Key considerations: 需要确保所有后台任务都能正确响应取消信号

- [ ] 7. 改进流处理中的提前返回逻辑
  - 检查 `crates/forge_main/src/ui.rs:3201-3212` 中的 TaskComplete 处理
  - 确保提前 return 时执行正确的资源清理
  - 使用 RAII 模式管理资源生命周期
  - Rationale: TaskComplete 时直接返回可能跳过清理逻辑
  - Dependencies: 任务 6
  - Key considerations: 需要审查所有提前返回路径

- [ ] 8. 添加集成测试验证修复
  - 创建并发场景测试用例
  - 创建长时间运行测试用例
  - 验证重复消息不会重复输出
  - Rationale: 需要验证修复的有效性
  - Dependencies: 任务 7
  - Key considerations: 测试需要覆盖边界条件

## Verification Criteria

- [测试通过]: 所有现有测试继续通过，使用 `cargo insta test --accept`
- [并发安全]: 多个 UI 窗口同时运行时不再出现闪退
- [消息去重]: 长时间运行测试中不出现重复的 "I should think step by step" 输出
- [优雅关闭]: Ctrl+C 后程序能在合理时间内（< 5秒）完全退出
- [资源清理]: 使用 `cargo leak` 确认无内存泄漏

## Potential Risks and Mitigations

1. **锁超时导致性能下降**
   - Impact: 添加锁超时可能导致 UI 响应变慢
   - Likelihood: Medium
   - Mitigation: 使用非阻塞超时（try_lock），失败时记录警告但继续执行
   - Contingency: 提供配置选项允许用户调整超时时间

2. **消息 ID 变更影响序列化**
   - Impact: 添加 message_id 可能破坏现有对话持久化
   - Likelihood: Medium
   - Mitigation: 使用 `#[serde(default)]` 保持向后兼容
   - Contingency: 提供数据库迁移脚本

3. **超时机制影响正常流程**
   - Impact: 超时设置不合理可能导致正常工具调用失败
   - Likelihood: High
   - Mitigation: 默认超时设置较长（30秒），提供配置选项
   - Contingency: 在超时时记录详细日志供调试

4. **任务取消破坏数据一致性**
   - Impact: 任务被取消时可能留下不一致的状态
   - Likelihood: Medium
   - Mitigation: 实现事务性回滚机制
   - Contingency: 在重启时检测并修复不一致状态

## Alternative Approaches

1. **使用 RwLock 替代 Mutex**
   - Description: 将 SharedSpinner 中的 Mutex 改为 RwLock，允许并发读取
   - Pros: 提高并发性能，减少锁竞争
   - Cons: 写入时仍需独占锁，改动较大
   - Recommendation: 作为后续优化方向，暂不优先实施

2. **使用事件溯源模式**
   - Description: 为所有消息添加序列号，使用事件溯源追踪状态
   - Pros: 天然支持去重和重放
   - Cons: 需要大幅重构，引入复杂性
   - Recommendation: 作为长期架构改进方向

3. **使用外部消息队列**
   - Description: 引入 Redis 或 Kafka 处理消息流
   - Pros: 天然支持消息去重和高可用
   - Cons: 增加系统复杂度，需要额外基础设施
   - Recommendation: 不适用于本地 CLI 工具场景

## Assumptions

- 用户运行在支持 Rust 1.75+ 的环境（使用 try_lock 等特性）
- 多窗口场景是指在短时间内多次启动 forge 命令
- 重复输出问题主要发生在流处理层面，不是 LLM 返回重复内容
- 用户愿意接受适度的配置灵活性以换取稳定性

## Dependencies

- tokio 1.x: 异步运行时（已存在于项目）
- uuid: 消息唯一 ID 生成（需要添加到 Cargo.toml）
- 现有测试框架：insta（已存在于项目）

## Notes

- 修复过程中需要特别注意保持与现有 API 的兼容性
- 建议分步实施，每次只做一个改动并验证
- 考虑到用户反馈的 "I should think step by step" 字符重复，需要特别关注流处理中的消息合并逻辑