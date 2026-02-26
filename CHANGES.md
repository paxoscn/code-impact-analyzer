# 代码影响分析器 - 变更日志

## [未发布] - 支持多 Patch 文件目录

### 新增功能

实现了 `--diff` 参数支持指向包含多个 patch 文件的目录，而不仅仅是单个 patch 文件。这使得工具能够同时分析多个项目的变更影响。

### 修改内容

#### 1. CLI 参数更新

**文件**: `code-impact-analyzer/src/cli.rs`

更新了 `--diff` 参数的描述：
```rust
/// Git diff 补丁文件目录路径，包含以项目命名的多个 patch 文件
#[arg(short = 'd', long = "diff", value_name = "PATH")]
pub diff_path: PathBuf,
```

#### 2. Orchestrator 核心逻辑

**文件**: `code-impact-analyzer/src/orchestrator.rs`

新增 `parse_patches_from_directory` 方法：
- 检查路径是文件还是目录
- 如果是文件，直接解析（向后兼容）
- 如果是目录，扫描所有 `.patch` 文件并逐个解析
- **从文件名提取项目名，并作为目录前缀添加到文件路径**
- 合并所有 patch 文件的变更信息

```rust
pub fn parse_patches_from_directory(&mut self, patch_dir: &Path) -> Result<Vec<FileChange>, AnalysisError>
```

更新 `parse_patch` 方法：
- 新增 `project_prefix` 参数
- 如果提供了项目前缀，自动添加到所有文件路径前

```rust
fn parse_patch(&mut self, patch_path: &Path, project_prefix: Option<String>) -> Result<Vec<FileChange>, AnalysisError>
```

#### 3. 文件路径前缀处理

**关键特性**: 工具会自动为 patch 文件中的每个文件路径添加项目名前缀。

**示例**:
- Patch 文件名: `project_a.patch`
- Patch 中的文件路径: `src/ServiceA.java`
- 实际解析后的路径: `project_a/src/ServiceA.java`

这样可以确保：
1. 多个项目可以有相同的文件路径结构
2. 工具能够正确定位每个项目中的文件
3. 避免文件路径冲突

#### 4. 使用方式

**目录结构示例**:
```
workspace/
├── project_a/
├── project_b/
└── project_c/

patches/
├── project_a.patch
├── project_b.patch
└── project_c.patch
```

**命令示例**:
```bash
code-impact-analyzer \
  --workspace workspace \
  --diff patches \
  --output impact.dot
```

**重要**: Patch 文件名（去掉 .patch 扩展名）必须与 workspace 中的项目目录名一致。

### 功能特性

1. **自动扫描**: 自动扫描目录中的所有 `.patch` 文件
2. **批量解析**: 逐个解析每个 patch 文件，提取文件变更
3. **错误容忍**: 如果某个 patch 文件解析失败，记录警告并继续处理其他文件
4. **向后兼容**: 仍然支持传入单个 patch 文件路径
5. **智能过滤**: 只处理 `.patch` 扩展名的文件，忽略其他文件

### 测试验证

新增 7 个测试用例：
- `test_parse_patches_from_directory`: 测试解析多个 patch 文件
- `test_parse_patches_from_directory_with_non_patch_files`: 测试过滤非 patch 文件
- `test_parse_patches_from_directory_empty`: 测试空目录处理
- `test_parse_patches_from_single_file_backward_compatibility`: 测试向后兼容性
- `test_parse_patch_with_invalid_file`: 测试错误处理
- `test_parse_patch_with_project_prefix`: 测试项目前缀功能
- `test_parse_patches_from_directory_with_project_prefix`: 测试目录解析时的项目前缀

所有测试（共 103 个）全部通过。

### 文档更新

#### README.md
- 更新了命令行参数说明
- 新增了多 patch 文件的使用示例
- 添加了工作空间和 Patch 目录结构说明
- 更新了分析流程描述

#### USAGE.md
- 更新了快速开始指南
- 详细说明了 `--diff` 参数的两种使用方式
- 新增了多项目分析的实际案例
- 更新了最佳实践建议

#### 新增示例
创建了 `examples/multi-patch/` 示例目录：
- `patches/project_a.patch`: 示例 patch 文件 A
- `patches/project_b.patch`: 示例 patch 文件 B
- `README.md`: 使用说明

### 使用场景

