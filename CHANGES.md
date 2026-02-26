# 代码影响分析器 - 外部库过滤功能

## 修改摘要

实现了智能过滤外部库方法调用的功能，使影响分析只关注项目内部代码，提高分析结果的可读性和实用性。

## 修改内容

### 1. 核心逻辑修改

**文件**: `code-impact-analyzer/src/impact_tracer.rs`

在方法调用链追溯过程中添加了外部库过滤逻辑：

#### 上游追溯 (`trace_method_upstream`)
```rust
for caller in callers {
    // 检查调用者是否在索引中（忽略外部库）
    if self.index.find_method(caller).is_none() {
        continue;
    }
    // ... 继续处理内部方法
}
```

#### 下游追溯 (`trace_method_downstream`)
```rust
for callee in callees {
    // 检查被调用者是否在索引中（忽略外部库）
    if self.index.find_method(callee).is_none() {
        continue;
    }
    // ... 继续处理内部方法
}
```

### 2. 工作原理

1. **索引阶段**: 工具只索引工作空间内的源代码文件
2. **追溯阶段**: 
   - 当发现方法调用时，先检查被调用方法是否在索引中
   - 如果不在索引中（即外部库），则跳过该调用
   - 只追溯和记录项目内部的方法调用关系

### 3. 影响范围

这个修改会自动过滤以下类型的外部调用：

#### Java
- JDK 标准库: `System.out.println()`, `String.format()`, `Objects.requireNonNull()` 等
- 第三方库: Spring Framework, Apache Commons, Guava 等（如果不在工作空间中）

#### Rust
- 标准库: `println!()`, `format!()`, `assert!()` 等
- 第三方 crate: 所有不在工作空间中的依赖

### 4. 优势

1. **更清晰的影响图**: 只显示项目内部的调用关系，避免大量外部库节点
2. **更快的分析速度**: 减少不必要的追溯，提高性能
3. **更准确的影响范围**: 聚焦于实际需要关注的代码变更影响
4. **自动化处理**: 无需手动配置，自动识别和过滤

## 测试验证

所有现有测试（共 126 个）全部通过：
- 95 个单元测试
- 31 个集成测试

测试覆盖：
- 基本功能测试
- 跨服务追溯测试
- 配置关联测试
- 并行处理测试
- 端到端测试

## 使用示例

### 修改前
影响图可能包含大量外部库节点：
```
processData -> validateData
processData -> System.out.println
processData -> String.format
validateData -> Objects.requireNonNull
validateData -> IllegalArgumentException.<init>
```

### 修改后
影响图只包含项目内部节点：
```
processData -> validateData
```

## 兼容性

- 完全向后兼容
- 不需要修改任何配置或命令行参数
- 对现有功能无影响

## 文档更新

已更新 `README.md`，添加了以下内容：
- 技术特点中增加"智能过滤外部库调用"说明
- 高级功能中新增"外部库调用过滤"章节，详细说明工作原理和示例

## 总结

这个修改通过在追溯过程中检查方法是否在索引中，实现了自动过滤外部库调用的功能。这使得代码影响分析工具更加实用，生成的影响图更加清晰，帮助开发者更好地理解代码变更的实际影响范围。
