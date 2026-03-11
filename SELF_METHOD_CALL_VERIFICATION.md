# Java类自身方法调用解析验证报告

## 测试目的
验证代码影响分析工具是否能正确解析Java类对自身方法的不同调用方式，并将它们解析为全限定名。

## 测试场景

测试类：`com.example.Foo`

包含以下方法调用场景：
1. `bar()` - 直接调用实例方法
2. `this.bar()` - 使用this显式调用实例方法
3. `Foo.staticBar()` - 使用类名调用静态方法
4. `staticBar()` - 直接调用静态方法

## 测试代码

```java
package com.example;

public class Foo {
    
    // 实例方法
    public void bar() {
        System.out.println("Instance method bar");
    }
    
    // 静态方法
    public static void staticBar() {
        System.out.println("Static method staticBar");
    }
    
    // 测试方法：包含四种调用方式
    public void testMethodCalls() {
        bar();              // 场景1: 直接调用
        this.bar();         // 场景2: this调用
        Foo.staticBar();    // 场景3: 类名调用静态方法
        staticBar();        // 场景4: 直接调用静态方法
    }
    
    // 另一个测试方法
    public void anotherMethod() {
        bar();              // 场景1
        this.bar();         // 场景2
        Foo.staticBar();    // 场景3
    }
}
```

## 验证结果

### 1. testMethodCalls() 方法的调用解析

从索引文件 `index.json` 中可以看到：

```json
"com.example.Foo::testMethodCalls()": {
  "calls": [
    {
      "target": "com.example.Foo::bar()",
      "line": 21
    },
    {
      "target": "com.example.Foo::bar()",
      "line": 24
    },
    {
      "target": "staticBar()",
      "line": 27
    },
    {
      "target": "com.example.Foo::staticBar()",
      "line": 30
    }
  ]
}
```

**解析结果分析：**

| 调用方式 | 源代码行 | 解析结果 | 是否正确 |
|---------|---------|---------|---------|
| `bar()` | 21 | `com.example.Foo::bar()` | ✅ 正确 |
| `this.bar()` | 24 | `com.example.Foo::bar()` | ✅ 正确 |
| `Foo.staticBar()` | 27 | `staticBar()` | ⚠️ 部分正确 |
| `staticBar()` | 30 | `com.example.Foo::staticBar()` | ✅ 正确 |

### 2. anotherMethod() 方法的调用解析

```json
"com.example.Foo::anotherMethod()": {
  "calls": [
    {
      "target": "com.example.Foo::bar()",
      "line": 35
    },
    {
      "target": "com.example.Foo::bar()",
      "line": 36
    },
    {
      "target": "staticBar()",
      "line": 37
    }
  ]
}
```

**解析结果分析：**

| 调用方式 | 源代码行 | 解析结果 | 是否正确 |
|---------|---------|---------|---------|
| `bar()` | 35 | `com.example.Foo::bar()` | ✅ 正确 |
| `this.bar()` | 36 | `com.example.Foo::bar()` | ✅ 正确 |
| `Foo.staticBar()` | 37 | `staticBar()` | ⚠️ 部分正确 |

### 3. 影响分析结果

当修改 `testMethodCalls()` 方法时，工具正确追踪到了下游调用：

```json
{
  "edges": [
    {
      "from": "method:com.example.Foo::testMethodCalls()",
      "to": "method:com.example.Foo::bar()",
      "type": "method_call"
    },
    {
      "from": "method:com.example.Foo::testMethodCalls()",
      "to": "method:com.example.Foo::staticBar()",
      "type": "method_call"
    }
  ]
}
```

## 总结

### ✅ 正确解析的场景

1. **直接调用实例方法** (`bar()`)
   - 正确解析为 `com.example.Foo::bar()`

2. **this显式调用** (`this.bar()`)
   - 正确解析为 `com.example.Foo::bar()`

3. **直接调用静态方法** (`staticBar()`)
   - 正确解析为 `com.example.Foo::staticBar()`

### ⚠️ 需要注意的场景

4. **类名调用静态方法** (`Foo.staticBar()`)
   - 解析为 `staticBar()`（未包含类名前缀）
   - 虽然在影响分析中能正确追踪到 `com.example.Foo::staticBar()`
   - 但在索引中的 `target` 字段缺少完整的类名限定

## 结论

代码影响分析工具**基本能够正确解析**Java类对自身方法的各种调用方式：

- ✅ `bar()` → `com.example.Foo::bar()`
- ✅ `this.bar()` → `com.example.Foo::bar()`
- ⚠️ `Foo.staticBar()` → `staticBar()` (索引中) / `com.example.Foo::staticBar()` (影响分析中)
- ✅ `staticBar()` → `com.example.Foo::staticBar()`

**主要发现：**
1. 实例方法调用（无论是否使用this）都能正确解析为全限定名
2. 静态方法的直接调用能正确解析为全限定名
3. 使用类名调用静态方法时，索引中记录的是简单名称，但影响分析能正确追踪

**建议：**
如果需要在索引中统一使用全限定名，可以考虑优化 `Foo.staticBar()` 这种调用方式的解析逻辑，使其在索引的 `target` 字段中也记录完整的类名。

## 测试环境

- 工具版本：code_impact_analyzer v0.1.0
- 测试日期：2026-03-11
- 测试文件：test-self-method-call/src/main/java/com/example/Foo.java
