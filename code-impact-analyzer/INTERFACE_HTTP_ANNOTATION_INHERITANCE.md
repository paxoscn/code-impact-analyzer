# 接口HTTP注解继承功能

## 功能概述

当Java类实现了接口时，如果接口方法有HTTP注解（如`@GetMapping`、`@PostMapping`等），这些注解会自动传播到实现类的同名方法上。

## 使用场景

在Spring MVC中，常见的做法是在接口中定义HTTP端点，然后在实现类中提供具体实现：

```java
@RequestMapping("bar")
public interface Bar {
    @PostMapping("/tac")
    String tac();
}

public class FooController implements Bar {
    public String tac() {
        return "ok";
    }
}
```

在这个例子中，`FooController.tac()`方法虽然没有显式的HTTP注解，但它实际上继承了接口`Bar.tac()`的`@PostMapping("/tac")`注解。

## 实现原理

### 1. 两阶段解析

系统使用两阶段解析来构建完整的代码索引：

- **第一阶段**：快速解析所有文件，提取类、接口、方法签名和返回类型
- **第二阶段**：使用全局类型信息重新解析，提取方法调用和其他详细信息

### 2. 接口关系索引

在解析过程中，系统会记录：
- `class_interfaces`: 每个类实现的接口列表
- `interface_implementations`: 每个接口的实现类列表

### 3. HTTP注解传播

在两阶段解析完成后，系统调用`propagate_interface_http_annotations()`方法：

```rust
pub fn propagate_interface_http_annotations(&mut self) {
    // 遍历所有实现了接口的类
    for (class_name, interfaces) in &self.class_interfaces {
        // 遍历该类的所有接口
        for interface_name in interfaces {
            // 查找接口的所有方法
            for interface_method in interface_methods {
                // 如果接口方法有HTTP注解
                if let Some(http_annotation) = &interface_method.http_annotations {
                    // 构建实现类的对应方法名
                    let impl_method_name = format!("{}{}", class_name, method_signature);
                    
                    // 如果实现类的方法没有HTTP注解，则从接口继承
                    if impl_method.http_annotations.is_none() {
                        impl_method.http_annotations = Some(http_annotation.clone());
                    }
                }
            }
        }
    }
}
```

### 4. 注解覆盖规则

- 如果实现类的方法已经有HTTP注解，则不会被接口的注解覆盖
- 只有当实现类方法没有HTTP注解时，才会从接口继承

## 测试用例

### 基本继承测试

```rust
#[test]
fn test_interface_http_annotation_inheritance() {
    // 接口定义HTTP注解
    // 实现类没有HTTP注解
    // 验证实现类方法继承了接口的HTTP注解
}
```

### 带参数方法测试

```rust
#[test]
fn test_interface_http_annotation_with_parameters() {
    // 测试带参数的方法也能正确继承HTTP注解
    // 验证方法签名匹配（包括参数类型）
}
```

### 不覆盖现有注解测试

```rust
#[test]
fn test_interface_http_annotation_not_override_existing() {
    // 实现类方法已有HTTP注解
    // 验证不会被接口的注解覆盖
}
```

## 调用时机

在`CodeIndex`的两阶段索引方法中，HTTP注解传播在以下时机调用：

```rust
pub fn index_workspace_two_pass(...) {
    // ... 第一阶段和第二阶段解析 ...
    
    // 传播继承的成员
    self.propagate_inherited_members();
    
    // 传播接口的HTTP注解
    self.propagate_interface_http_annotations();
    
    // 传播多态调用
    self.propagate_polymorphic_calls();
}
```

## 注意事项

1. **方法签名匹配**：接口方法和实现类方法必须有相同的方法名和参数类型
2. **参数类型规范化**：基本类型会被自动装箱（如`long` -> `Long`）
3. **完整类名**：参数类型使用完整类名（如`java.lang.String`）
4. **路径组合**：HTTP路径会组合类级别和方法级别的路径

## 相关文件

- `src/code_index.rs`: 实现`propagate_interface_http_annotations()`方法
- `tests/interface_http_annotation_test.rs`: 测试用例
- `src/java_parser.rs`: 解析接口和实现关系
