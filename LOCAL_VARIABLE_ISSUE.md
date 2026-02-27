# Java 本地变量调用解析问题

## 问题描述

当前的 Java 解析器**不能正确解析方法内本地变量的方法调用**。

### 示例代码

```java
public class Example {
    public void go() {
        Foo foo = new Foo();  // 本地变量声明
        foo.bar();            // 本地变量的方法调用
    }
}
```

### 当前行为

- 只记录调用了 `bar`
- 无法解析为完整的 `Foo::bar` 或 `com.example.Foo::bar`

### 期望行为

- 应该记录调用了 `Foo::bar` 或 `com.example.Foo::bar`（取决于是否有导入信息）

## ✅ 问题已修复

**修复日期**: 2026-02-27

### 修复内容

修改了 `src/java_parser.rs` 中的 `extract_field_types` 方法，使其能够提取方法内的本地变量类型：

1. **新增 `extract_local_variable_types` 方法**
   - 提取方法体内的本地变量声明

2. **新增 `walk_node_for_local_vars` 方法**
   - 递归遍历方法体，查找 `local_variable_declaration` 节点

3. **扩展 `extract_field_types` 方法**
   - 同时提取类字段和方法内的本地变量

### 修复后的行为

运行测试示例：

```bash
cd code-impact-analyzer
cargo run --example test_local_variable
```

输出结果：
```
=== 解析结果 ===
类名: com.example.TestLocalVariable
  方法: go
  完整名称: com.example.TestLocalVariable::go
  方法调用:
    - Foo::bar (行 7)  ← ✅ 正确解析为 Foo::bar

=== 验证结果 ===
✓ 成功检测到 bar() 方法调用
✓ 成功解析为完整的类名::方法名格式
```

### 测试覆盖

新增了以下单元测试：

1. **test_extract_local_variable_method_calls**
   - 测试简单本地变量的方法调用解析

2. **test_extract_local_variable_with_imports**
   - 测试使用导入类的本地变量调用解析

3. **test_extract_mixed_field_and_local_variable_calls**
   - 测试混合使用类字段和本地变量的场景

所有测试均通过：
```bash
cargo test --lib java_parser::tests
# test result: ok. 18 passed; 0 failed
```

### 支持的场景

修复后支持以下所有场景：

1. ✅ **简单本地变量**
   ```java
   Foo foo = new Foo();
   foo.bar();  // 解析为 Foo::bar
   ```

2. ✅ **导入的类的本地变量**
   ```java
   import com.example.Service;
   Service service = new Service();
   service.doWork();  // 解析为 com.example.Service::doWork
   ```

3. ✅ **类字段调用**（原本就支持）
   ```java
   private Service service;
   service.doWork();  // 解析为 com.example.Service::doWork
   ```

4. ✅ **多个本地变量**
   ```java
   Bar bar1 = new Bar();
   Bar bar2 = new Bar();
   bar1.doSomething();     // 解析为 Bar::doSomething
   bar2.doSomethingElse(); // 解析为 Bar::doSomethingElse
   ```

5. ✅ **链式调用**
   ```java
   foo.getBar().doSomething();
   // 解析为 Foo::getBar 和 doSomething
   ```

## 根本原因（已解决）

在 `src/java_parser.rs` 中：

1. **`extract_field_types` 方法**（第630-657行）
   - ~~只提取类级别的字段声明（`private Foo foo;`）~~
   - ~~不提取方法内的本地变量声明（`Foo foo = new Foo();`）~~
   - ✅ 现在同时提取类字段和本地变量

2. **`walk_node_for_calls` 方法**（第698-785行）
   - 在第760行尝试从 `field_types` 查找对象类型
   - ~~对于本地变量，查找失败，只能记录方法名~~
   - ✅ 现在可以找到本地变量的类型

## Tree-sitter 节点结构

Java 本地变量声明的 AST 结构：

```
local_variable_declaration
  ├── type_identifier: "Foo"
  └── variable_declarator
      ├── identifier: "foo"
      └── object_creation_expression
          └── ...
```

## 影响范围

这个修复改善了所有依赖方法调用解析的功能：

1. ✅ **影响追踪**：现在可以正确追踪通过本地变量调用的方法
2. ✅ **调用图生成**：调用图中包含本地变量的方法调用边
3. ✅ **代码分析**：可以准确分析方法间的依赖关系

## 相关代码

- `src/java_parser.rs`:
  - `extract_field_types` (第630-680行) - ✅ 已修复
  - `extract_local_variable_types` (第682-690行) - ✅ 新增
  - `walk_node_for_local_vars` (第692-705行) - ✅ 新增
  - `walk_node_for_calls` (第740-827行) - 使用修复后的类型映射

## 测试用例

- `examples/test_local_variable.rs` - 基本验证
- `examples/test_local_variable_advanced.rs` - 高级场景验证
- `src/java_parser.rs` - 单元测试（3个新增测试）
