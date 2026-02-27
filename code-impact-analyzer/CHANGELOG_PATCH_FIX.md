# Changelog - Patch 尾部内容处理修复

## [修复] 2024-02-27

### 问题
- 读取 `git format-patch` 生成的 patch 文件时，如果在 `--` 分隔符后面还有文字（如版本号），会导致程序 panic

### 原因
- `gitpatch` crate 无法处理 `--` 分隔符后面的非空行内容
- `git format-patch` 生成的文件通常在末尾包含 `-- ` 和版本号（如 `2.39.0`）

### 解决方案
- 在 `src/patch_parser.rs` 中添加了 `remove_trailing_content()` 方法
- 在调用 `gitpatch::Patch::from_multiple()` 之前预处理 patch 内容
- 自动移除 `--` 分隔符后面的所有非空行内容

### 变更文件
- `src/patch_parser.rs`
  - 修改 `parse_patch_file()` 方法，添加内容预处理
  - 新增 `remove_trailing_content()` 私有方法
  - 新增 5 个测试用例

### 测试
- 所有新增测试通过 (13/13)
- 使用实际 patch 文件验证通过
- 向后兼容性保持完好

### 影响
- 修复了 panic 问题
- 提高了对不同格式 patch 文件的兼容性
- 不影响现有功能
