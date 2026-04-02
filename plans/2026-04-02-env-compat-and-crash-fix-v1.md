# 修复环境变量兼容和闪退问题

## Objective

修复 Forge 项目中存在的两类问题：
1. **环境变量兼容问题**：统一各 provider 的 API key 环境变量命名（支持新旧两种命名方式），并确保 agent 配置完全从 `.env` 迁移到 `forge.yaml`
2. **闪退和重复输出问题**：解决多窗口并发时程序崩溃和长时间运行后重复输出 "I should think step by step..." 的问题

通过改进环境变量处理、资源管理、消息去重机制和任务生命周期管理，提升系统稳定性。

## Implementation Plan

### 第一部分：环境变量兼容（已确认废弃）

经过深度检查，确认环境变量方式已经废弃：

- [x] ~~添加 Claude Code 环境变量别名支持~~ - **已废弃，无需添加**
- [x] ~~统一 Anthropic 系列 provider 环境变量~~ - **已废弃**
- [x] ~~添加环境变量别名配置支持~~ - **已废弃**
- [x] ~~更新文档中的环境变量说明~~ - **已废弃**

**结论**：`migrate_env_to_file` 方法会尝试从环境变量读取凭证并迁移到文件，但只有当凭证文件不存在时才会执行。如果用户已经有 `credentials.json` 文件，则环境变量不再使用。

### 第二部分：闪退和重复输出问题

#### 已完成修复

- [x] 1. **修复 SharedSpinner 锁中毒问题** ✅
  - 位置：`crates/forge_main/src/stream_renderer.rs:30-64`
  - 已使用 `match` 处理锁中毒，不再 panic
  - 代码：
    ```rust
    pub fn start(&self, message: Option<&str>) -> Result<()> {
        match self.0.lock() {
            Ok(mut spinner) => spinner.start(message),
            Err(e) => {
                let mut spinner = e.into_inner();
                spinner.start(message).ok();
                Ok(())
            }
        }
    }
    ```

- [x] 2. **改进 MpscStream 优雅关闭机制** ✅
  - 位置：`crates/forge_stream/src/mpsc_stream.rs:60-72`
  - 已实现 `graceful_shutdown` 方法
  - 代码：
    ```rust
    pub async fn graceful_shutdown(&mut self) {
        self.receiver.close();
        let timeout = Duration::from_secs(1);
        let completed = tokio::time::timeout(timeout, &mut self.join_handle).await;
        if completed.is_err() {
            self.join_handle.abort();
        }
    }
    ```

#### 剩余未完成任务

- [ ] 3. 为 ChatResponse 添加唯一标识符
  - 修改 `crates/forge_domain/src/chat_response.rs:54-75`
  - 为 `ChatResponse` 枚举添加 `message_id: Uuid` 字段
  - 实现消息去重缓存机制
  - Rationale: 当前消息没有唯一 ID，无法追踪已发送的消息，导致重复输出
  - Dependencies: 无
  - Key considerations: 需要考虑向后兼容性，使用 `#[serde(default)]`

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

- [环境变量]: 已确认环境变量方式已废弃，凭证存储在 `credentials.json` 文件中
- [测试通过]: 所有现有测试继续通过，使用 `cargo insta test --accept`
- [并发安全]: 多个 UI 窗口同时运行时不再出现闪退（已完成锁中毒修复）
- [消息去重]: 长时间运行测试中不出现重复的 "I should think step by step" 输出
- [优雅关闭]: Ctrl+C 后程序能在合理时间内（< 5秒）完全退出
- [资源清理]: 使用 `cargo leak` 确认无内存泄漏

## Potential Risks and Mitigations

1. **消息 ID 变更影响序列化**
   - Impact: 添加 message_id 可能破坏现有对话持久化
   - Likelihood: Medium
   - Mitigation: 使用 `#[serde(default)]` 保持向后兼容
   - Contingency: 提供数据库迁移脚本

2. **超时机制影响正常流程**
   - Impact: 超时设置不合理可能导致正常工具调用失败
   - Likelihood: High
   - Mitigation: 默认超时设置较长（30秒），提供配置选项
   - Contingency: 在超时时记录详细日志供调试

3. **任务取消破坏数据一致性**
   - Impact: 任务被取消时可能留下不一致的状态
   - Likelihood: Medium
   - Mitigation: 实现事务性回滚机制
   - Contingency: 在重启时检测并修复不一致状态

## Alternative Approaches

1. **使用 RwLock 替代 Mutex（闪退修复）**
   - Description: 将 SharedSpinner 中的 Mutex 改为 RwLock，允许并发读取
   - Pros: 提高并发性能，减少锁竞争
   - Cons: 写入时仍需独占锁，改动较大
   - Recommendation: 作为后续优化方向，暂不优先实施（当前已用 match 处理锁中毒）

2. **使用事件溯源模式（消息去重）**
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
- 环境变量兼容和闪退问题是两个独立的问题域，可以并行开发但需要按依赖顺序合并
- 已完成的修复：任务 1（锁中毒）和任务 2（优雅关闭）已完成并验证
