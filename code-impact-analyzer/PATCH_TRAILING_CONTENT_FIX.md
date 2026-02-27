# Patch 文件尾部内容处理修复

## 问题描述

当读取 `git format-patch` 生成的 patch 文件时，如果文件在 `--` 分隔符后面还有额外的文本（如版本号），会导致程序 panic。

### 问题示例

```diff
diff --git a/test.txt b/test.txt
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+line 2
 line 3
-- 
2.39.0
```

在上面的例子中，`--` 后面的 `2.39.0` 会导致 `gitpatch` crate panic。

## 根本原因

`gitpatch` crate (v0.7) 在解析 patch 文件时，如果遇到 `--` 分隔符后面有非空行内容，会触发 panic：

```
thread panicked at gitpatch-0.7.1/src/parser.rs:85:5:
bug: failed to parse entire input. Remaining: '2.39.0\n'
```

这是因为 `gitpatch` 期望 patch 文件在 `--` 后面只有空行或者没有内容。

## 解决方案

在调用 `gitpatch::Patch::from_multiple()` 之前，预处理 patch 内容，移除 `--` 分隔符后面的所有非空行内容。

### 实现细节

添加了 `PatchParser::remove_trailing_content()` 方法：

1. 逐行扫描 patch 内容
2. 当遇到 `--` 或 `-- ` 开头的行时：
   - 保留该行（因为它是 patch 格式的一部分）
   - 检查下一行是否为空
   - 如果下一行不为空，则截断后续所有内容
3. 返回清理后的内容

### 代码变更

在 `src/patch_parser.rs` 中：

```rust
pub fn parse_patch_file(path: &Path) -> Result<Vec<FileChange>, ParseError> {
    let content = fs::read_to_string(path)?;
    
    // 预处理：移除 "-- " 分隔符后面的所有内容
    let cleaned_content = Self::remove_trailing_content(&content);
    
    let patches = gitpatch::Patch::from_multiple(&cleaned_content)?;
    // ... 其余处理逻辑
}

fn remove_trailing_content(content: &str) -> String {
    // 实现细节见源代码
}
```

## 测试覆盖

添加了以下测试用例：

1. `test_parse_patch_with_trailing_content` - 测试 `git format-patch` 格式的 patch
2. `test_parse_patch_with_version_line_after_separator` - 测试 `--` 后面有版本号的情况
3. `test_parse_patch_with_multiple_trailing_lines` - 测试 `--` 后面有多行内容的情况
4. `test_remove_trailing_content` - 测试移除尾部内容的功能
5. `test_remove_trailing_content_with_empty_line` - 测试 `--` 后面只有空行的情况
6. `test_remove_trailing_content_no_separator` - 测试没有 `--` 分隔符的情况

所有测试均通过。

## 兼容性

此修复保持了向后兼容性：

- 对于标准的 `git diff` 输出（没有 `--` 分隔符），不会有任何影响
- 对于 `git format-patch` 输出（有 `--` 分隔符但后面只有空行），也能正常处理
- 对于有额外尾部内容的 patch 文件，现在可以正确解析而不会 panic

## 验证

使用实际的 patch 文件进行了验证：

```bash
cargo run -- --workspace examples/added-one-line --diff examples/added-one-line/patches
cargo run -- --workspace examples/removed-one-line --diff examples/removed-one-line/patches
cargo run -- --workspace examples/single-call --diff examples/single-call/patches
```

所有测试均成功运行，没有 panic。
