# Mapper类数据库操作识别

## 功能说明

Java解析器现在支持自动识别Mapper类的数据库操作，无需通过SQL语句匹配。

## 识别规则

### 1. Mapper类识别
- 类名以 `Mapper` 结尾的类（接口或普通类）会被识别为Mapper类
- 例如：`UserMapper`, `OrderMapper`, `ProductMapper`

### 2. 表名提取
- 表名 = 类名去掉 `Mapper` 后缀
- 例如：
  - `UserMapper` → 表名为 `User`
  - `OrderMapper` → 表名为 `Order`
  - `ProductMapper` → 表名为 `Product`

### 3. 操作类型判断

根据方法的返回类型判断数据库操作类型：

#### 写操作（DbOpType::Update）
- 返回类型为 `void`
- 返回类型为 `int`

#### 读操作（DbOpType::Select）
- 返回其他任何类型（对象、List、数组等）

## 示例

### UserMapper接口

```java
package com.example.mapper;

import java.util.List;

public interface UserMapper {
    // 读操作 - 返回User对象
    User selectById(Long id);
    
    // 读操作 - 返回List
    List<User> selectAll();
    
    // 写操作 - 返回void
    void insert(User user);
    
    // 写操作 - 返回int（影响行数）
    int update(User user);
    
    // 写操作 - 返回int
    int deleteById(Long id);
}
```

### 识别结果

| 方法 | 返回类型 | 操作类型 | 表名 |
|------|---------|---------|------|
| selectById | User | Select | User |
| selectAll | List<User> | Select | User |
| insert | void | Update | User |
| update | int | Update | User |
| deleteById | int | Update | User |

## 非Mapper类

非Mapper类（类名不以Mapper结尾）的方法不会被自动识别为数据库操作，仍然使用原有的SQL语句匹配逻辑。

### 示例

```java
package com.example.service;

public class UserService {
    private UserMapper userMapper;
    
    // 这个方法不会被识别为数据库操作
    public User findById(Long id) {
        return userMapper.selectById(id);
    }
    
    // 这个方法也不会被识别为数据库操作
    public void saveUser(User user) {
        userMapper.insert(user);
    }
}
```

## 兼容性

- 新功能与现有的SQL语句匹配逻辑完全兼容
- Mapper类使用新的自动识别规则
- 非Mapper类继续使用SQL语句匹配
- 所有现有测试均通过

## 测试

运行以下命令测试Mapper功能：

```bash
# 运行Mapper相关测试
cargo test test_extract_mapper_db_operations --lib

# 运行非Mapper类测试
cargo test test_non_mapper_class_no_auto_db_operations --lib

# 运行示例程序
cargo run --example test_mapper_db_operations
```

## 实现细节

### 修改的文件
- `code-impact-analyzer/src/java_parser.rs`

### 主要修改
1. `extract_methods_from_class`: 提取简单类名（不含包名）
2. `extract_method_info`: 添加 `simple_class_name` 参数，提取返回类型
3. `extract_return_type`: 新增方法，提取方法返回类型
4. `extract_db_operations`: 修改为接受类名和返回类型参数，实现Mapper类识别逻辑

### 返回类型支持
- `void_type`: void类型
- `type_identifier`: 简单类型（如User、String）
- `integral_type`: 基本类型（如int、long）
- `generic_type`: 泛型类型（如List<User>）
- `array_type`: 数组类型（如User[]）
