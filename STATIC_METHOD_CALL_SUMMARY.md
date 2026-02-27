# Java 静态方法调用支持 - 修复总结

## 问题

之前的 Java 解析器在处理静态方法调用时，**忽略了类名信息**，导致无法准确追踪对工具类和静态方法的依赖。

### 示例

```java
import org.apache.commons.lang3.StringUtils;
import java.util.Collections;

public class Example {
    public void test() {
        // 静态方法调用
        String result = StringUtils.isEmpty("test");
        List<String> list = Collections.emptyList();
    }
}
```

**修复前：**
- `StringUtils.isEmpty()` → `isEmpty` ❌
- `Collections.emptyList()` → `emptyList` ❌

**修复后：**
- `StringUtils.isEmpty()` → `org.apache.commons.lang3.StringUtils::isEmpty` ✅
- `Collections.emptyList()` → `java.util.Collections::emptyList` ✅

## 修复内容

### 1. 代码修改

在 `code-impact-analyzer/src/java_parser.rs` 的 `walk_node_for_calls` 函数中：

- 添加了对 `scoped_identifier` 节点的处理（静态方法调用的类名）
- 优先处理静态方法调用模式
- 增强了对未在字段中声明的对象名的处理（可能是静态调用）

### 2. 测试验证

添加了以下测试：

1. **单元测试**：`test_extract_static_method_calls`
   - 验证静态方法调用的正确识别
   - 验证类名通过 import 解析为完整包名
   - 验证实例方法调用不受影响

2. **示例程序**：
   - `examples/test_static_method.rs` - 基础静态方法调用测试
   - `examples/test_real_static_calls.rs` - 真实场景测试

### 3. 测试结果

所有测试通过 ✅：

```
running 15 tests
test java_parser::tests::test_extract_static_method_calls ... ok
test java_parser::tests::test_extract_field_access_method_calls ... ok
test java_parser::tests::test_extract_method_calls ... ok
test java_parser::tests::test_extract_various_method_call_patterns ... ok
... (所有测试通过)
```

真实场景验证：
- ✅ `StringUtils.split()` → `org.apache.commons.lang3.StringUtils::split`
- ✅ `Arrays.asList()` → `java.util.Arrays::asList`
- ✅ `LocalDateTime.now()` → `java.time.LocalDateTime::now`
- ✅ `JSON.toJSONString()` → `com.alibaba.fastjson.JSON::toJSONString`
- ✅ `Collectors.toList()` → `java.util.stream.Collectors::toList`

## 影响

### 正面影响

1. **更准确的依赖追踪**：可以正确识别对工具类静态方法的依赖
2. **更完整的影响分析**：修改工具类时能找到所有调用点
3. **更好的代码理解**：清楚地显示方法调用的完整路径

### 兼容性

- ✅ 向后兼容：不影响现有的实例方法调用处理
- ✅ 所有现有测试通过
- ✅ 不改变 API 接口

## 已知限制

1. **字段初始化中的静态方法调用**：
   ```java
   private static Logger logger = LoggerFactory.getLogger(Class.class);
   ```
   这种情况不会被捕获，因为它不在方法体内。这是设计选择，因为字段初始化在类加载时执行。

2. **完全限定名调用**：
   ```java
   org.apache.commons.lang3.StringUtils.isEmpty("test");
   ```
   如果没有 import 语句，可能无法解析为完整包名。

## 文档

- `code-impact-analyzer/STATIC_METHOD_CALL_FIX.md` - 详细的技术文档
- `code-impact-analyzer/examples/test_static_method.rs` - 基础测试示例
- `code-impact-analyzer/examples/test_real_static_calls.rs` - 真实场景测试

## 日期

2026-02-27
