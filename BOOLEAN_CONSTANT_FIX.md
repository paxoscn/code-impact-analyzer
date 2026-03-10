# Boolean 常量类型识别修复

## 问题描述

当 Java 代码中使用 `Boolean.TRUE` 或 `Boolean.FALSE` 作为方法参数时，参数类型被错误地识别为 `Object`，而不是 `Boolean`。

示例代码：
```java
class Foo {
    void bar() {
        tac(Boolean.TRUE);  // 参数类型应该是 Boolean，但被识别为 Object
    }
    
    void tac(Boolean bool) {
    }
}
```

## 根本原因

在 `java_parser.rs` 的 `infer_argument_type_with_return_types` 函数中，对于 `field_access` 类型的参数（如 `Boolean.TRUE`），代码简单地返回 `"Object"`，没有进行类型推断。

原代码（第 2259-2264 行）：
```rust
// 字段访问：obj.field
"field_access" => {
    // 尝试推断字段访问的类型
    // 这里简化处理，返回 Object
    Some("Object".to_string())
}
```

## 解决方案

修改 `infer_argument_type_with_return_types` 函数，使用已有的 `infer_field_access_type` 方法来正确推断字段访问的类型。

修改后的代码：
```rust
// 字段访问：obj.field
"field_access" => {
    // 尝试推断字段访问的类型
    if let Some(field_access_text) = source.get(arg_node.byte_range()) {
        // 使用 infer_field_access_type 推断类型
        self.infer_field_access_type(field_access_text, import_map, package_name)
            .or(Some("Object".to_string()))
    } else {
        Some("Object".to_string())
    }
}
```

## 工作原理

`infer_field_access_type` 方法会：
1. 解析字段访问表达式（如 `Boolean.TRUE`）
2. 提取类型名（`Boolean`）和字段名（`TRUE`）
3. 检查字段名是否首字母大写（静态常量的常见命名）
4. 返回类型名作为字段的类型

这样，`Boolean.TRUE` 的类型就被正确识别为 `Boolean`。

## 测试验证

创建了两个测试文件来验证修复：

### 1. test_boolean_constant.rs
测试 `Boolean.TRUE` 和 `Boolean.FALSE` 的类型识别。

测试结果：
```
✓ SUCCESS: Boolean.TRUE is correctly recognized as type 'Boolean'
  Method call target: Foo::tac(Boolean)

✓ SUCCESS: Boolean.FALSE is correctly recognized as type 'Boolean'
  Method call target: Foo::tac(Boolean)
```

### 2. test_static_constants.rs
测试多种静态常量的类型识别，包括：
- `Boolean.TRUE` / `Boolean.FALSE`
- `Integer.MAX_VALUE` / `Integer.MIN_VALUE`
- `Long.MAX_VALUE`
- `Double.NaN` / `Double.POSITIVE_INFINITY`

测试结果：
```
Summary:
  Total: 7
  Success: 7
  Failed: 0

✓ All static constants are correctly recognized!
```

## 影响范围

此修复适用于所有使用静态字段访问作为方法参数的场景，包括但不限于：
- 包装类的常量：`Boolean.TRUE`, `Integer.MAX_VALUE`, `Long.MIN_VALUE` 等
- 枚举常量：`Status.ACTIVE`, `Color.RED` 等
- 自定义静态常量：`MyClass.CONSTANT_VALUE` 等

## 文件修改

- `code-impact-analyzer/src/java_parser.rs`：修改 `infer_argument_type_with_return_types` 函数
- `code-impact-analyzer/examples/test_boolean_constant.rs`：新增测试文件
- `code-impact-analyzer/examples/test_static_constants.rs`：新增测试文件
