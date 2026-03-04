# Changelog - MyBatis Mapper 支持

## [2024-03-04] - MyBatis Mapper 数据库操作自动识别

### 新增功能

#### 1. Mapper 类自动识别
- 自动识别以 `Mapper` 结尾的类/接口
- 从类名提取数据库表名（去掉 `Mapper` 后缀）
- 支持完整包名的类（如 `com.example.mapper.UserMapper`）

#### 2. 基于返回类型的操作判断
- 返回 `void` 或 `int` → 写操作
- 返回其他类型 → 读操作
- 支持的返回类型：
  - 基本类型（`void`, `int`）
  - 对象类型（`User`, `Order` 等）
  - 泛型类型（`List<User>`, `Optional<User>` 等）
  - 数组类型（`User[]`, `String[]` 等）

#### 3. 兼容性保证
- 保留原有的 SQL 语句匹配功能
- 非 Mapper 类继续使用 SQL 匹配
- 两种识别方式可以共存

### 代码变更

#### 修改的文件
- `src/java_parser.rs`

#### 新增方法
```rust
fn extract_return_type(&self, source: &str, method_node: &tree_sitter::Node) -> Option<String>
```
- 提取方法的返回类型
- 支持 void、基本类型、对象、泛型、数组

#### 修改方法
```rust
fn extract_method_info(...)
```
- 添加返回类型提取
- 将返回类型传递给数据库操作提取

```rust
fn extract_db_operations(..., class_name: &str, return_type: &Option<String>)
```
- 添加类名和返回类型参数
- 实现 Mapper 类的特殊处理逻辑
- 保留原有 SQL 匹配作为后备

### 测试

#### 新增测试
- `test_extract_mapper_db_operations`: 测试 Mapper 功能
  - 测试 void 返回类型
  - 测试 int 返回类型
  - 测试对象返回类型
  - 测试 List 返回类型
  - 测试数组返回类型

#### 测试结果
- ✅ 所有新测试通过
- ✅ 所有现有测试通过（23 个 Java 解析器测试）
- ✅ 无回归问题

### 示例程序

#### 1. 基础示例
- 文件：`examples/test_mapper_db_operations.rs`
- 演示 UserMapper、OrderMapper 的解析
- 演示非 Mapper 类的 SQL 匹配

#### 2. 项目分析示例
- 文件：`examples/analyze_mapper_project.rs`
- 模拟完整的 MyBatis 项目
- 生成项目级别统计
- 美观的输出格式

### 文档

#### 新增文档
1. `MAPPER_DB_OPERATIONS.md` - 功能详细文档
2. `MAPPER_FEATURE_SUMMARY.md` - 实现总结
3. `MAPPER_QUICK_START.md` - 快速开始指南
4. `CHANGELOG_MAPPER.md` - 变更日志（本文件）

#### 更新文档
- `README.md` - 添加 MyBatis Mapper 支持说明

### 性能影响

- 新增逻辑非常轻量，性能影响可忽略
- 对于 Mapper 类，避免了复杂的正则匹配
- 对于非 Mapper 类，保持原有性能

### 使用示例

#### Java 代码
```java
public interface UserMapper {
    void insertUser(User user);        // 写操作 → User 表
    int updateUser(User user);         // 写操作 → User 表
    User selectUserById(Long id);      // 读操作 → User 表
    List<User> selectAllUsers();       // 读操作 → User 表
}
```

#### 运行分析
```bash
cargo run --example test_mapper_db_operations
cargo run --example analyze_mapper_project
```

### 已知限制

1. 表名提取基于类名，不支持 `@Table` 注解
2. 写操作统一标记为 `Update`，不区分 Insert/Update/Delete
3. 依赖于 `Mapper` 后缀的命名约定

### 未来改进

1. 支持通过方法名前缀区分操作类型
2. 支持 `@Table` 注解解析
3. 支持配置自定义后缀
4. 支持 MyBatis 注解（`@Select`, `@Insert` 等）

### 贡献者

- 实现：AI Assistant
- 测试：完整的单元测试和集成测试
- 文档：完善的使用文档和示例

### 相关链接

- 功能文档：[MAPPER_DB_OPERATIONS.md](MAPPER_DB_OPERATIONS.md)
- 快速开始：[MAPPER_QUICK_START.md](MAPPER_QUICK_START.md)
- 实现总结：[MAPPER_FEATURE_SUMMARY.md](../MAPPER_FEATURE_SUMMARY.md)
