# MyBatis Mapper 功能快速开始

## 快速体验

### 1. 运行基础示例

```bash
cd code-impact-analyzer
cargo run --example test_mapper_db_operations
```

输出示例：
```
=== 测试 Mapper 类的数据库操作识别 ===

1. 解析 UserMapper:
   类名: com.example.mapper.UserMapper
   - 方法: insertUser
     * 写操作 - 表: User (行: 8)
   - 方法: selectUserById
     * 读操作 - 表: User (行: 17)
   ...
```

### 2. 运行项目分析示例

```bash
cargo run --example analyze_mapper_project
```

输出示例：
```
📋 分析 UserMapper:
   ==================================================
   📊 统计:
      - 总方法数: 6
      - 读操作: 3
      - 写操作: 3
      - 操作的表: User

   📝 方法详情:
      ✏️ insert - 写 (表: User)
      🔍 selectById - 读 (表: User)
      ...
```

## 在你的项目中使用

### 1. 创建 Mapper 接口

```java
package com.example.mapper;

import java.util.List;

public interface UserMapper {
    // 写操作：返回 void 或 int
    void insertUser(User user);
    int updateUser(User user);
    int deleteUser(Long id);
    
    // 读操作：返回对象、List 或数组
    User selectUserById(Long id);
    List<User> selectAllUsers();
}
```

### 2. 运行代码影响分析

```bash
# 构建索引
cargo run -- --workspace /path/to/your/project --build-index

# 分析 patch 文件
cargo run -- --workspace /path/to/your/project --diff /path/to/your.patch
```

### 3. 查看结果

工具会自动识别：
- 所有以 `Mapper` 结尾的接口
- 每个方法操作的数据库表
- 操作类型（读/写）

在生成的影响图中，你会看到：
- 数据库表节点（圆柱形）
- 从 Mapper 方法到数据库表的边
- 完整的调用链路

## 识别规则

### Mapper 类
- 类名必须以 `Mapper` 结尾
- 例如：`UserMapper`, `OrderMapper`, `ProductMapper`

### 表名
- 自动提取：类名去掉 `Mapper` 后缀
- `UserMapper` → `User` 表
- `OrderMapper` → `Order` 表

### 操作类型

| 返回类型 | 操作类型 | 示例 |
|---------|---------|------|
| `void` | 写操作 | `void insertUser(User user)` |
| `int` | 写操作 | `int updateUser(User user)` |
| 对象 | 读操作 | `User selectUserById(Long id)` |
| `List<T>` | 读操作 | `List<User> selectAll()` |
| `T[]` | 读操作 | `User[] selectArray()` |

## 测试

运行单元测试：

```bash
cargo test test_extract_mapper_db_operations
```

运行所有 Java 解析器测试：

```bash
cargo test java_parser
```

## 常见问题

### Q: 我的 Mapper 类不是以 Mapper 结尾怎么办？

A: 目前只支持 `Mapper` 后缀。如果你的项目使用其他命名约定（如 `Dao`），可以：
1. 修改代码中的 `ends_with("Mapper")` 为 `ends_with("Dao")`
2. 或者提交 issue 请求支持配置化的后缀

### Q: 如何区分 Insert、Update、Delete 操作？

A: 当前版本将所有写操作统一标记为 `Update`。如果需要区分，可以：
1. 通过方法名前缀判断（`insert*`, `update*`, `delete*`）
2. 这个功能在未来版本中可能会添加

### Q: 表名不对怎么办？

A: 表名是从类名提取的。如果你的表名与类名不一致：
1. 当前版本不支持 `@Table` 注解
2. 可以在未来版本中添加注解支持
3. 或者在 XML 映射文件中定义表名（需要额外的解析逻辑）

### Q: 非 Mapper 类的数据库操作还能识别吗？

A: 可以！非 Mapper 类会继续使用 SQL 语句匹配：
- `INSERT INTO table_name`
- `UPDATE table_name SET`
- `DELETE FROM table_name`
- `SELECT * FROM table_name`

## 更多信息

- 详细文档：[MAPPER_DB_OPERATIONS.md](MAPPER_DB_OPERATIONS.md)
- 实现总结：[MAPPER_FEATURE_SUMMARY.md](../MAPPER_FEATURE_SUMMARY.md)
- 项目 README：[README.md](README.md)
