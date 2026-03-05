# 跨文件类型推断实现说明

## 当前状态

### 已实现功能

1. **返回类型存储在索引中** ✅
   - `MethodInfo` 和 `FunctionInfo` 包含 `return_type` 字段
   - Java 方法的返回类型在解析时被提取
   - 自动生成的 getter 方法包含正确的返回类型
   - 返回类型存储在索引中，可通过 `find_method_return_type` 查询

2. **文件内类型推断** ✅
   - 使用两遍解析策略：
     - 第一遍：提取所有类和方法，建立方法返回类型映射
     - 第二遍：使用返回类型映射重新提取方法调用
   - 支持嵌套方法调用的类型推断（如 `process(repo.findUser("123"))`）
   - 支持链式调用的类型推断（如 `user.getAddress().getCity()`）

3. **索引查询 API** ✅
   - `CodeIndex::find_method_return_type(method_signature)` 可查询任何方法的返回类型
   - 方法签名格式：`ClassName::methodName(Type1,Type2,...)`

### 当前限制

**解析器在解析时不使用索引中的返回类型**

原因：
- 解析器在解析单个文件时，索引可能还未完全构建
- 解析器设计为无状态，不依赖外部索引
- 并行解析时，索引处于不完整状态

## 跨文件类型推断的使用方式

虽然解析器在解析时不使用索引，但索引已经存储了所有方法的返回类型，可以在影响分析时使用。

### 方式 1：在影响分析时查询（推荐）

在追踪方法调用链时，可以使用索引查询返回类型：

```rust
// 示例：在影响分析器中使用
let method = index.find_method("com.example.Service::process").unwrap();
for call in &method.calls {
    // 查询被调用方法的返回类型
    if let Some(return_type) = index.find_method_return_type(&call.target) {
        println!("调用 {} 返回 {}", call.target, return_type);
        
        // 可以根据返回类型进一步分析
        // 例如：查找使用该返回类型的其他方法
    }
}
```

### 方式 2：手动构建跨文件类型映射

如果需要在特定场景下进行跨文件类型推断，可以从索引中提取所有方法的返回类型：

```rust
// 构建全局返回类型映射
let mut global_return_types = std::collections::HashMap::new();
for (method_sig, method_info) in index.methods() {
    if let Some(return_type) = &method_info.return_type {
        global_return_types.insert(method_sig.clone(), return_type.clone());
    }
}

// 使用全局映射进行类型推断
// ...
```

## 为什么不在解析时使用索引？

### 问题 1：循环依赖

如果解析器依赖索引，而索引又依赖解析器，会形成循环依赖：
- 解析文件 A 需要文件 B 的返回类型
- 但文件 B 可能还未被解析
- 或者文件 B 也需要文件 A 的返回类型

### 问题 2：并行解析

当前实现使用 rayon 并行解析所有文件以提高性能：
- 多个文件同时被解析
- 索引在所有文件解析完成后才完整
- 解析过程中查询索引会得到不完整的结果

### 问题 3：性能开销

如果要在解析时使用索引，需要：
- 第一遍：解析所有文件，只提取方法签名和返回类型
- 第二遍：使用完整索引重新解析所有文件
- 这会使解析时间翻倍

## 未来改进方向

如果确实需要在解析时进行跨文件类型推断，可以考虑以下方案：

### 方案 A：两遍索引（完整但慢）

1. **第一遍**：快速解析，只提取方法签名和返回类型
2. **构建索引**：建立全局返回类型映射
3. **第二遍**：使用全局映射重新解析方法调用

**优点**：
- 完整的跨文件类型推断
- 不改变现有的解析器接口

**缺点**：
- 解析时间翻倍
- 内存占用增加

### 方案 B：增量更新（复杂但高效）

1. **初始解析**：正常解析，使用文件内类型推断
2. **索引完成后**：遍历所有方法调用，使用索引更新参数类型
3. **只更新跨文件调用**：文件内调用已经正确

**优点**：
- 不需要重新解析文件
- 只更新需要的部分

**缺点**：
- 实现复杂
- 需要存储额外信息（如 AST 节点位置）

### 方案 C：延迟推断（最灵活）

1. **解析时**：不推断跨文件类型，使用占位符（如 "Object"）
2. **使用时**：在影响分析时动态查询索引获取准确类型

**优点**：
- 解析速度快
- 灵活性高
- 当前已经部分实现

**缺点**：
- 类型信息不存储在索引中
- 需要在使用时进行推断

## 当前推荐做法

**使用方案 C（延迟推断）**，因为：

1. **已经实现**：索引已经存储了所有返回类型
2. **性能最优**：不需要重新解析文件
3. **足够灵活**：在需要时查询索引即可
4. **实际场景**：大多数影响分析不需要精确的参数类型

### 示例：在影响分析中使用

```rust
// 追踪方法调用链时，可以查询返回类型
fn trace_call_chain(index: &CodeIndex, method_sig: &str) {
    if let Some(method) = index.find_method(method_sig) {
        for call in &method.calls {
            println!("调用: {}", call.target);
            
            // 查询被调用方法的返回类型
            if let Some(return_type) = index.find_method_return_type(&call.target) {
                println!("  返回类型: {}", return_type);
            }
            
            // 递归追踪
            trace_call_chain(index, &call.target);
        }
    }
}
```

## 测试验证

### 文件内类型推断测试 ✅

```java
// 同一文件内
public class UserRepository {
    public User findUser(String id) { return null; }
}

public class TestService {
    private UserRepository repo;
    
    public void test() {
        // findUser() 返回 User，参数类型被正确推断为 User
        processor.process(repo.findUser("123"));
        // 方法调用目标：Processor::process(User)
    }
}
```

### 跨文件类型推断测试（通过索引查询）

```rust
// 文件 A
public class UserRepository {
    public User findUser(String id) { return null; }
}

// 文件 B
public class TestService {
    private UserRepository repo;
    
    public void test() {
        processor.process(repo.findUser("123"));
    }
}

// 使用索引查询
let return_type = index.find_method_return_type("com.example.UserRepository::findUser(String)");
assert_eq!(return_type, Some("User"));
```

## 总结

1. **当前实现已经支持跨文件类型查询**：通过 `find_method_return_type` API
2. **解析器专注于文件内类型推断**：保持高性能和简单性
3. **影响分析器可以使用索引**：在需要时查询跨文件类型信息
4. **这是一个合理的设计权衡**：在性能和功能之间取得平衡

如果未来确实需要在解析时进行跨文件类型推断，可以实现方案 A 或 B，但当前的方案 C 已经足够满足大多数使用场景。
