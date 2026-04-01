---
name: tdd-workflow
description: |
  Test-Driven Development workflow for Rust projects using insta for snapshot testing.
  Use when: (1) User requests TDD or test-first development, (2) Fixing bugs that need regression tests,
  (3) Adding new features that need verification tests, (4) Creating test cases for specific issues.
  The workflow emphasizes: writing failing tests first, making minimal code changes to pass tests,
  using insta for snapshot assertions, and maintaining high test coverage for stability fixes.
---

# TDD Workflow

This skill provides a structured Test-Driven Development workflow for Rust projects.

## Core Principles

1. **Write failing tests first** - Tests must fail before implementation
2. **Minimal changes** - Make only the code changes needed to pass tests
3. **Fast feedback loop** - Run tests frequently during development
4. **Use insta for snapshots** - Leverage insta for deterministic snapshot testing

## Workflow Steps

### Step 1: Analyze the Problem

Before writing any code, understand:
- What behavior should be tested?
- What are the edge cases?
- What is the expected vs actual behavior?

### Step 2: Write the Failing Test

Create a test that:
- Uses `#[tokio::test]` for async tests or `#[test]` for sync
- Follows the three-part test structure: setup, act, assert
- Uses `pretty_assertions::assert_eq` for clear diffs
- Has descriptive name: `test_<feature>_<expected_behavior>`

Example test structure:
```rust
#[tokio::test]
async fn test_<description>() {
    // 1. Setup: Create fixtures and test data
    let fixture = ...;
    
    // 2. Act: Execute the behavior under test
    let actual = fixture.execute().await;
    
    // 3. Assert: Verify expected outcome
    let expected = ...;
    assert_eq!(actual, expected);
}
```

### Step 3: Run the Test to Verify It Fails

```bash
cargo insta test --accept -p <package-name> -- <test-name>
```

The test MUST fail at this stage. If it passes, the test is not properly validating the issue.

### Step 4: Implement the Fix

Make minimal code changes to make the test pass. Focus on:
- Fixing the root cause, not symptoms
- Maintaining backward compatibility
- Following existing code patterns

### Step 5: Verify the Test Passes

```bash
cargo insta test --accept -p <package-name> -- <test-name>
```

### Step 6: Run Full Test Suite

```bash
cargo insta test --accept
```

Ensure no regressions were introduced.

## Test Patterns for Common Issues

### Concurrency/Thread Safety Tests

```rust
#[tokio::test]
async fn test_concurrent_access_no_panic() {
    let spinner = SharedSpinner::new(SpinnerManager::new(printer));
    
    // Spawn multiple concurrent tasks
    let mut handles = vec![];
    for _ in 0..10 {
        let s = spinner.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let _ = s.start(Some("test"));
                tokio::time::sleep(Duration::from_micros(1)).await;
                let _ = s.stop(None);
            }
        }));
    }
    
    // All tasks should complete without panic
    for handle in handles {
        handle.await.unwrap();
    }
}
```

### Message Deduplication Tests

```rust
#[tokio::test]
async fn test_message_deduplication() {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let sent_clone = sent_messages.clone();
    
    // Simulate sending duplicate messages
    let duplicates = vec![
        ChatResponse::TaskMessage { content: content.clone() },
        ChatResponse::TaskMessage { content: content.clone() },
        ChatResponse::TaskMessage { content: content.clone() },
    ];
    
    // Create deduplication filter
    let filter = MessageDeduplicator::new();
    
    let unique: Vec<_> = duplicates
        .into_iter()
        .filter(|msg| filter.should_send(msg))
        .collect();
    
    // Only one message should pass through
    assert_eq!(unique.len(), 1);
}
```

### Resource Cleanup Tests

```rust
#[tokio::test]
async fn test_graceful_shutdown() {
    let stream = MpscStream::spawn(|tx| async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let _ = tx.send("message").await;
    });
    
    // Drop after short delay
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(stream);
    
    // Should complete without panic or hang
    // Verify in separate test that resources are released
}
```

## Running Tests

### Run specific test
```bash
cargo insta test -p <package> -- <test-name>
```

### Run with snapshot update
```bash
cargo insta test --accept
```

### Run with coverage (if available)
```bash
cargo tarpaulin -o html
```

## Key Files Reference

- Test files: `src/<module>_test.rs` or `tests/` directory
- Snapshots: `snapshots/` directory with `.snap` files
- Fixtures: Use `new()`, `Default`, and `derive_setters::Setters` for test data
- Assertions: Use `pretty_assertions::assert_eq!` for better diffs