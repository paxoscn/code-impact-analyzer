# 方法返回类型索引实现

## 概述

为了支持更准确的方法调用参数类型推断，我们在索引中添加了方法返回类型信息。

## 当前实现

### 1. 数据结构

在 `MethodInfo` 和 `FunctionInfo` 中添加了 `return_type` 字段：

```rust
pub struct MethodInfo {
    // ... 其他字段
    /// 方法返回类型（用于类型推断）
    pub return_type: Option<String>,
}
```

### 2. 返回类型提取

- **Java 方法**：使用 `extract_return_type` 从方法声明中提取返回类型
- **自动生成的 getter**：返回类型与字段类型相同
- **Rust 函数**：暂时设为 `None`（未实现）

### 3. 索引查询

添加了 `find_method_return_type` 方法，可以根据方法签名查询返回类型：

```rust
pub fn find_method_return_type(&self, method_signature: &str) -> Option<&str>
```

## 类型推断的两个层次

### 层次 1：文件内类型推断（已实现 ✅）

在解析单个文件时，使用文件内的方法返回类型映射进行类型推断。

**工作原理：**
1. 第一遍：提取所有类和方法，建立方法返回类型映射
2. 第二遍：使用返回类型映射重新提取方法调用

**示例：**
```java
// 同一文件内
public class UserRepository {
    public User findUser(String id) { return null; }
}

public class TestService {
    private UserRepository repo;
    
    public void test() {
        // findUser() 返回 User，所以 process 的参数类型被推断为 User
        processor.process(repo.findUser("123"));
        // 方法调用目标：Processor::process(User)
    }
}
```

**支持的特性：**
- ✅ 嵌套方法调用类型推断
- ✅ 链式调用类型推断
- ✅ 自动生成的 getter 方法
- ✅ 类字段、方法参数、本地变量类型解析

### 层次 2：跨文件类型推断（通过索引查询实现 ✅）

**当前实现：**
- ✅ 返回类型已存储在索引中
- ✅ 提供了查询接口 `find_method_return_type`
- ✅ 可在影响分析时使用索引查询跨文件类型

**使用方式：**

```rust
// 在影响分析时查询返回类型
let method = index.find_method("com.example.Service::process").unwrap();
for call in &method.calls {
    // 查询被调用方法的返回类型（可能在其他文件中）
    if let Some(return_type) = index.find_method_return_type(&call.target) {
        println!("调用 {} 返回 {}", call.target, return_type);
    }
}
```

**设计决策：**

解析器在解析时**不使用**索引中的返回类型，原因：
1. **避免循环依赖**：解析器和索引之间的循环依赖
2. **支持并行解析**：多个文件同时解析时索引不完整
3. **保持高性能**：避免两遍解析带来的性能开销

**推荐做法：**

使用**延迟推断**策略（方案 B）：
- 解析时：使用文件内类型推断
- 使用时：在影响分析时查询索引获取跨文件类型

这是性能和功能之间的最佳权衡。

**未来可选改进方向：**

如果确实需要在解析时进行跨文件类型推断，可以考虑：

#### 方案 A：两遍索引

1. 第一遍：快速解析，只提取方法签名和返回类型
2. 第二遍：使用完整索引重新解析方法调用

**缺点：** 解析时间翻倍

#### 方案 C：增量更新

在索引完成后，遍历所有方法调用，使用索引更新参数类型。

**缺点：** 实现复杂，需要存储额外信息

详细说明请参考 `CROSS_FILE_TYPE_INFERENCE.md`

## 使用示例

### 查询方法返回类型

```rust
let index = CodeIndex::new();
// ... 构建索引

// 查询方法返回类型
if let Some(return_type) = index.find_method_return_type("com.example.UserRepository::findUser(String)") {
    println!("findUser 返回类型: {}", return_type);
}
```

### 在影响分析中使用

```rust
// 在追踪方法调用时，可以使用返回类型信息
let method = index.find_method("com.example.Service::process").unwrap();
for call in &method.calls {
    // 如果调用的是另一个方法，可以查询其返回类型
    if let Some(return_type) = index.find_method_return_type(&call.target) {
        println!("调用 {} 返回 {}", call.target, return_type);
    }
}
```

## 测试覆盖

- ✅ 返回类型正确提取并存储
- ✅ 自动生成的 getter 包含正确的返回类型
- ✅ 索引查询方法正常工作
- ✅ 文件内类型推断正常工作
- ✅ 跨文件类型查询（通过索引 API）

## 当前状态总结

**已完成：**
1. 返回类型存储在索引中
2. 文件内类型推断完全实现
3. 跨文件类型查询 API 可用

**使用建议：**
- 在影响分析时使用 `find_method_return_type` 查询跨文件返回类型
- 这是性能和功能的最佳权衡
- 详细说明请参考 `CROSS_FILE_TYPE_INFERENCE.md`

## 相关文件

- `src/language_parser.rs` - MethodInfo 和 FunctionInfo 定义
- `src/java_parser.rs` - Java 返回类型提取和文件内类型推断
- `src/code_index.rs` - 索引存储和查询
- `METHOD_SIGNATURE_IMPLEMENTATION.md` - 方法签名实现文档
- `CROSS_FILE_TYPE_INFERENCE.md` - 跨文件类型推断详细说明
