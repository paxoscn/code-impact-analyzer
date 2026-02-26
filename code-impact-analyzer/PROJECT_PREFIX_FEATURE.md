# 项目前缀功能说明

## 概述

代码影响分析工具现在支持自动为 patch 文件中的文件路径添加项目名前缀。这个功能使得工具能够正确处理多项目场景，避免文件路径冲突。

## 问题背景

在多项目的 workspace 中，不同项目可能有相同的文件路径结构。例如：

```
workspace/
├── project_a/
│   └── src/ServiceA.java
└── project_b/
    └── src/ServiceB.java
```

如果 patch 文件中只包含相对路径（如 `src/ServiceA.java`），工具无法确定该文件属于哪个项目。

## 解决方案

工具会从 patch 文件名中提取项目名，并自动添加到文件路径前。

### 工作流程

1. **扫描 patch 目录**
   ```
   patches/
   ├── project_a.patch
   └── project_b.patch
   ```

2. **提取项目名**
   - `project_a.patch` → 项目名: `project_a`
   - `project_b.patch` → 项目名: `project_b`

3. **解析 patch 文件**
   
   `project_a.patch` 内容：
   ```diff
   diff --git a/src/ServiceA.java b/src/ServiceA.java
   index 1234567..abcdefg 100644
   --- a/src/ServiceA.java
   +++ b/src/ServiceA.java
   @@ -10,7 +10,8 @@ public class ServiceA {
        }
   ```

4. **添加项目前缀**
   - 原始路径: `src/ServiceA.java`
   - 添加前缀后: `project_a/src/ServiceA.java`

5. **在 workspace 中定位文件**
   - 完整路径: `workspace/project_a/src/ServiceA.java`

## 使用要求

### 必须遵守的规则

1. **Patch 文件名必须与项目目录名一致**
   
   ✅ 正确：
   ```
   workspace/project_a/  ←→  patches/project_a.patch
   workspace/project_b/  ←→  patches/project_b.patch
   ```
   
   ❌ 错误：
   ```
   workspace/project_a/  ←→  patches/projectA.patch  (名称不匹配)
   workspace/project_b/  ←→  patches/proj_b.patch   (名称不匹配)
   ```

2. **Patch 文件中的路径应该是相对于项目根目录的**
   
   ✅ 正确：
   ```diff
   diff --git a/src/ServiceA.java b/src/ServiceA.java
   ```
   
   ❌ 错误：
   ```diff
   diff --git a/project_a/src/ServiceA.java b/project_a/src/ServiceA.java
   ```

### 生成 Patch 文件的正确方式

```bash
# 创建 patches 目录
mkdir patches

# 在每个项目目录下生成 patch
cd workspace/project_a
git diff main..feature-branch > ../../patches/project_a.patch

cd ../project_b
git diff main..feature-branch > ../../patches/project_b.patch
```

## 代码实现

### 核心逻辑

```rust
// 从文件名提取项目名
let project_name = patch_file
    .file_stem()
    .and_then(|s| s.to_str())
    .map(|s| s.to_string());

// 解析 patch 并添加前缀
match self.parse_patch(&patch_file, project_name) {
    Ok(mut changes) => {
        // changes 中的文件路径已经包含项目前缀
    }
}
```

```rust
// parse_patch 方法
fn parse_patch(&mut self, patch_path: &Path, project_prefix: Option<String>) 
    -> Result<Vec<FileChange>, AnalysisError> 
{
    match PatchParser::parse_patch_file(patch_path) {
        Ok(mut changes) => {
            // 如果提供了项目前缀，添加到所有文件路径前
            if let Some(prefix) = project_prefix {
                for change in &mut changes {
                    change.file_path = format!("{}/{}", prefix, change.file_path);
                }
            }
            Ok(changes)
        }
    }
}
```

## 测试验证

### 测试用例

```rust
#[test]
fn test_parse_patch_with_project_prefix() {
    // 创建 patch 文件
    let patch_content = r#"diff --git a/src/ServiceA.java b/src/ServiceA.java
index 1234567..abcdefg 100644
--- a/src/ServiceA.java
+++ b/src/ServiceA.java
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
"#;
    
    // 解析时带上项目前缀
    let result = orchestrator.parse_patch(&patch_path, Some("project_a".to_string()));
    
    // 验证文件路径包含项目前缀
    assert_eq!(changes[0].file_path, "project_a/src/ServiceA.java");
}
```

### 测试结果

所有 103 个测试全部通过，包括：
- 基本功能测试
- 项目前缀功能测试
- 多文件解析测试
- 向后兼容性测试

## 优势

1. **避免路径冲突**: 多个项目可以有相同的文件路径结构
2. **自动化处理**: 无需手动修改 patch 文件
3. **正确定位**: 工具能够准确找到每个项目中的文件
4. **简化工作流**: 用户只需按照规范命名 patch 文件

## 向后兼容性

- 单个 patch 文件仍然支持（不添加前缀）
- 现有的使用方式不受影响
- 只有在目录模式下才会自动添加前缀

## 日志输出

工具会在日志中显示项目名和前缀处理过程：

```
Processing patch file: "project_a.patch"
  - Project name: project_a
  - Prefixed file path: project_a/src/ServiceA.java
  - Parsed 1 file changes from "project_a.patch"
```

## 故障排除

### 问题：文件找不到

**症状**:
```
Warning: File does not exist: workspace/src/ServiceA.java
```

**原因**: Patch 文件名与项目目录名不匹配

**解决方案**:
1. 检查 patch 文件名是否与项目目录名完全一致
2. 确保 patch 文件名使用 `.patch` 扩展名
3. 检查项目目录是否存在于 workspace 中

### 问题：路径重复

**症状**:
```
Warning: File does not exist: workspace/project_a/project_a/src/ServiceA.java
```

**原因**: Patch 文件中的路径已经包含了项目名

**解决方案**:
1. 确保 patch 文件中的路径是相对于项目根目录的
2. 不要在 patch 文件中包含项目名前缀

## 相关文档

- [README.md](README.md) - 项目概述
- [USAGE.md](USAGE.md) - 详细使用指南
- [MULTI_PATCH_SUPPORT.md](MULTI_PATCH_SUPPORT.md) - 多 patch 文件支持
- [CHANGES.md](../CHANGES.md) - 变更日志
