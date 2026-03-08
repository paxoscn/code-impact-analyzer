# Java 枚举类型识别总结

## 问题

给定Java枚举：
```java
enum Foo {
    BAR;
    String tac;
    int tic() { return 0; }
}
```

需要识别以下调用的类型：
1. `Foo.BAR` → `com.example.Foo`
2. `Foo.BAR.tac` → `String`
3. `Foo.BAR.tic()` → `int`
4. `Foo.BAR.getTac()` → `String`

## 当前状态

运行 `cargo run --example test_enum_type_inference` 发现：

### 问题1：枚举未被识别
- 枚举 `Foo` 没有出现在解析结果中
- 只识别了 `TestEnum` 类

### 问题2：方法调用类型错误
- `Foo.BAR.tic()` 被错误识别为 `com.example.TestEnum::tic()`
- 应该识别为 `com.example.Foo::tic()`

## AST 结构分析

通过 `cargo run --example debug_enum_ast` 发现枚举的AST结构：

```
enum_declaration [named]
├── enum [unnamed] "enum"
├── identifier [named] "Foo"
└── enum_body [named]
    ├── { [unnamed]
    ├── enum_constant [named] "BAR"
    │   └── identifier [named] "BAR"
    ├── enum_body_declarations [named]
    │   ├── ; [unnamed]
    │   ├── field_declaration [named] "String tac;"
    │   │   ├── type_identifier [named] "String"
    │   │   ├── variable_declarator [named] "tac"
    │   │   └── ; [unnamed]
    │   └── method_declaration [named] "int tic() {...}"
    │       ├── integral_type [named] "int"
    │       ├── identifier [named] "tic"
    │       ├── formal_parameters [named] "()"
    │       └── block [named] "{...}"
    └── } [unnamed]
```

## 实现方案

### 1. 识别枚举声明

在 `java_parser.rs` 的 `walk_node_for_classes_with_return_types` 中添加：

```rust
"enum_declaration" => {
    if let Some(class_info) = self.extract_enum_info_with_return_types(
        source, file_path, node, tree, app_config, return_type_map
    ) {
        classes.push(class_info);
    }
}
```

### 2. 提取枚举信息

创建新方法 `extract_enum_info_with_return_types`：

```rust
fn extract_enum_info_with_return_types(
    &self,
    source: &str,
    file_path: &Path,
    enum_node: tree_sitter::Node,
    tree: &tree_sitter::Tree,
    app_config: &ApplicationConfig,
    return_type_map: &mut MethodReturnTypeMap,
) -> Option<ClassInfo> {
    // 1. 提取枚举名称
    let enum_name = self.extract_class_name(source, &enum_node)?;
    
    // 2. 获取包名
    let package_name = self.extract_package_name(source, tree);
    let full_name = if let Some(pkg) = package_name {
        format!("{}.{}", pkg, enum_name)
    } else {
        enum_name.clone()
    };
    
    // 3. 提取枚举常量
    let enum_constants = self.extract_enum_constants(source, &enum_node);
    
    // 4. 提取字段和方法（从 enum_body_declarations）
    let mut methods = Vec::new();
    let mut fields = Vec::new();
    
    if let Some(body_node) = enum_node.child_by_field_name("body") {
        // 查找 enum_body_declarations
        for i in 0..body_node.child_count() {
            if let Some(child) = body_node.child(i) {
                if child.kind() == "enum_body_declarations" {
                    // 提取字段
                    fields = self.extract_class_fields(source, &child, tree);
                    
                    // 提取方法
                    methods = self.extract_methods_with_return_types(
                        source, file_path, &child, tree, &full_name, 
                        app_config, return_type_map
                    );
                }
            }
        }
    }
    
    // 5. 为字段生成 getter/setter
    let generated_getters = self.generate_getters_for_fields(&fields, &full_name, file_path);
    let generated_setters = self.generate_setters_for_fields(&fields, &full_name, file_path);
    methods.extend(generated_getters);
    methods.extend(generated_setters);
    
    // 6. 返回 ClassInfo
    Some(ClassInfo {
        name: full_name,
        methods,
        line_range: (enum_node.start_position().row + 1, enum_node.end_position().row + 1),
        is_interface: false,  // 枚举不是接口
        implements: vec![],
    })
}
```

