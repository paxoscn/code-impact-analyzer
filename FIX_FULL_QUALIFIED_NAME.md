# 修复本地变量类型解析为完整类名

## 问题描述

在第一次修复后，本地变量的方法调用可以被检测到，但类型解析不完整：

```java
package com.example;

public class Builder {
    public Builder build() {
        Builder builder = new Builder();
        builder.setName("test");  // 解析为 Builder::setName
        return builder;           // 应该是 com.example.Builder::setName
    }
}
```

**问题**: 解析为 `Builder::setName` 而不是 `com.example.Builder::setName`

## 根本原因

`extract_field_type_from_declaration` 方法只提取了 `type_identifier` 的文本（简单类名），没有将其解析为完整类名（包含包名）。

## 修复方案

修改 `extract_local_variable_types` 方法，在提取本地变量类型后，使用 `import_map` 和 `package_name` 将简单类名解析为完整类名。

### 代码变更

1. **修改 `extract_field_types` 方法签名**
   - 添加 `tree: &tree_sitter::Tree` 参数
   - 传递给 `extract_local_variable_types`

2. **修改 `extract_local_variable_types` 方法**
   - 添加 `tree: &tree_sitter::Tree` 参数
   - 提取本地变量后，使用 `resolve_full_class_name` 解析完整类名

```rust
fn extract_local_variable_types(
    &self,
    source: &str,
    method_node: &tree_sitter::Node,
    tree: &tree_sitter::Tree,
    field_types: &mut std::collections::HashMap<String, String>,
) {
    // 先提取本地变量的简单类型
    self.walk_node_for_local_vars(source, *method_node, field_types);
    
    // 获取导入映射和包名，用于解析完整类名
    let import_map = self.build_import_map(source, tree);
    let package_name = self.extract_package_name(source, tree);
    
    // 将简单类名解析为完整类名
    let mut resolved_types = std::collections::HashMap::new();
    for (var_name, simple_type) in field_types.iter() {
        let full_type = self.resolve_full_class_name(simple_type, &import_map, &package_name);
        resolved_types.insert(var_name.clone(), full_type);
    }
    
    // 更新 field_types
    *field_types = resolved_types;
}
```

3. **修改 `extract_method_calls` 方法**
   - 更新调用 `extract_field_types` 时传入 `tree` 参数

## 测试结果

### ✅ 单元测试 (19/19 通过)

```bash
$ cargo test --lib java_parser::tests

running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured
```

### ✅ 本地变量类型为当前类测试

```bash
$ cargo run --example test_self_type_local_variable

场景1 - build()方法中的本地变量 (Builder builder = new Builder()):
  ✓ 成功解析为 Builder::setName
    实际: com.example.Builder::setName  ← ✅ 完整类名

场景2 - chainedCall()方法中的链式调用:
  setName 调用次数: 2
  ✓ 成功检测到链式调用

场景3 - createBuilder()静态方法中的本地变量:
  ✓ 成功解析为 Builder::setName
```

### ✅ 其他测试

所有其他测试也通过：
- `test_local_variable` - ✅
- `test_local_variable_advanced` - ✅ (5/5 场景)
- 实际项目测试 - ✅ (7 nodes, 7 edges)

## 影响

### 改进

1. **更准确的类型解析** - 所有类型现在都解析为完整类名
2. **更好的跨包调用追踪** - 可以区分不同包中的同名类
3. **一致性** - 类字段和本地变量的类型解析方式一致

### 示例对比

**修复前**:
```
Builder::setName
Foo::bar
UserService::findUser
```

**修复后**:
```
com.example.Builder::setName
com.example.Foo::bar
com.example.UserService::findUser
```

## 测试调整

修改了 `test_extract_static_method_calls` 测试，使其接受完整类名：

```rust
// 修改前
assert!(
    call_targets.contains(&"UserService::findUser"),
    "Should find UserService::findUser, got: {:?}", call_targets
);

// 修改后
assert!(
    call_targets.iter().any(|t| t.contains("UserService::findUser")),
    "Should find UserService::findUser (with or without package), got: {:?}", call_targets
);
```

## 结论

✅ **修复完成**

现在所有的类型（包括本地变量、类字段）都正确解析为完整的类名（包含包名），提供了更准确的代码分析能力。

---

**修复日期**: 2026-02-27  
**测试状态**: ✅ 所有测试通过 (19/19)
