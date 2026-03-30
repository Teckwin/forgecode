# Test Patterns for Stability Issues

## 1. Concurrency/Thread Safety Test Patterns

### Test: SharedSpinner Concurrent Access

Tests that concurrent access to SharedSpinner does not cause panic due to lock poisoning.

**File to test**: `crates/forge_main/src/stream_renderer.rs`

**Test case**:
- Spawn 10 concurrent tasks
- Each task attempts to lock and use spinner 100 times
- Verify no panic occurs

### Test: MpscStream Graceful Shutdown

Tests that dropping MpscStream properly cleans up resources without hanging.

**File to test**: `crates/forge_stream/src/mpsc_stream.rs`

**Test cases**:
- Drop stream while task is running - should not panic
- Verify task is properly aborted
- Verify no resource leaks

## 2. Message Deduplication Test Patterns

### Test: ChatResponse Unique Identification

Tests that ChatResponse can be uniquely identified for deduplication.

**File to test**: `crates/forge_domain/src/chat_response.rs`

**Test cases**:
- Two ChatResponse with same content but different IDs should be unique
- Same ChatResponse sent twice should be detected as duplicate
- Different ChatResponse types should not be considered duplicates

### Test: Message Stream Deduplication

Tests that message stream properly filters duplicates.

**File to test**: `crates/forge_stream/` or `crates/forge_domain/src/`

**Test cases**:
- Stream with duplicate messages should filter correctly
- Message sequence should be preserved after deduplication

## 3. Timeout/Resource Management Test Patterns

### Test: ToolCall Notification Timeout

Tests that waiting for UI notification has proper timeout.

**File to test**: `crates/forge_app/src/orch.rs`

**Test cases**:
- When UI never responds, should timeout after configured duration
- Timeout should not panic or hang
- Normal flow should complete within timeout

### Test: Graceful Shutdown on Ctrl+C

Tests that the application responds to interrupt signals properly.

**File to test**: `crates/forge_main/src/ui.rs`

**Test cases**:
- Ctrl+C should trigger graceful shutdown
- Background tasks should complete or be cancelled
- Application should exit within reasonable time

## 4. Integration Test Patterns

### Test: Multiple UI Windows

Tests stability when multiple instances run concurrently.

**Test cases**:
- Run multiple forge instances
- Verify no crashes or resource conflicts

### Test: Long Running Session

Tests stability over extended operation time.

**Test cases**:
- Simulate long conversation with many turns
- Verify no memory leaks
- Verify no message duplication