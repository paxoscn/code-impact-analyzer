# 多 Patch 文件支持

## 概述

代码影响分析工具现在支持 `--diff` 参数指向一个包含多个 patch 文件的目录，而不仅仅是单个 patch 文件。这使得工具能够同时分析多个项目的变更影响。

## 使用方法

### 目录结构

```
workspace/
├── project_a/
│   └── src/
├── project_b/
│   └── src/
└── project_c/
    └── src/

patches/
├── project_a.patch    # 对 project_a 的修改
├── project_b.patch    # 对 project_b 的修改
└── project_c.patch    # 对 project_c 的修改
```

### 命令示例

```bash
# 分析多个 patch 文件
code-impact-analyzer \
  --workspace workspace \
  --diff patches \
  --output impact.dot

# 仍然支持单个 patch 文件（向后兼容）
code-impact-analyzer \
  --workspace workspace \
  --diff single.patch \
  --output impact.dot
```

## 功能特性

1. **自动扫描**: 自动扫描目录中的所有 `.patch` 文件
2. **批量解析**: 逐个解析每个 patch 文件，提取文件变更
3. **错误容忍**: 如果某个 patch 文件解析失败，记录警告并继续处理其他文件
4. **向后兼容**: 仍然支持传入单个 patch 文件路径
5. **智能过滤**: 只处理 `.patch` 扩展名的文件，忽略其他文件

## 工作流程

1. 检查 `--diff` 参数指向的路径
2. 如果是文件，直接解析（向后兼容）
3. 如果是目录：
   - 扫描目录中的所有 `.patch` 文件
   - 逐个解析每个 patch 文件
   - 合并所有文件变更信息
   - 如果某个文件解析失败，记录警告并继续
4. 构建代码索引
5. 追溯影响范围
6. 生成统一的影响图

## 输出示例

```
Starting code impact analysis
Workspace: "workspace"
Patch directory: "patches"
Step 1: Parsing patch files from directory
Found 3 patch files to process
Processing patch file: "project_a.patch"
  - Parsed 2 file changes from "project_a.patch"
Processing patch file: "project_b.patch"
  - Parsed 1 file changes from "project_b.patch"
Processing patch file: "project_c.patch"
  - Parsed 3 file changes from "project_c.patch"
Total file changes from all patches: 6
Found 6 file changes
Step 2: Building code index
...
```

## 使用场景

1. **微服务架构**: 同时分析多个微服务的变更影响
2. **Monorepo**: 分析单个仓库中多个项目的变更
3. **批量分析**: 一次性分析多个功能分支的影响
4. **CI/CD 集成**: 自动收集多个项目的 patch 并统一分析

## 生成 Patch 文件

### 为每个项目生成独立的 patch

```bash
# 创建 patches 目录
mkdir patches

# 为每个项目生成 patch
cd project_a
git diff main..feature-branch > ../patches/project_a.patch

cd ../project_b
git diff main..feature-branch > ../patches/project_b.patch

cd ../project_c
git diff main..feature-branch > ../patches/project_c.patch
```

### 在 CI/CD 中使用

```yaml
# GitHub Actions 示例
- name: Generate patches
  run: |
    mkdir patches
    for project in project_a project_b project_c; do
      cd $project
      git diff origin/main...HEAD > ../patches/${project}.patch
      cd ..
    done

- name: Run impact analysis
  run: |
    code-impact-analyzer \
      --workspace . \
      --diff patches \
      --output-format json \
      --output impact.json
```

## 优势

1. **简化工作流**: 不需要多次运行工具，一次分析所有变更
2. **统一视图**: 生成包含所有项目变更的统一影响图
3. **提高效率**: 减少重复的索引构建时间
4. **灵活性**: 支持目录和单文件两种方式

## 注意事项

1. Patch 文件应该是标准的 Git unified diff 格式
2. 建议 patch 文件名与项目名对应，便于识别
3. 单个 patch 文件建议不超过 10MB
4. 如果某个 patch 文件解析失败，工具会记录警告并继续处理其他文件

## 测试

新增了 5 个测试用例来验证功能：

- `test_parse_patches_from_directory`: 测试解析多个 patch 文件
- `test_parse_patches_from_directory_with_non_patch_files`: 测试过滤非 patch 文件
- `test_parse_patches_from_directory_empty`: 测试空目录处理
- `test_parse_patches_from_single_file_backward_compatibility`: 测试向后兼容性
- `test_parse_patch_with_invalid_file`: 测试错误处理

所有测试都已通过。

## 相关文档

- [README.md](README.md) - 项目概述
- [USAGE.md](USAGE.md) - 详细使用指南
- [CHANGES.md](../CHANGES.md) - 变更日志
