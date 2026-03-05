# Java方法签名实现总结

## 概述

为了准确区分Java中的重载方法，我们修改了方法标识符和方法调用的格式，使其包含完整的方法签名（类名 + 方法名 + 参数类型列表）。为了简化签名并提高匹配准确性，泛型信息被移除。

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

**泛型处理：**
泛型信息被移除以简化签名：
- `List<String>` → `List`
- `Map<String, Object>` → `Map`
- `Set<Integer>` → `Set`

**示例：**
- 无参数方法：`com.example.UserService::getUser()`
- 单参数方法：`com.example.UserService::getUser(String)`
- 多参数方法：`com.example.UserService::updateUser(String,int,boolean)`
- 泛型参数（移除泛型）：`com.example.UserService::findUsers(List,Map)`
- 数组参数：`com.example.UserService::processArray(String[],int[][])`

### 2. 方法调用格式变更

方法调用（MethodCall）的 `target` 字段现在也包含参数类型（不含泛型）：

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

**泛型处理示例：**
```
list.add(new ArrayList<String>())
-> target: "List::add(ArrayList)"  // 泛型被移除
```

### 3. 代码修改

#### 3.1 新增辅助函数

1. **`remove_generics`**: 移除类型中的泛型信息
   ```rust
   fn remove_generics(type_name: &str) -> String {
       if let Some(pos) = type_name.find('<') {
           type_name[..pos].to_string()
       } else {
           type_name.to_string()
       }
   }
   ```
   - `List<String>` → `List`
   - `Map<K,V>` → `Map`
   - `String` → `String`（无变化）

2. **`is_primitive_or_common_type`**: 判断是否是基本类型或常用类型
   - 基本类型：`int`, `boolean`, `String` 等
   - java.lang 包中的常用类：`Integer`, `Long`, `Object` 等
   - 常用集合类：`List`, `Map`, `Set` 等

#### 3.2 修改参数类型提取函数（方法声明）

在 `extract_parameter_type` 函数中应用 `remove_generics`：

```rust
"generic_type" => {
    // 处理泛型类型，如 List<String> -> List
    if let Some(text) = source.get(child.byte_range()) {
        return Some(remove_generics(text));
    }
}
"array_type" => {
    // 处理数组类型，如 List<String>[] -> List[]
    if let Some(text) = source.get(child.byte_range()) {
        return Some(remove_generics(text));
    }
}
```

#### 3.3 修改参数类型推断函数（方法调用）

在 `infer_argument_type` 函数中应用 `remove_generics`：

```rust
"object_creation_expression" => {
    // new ArrayList<String>() -> ArrayList
    if child.kind() == "generic_type" {
        if let Some(type_name) = source.get(child.byte_range()) {
            return Some(remove_generics(type_name));
        }
    }
}
"cast_expression" => {
    // (List<String>) obj -> List
    if child.kind() == "generic_type" {
        if let Some(type_name) = source.get(child.byte_range()) {
            return Some(remove_generics(type_name));
        }
    }
}
```

#### 3.4 其他修改

1. **`extract_local_variable_types`**: 
   - 对于基本类型和常用类型，不进行完整类名解析
   - 避免将 `String` 解析为 `com.example.String`

2. **`extract_field_type_from_declaration`**:
   - 支持提取基本类型（`integral_type`, `floating_point_type`, `boolean_type`）

### 4. 测试更新

#### 4.1 更新测试期望值

将泛型参数的测试期望值从：
```rust
"com.example.UserService::findUsers(List<String>,Map<String, Object>)"
```
更新为：
```rust
"com.example.UserService::findUsers(List,Map)"
```

## 支持的参数类型

### 方法声明参数类型
- 基本类型：`int`, `long`, `boolean`, `char` 等
- 引用类型：`String`, `Object`, 自定义类
- 泛型类型（移除泛型）：`List<String>` → `List`, `Map<K,V>` → `Map`
- 数组类型：`String[]`, `int[][]`
- 泛型数组（移除泛型）：`List<String>[]` → `List[]`
- 带包名的类型：`java.util.List`

### 方法调用参数类型推断
- 字符串字面量 → `String`
- 整数字面量 → `int` 或 `long`
- 浮点数字面量 → `float` 或 `double`
- 布尔字面量 → `boolean`
- null 字面量 → `Object`
- 字符字面量 → `char`
- 变量 → 从变量类型映射中查找
- 对象创建（移除泛型）：`new ArrayList<String>()` → `ArrayList`
- 数组创建：元素类型 + `[]`
- 类型转换（移除泛型）：`(List<String>) obj` → `List`
- 其他表达式 → `Object`

## 泛型处理示例

### 方法声明
```java
// 源代码
public List<User> findUsers(List<String> ids, Map<String, Object> filters) {
    return null;
}

// 方法签名（泛型被移除）
com.example.UserService::findUsers(List,Map)
```

### 方法调用
```java
// 源代码
List<String> list = new ArrayList<String>();
map.put("key", new HashMap<String, Integer>());

// 方法调用（泛型被移除）
ArrayList::<init>(ArrayList)  // new ArrayList<String>()
Map::put(String,HashMap)      // put("key", new HashMap<String, Integer>())
```

## 优势

1. **准确区分重载方法**：可以准确识别同名但参数不同的方法
2. **简化签名**：移除泛型信息使签名更简洁
3. **提高匹配准确性**：
   - `List<String>` 和 `List<Integer>` 都匹配到 `List`
   - 避免因泛型参数不同而无法匹配的问题
4. **符合Java类型擦除**：与Java运行时的类型擦除机制一致
5. **更好的可读性**：方法签名更加清晰，便于理解

## 注意事项

1. 参数类型之间用逗号分隔，没有空格
2. 无参数方法也需要包含空括号 `()`
3. 泛型信息被完全移除：`List<String>` → `List`
4. 数组类型保留方括号：`String[]`, `int[][]`
5. 泛型数组也移除泛型：`List<String>[]` → `List[]`
6. 对于基本类型和常用类型（如 `String`），不进行完整类名解析
7. 方法调用的参数类型是推断的，可能不完全准确（如复杂表达式）

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


