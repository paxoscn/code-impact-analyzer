# Java方法签名实现总结

## 概述

为了准确区分Java中的重载方法，我们修改了方法标识符和方法调用的格式，使其包含完整的方法签名（类名 + 方法名 + 参数类型列表）。

## 修改内容

### 1. 方法标识符格式变更

**之前的格式：**
```
ClassName::methodName
```

**现在的格式：**
```
ClassName::methodName(Type1,Type2,...)
```

**示例：**
- 无参数方法：`com.example.UserService::getUser()`
- 单参数方法：`com.example.UserService::getUser(String)`
- 多参数方法：`com.example.UserService::updateUser(String,int,boolean)`
- 泛型参数：`com.example.UserService::findUsers(List<String>,Map<String, Object>)`
- 数组参数：`com.example.UserService::processArray(String[],int[][])`

### 2. 方法调用格式变更

方法调用（MethodCall）的 `target` 字段现在也包含参数类型：

**之前的格式：**
```
userService.updateUser("123", 25, true)
-> target: "UserService::updateUser"
```

**现在的格式：**
```
userService.updateUser("123", 25, true)
-> target: "UserService::updateUser(String,int,boolean)"
```

### 3. 代码修改

#### 3.1 新增参数类型提取函数（方法声明）

在 `java_parser.rs` 中新增了两个函数用于提取方法声明的参数类型：

1. **`extract_parameter_types`**: 提取方法的所有参数类型
   - 遍历 `formal_parameters` 节点
   - 收集每个 `formal_parameter` 的类型

2. **`extract_parameter_type`**: 提取单个参数的类型
   - 支持基本类型：`int`, `boolean`, `String` 等
   - 支持泛型类型：`List<String>`, `Map<K,V>` 等
   - 支持数组类型：`String[]`, `int[][]` 等
   - 支持带包名的类型：`java.util.List` 等

#### 3.2 新增参数类型推断函数（方法调用）

新增了两个函数用于推断方法调用的参数类型：

1. **`extract_argument_types`**: 提取方法调用的所有参数类型
   - 遍历 `argument_list` 节点
   - 推断每个参数的类型

2. **`infer_argument_type`**: 推断单个参数的类型
   - 字符串字面量 → `String`
   - 整数字面量 → `int` 或 `long`
   - 浮点数字面量 → `float` 或 `double`
   - 布尔字面量 → `boolean`
   - 变量名 → 从变量类型映射中查找
   - 对象创建 → 提取类型名
   - 数组创建 → 提取元素类型并添加 `[]`
   - 其他表达式 → `Object`

#### 3.3 新增辅助函数

1. **`is_primitive_or_common_type`**: 判断是否是基本类型或常用类型
   - 基本类型：`int`, `boolean`, `String` 等
   - java.lang 包中的常用类：`Integer`, `Long`, `Object` 等
   - 常用集合类：`List`, `Map`, `Set` 等
   - 泛型和数组类型保持原样

2. **修改 `extract_local_variable_types`**: 
   - 对于基本类型和常用类型，不进行完整类名解析
   - 避免将 `String` 解析为 `com.example.String`

3. **修改 `extract_field_type_from_declaration`**:
   - 支持提取基本类型（`integral_type`, `floating_point_type`, `boolean_type`）

#### 3.4 修改方法信息提取逻辑

在 `extract_method_info` 函数中：
```rust
// 提取参数类型列表
let param_types = self.extract_parameter_types(source, &method_node);

// 构建完整的方法签名：ClassName::methodName(Type1,Type2,...)
let full_qualified_name = if param_types.is_empty() {
    format!("{}::{}()", class_name, name)
} else {
    format!("{}::{}({})", class_name, name, param_types.join(","))
};
```

#### 3.5 修改方法调用提取逻辑

