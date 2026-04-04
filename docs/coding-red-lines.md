# 编码红线规范

本文档定义了项目中**禁止**出现的编码模式，作为红线规范来确保代码质量。任何违反这些规范的做法都应在代码审查中被拒绝。

---

## 1. 测试中断言规范

### 1.1 禁止使用 `assert!(true)` 或类似的恒真断言

**错误示例：**

```rust
#[test]
fn test_something() {
    let result = do_something();
    // 永远为真的断言，没有测试价值
    assert!(true);
}
```

**正确做法：**
- 如果测试只是验证函数不panic，可以直接省略断言
- 如果需要验证行为，应使用有意义的断言

```rust
#[test]
fn test_something() {
    let result = do_something();
    // 直接调用，不添加无意义的断言
}

#[test]
fn test_something() {
    let result = do_something();
    assert!(result.is_ok()); // 有意义的断言
}
```

---

### 1.2 禁止使用逻辑上恒真的断言表达式

**错误示例：**

```rust
#[test]
fn test_something() {
    let result = some_function();
    // 逻辑错误：!result || result 永远为真
    assert!(!result || result);
    
    // 另一个错误示例
    let value = get_value();
    assert!(value == value); // 永远为真
}
```

**正确做法：**

```rust
#[test]
fn test_something() {
    let result = some_function();
    // 明确测试期望的结果
    assert!(result);
    
    // 或者忽略不使用的返回值
    let _result = some_function();
}
```

---

## 2. Clippy 警告处理规范

### 2.1 必须修复的 Clippy 错误

项目中启用了严格的 Clippy 检查 (`-Dwarnings`)，以下错误必须修复：

- `clippy::assertions_on_constants` - 恒真断言
- `clippy::overly_complex_bool_expr` - 过于复杂的布尔表达式

### 2.2 在提交前运行 Clippy

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

---

## 3. 常见错误模式汇总

| 错误模式 | 问题 | 正确做法 |
|---------|------|---------|
| `assert!(true)` | 恒真断言，无测试价值 | 删除或替换为有意义的断言 |
| `assert!(!x \|\| x)` | 逻辑错误，永远为真 | 改为 `assert!(x)` 或使用实际值 |
| `assert!(x == x)` | 永远为真 | 删除或使用有意义的比较 |
| `#[ignore]` 的测试 | 可能遗漏的测试 | 确保测试被正确执行或删除 |

---

## 4. 代码审查检查清单

在代码审查中，请确认：

- [ ] 没有恒真断言 (`assert!(true)`, `assert!(false)`)
- [ ] 没有逻辑上恒真的表达式
- [ ] Clippy 检查通过
- [ ] 所有测试实际验证了预期的行为

---

## 5. 违规示例（来自项目实际案例）

### 案例 1: forge_config_adapter/src/detector.rs

**原始代码：**
```rust
#[test]
fn test_detect_claude_md() {
    let temp_dir = std::env::temp_dir();
    let _configs = ConfigDetector::detect_claude_code_configs(&temp_dir);
    
    // Just verify the function works without panicking
    assert!(true);  // ❌ 恒真断言
}

#[test]
fn test_has_external_configs() {
    let temp_dir = std::env::temp_dir();
    let result = ConfigDetector::has_external_configs(&temp_dir);
    assert!(!result || result);  // ❌ 逻辑错误，永远为真
}
```

**修复后：**
```rust
#[test]
fn test_detect_claude_md() {
    let temp_dir = std::env::temp_dir();
    let _configs = ConfigDetector::detect_claude_code_configs(&temp_dir);
    // 函数不panic即表示通过
}

#[test]
fn test_has_external_configs() {
    let temp_dir = std::env::temp_dir();
    let _result = ConfigDetector::has_external_configs(&temp_dir);
    // 函数不panic即表示通过
}
```

---

## 6. 相关工具配置

项目在 `.github/workflows/ci.yml` 中启用了严格的 Rust 编译警告：

```yaml
env:
  RUSTFLAGS: '-Dwarnings'
```

这意味着任何 Clippy 警告都会导致 CI 失败。