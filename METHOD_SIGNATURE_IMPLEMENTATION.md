# Java方法签名实现总结

## 概述

为了准确区分Java中的重载方法，我们修改了方法标识符和方法调用的格式，使其包含完整的方法签名（类名 + 方法名 + 参数类型列表）。为了简化签名并提高匹配准确性，泛型信息被移除。此外，实现了方法返回类型推断，用于嵌套方法调用的参数类型推断。

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

### 3. 变量类型识别

现在支持识别以下类型的变量：

1. **类字段**：在类中声明的字段
   ```java
   private UserService userService;
   ```

2. **方法参数**：方法声明中的参数
   ```java
   public void processUser(String userId, int age) {
       // userId 和 age 的类型会被识别
   }
   ```

3. **本地变量**：方法内声明的变量
   ```java
   String name = "test";
   int count = 10;
   ```

所有这些变量在方法调用时都能正确推断其类型。

### 4. 方法返回类型推断（新功能）

实现了同文件内的方法返回类型推断，用于嵌套方法调用的参数类型推断。

**工作原理：**

采用两遍解析策略：

1. **第一遍**：提取所有类和方法，建立方法返回类型映射
   - 遍历所有类和方法
   - 提取每个方法的返回类型
   - 建立映射：`方法签名 -> 返回类型`

2. **第二遍**：使用返回类型映射重新提取方法调用
   - 对每个方法，使用返回类型映射推断嵌套调用的参数类型
   - 当遇到 `foo.bar()` 作为参数时，查找 `bar()` 的返回类型

**示例：**

```java
public class UserRepository {
    public User findUser(String id) {
        return null;
    }
}

public class DataProcessor {
    public void process(User user) {
        // process
    }
}

public class TestService {
    private UserRepository userRepository;
    private DataProcessor processor;
    
    public void processData() {
        // 嵌套方法调用
        processor.process(userRepository.findUser("123"));
    }
}
```

**推断结果：**
- `userRepository.findUser("123")` 的返回类型是 `User`
- 因此 `processor.process(...)` 的调用被识别为 `DataProcessor::process(User)`
- 而不是 `DataProcessor::process(Object)`

**限制：**
- 仅支持同文件内的方法返回类型推断
- 跨文件的返回类型推断需要全局索引（未实现）
- 链式调用（如 `foo.bar().baz()`）的中间返回类型推断有限

### 5. 代码修改

#### 5.1 新增辅助函数

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

#### 3.4 新增方法参数类型提取

新增了 `extract_method_parameter_types` 和 `extract_parameter_name_and_type` 函数：

1. **`extract_method_parameter_types`**: 提取方法参数的类型
   - 遍历 `formal_parameters` 节点
   - 提取每个参数的名称和类型
   - 解析为完整类名

2. **`extract_parameter_name_and_type`**: 提取单个参数的名称和类型
   - 支持基本类型、引用类型、泛型类型（移除泛型）、数组类型

#### 3.5 修改变量类型提取逻辑

修改了 `extract_field_types` 函数，现在按顺序提取：
1. 类字段（并解析为完整类名）
2. 方法参数（并解析为完整类名）
3. 本地变量（并解析为完整类名）

修改了 `extract_local_variable_types` 函数：
- 只解析新添加的本地变量
- 避免重复解析已经处理过的类字段和方法参数

#### 3.6 其他修改

1. **`extract_local_variable_types`**: 
   - 对于基本类型和常用类型，不进行完整类名解析
   - 避免将 `String` 解析为 `com.example.String`

2. **`extract_field_type_from_declaration`**:
   - 支持提取基本类型（`integral_type`, `floating_point_type`, `boolean_type`）

### 4. 测试更新

#### 4.1 新增测试

1. **`test_extract_method_with_parameters`**: 测试方法声明的参数类型提取
2. **`test_extract_method_calls_with_arguments`**: 测试方法调用的参数类型推断（字面量和本地变量）
3. **`test_extract_method_calls_with_method_parameters`**: 测试方法调用的参数类型推断（方法参数）

#### 4.2 更新测试期望值

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

## 当前限制

### 1. 嵌套方法调用的返回类型推断

对于嵌套的方法调用（如 `go(foo.getBar())`），当前实现无法准确推断 `foo.getBar()` 的返回类型，会将其推断为 `Object`。

**示例：**
```java
processor.process(userRepository.findUser("123"));
// 实际：processor.process(Object)
// 理想：processor.process(User)  // 如果 findUser 返回 User
```

**原因：**
- 需要完整的类型系统和方法索引来查找方法的返回类型
- 需要跨文件的类型解析能力

**影响：**
- 对于重载方法，可能无法精确匹配到正确的方法签名
- 但对于大多数情况（使用字面量、变量、对象创建作为参数），类型推断是准确的

**未来改进：**
- 建立方法返回类型的索引
- 实现跨文件的类型解析
- 支持泛型返回类型的推断

## 测试结果

所有测试通过：
- 124个单元测试全部通过
- 包括新增的方法参数类型识别测试
- 包括更新后的所有现有测试

## 后续工作

可能需要考虑的改进：
1. 支持可变参数（varargs）：`String...`
2. 支持注解参数：`@NotNull String`
3. 更精确的参数类型推断（如方法返回类型）
4. 参数类型的简化显示（可选）：使用简单类名而不是完整包名
5. 支持Lambda表达式的参数类型推断


