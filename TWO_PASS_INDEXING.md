# 两遍索引策略实现

## 概述

为了支持跨文件的方法调用参数类型推断，我们实现了两遍索引策略。

## 问题背景

在构建方法调用时，需要推断调用的参数类型。对于跨文件的方法调用，单遍解析无法获取其他文件中方法的返回类型信息。

**示例：**

```java
// 文件 A: UserRepository.java
public class UserRepository {
    public User findById(String id) {
        return new User();
    }
}

// 文件 B: UserService.java
public class UserService {
    private UserRepository repository;
    
    public void processUser(String userId) {
        // repository.findById() 返回 User
        // processor.process() 的参数类型应该是 User，而不是 Object
        processor.process(repository.findById(userId));
    }
}
```

## 解决方案：两遍索引

### 第一遍：提取方法签名和返回类型

- 快速解析所有文件
- 提取每个方法的签名和返回类型
- 构建全局返回类型映射

### 第二遍：使用全局类型信息重新解析

- 使用全局返回类型映射重新解析所有文件
- 推断跨文件的方法调用参数类型
- 生成更精确的方法调用信息

## 实现细节

### 1. JavaParser 新增方法

```rust
pub fn parse_file_with_global_types(
    &self,
    content: &str,
    file_path: &Path,
    global_return_types: &rustc_hash::FxHashMap<String, String>,
) -> Result<ParsedFile, ParseError>
```

此方法接受全局返回类型映射，用于第二遍解析。

### 2. CodeIndex 新增方法

```rust
pub fn index_workspace_two_pass(
    &mut self,
    workspace_path: &Path,
    parsers: &[Box<dyn LanguageParser>],
) -> Result<(), IndexError>

pub fn index_project_two_pass(
    &mut self,
    project_path: &Path,
    parsers: &[Box<dyn LanguageParser>],
) -> Result<(), IndexError>
```

这些方法实现了两遍索引策略。

## 使用方式

### 方式 1：使用两遍索引（推荐用于跨文件类型推断）

```rust
use code_impact_analyzer::{CodeIndex, JavaParser};
use code_impact_analyzer::language_parser::LanguageParser;

let java_parser = Box::new(JavaParser::new().unwrap());
let parsers: Vec<Box<dyn LanguageParser>> = vec![java_parser];

let mut index = CodeIndex::new();
index.index_workspace_two_pass(workspace_path, &parsers)?;
```

### 方式 2：使用单遍索引（更快，但跨文件类型推断不精确）

```rust
let mut index = CodeIndex::new();
index.index_workspace(workspace_path, &parsers)?;
```

## 性能对比

### 单遍索引

- **优点**：速度快，只解析一次
- **缺点**：跨文件调用的参数类型为 `Object`

### 两遍索引

- **优点**：跨文件类型推断精确
- **缺点**：解析时间约为单遍的 2 倍

## 测试结果

运行 `cargo run --example test_two_pass_indexing` 的输出：

```
2. 单遍索引（对比）

   UserService::processUser 的调用:
     - process(Object)                    ← 参数类型不精确
     - com.example.UserRepository::findById(String)

3. 两遍索引（跨文件类型推断）

   UserService::processUser 的调用:
     - process(User)                      ← 参数类型精确
       ✓ 成功推断出跨文件参数类型: User
     - com.example.UserRepository::findById(String)

   UserService::processAddress 的调用:
     - format(Address)                    ← 参数类型精确
       ✓ 成功推断出跨文件参数类型: Address
     - com.example.UserRepository::getAddress(User)
       ✓ 成功推断出跨文件参数类型: User
     - com.example.UserRepository::findById(String)
```

## 工作流程

```
第一遍解析
  ↓
提取所有方法的返回类型
  ↓
构建全局返回类型映射
  ↓
第二遍解析（使用全局映射）
  ↓
推断跨文件的参数类型
  ↓
构建最终索引
```

## 适用场景

### 推荐使用两遍索引的场景

1. 需要精确的方法调用参数类型
2. 进行影响分析时需要准确的调用关系
3. 代码规模不是特别大（< 10万行）

### 推荐使用单遍索引的场景

1. 代码规模很大（> 10万行）
2. 只需要基本的调用关系，不需要精确的参数类型
3. 性能要求高，可以接受参数类型不精确

## 未来优化方向

1. **增量更新**：只对修改的文件重新解析
2. **缓存机制**：缓存第一遍的结果
3. **并行优化**：优化第二遍的并行解析策略
4. **选择性两遍**：只对需要精确类型的文件进行两遍解析

## 相关文件

- `src/java_parser.rs` - 实现 `parse_file_with_global_types` 方法
- `src/code_index.rs` - 实现 `index_workspace_two_pass` 和 `index_project_two_pass` 方法
- `examples/test_two_pass_indexing.rs` - 测试示例
- `CROSS_FILE_TYPE_INFERENCE.md` - 跨文件类型推断详细说明
- `RETURN_TYPE_INDEX.md` - 返回类型索引实现文档

## 总结

两遍索引策略成功解决了跨文件类型推断的问题：

1. ✅ 第一遍提取所有方法的返回类型
2. ✅ 第二遍使用全局类型信息重新解析
3. ✅ 方法调用的参数类型更加精确
4. ✅ 支持嵌套和链式调用的跨文件类型推断

这是一个实用的解决方案，在性能和功能之间取得了良好的平衡。
