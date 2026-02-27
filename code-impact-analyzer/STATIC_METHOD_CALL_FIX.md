# 静态方法调用支持修复

## 问题描述

之前的实现在解析 Java 代码时，**忽略了静态方法调用的类名**。例如：

```java
import org.apache.commons.lang3.StringUtils;
import java.util.Collections;

public class Example {
    private UserService userService;
    
    public void testMethod() {
        // 静态方法调用
        String result = StringUtils.isEmpty("test");
        List<String> list = Collections.emptyList();
        
        // 实例方法调用
        userService.findUser();
    }
}
```

**修复前的输出：**
- `StringUtils.isEmpty()` → `isEmpty` ❌（丢失了类名）
- `Collections.emptyList()` → `emptyList` ❌（丢失了类名）
- `userService.findUser()` → `UserService::findUser` ✅（实例方法正常）

**修复后的输出：**
- `StringUtils.isEmpty()` → `org.apache.commons.lang3.StringUtils::isEmpty` ✅
- `Collections.emptyList()` → `java.util.Collections::emptyList` ✅
- `userService.findUser()` → `UserService::findUser` ✅

## 根本原因

在 `walk_node_for_calls` 函数中，代码只检查了 `identifier` 节点类型，但静态方法调用在 tree-sitter-java 的 AST 中会产生 `scoped_identifier` 节点。

例如，对于 `StringUtils.isEmpty()`：
- `StringUtils` 是一个 `scoped_identifier` 节点
- `isEmpty` 是一个 `identifier` 节点

之前的代码只收集 `identifier` 节点，导致类名信息丢失。

## 修复方案

在 `src/java_parser.rs` 的 `walk_node_for_calls` 函数中：

1. **添加 `scoped_identifier` 节点的处理**：
   ```rust
   let mut scoped_identifiers = Vec::new();
   
   for child in node.children(&mut cursor) {
       if child.kind() == "identifier" {
           // ... 现有代码
       } else if child.kind() == "scoped_identifier" {
           // 处理静态方法调用
           if let Some(text) = source.get(child.byte_range()) {
               scoped_identifiers.push(text.to_string());
           }
       }
   }
   ```

2. **优先处理静态方法调用**：
   ```rust
   // 处理静态方法调用：ClassName.staticMethod()
   if !scoped_identifiers.is_empty() && !identifiers.is_empty() {
       let class_name = &scoped_identifiers[0];
       let method_name = &identifiers[identifiers.len() - 1];
       
       // 尝试将简单类名转换为完整类名
       let full_class_name = import_map.get(class_name)
           .unwrap_or(class_name);
       
       let target = format!("{}::{}", full_class_name, method_name);
       calls.push(MethodCall { target, line });
       return;
   }
   ```

3. **增强实例方法调用的处理**：
   对于没有在 `field_types` 中找到的对象名，尝试从 `import_map` 中查找，以支持静态方法调用的另一种形式。

## 测试验证

添加了新的测试用例 `test_extract_static_method_calls`，验证：

1. ✅ 静态方法调用被正确识别为 `ClassName::methodName` 格式
2. ✅ 类名通过 import 语句解析为完整的包名
3. ✅ 实例方法调用仍然正常工作
4. ✅ 链式调用中的静态方法也被正确识别

## 影响范围

这个修复提升了代码影响分析的准确性：

- **更准确的依赖追踪**：现在可以正确追踪对工具类静态方法的调用
- **更完整的影响分析**：修改工具类的静态方法时，可以找到所有调用点
- **向后兼容**：不影响现有的实例方法调用处理逻辑

## 相关文件

- `src/java_parser.rs` - 主要修复代码
- `examples/test_static_method.rs` - 手动测试示例
- 新增测试：`test_extract_static_method_calls`

## 日期

2026-02-27
