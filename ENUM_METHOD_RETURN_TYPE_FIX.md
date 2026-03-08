# 枚举方法返回值作为参数的类型推断修复

## 问题描述

当枚举方法的返回值作为参数传递给另一个方法时，参数类型推断不正确。

### 示例代码

```java
enum Status {
    ACTIVE;
    String description;
    String getDescription() { return description; }
}

class Service {
    void processStatus() {
        go(Status.ACTIVE.getDescription());  // getDescription() 返回 String
    }
    void go(int i) {}
}
```

### 问题

- 预期：识别为 `Service::go(String)`
- 实际：识别为 `Service::go(Object)`

## 根本原因

`infer_method_return_type_with_map` 方法在推断方法返回类型时，没有处理 `field_access` 节点作为对象的情况。

当遇到 `Status.ACTIVE.getDescription()` 时：
1. AST 结构是：`method_invocation` → `field_access`（Status.ACTIVE）+ `identifier`（getDescription）
2. 但代码只处理了 `identifier` 和 `method_invocation` 作为对象的情况
3. 没有处理 `field_access` 作为对象的情况

## 解决方案

在 `infer_method_return_type_with_map` 方法中添加对 `field_access` 的处理。

### 代码修改

**文件**: `code-impact-analyzer/src/java_parser.rs`

**方法**: `infer_method_return_type_with_map` (Line 2291)

```rust
fn infer_method_return_type_with_map(
    &self,
    source: &str,
    method_invocation_node: &tree_sitter::Node,
    field_types: &std::collections::HashMap<String, String>,
    import_map: &std::collections::HashMap<String, String>,
    method_return_types: &MethodReturnTypeMap,
    package_name: &Option<String>,
) -> Option<String> {
    // 提取方法名和对象类型
    let mut cursor = method_invocation_node.walk();
    let mut identifiers = Vec::new();
    let mut argument_list_node = None;
    let mut object_method_invocation = None;
    let mut field_access_object = None;  // 新增
    
    for child in method_invocation_node.children(&mut cursor) {
        if child.kind() == "identifier" {
            if let Some(text) = source.get(child.byte_range()) {
                identifiers.push(text.to_string());
            }
        } else if child.kind() == "method_invocation" {
            object_method_invocation = Some(child);
        } else if child.kind() == "field_access" {  // 新增
            field_access_object = Some(child);
        } else if child.kind() == "argument_list" {
            argument_list_node = Some(child);
        }
    }
    
    // ... 提取参数类型 ...
    
    // 新增：处理 field_access 作为对象的情况
    if let Some(field_access_node) = field_access_object {
        if !identifiers.is_empty() {
            let method_name = &identifiers[identifiers.len() - 1];
            let field_access_text = &source[field_access_node.byte_range()];
            
            // 推断 field_access 的类型（如 Status.ACTIVE → Status）
            if let Some(object_type) = self.infer_field_access_type(
                field_access_text,
                import_map,
                package_name,
            ) {
                // 构建方法签名
                let method_signature = if arg_types.is_empty() {
                    format!("{}::{}()", object_type, method_name)
                } else {
                    format!("{}::{}({})", object_type, method_name, arg_types.join(","))
                };
                
                // 从映射中查找返回类型
                if let Some(return_type) = method_return_types.get(&method_signature) {
                    return Some(return_type.clone());
                }
            }
        }
    }
    
    // ... 其他处理逻辑 ...
}
```

## 测试验证

### 测试用例

```java
enum Status {
    ACTIVE;
    String name;
    int code;
    
    String getName() { return name; }
    int getCode() { return code; }
}

class Service {
    void test() {
        processString(Status.ACTIVE.getName());  // String
        processInt(Status.ACTIVE.getCode());     // int
    }
    
    void processString(String s) {}
    void processInt(int i) {}
}
```

### 测试结果

```
✓ com.example.Service::processString(String) - 枚举方法返回String作为参数
✓ com.example.Service::processInt(int) - 枚举方法返回int作为参数
✓ com.example.Status::getName() - 调用枚举方法
✓ com.example.Status::getCode() - 调用枚举方法
```

所有测试通过！

## 运行测试

```bash
# 测试枚举方法返回值作为参数
cargo run --example test_enum_method_return_as_param

# 完整枚举功能测试
cargo run --example test_enum_complete
```

## 相关功能

这个修复依赖于之前实现的功能：

1. **枚举识别** - 识别 `enum_declaration` 节点
2. **枚举方法提取** - 从 `enum_body_declarations` 提取方法
3. **字段访问类型推断** - `infer_field_access_type` 方法推断 `Foo.BAR` 的类型为 `Foo`
4. **方法返回类型映射** - `MethodReturnTypeMap` 存储方法签名到返回类型的映射

## 影响范围

这个修复不仅适用于枚举，也适用于所有使用 `field_access` 作为对象的方法调用，例如：

```java
class Outer {
    static Inner inner = new Inner();
    static class Inner {
        String getValue() { return "value"; }
    }
}

// 现在可以正确推断
process(Outer.inner.getValue());  // 识别为 process(String)
```

## 总结

通过在 `infer_method_return_type_with_map` 中添加对 `field_access` 节点的处理，现在可以正确推断枚举方法返回值作为参数时的类型，从而准确识别方法调用。