### 3. 提取枚举常量

```rust
fn extract_enum_constants(
    &self,
    source: &str,
    enum_node: &tree_sitter::Node,
) -> Vec<String> {
    let mut constants = Vec::new();
    
    if let Some(body_node) = enum_node.child_by_field_name("body") {
        for i in 0..body_node.child_count() {
            if let Some(child) = body_node.child(i) {
                if child.kind() == "enum_constant" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = &source[name_node.byte_range()];
                        constants.push(name.to_string());
                    }
                }
            }
        }
    }
    
    constants
}
```

### 4. 枚举常量类型推断

在类型推断时，需要识别 `EnumType.CONSTANT` 模式：

```rust
// 在 infer_argument_type_with_return_types 中添加
if let Some(dot_pos) = arg_text.rfind('.') {
    let potential_type = &arg_text[..dot_pos];
    let potential_constant = &arg_text[dot_pos + 1..];
    
    // 检查是否是枚举常量（全大写或首字母大写）
    if potential_constant.chars().next().map_or(false, |c| c.is_uppercase()) {
        // 尝试解析为完整类名
        if let Some(full_type) = self.resolve_full_class_name(
            potential_type, &import_map, &package_name, global_index
        ) {
            // 检查是否是枚举类型
            if is_enum_type(&full_type, global_index) {
                return Some(full_type);
            }
        }
    }
}
```

### 5. 修复方法调用解析

在 `walk_node_for_calls_with_return_types` 中，对于 `object.method()` 形式：

```rust
if let Some(object_node) = method_invocation_node.child_by_field_name("object") {
    let object_text = &source[object_node.byte_range()];
    
    // 推断对象类型
    let caller_type = self.infer_argument_type_with_return_types(
        object_text, &import_map, &package_name, &field_types, 
        &param_types, &local_var_types, &lambda_param_types, 
        return_type_map, global_index
    );
    
    if let Some(ref caller_type_str) = caller_type {
        // 使用推断出的类型构建完整方法名
        let full_method_name = format!("{}::{}", caller_type_str, method_name);
        // ...
    }
}
```

## 关键点

1. **枚举是类的特殊形式**：在Java中，枚举本质上是类，每个枚举常量都是该枚举类型的实例

2. **枚举常量的类型**：`Foo.BAR` 的类型就是 `Foo`

3. **字段和方法**：枚举可以有字段和方法，就像普通类一样

4. **自动生成getter/setter**：对于枚举字段，应该自动生成getter/setter方法

5. **AST节点类型**：
   - `enum_declaration` - 枚举声明
   - `enum_constant` - 枚举常量
   - `enum_body_declarations` - 枚举体声明（字段和方法）

## 测试验证

运行以下命令验证实现：

```bash
# 查看AST结构
cargo run --example debug_enum_ast

# 测试枚举类型推断
cargo run --example test_enum_type_inference
```

## 预期结果

实现后，`test_enum_type_inference` 应该输出：

```
发现的类:
  - com.example.Foo (接口: false)
    方法:
      - tic() [返回类型: int]
      - getTac() [返回类型: String]
      - setTac(String) [返回类型: void]

  - com.example.TestEnum (接口: false)
    方法:
      - testEnumAccess() [返回类型: void]
        调用:
          - com.example.Foo::tic() (行: 23)
          - com.example.Foo::getTac() (行: 26)
```

## 相关文件

- `code-impact-analyzer/src/java_parser.rs` - 主要实现
- `code-impact-analyzer/examples/test_enum_type_inference.rs` - 测试用例
- `code-impact-analyzer/examples/debug_enum_ast.rs` - AST调试工具
- `code-impact-analyzer/ENUM_TYPE_INFERENCE.md` - 详细设计文档
