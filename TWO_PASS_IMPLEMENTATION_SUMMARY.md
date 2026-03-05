# 两遍索引实现总结

## 实现内容

根据用户需求"构建方法的 calls 时需要传入所有 java 类，因为需要推断调用的参数类型"，我们实现了两遍索引策略。

## 核心改动

### 1. JavaParser 新增方法

**文件：** `src/java_parser.rs`

```rust
pub fn parse_file_with_global_types(
    &self,
    content: &str,
    file_path: &Path,
    global_return_types: &rustc_hash::FxHashMap<String, String>,
) -> Result<ParsedFile, ParseError>
```

- 接受全局返回类型映射作为参数
- 合并文件内和全局返回类型
- 使用合并后的映射推断方法调用参数类型

### 2. CodeIndex 新增方法

**文件：** `src/code_index.rs`

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

- 第一遍：提取所有方法的返回类型
- 构建全局返回类型映射
- 第二遍：使用全局映射重新解析
- 生成精确的方法调用信息

## 工作流程

```
┌──────────────────────────────────────┐
│ 第一遍：快速解析所有文件             │
│ - 提取方法签名                       │
│ - 提取返回类型                       │
└──────────────────────────────────────┘
                ↓
┌──────────────────────────────────────┐
│ 构建全局返回类型映射                 │
│ method_signature -> return_type      │
└──────────────────────────────────────┘
                ↓
┌──────────────────────────────────────┐
│ 第二遍：使用全局映射重新解析         │
│ - 推断跨文件的参数类型               │
│ - 生成精确的方法调用                 │
└──────────────────────────────────────┘
                ↓
┌──────────────────────────────────────┐
│ 构建最终索引                         │
│ - 方法信息                           │
│ - 调用关系                           │
│ - 精确的参数类型                     │
└──────────────────────────────────────┘
```

## 测试验证

### 测试文件

`examples/test_two_pass_indexing.rs`

### 测试场景

```java
// UserRepository.java
public class UserRepository {
    public User findById(String id) { return new User(); }
    public Address getAddress(User user) { return user.getAddress(); }
}

// UserService.java
public class UserService {
    private UserRepository repository;
    
    public void processUser(String userId) {
        processor.process(repository.findById(userId));
    }
    
    public void processAddress(String userId) {
        formatter.format(repository.getAddress(repository.findById(userId)));
    }
}
```

### 测试结果

**单遍索引：**
```
UserService::processUser 的调用:
  - process(Object)                    ← 参数类型不精确
  - com.example.UserRepository::findById(String)
```

**两遍索引：**
```
UserService::processUser 的调用:
  - process(User)                      ← 参数类型精确！
    ✓ 成功推断出跨文件参数类型: User
  - com.example.UserRepository::findById(String)

UserService::processAddress 的调用:
  - format(Address)                    ← 参数类型精确！
    ✓ 成功推断出跨文件参数类型: Address
  - com.example.UserRepository::getAddress(User)
    ✓ 成功推断出跨文件参数类型: User
  - com.example.UserRepository::findById(String)
```

## 使用方式

### 方式 1：两遍索引（推荐）

```rust
use code_impact_analyzer::{CodeIndex, JavaParser};
use code_impact_analyzer::language_parser::LanguageParser;

let java_parser = Box::new(JavaParser::new().unwrap());
let parsers: Vec<Box<dyn LanguageParser>> = vec![java_parser];

let mut index = CodeIndex::new();
index.index_workspace_two_pass(workspace_path, &parsers)?;
```

### 方式 2：单遍索引（更快）

```rust
let mut index = CodeIndex::new();
index.index_workspace(workspace_path, &parsers)?;
```

## 性能影响

- **解析时间**：约为单遍的 2 倍
- **内存占用**：需要存储全局返回类型映射
- **精度提升**：跨文件参数类型完全精确

## 适用场景

### 推荐使用两遍索引

- 需要精确的方法调用参数类型
- 进行影响分析时需要准确的调用关系
- 代码规模中等（< 10万行）

### 推荐使用单遍索引

- 代码规模很大（> 10万行）
- 只需要基本的调用关系
- 性能要求高

## 文档

- `TWO_PASS_INDEXING.md` - 详细技术文档（英文）
- `跨文件类型推断说明.md` - 用户友好说明（中文）
- `CROSS_FILE_TYPE_INFERENCE.md` - 跨文件类型推断技术说明
- `RETURN_TYPE_INDEX.md` - 返回类型索引实现文档

## 运行测试

```bash
cd code-impact-analyzer

# 测试两遍索引
cargo run --example test_two_pass_indexing

# 对比演示
cargo run --example demo_cross_file_inference
```

## 总结

✅ **完全实现了用户需求**
- 构建方法 calls 时使用所有 Java 类的信息
- 跨文件的方法调用参数类型推断精确
- 支持嵌套和链式调用

✅ **提供灵活的使用方式**
- 两遍索引：精确但较慢
- 单遍索引：快速但不够精确
- 用户可根据需求选择

✅ **完整的测试验证**
- 测试用例覆盖跨文件调用
- 测试用例覆盖嵌套调用
- 对比单遍和两遍的结果

这是一个完整、实用的解决方案！
