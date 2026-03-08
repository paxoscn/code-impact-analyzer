# Java 枚举类型推断

## 问题描述

给定Java枚举定义：
```java
enum Foo {
    BAR;
    String tac;
    int tic() {
        return 0;
    }
}
```

需要正确识别以下调用的类型：

1. `Foo.BAR` → 类型应为 `com.example.Foo`
2. `Foo.BAR.tac` → 类型应为 `String`
3. `Foo.BAR.tic()` → 返回类型应为 `int`
4. `Foo.BAR.getTac()` → 返回类型应为 `String`（自动生成的getter）

## 当前问题

运行测试 `test_enum_type_inference` 发现：

1. **枚举未被识别为类**：`Foo` 枚举没有出现在解析结果中
2. **方法调用类型错误**：`Foo.BAR.tic()` 被错误识别为 `com.example.TestEnum::tic()`

## 实现要点

### 1. 识别枚举定义

需要在 `java_parser.rs` 中识别 `enum_declaration` 节点：

```rust
// 在 walk_node_for_classes 中添加
"enum_declaration" => {
    if let Some(class_info) = self.extract_enum_info(source, file_path, node, tree, app_config) {
        classes.push(class_info);
    }
}
```

### 2. 枚举常量类型推断

枚举常量（如 `BAR`）的类型就是枚举本身：

```java
Foo bar = Foo.BAR;  // BAR 的类型是 Foo
```

实现策略：
- 在索引中记录枚举常量及其所属枚举类型
- 在类型推断时，识别 `EnumType.CONSTANT` 模式
- 返回枚举类型作为常量的类型

### 3. 枚举字段访问

枚举可以有字段：

```java
enum Foo {
    BAR;
    String tac;  // 字段
}

String value = Foo.BAR.tac;  // 访问枚举实例的字段
```

实现策略：
- 解析枚举中的字段声明
- 在类型推断时，识别 `EnumConstant.field` 模式
- 返回字段的声明类型

### 4. 枚举方法调用

枚举可以有方法：

```java
enum Foo {
    BAR;
    int tic() { return 0; }
}

int result = Foo.BAR.tic();  // 调用枚举实例的方法
```

实现策略：
- 解析枚举中的方法声明
- 在方法调用解析时，识别 `EnumConstant.method()` 模式
- 正确解析为 `EnumType::method()` 而不是当前类的方法

### 5. 自动生成的Getter/Setter

对于枚举字段，应该自动生成getter/setter：

```java
enum Foo {
    BAR;
    String tac;
}

// 自动生成：
// String getTac() { return tac; }
// void setTac(String tac) { this.tac = tac; }
```

## 实现步骤

### 步骤1：识别枚举声明

在 `extract_class_info` 或创建新的 `extract_enum_info` 方法：

```rust
fn extract_enum_info(
    &self,
    source: &str,
    file_path: &Path,
    enum_node: tree_sitter::Node,
    tree: &tree_sitter::Tree,
    app_config: &ApplicationConfig,
) -> Option<ClassInfo> {
    // 1. 提取枚举名称
    // 2. 提取枚举常量
    // 3. 提取枚举字段
    // 4. 提取枚举方法
    // 5. 生成getter/setter
}
```

### 步骤2：解析枚举常量

```rust
fn extract_enum_constants(
    &self,
    source: &str,
    enum_node: &tree_sitter::Node,
) -> Vec<String> {
    // 遍历 enum_constant 节点
    // 返回常量名称列表
}
```

### 步骤3：增强类型推断

在 `infer_argument_type_with_return_types` 中添加枚举常量识别：

```rust
// 识别 EnumType.CONSTANT 模式
if let Some(dot_pos) = arg_text.rfind('.') {
    let potential_enum = &arg_text[..dot_pos];
    let potential_constant = &arg_text[dot_pos + 1..];
    
    // 检查是否是枚举常量
    if is_enum_constant(potential_enum, potential_constant, global_index) {
        return Some(resolve_full_class_name(potential_enum, ...));
    }
}
```

### 步骤4：修复方法调用解析

在 `walk_node_for_calls_with_return_types` 中：

```rust
// 对于 object.method() 形式
if let Some(object_node) = method_invocation_node.child_by_field_name("object") {
    let object_text = &source[object_node.byte_range()];
    
    // 检查是否是枚举常量访问
    if is_enum_constant_access(object_text) {
        // 解析为枚举类型的方法
        let enum_type = infer_enum_constant_type(object_text);
        // ...
    }
}
```

## Tree-sitter 节点结构

Java枚举的AST结构：

```
enum_declaration
├── modifiers (optional)
├── name: identifier
├── enum_body
    ├── enum_constant
    │   └── identifier (e.g., "BAR")
    ├── field_declaration
    │   ├── type
    │   └── variable_declarator
    └── method_declaration
        ├── return_type
        ├── name
        └── body
```

## 测试用例

```java
package com.example;

enum Foo {
    BAR, BAZ;
    
    String tac;
    int value;
    
    int tic() {
        return 0;
    }
    
    String getTac() {
        return tac;
    }
}

class TestEnum {
    void test() {
        Foo bar = Foo.BAR;              // 类型: Foo
        String t = Foo.BAR.tac;         // 类型: String
        int v = Foo.BAR.tic();          // 返回: int
        String g = Foo.BAR.getTac();    // 返回: String
        
        // 链式调用
        int len = Foo.BAR.getTac().length();  // String.length()
    }
}
```

## 预期输出

```
发现的类:
  - com.example.Foo (枚举)
    常量: [BAR, BAZ]
    字段:
      - tac: String
      - value: int
    方法:
      - tic() -> int
      - getTac() -> String
      - setTac(String) -> void
      - getValue() -> int
      - setValue(int) -> void

  - com.example.TestEnum (类)
    方法:
      - test()
        调用:
          - com.example.Foo::tic() (行: 23)
          - com.example.Foo::getTac() (行: 24)
          - java.lang.String::length() (行: 27)
```

## 相关文件

- `src/java_parser.rs` - 主要实现文件
- `src/language_parser.rs` - ClassInfo 结构定义
- `examples/test_enum_type_inference.rs` - 测试用例

## 参考

- Java枚举本质上是类的特殊形式
- 每个枚举常量都是该枚举类型的实例
- 枚举可以有字段、方法、构造函数
- 枚举自动继承 `java.lang.Enum`
