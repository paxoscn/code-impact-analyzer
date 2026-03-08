# Java 字段修改检测功能

## 概述

当分析 patch 文件时，系统会自动检测 Java 类属性（字段）的修改，并将其视为对相应的 getter 和 setter 方法的修改。这样可以更准确地追踪字段变更对代码的影响。

## 功能说明

### 自动生成规则

当检测到 Java 类字段的修改时，系统会自动生成对应的方法名：

1. **Getter 方法**
   - 普通字段：`fieldName` → `getFieldName()`
   - Boolean 字段：`isActive` → `isActive()` (保持原样)

2. **Setter 方法**
   - 普通字段：`fieldName` → `setFieldName(FieldType)`
   - Boolean 字段：`isActive` → `setActive(boolean)` (移除 `is` 前缀)

### 支持的场景

- ✅ 字段类型修改
- ✅ 字段名称修改
- ✅ 新增字段
- ✅ 删除字段
- ✅ 泛型类型字段
- ✅ 数组类型字段
- ✅ 静态字段和常量

## 使用示例

### 示例 1: 修改字符串字段

**Patch 内容:**
```diff
diff --git a/User.java b/User.java
--- a/User.java
+++ b/User.java
@@ -5,7 +5,7 @@ public class User {
     private Long id;
     
-    private String userName;
+    private String username;
     
     private int age;
```

**检测结果:**
系统会将此修改视为对以下方法的修改：
- `getUserName()`
- `setUserName(String)`
- `getUsername()`
- `setUsername(String)`

### 示例 2: 修改 Boolean 字段

**Patch 内容:**
```diff
diff --git a/Account.java b/Account.java
--- a/Account.java
+++ b/Account.java
@@ -3,7 +3,7 @@ public class Account {
     private String accountId;
     
-    private boolean isActive;
+    private boolean isEnabled;
```

**检测结果:**
系统会将此修改视为对以下方法的修改：
- `isActive()` (getter 保持 `isXxx` 格式)
- `setActive(boolean)` (setter 移除 `is` 前缀)
- `isEnabled()`
- `setEnabled(boolean)`

### 示例 3: 添加新字段

**Patch 内容:**
```diff
diff --git a/Product.java b/Product.java
--- a/Product.java
+++ b/Product.java
@@ -5,6 +5,8 @@ public class Product {
     private String name;
     private BigDecimal price;
     
+    private String description;
+    
     public String getName() {
```

**检测结果:**
系统会将此修改视为对以下方法的修改：
- `getDescription()`
- `setDescription(String)`

### 示例 4: 修改泛型字段

**Patch 内容:**
```diff
diff --git a/Container.java b/Container.java
--- a/Container.java
+++ b/Container.java
@@ -3,7 +3,7 @@ public class Container {
     private String id;
     
-    private List<String> items;
+    private List<Item> items;
```

**检测结果:**
系统会将此修改视为对以下方法的修改：
- `getItems()`
- `setItems(List<String>)`
- `setItems(List<Item>)`

## API 使用

### 基本用法

```rust
use code_impact_analyzer::patch_parser::PatchParser;

// 解析 patch 文件
let changes = PatchParser::parse_patch_file(patch_path)?;

// 提取字段修改对应的方法
for change in &changes {
    let methods = PatchParser::extract_modified_field_methods(change);
    
    println!("文件: {}", change.file_path);
    println!("受影响的方法:");
    for method in methods {
        println!("  - {}", method);
    }
}
```

### 运行示例

```bash
# 运行字段修改检测示例
cargo run --example test_field_modification

# 运行测试
cargo test --lib patch_parser::tests::test_extract_modified_field
```

## 实现细节

### 字段检测逻辑

系统使用正则表达式和语法分析来检测 Java 字段声明：

1. **识别字段声明模式**
   - 格式：`[修饰符] 类型 字段名 [= 初始值];`
   - 修饰符：`private`, `protected`, `public`, `static`, `final`, `transient`, `volatile`

2. **排除非字段声明**
   - 方法声明（包含括号）
   - 类/接口/枚举声明
   - 注解
   - 导入语句
   - 注释

3. **提取字段信息**
   - 字段类型（包括泛型）
   - 字段名称

### 方法名生成规则

```rust
// Getter 生成
fn generate_getter_name(field_name: &str) -> String {
    if field_name.starts_with("is") && field_name.len() > 2 {
        field_name.to_string()  // boolean 字段保持原样
    } else {
        format!("get{}{}", first_char.to_uppercase(), rest)
    }
}

// Setter 生成
fn generate_setter_name(field_name: &str) -> String {
    let name = if field_name.starts_with("is") {
        &field_name[2..]  // 移除 "is" 前缀
    } else {
        field_name
    };
    format!("set{}{}", first_char.to_uppercase(), rest)
}
```

## 测试覆盖

项目包含完整的测试套件：

- ✅ `test_generate_getter_name` - 测试 getter 方法名生成
- ✅ `test_generate_setter_name` - 测试 setter 方法名生成
- ✅ `test_detect_java_field` - 测试字段声明检测
- ✅ `test_extract_modified_field_methods` - 测试字段修改提取
- ✅ `test_extract_modified_field_methods_boolean` - 测试 boolean 字段
- ✅ `test_extract_modified_field_methods_non_java` - 测试非 Java 文件

运行测试：
```bash
cargo test --lib patch_parser::tests
```

## 注意事项

1. **仅支持 Java 文件**
   - 只有 `.java` 文件会触发字段检测
   - 其他文件类型会被忽略

2. **简化的检测逻辑**
   - 使用基于文本的模式匹配
   - 不依赖完整的 AST 解析
   - 可能在复杂情况下产生误报

3. **方法签名格式**
   - Getter: 只包含方法名，如 `getUserName`
   - Setter: 包含参数类型，如 `setUserName(String)`

4. **去重处理**
   - 同一字段的多次修改只会生成一组方法
   - 结果会自动排序和去重

## 未来改进

- [ ] 支持 Lombok 注解（`@Data`, `@Getter`, `@Setter`）
- [ ] 支持自定义 getter/setter 命名规则
- [ ] 支持 Kotlin 数据类
- [ ] 使用 tree-sitter 进行更精确的字段检测
- [ ] 支持字段访问权限分析

## 相关文档

- [Patch 解析器文档](./src/patch_parser.rs)
- [影响分析文档](./USAGE.md)
- [快速开始指南](./README.md)