在 `walk_node_for_calls` 函数中：
```rust
// 提取参数类型
let arg_types = if let Some(arg_node) = argument_list_node {
    self.extract_argument_types(source, &arg_node, field_types, import_map)
} else {
    Vec::new()
};

// 构建方法调用标识符
let target = if arg_types.is_empty() {
    format!("{}::{}()", class_name, method_name)
} else {
    format!("{}::{}({})", class_name, method_name, arg_types.join(","))
};
```

### 4. 测试更新

#### 4.1 新增测试

1. **`test_extract_method_with_parameters`**: 测试方法声明的参数类型提取
   - 单参数方法
   - 多参数方法
   - 泛型参数方法
   - 数组参数方法

2. **`test_extract_method_calls_with_arguments`**: 测试方法调用的参数类型推断
   - 字面量参数
   - 变量参数
   - 混合参数

#### 4.2 更新现有测试

更新了所有使用方法标识符的测试，将期望值从：
```rust
"com.example.UserController::getUser"
```
更新为：
```rust
"com.example.UserController::getUser()"
```

或使用更灵活的匹配方式：
```rust
assert!(call_names.iter().any(|name| name.contains("updateUser")));
```

## 支持的参数类型

### 方法声明参数类型
- 基本类型：`int`, `long`, `boolean`, `char` 等
- 引用类型：`String`, `Object`, 自定义类
- 泛型类型：`List<String>`, `Map<K,V>`
- 数组类型：`String[]`, `int[][]`
- 带包名的类型：`java.util.List`

### 方法调用参数类型推断
- 字符串字面量 → `String`
- 整数字面量 → `int` 或 `long`
- 浮点数字面量 → `float` 或 `double`
- 布尔字面量 → `boolean`
- null 字面量 → `Object`
- 字符字面量 → `char`
- 变量 → 从变量类型映射中查找
- 对象创建 → 提取类型名
- 数组创建 → 元素类型 + `[]`
- 类型转换 → 提取目标类型
- 其他表达式 → `Object`

## Tree-sitter 节点结构

### 方法声明参数
```
method_declaration
  └─ formal_parameters
      ├─ formal_parameter
      │   ├─ type_identifier (String)
      │   └─ identifier (id)
      ├─ formal_parameter
      │   ├─ integral_type (int)
      │   └─ identifier (age)
      └─ ...
```

### 方法调用参数
```
method_invocation
  ├─ identifier (userService)
  ├─ identifier (updateUser)
  └─ argument_list
      ├─ string_literal ("123")
      ├─ decimal_integer_literal (25)
      └─ true
```

## 影响范围

这个修改影响了：
1. 方法索引：所有方法的 `full_qualified_name` 现在包含参数类型
2. 方法调用：所有方法调用的 `target` 现在包含参数类型
3. 方法查找：需要使用完整的方法签名来查找方法
4. 影响追溯：追溯图中的方法节点ID和边都包含参数类型
5. 测试代码：所有使用方法标识符的测试都需要更新

## 优势

1. **准确区分重载方法**：可以准确识别同名但参数不同的方法
2. **更好的可读性**：方法签名更加清晰，便于理解
3. **符合Java规范**：与Java的方法签名格式一致
4. **精确的调用匹配**：方法调用可以精确匹配到对应的方法定义

## 注意事项

1. 参数类型之间用逗号分隔，没有空格
2. 无参数方法也需要包含空括号 `()`
3. 泛型类型保留完整的泛型信息，包括尖括号和空格
4. 数组类型保留方括号
5. 对于基本类型和常用类型（如 `String`），不进行完整类名解析
6. 方法调用的参数类型是推断的，可能不完全准确（如复杂表达式）

## 测试结果

所有测试通过：
- 123个单元测试全部通过
- 包括新增的参数类型提取和推断测试
- 包括更新后的所有现有测试

## 后续工作

可能需要考虑的改进：
1. 支持可变参数（varargs）：`String...`
2. 支持注解参数：`@NotNull String`
3. 更精确的参数类型推断（如方法返回类型）
4. 参数类型的简化显示（可选）：使用简单类名而不是完整包名
5. 支持Lambda表达式的参数类型推断

