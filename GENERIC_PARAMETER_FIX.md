# 泛型参数类型推断修复

## 问题描述

在方法调用识别中，当参数是带泛型的类型时（如 `Map<String, Object>`），方法调用的签名没有正确去除泛型信息，导致无法正确匹配方法定义。

### 示例代码
```java
class Foo {
    void bar() {
        Map<String, Object> map = null;
        tac(map);  // 期望识别为 Foo::tac(Map)
    }
    
    void tac(Map<String, Object> map) {
        // 方法签名应该是 Foo::tac(Map)
    }
}
```

### 问题表现
- 方法定义签名：`Foo::tac(Map)` ✓ 正确
- 方法调用识别：`Foo::tac(Map<String, Object>)` ✗ 错误（包含泛型）

这导致方法调用无法正确匹配到方法定义。

## 根本原因

在 `java_parser.rs` 的 `infer_argument_type_with_return_types` 方法中，当处理 `identifier` 类型的参数（变量名）时：

```rust
"identifier" => {
    if let Some(var_name) = source.get(arg_node.byte_range()) {
        // 从 field_types 中查找变量类型，并进行自动装箱
        field_types.get(var_name).map(|t| autobox_type(t))
    } else {
        None
    }
}
```

这里直接使用了 `field_types` 中存储的类型，但该类型可能包含泛型信息（如 `Map<String, Object>`），没有调用 `remove_generics` 函数去除泛型。

## 解决方案

在返回类型之前，先调用 `remove_generics` 函数去除泛型信息：

```rust
"identifier" => {
    if let Some(var_name) = source.get(arg_node.byte_range()) {
        // 从 field_types 中查找变量类型，去除泛型，并进行自动装箱
        field_types.get(var_name).map(|t| {
            let type_without_generics = remove_generics(t);
            autobox_type(&type_without_generics)
        })
    } else {
        None
    }
}
```

## 修改文件

- `code-impact-analyzer/src/java_parser.rs` (第 2247-2255 行)

## 测试验证

创建了测试文件 `code-impact-analyzer/examples/test_generic_parameter.rs` 验证以下场景：

### 1. 单个泛型参数
```java
Map<String, Object> map = null;
tac(map);  // 识别为 Foo::tac(Map) ✓
```

### 2. 多个泛型参数
```java
List<String> list = null;
HashMap<Integer, List<String>> complexMap = null;
process(list, complexMap);  // 识别为 Foo::process(List,HashMap) ✓
```

### 3. 嵌套泛型
```java
Map<String, List<Map<Integer, String>>> nested = null;
handleNested(nested);  // 识别为 Foo::handleNested(Map) ✓
```

## 测试结果

```
=== 验证 bar 方法 ===
✓ 成功: 方法调用被正确识别为 Foo::tac(Map)

=== 验证 tac 方法签名 ===
✓ 成功: 方法签名正确去掉了泛型信息

=== 验证多个泛型参数 ===
✓ 成功: 多个泛型参数被正确去除

=== 验证嵌套泛型 ===
✓ 成功: 嵌套泛型被正确去除
```

所有现有测试也继续通过：
```
test result: ok. 33 passed; 0 failed; 0 ignored
```

## 影响范围

此修复确保了在方法调用识别时，所有通过变量传递的参数类型都会正确去除泛型信息，使得：

1. 方法调用签名与方法定义签名一致
2. 影响追踪能够正确建立调用关系
3. 代码分析结果更加准确

## 运行测试

```bash
cd code-impact-analyzer
cargo run --example test_generic_parameter
```
