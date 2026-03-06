# 返回类型完整包名修复

## 问题描述

方法调用中的参数类型没有包含完整包名。例如：
- 期望：`process(com.user.User)`
- 实际：`process(User)`

## 根本原因

1. `extract_return_type` 函数只提取简单类型名称，没有解析为完整包名
2. 自动生成的 getter 方法的返回类型也是简单名称

## 解决方案

### 1. 修改 `extract_return_type` 函数

添加 `tree` 参数，使其能够解析完整包名：

```rust
fn extract_return_type(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Option<String>
```

**关键改动：**
- 提取简单类型后，使用 `resolve_full_class_name` 解析为完整包名
- 支持 `scoped_type_identifier` 节点（完整包名类型）
- 移除泛型信息
- 对基本类型和常用类型保持简单名称

### 2. 修改 `generate_getters_for_fields` 函数

添加 `tree` 参数，解析字段类型为完整包名：

```rust
fn generate_getters_for_fields(
    &self,
    source: &str,
    file_path: &Path,
    class_node: &tree_sitter::Node,
    class_name: &str,
    methods: &mut Vec<MethodInfo>,
    method_return_types: &mut MethodReturnTypeMap,
    tree: &tree_sitter::Tree,
)
```

**关键改动：**
- 获取导入映射和包名
- 对每个字段类型使用 `resolve_full_class_name` 解析完整包名
- 将完整包名存储到返回类型映射中

### 3. 更新所有调用点

更新以下函数中对 `extract_return_type` 的调用：
- `extract_method_info` - 添加 `tree` 参数
- `extract_method_info_with_return_types` - 添加 `tree` 参数
- `extract_methods_with_return_types` - 添加 `tree` 参数

更新对 `generate_getters_for_fields` 的调用：
- `extract_methods_with_return_types` - 添加 `tree` 参数

## 测试结果

### 所有单元测试通过
```
running 129 tests
test result: ok. 129 passed; 0 failed; 0 ignored
```

### 验证测试

**测试1: 导入类型**
```java
import com.user.User;

public class UserRepository {
    public User getUser() { return null; }
}

processor.process(userRepository.getUser());
```
结果：`process(com.user.User)` ✅

**测试2: 同包类型**
```java
package com.example;

public class UserRepository {
    public User findUser(String id) { return null; }
}

processor.process(userRepository.findUser("123"));
```
结果：`process(com.example.User)` ✅

**测试3: 自动生成的 getter**
```java
package com.example;
import com.user.User;

public class UserRepository {
    private User user;  // 自动生成 getUser()
}

processor.process(userRepository.getUser());
```
结果：`process(com.user.User)` ✅

## 影响范围

### 修改的文件
- `src/java_parser.rs`

### 修改的函数
1. `extract_return_type` - 添加 tree 参数，解析完整包名
2. `generate_getters_for_fields` - 添加 tree 参数，解析字段类型完整包名
3. `extract_method_info` - 更新调用
4. `extract_method_info_with_return_types` - 更新调用
5. `extract_methods_with_return_types` - 更新调用

### 更新的测试
1. `test_extract_method_calls_with_nested_calls` - 期望值改为完整包名
2. `test_extract_method_calls_with_nested_calls_with_getter` - 期望值改为完整包名
3. `test_extract_method_calls_with_nested_calls_this_with_getter` - 期望值改为完整包名

## 类型解析规则

### 基本类型和常用类型
保持简单名称：
- 基本类型：`int`, `long`, `boolean`, 等
- java.lang 包：`String`, `Object`, `Integer`, 等
- 常用集合：`List`, `ArrayList`, `HashMap`, 等

### 自定义类型
按优先级解析：
1. **已包含包名**：直接使用（`com.example.User`）
2. **导入映射**：从 import 语句查找
3. **同包类型**：添加当前包名前缀
4. **无法解析**：返回简单类名

## 总结

✅ 方法返回类型现在包含完整包名  
✅ 自动生成的 getter 返回类型包含完整包名  
✅ 方法调用参数类型推断使用完整包名  
✅ 所有测试通过（129/129）  
✅ 与方法声明参数类型规则一致  

现在方法调用中的参数类型和方法声明中的参数类型都使用完整包名，实现了完全一致！