这个功能特别适合以下场景：

1. **微服务架构**: 同时分析多个微服务的变更影响
2. **Monorepo**: 分析单个仓库中多个项目的变更
3. **批量分析**: 一次性分析多个功能分支的影响
4. **CI/CD 集成**: 自动收集多个项目的 patch 并统一分析

### 示例输出

```
Starting code impact analysis
Workspace: "examples/single-call"
Patch directory: "examples/multi-patch/patches"
Step 1: Parsing patch files from directory
Found 2 patch files to process
Processing patch file: "project_a.patch"
  - Parsed 1 file changes from "project_a.patch"
Processing patch file: "project_b.patch"
  - Parsed 1 file changes from "project_b.patch"
Total file changes from all patches: 2
Found 2 file changes
...
```

### 兼容性

- 完全向后兼容，仍支持传入单个 patch 文件
- 不需要修改现有的使用方式
- 对现有功能无影响

### 优势

1. **简化工作流**: 不需要多次运行工具，一次分析所有变更
2. **统一视图**: 生成包含所有项目变更的统一影响图
3. **提高效率**: 减少重复的索引构建时间
4. **灵活性**: 支持目录和单文件两种方式

---

## [之前] - 外部库过滤功能

### 修改摘要

实现了智能过滤外部库方法调用的功能，使影响分析只关注项目内部代码，提高分析结果的可读性和实用性。

### 修改内容

#### 1. 核心逻辑修改

**文件**: `code-impact-analyzer/src/impact_tracer.rs`

在方法调用链追溯过程中添加了外部库过滤逻辑：

##### 上游追溯 (`trace_method_upstream`)
```rust
for caller in callers {
    // 检查调用者是否在索引中（忽略外部库）
    if self.index.find_method(caller).is_none() {
        continue;
    }
    // ... 继续处理内部方法
}
```

##### 下游追溯 (`trace_method_downstream`)
```rust
for callee in callees {
    // 检查被调用者是否在索引中（忽略外部库）
    if self.index.find_method(callee).is_none() {
        continue;
    }
    // ... 继续处理内部方法
}
```

#### 2. 工作原理

1. **索引阶段**: 工具只索引工作空间内的源代码文件
2. **追溯阶段**: 
   - 当发现方法调用时，先检查被调用方法是否在索引中
   - 如果不在索引中（即外部库），则跳过该调用
   - 只追溯和记录项目内部的方法调用关系

#### 3. 影响范围

这个修改会自动过滤以下类型的外部调用：

##### Java
- JDK 标准库: `System.out.println()`, `String.format()`, `Objects.requireNonNull()` 等
- 第三方库: Spring Framework, Apache Commons, Guava 等（如果不在工作空间中）

##### Rust
- 标准库: `println!()`, `format!()`, `assert!()` 等
- 第三方 crate: 所有不在工作空间中的依赖

#### 4. 优势

1. **更清晰的影响图**: 只显示项目内部的调用关系，避免大量外部库节点
2. **更快的分析速度**: 减少不必要的追溯，提高性能
3. **更准确的影响范围**: 聚焦于实际需要关注的代码变更影响
4. **自动化处理**: 无需手动配置，自动识别和过滤

### 测试验证

所有现有测试（共 126 个）全部通过：
- 95 个单元测试
- 31 个集成测试

测试覆盖：
- 基本功能测试
- 跨服务追溯测试
- 配置关联测试
- 并行处理测试
- 端到端测试

### 使用示例

#### 修改前
影响图可能包含大量外部库节点：
```
processData -> validateData
processData -> System.out.println
processData -> String.format
validateData -> Objects.requireNonNull
validateData -> IllegalArgumentException.<init>
```

#### 修改后
影响图只包含项目内部节点：
```
processData -> validateData
```

### 兼容性

- 完全向后兼容
- 不需要修改任何配置或命令行参数
- 对现有功能无影响

### 文档更新

已更新 `README.md`，添加了以下内容：
- 技术特点中增加"智能过滤外部库调用"说明
- 高级功能中新增"外部库调用过滤"章节，详细说明工作原理和示例

### 总结

这个修改通过在追溯过程中检查方法是否在索引中，实现了自动过滤外部库调用的功能。这使得代码影响分析工具更加实用，生成的影响图更加清晰，帮助开发者更好地理解代码变更的实际影响范围。
