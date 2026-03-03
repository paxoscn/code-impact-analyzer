# 调试日志使用指南

## 概述

为了帮助调试接口方法调用的解析问题（如 `complete` 方法），我们在关键位置添加了详细的调试日志。

## 启用调试日志

### 方法1：使用环境变量

```bash
# 启用所有 DEBUG 级别日志
RUST_LOG=debug cargo run --example test_complete_method_debug

# 只启用特定模块的 DEBUG 日志
RUST_LOG=code_impact_analyzer::java_parser=debug cargo run

# 启用多个模块的日志
RUST_LOG=code_impact_analyzer::java_parser=debug,code_impact_analyzer::code_index=debug cargo run
```

### 方法2：在代码中初始化

```rust
// 在 main 函数开始处添加
env_logger::Builder::from_env(
    env_logger::Env::default().default_filter_or("debug")
).init();
```

## 调试日志内容

### 1. 方法调用提取 (java_parser.rs)

#### 开始提取方法调用
```
[DEBUG] 开始提取方法调用: method=finishTask
[DEBUG] 导入映射: {"TaskService": "com.example.TaskService"}
```

#### 字段类型提取
```
[DEBUG] 开始提取字段类型...
[DEBUG] 提取字段声明: name=taskService, type=TaskService
[DEBUG] 提取到类字段: {"taskService": "TaskService"}
[DEBUG] 提取字段类型完成，总计: {"taskService": "com.example.TaskService"}
```

#### 发现方法调用
```
[DEBUG] 发现方法调用 at line 20: identifiers=["taskService", "complete"], scoped_identifiers=[]
[DEBUG] 解析方法调用: object_name=Some("taskService"), method_name=complete
```

#### 解析方法调用目标
```
[DEBUG] 通过字段类型解析: obj=taskService, class_type=com.example.TaskService, 
        full_class_name=com.example.TaskService, target=com.example.TaskService::complete
[DEBUG] 添加方法调用: target=com.example.TaskService::complete, line=20
```

#### 提取结果汇总
```
[DEBUG] 方法 finishTask 提取到 1 个调用
[DEBUG]   - com.example.TaskService::complete
```

### 2. 索引构建 (code_index.rs)

#### 索引文件和类
```
[DEBUG] 索引文件: "/path/to/TestComplete.java"
[DEBUG]   索引类: com.example.TaskController, is_interface=false, implements=[]
```

#### 接口实现关系
```
[DEBUG]   索引类: com.example.TaskServiceImpl, is_interface=false, 
        implements=["com.example.TaskService"]
[DEBUG]     记录接口实现: com.example.TaskServiceImpl implements com.example.TaskService
```

#### 索引方法和调用关系
```
[DEBUG] 索引方法: com.example.TaskController::finishTask
[DEBUG]   索引调用关系: com.example.TaskController::finishTask -> com.example.TaskService::complete
```

### 3. 接口调用解析 (code_index.rs)

#### 解析接口调用
```
[DEBUG] 解析接口调用: com.example.TaskService::complete
[DEBUG]   class_name=com.example.TaskService, method_name=complete
[DEBUG] 查找接口实现: interface=com.example.TaskService, 
        implementations=["com.example.TaskServiceImpl"]
[DEBUG]   找到 1 个实现类: ["com.example.TaskServiceImpl"]
[DEBUG]   解析为实现类: com.example.TaskServiceImpl::complete
```

## 常见问题诊断

### 问题1：方法调用没有被解析出来

**检查点：**
1. 查看 "发现方法调用" 日志，确认是否检测到方法调用
2. 检查 identifiers 和 scoped_identifiers 是否正确
3. 查看字段类型映射，确认对象类型是否被正确提取

**可能原因：**
- 字段类型没有被提取（检查 "提取字段声明" 日志）
- 导入映射缺失（检查 "导入映射" 日志）
- 方法调用的 AST 节点结构不符合预期

### 问题2：接口方法调用没有解析为实现类

**检查点：**
1. 查看 "记录接口实现" 日志，确认接口实现关系是否被索引
2. 查看 "查找接口实现" 日志，确认是否找到实现类
3. 检查实现类数量（只有1个实现类时才会自动解析）

**可能原因：**
- 接口实现关系没有被正确解析
- 接口有多个实现类（无法唯一确定）
- 类名或接口名不匹配

### 问题3：对象类型解析失败

**检查点：**
1. 查看 "提取字段类型完成" 日志，确认字段类型映射
2. 检查是否有 "无法解析对象类型" 的警告日志
3. 查看导入映射是否包含所需的类

**可能原因：**
- 字段声明的 AST 节点解析失败
- 本地变量类型提取失败
- 导入语句没有被正确解析

## 测试示例

运行测试示例查看完整的调试日志：

```bash
cd code-impact-analyzer
cargo run --example test_complete_method_debug
```

## 日志级别说明

- **DEBUG**: 详细的调试信息，包括所有中间步骤
- **INFO**: 重要的进度信息和统计数据
- **WARN**: 警告信息，表示可能的问题
- **ERROR**: 错误信息，表示处理失败

## 性能注意事项

调试日志会影响性能，建议：
- 开发和调试时使用 DEBUG 级别
- 生产环境使用 INFO 或 WARN 级别
- 大规模索引时避免使用 DEBUG 级别

## 相关文件

- `src/java_parser.rs`: 方法调用提取和解析
- `src/code_index.rs`: 索引构建和接口解析
- `examples/test_complete_method_debug.rs`: 调试测试示例
