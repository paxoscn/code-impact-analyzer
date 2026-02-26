# 需求文档

## 简介

代码影响分析工具是一个用于分析 Git patch 文件对代码库影响的系统。该工具通过解析源代码和配置文件，追踪完整的调用链路，包括方法调用、HTTP 接口、数据库访问、消息队列和缓存操作，帮助开发者理解代码变更的影响范围。

## 术语表

- **System**: 代码影响分析工具
- **Workspace**: 包含多个项目源代码的根目录
- **Patch_File**: Git diff 格式的补丁文件，描述代码变更
- **Call_Chain**: 方法之间的调用关系链路
- **Upstream_Method**: 调用当前方法的方法
- **Downstream_Method**: 被当前方法调用的方法
- **HTTP_Interface**: RESTful API 端点或 HTTP 服务接口
- **Kafka_Topic**: Apache Kafka 消息队列的主题
- **Database_Table**: 关系型数据库中的表
- **Redis_Key_Prefix**: Redis 缓存键的前缀模式
- **Impact_Graph**: 展示代码变更影响范围的可视化图形
- **Language_Parser**: 针对特定编程语言的源代码解析器
- **Cross_Service_Trace**: 跨服务边界的调用追溯

## 需求

### 需求 1: 解析 Git Patch 文件

**用户故事:** 作为开发者，我想要解析 Git patch 文件，以便识别代码变更涉及的文件和方法。

#### 验收标准

1. WHEN 提供一个有效的 Git patch 文件路径，THE System SHALL 解析该文件并提取所有修改的文件路径
2. WHEN 解析 patch 文件，THE System SHALL 识别每个文件中被修改、添加或删除的方法
3. WHEN patch 文件格式无效，THE System SHALL 返回描述性错误信息
4. WHEN patch 文件包含二进制文件变更，THE System SHALL 跳过这些文件并记录警告
5. THE System SHALL 支持标准 Git unified diff 格式

### 需求 2: 多语言源代码解析

**用户故事:** 作为开发者，我想要系统能够解析不同编程语言的源代码，以便分析多语言项目的影响。

#### 验收标准

1. WHEN 分析 Java 源文件，THE System SHALL 提取类名、方法名和方法内的调用关系
2. WHEN 分析 Rust 源文件，THE System SHALL 提取模块、函数和函数内的调用关系
3. WHEN 遇到不支持的编程语言，THE System SHALL 记录警告并跳过该文件
4. THE System SHALL 识别文件的编程语言类型（基于文件扩展名或内容）
5. WHEN 源文件包含语法错误，THE System SHALL 尝试部分解析并报告错误位置

### 需求 3: 方法级调用链追溯

**用户故事:** 作为开发者，我想要追溯被修改方法的上游和下游调用链，以便了解方法变更的完整影响范围。

#### 验收标准

1. WHEN 识别到被修改的方法，THE System SHALL 查找所有直接调用该方法的上游方法
2. WHEN 识别到被修改的方法，THE System SHALL 查找该方法直接调用的所有下游方法
3. WHEN 追溯上游方法，THE System SHALL 递归查找上游方法的上游方法
4. WHEN 追溯下游方法，THE System SHALL 递归查找下游方法的下游方法
5. WHEN 检测到循环调用，THE System SHALL 终止该分支的递归追溯并标记循环
6. THE System SHALL 限制追溯深度以防止无限递归（可配置的最大深度）

### 需求 4: HTTP 接口跨服务追溯

**用户故事:** 作为开发者，我想要追溯 HTTP 接口的提供者和消费者，以便了解服务间的依赖关系。

#### 验收标准

1. WHEN 被修改的方法是 HTTP 接口提供者，THE System SHALL 识别该接口的 URL 路径和 HTTP 方法
2. WHEN 识别到 HTTP 接口提供者，THE System SHALL 在所有项目中搜索调用该接口的 HTTP 客户端代码
3. WHEN 被修改的方法包含 HTTP 客户端调用，THE System SHALL 识别目标 URL 并查找提供该接口的服务
4. WHEN HTTP 接口使用路径参数或查询参数，THE System SHALL 匹配接口模式而非精确 URL
5. THE System SHALL 支持常见的 HTTP 框架注解识别（如 Spring @RestController、Axum 路由等）

### 需求 5: Kafka 消息队列追溯

**用户故事:** 作为开发者，我想要追溯 Kafka 消息的生产者和消费者，以便了解异步消息流的影响。

#### 验收标准

1. WHEN 被修改的方法向 Kafka Topic 生产消息，THE System SHALL 识别该 Topic 名称
2. WHEN 识别到 Kafka 生产者，THE System SHALL 在所有项目中查找消费该 Topic 的消费者
3. WHEN 被修改的方法消费 Kafka Topic，THE System SHALL 识别该 Topic 名称
4. WHEN 识别到 Kafka 消费者，THE System SHALL 在所有项目中查找向该 Topic 生产消息的生产者
5. THE System SHALL 支持从配置文件和代码中提取 Topic 名称

### 需求 6: 数据库表访问追溯

**用户故事:** 作为开发者，我想要追溯数据库表的读写操作，以便了解数据依赖关系。

#### 验收标准

1. WHEN 被修改的方法写入数据库表，THE System SHALL 识别表名和写入操作类型（INSERT、UPDATE、DELETE）
2. WHEN 识别到数据库写入操作，THE System SHALL 在所有项目中查找读取该表的方法
3. WHEN 被修改的方法读取数据库表，THE System SHALL 识别表名和读取操作类型（SELECT）
4. WHEN 识别到数据库读取操作，THE System SHALL 在所有项目中查找写入该表的方法
5. THE System SHALL 支持从 SQL 语句、ORM 代码和配置文件中提取表名

### 需求 7: Redis 缓存访问追溯

**用户故事:** 作为开发者，我想要追溯 Redis 键的读写操作，以便了解缓存依赖关系。

#### 验收标准

1. WHEN 被修改的方法写入 Redis 键，THE System SHALL 识别键名或键前缀
2. WHEN 识别到 Redis 写入操作，THE System SHALL 在所有项目中查找读取该键或前缀的方法
3. WHEN 被修改的方法读取 Redis 键，THE System SHALL 识别键名或键前缀
4. WHEN 识别到 Redis 读取操作，THE System SHALL 在所有项目中查找写入该键或前缀的方法
5. THE System SHALL 支持键前缀模式匹配（如 "user:*" 匹配所有以 "user:" 开头的键）

### 需求 8: 通用搜索能力

**用户故事:** 作为开发者，我想要搜索代码库中的各种元素，以便快速定位相关代码。

#### 验收标准

1. WHEN 提供项目名称，THE System SHALL 返回该项目的所有源文件路径
2. WHEN 提供类名或方法名，THE System SHALL 返回所有匹配的定义位置
3. WHEN 搜索 HTTP 接口，THE System SHALL 返回所有匹配的接口提供者和消费者
4. WHEN 搜索 Kafka Topic，THE System SHALL 返回所有生产者和消费者
5. WHEN 搜索数据库表，THE System SHALL 返回所有读写该表的方法
6. WHEN 搜索 Redis 键前缀，THE System SHALL 返回所有读写该前缀的方法
7. THE System SHALL 支持模糊匹配和正则表达式搜索

### 需求 9: 影响图可视化

**用户故事:** 作为开发者，我想要以图形方式查看代码变更的影响范围，以便直观理解影响链路。

#### 验收标准

1. WHEN 完成影响分析，THE System SHALL 生成包含所有影响节点和边的图结构
2. WHEN 生成影响图，THE System SHALL 区分不同类型的节点（方法、HTTP 接口、Kafka Topic、数据库表、Redis 键）
3. WHEN 生成影响图，THE System SHALL 标记边的方向（上游或下游）和类型（方法调用、HTTP 调用、消息队列等）
4. THE System SHALL 输出可被可视化工具渲染的图格式（如 DOT、JSON 或 Mermaid）
5. WHEN 影响图包含循环依赖，THE System SHALL 在图中明确标记循环

### 需求 10: 配置文件解析

**用户故事:** 作为开发者，我想要系统能够解析配置文件，以便识别从配置中读取的接口地址、Topic 名称等信息。

#### 验收标准

1. WHEN 项目包含 XML 配置文件，THE System SHALL 解析并提取相关配置项
2. WHEN 项目包含 YAML 配置文件，THE System SHALL 解析并提取相关配置项
3. WHEN 配置文件包含 HTTP 接口地址，THE System SHALL 提取并关联到相关代码
4. WHEN 配置文件包含 Kafka Topic 名称，THE System SHALL 提取并关联到相关代码
5. WHEN 配置文件包含数据库连接信息，THE System SHALL 提取表名和连接配置

### 需求 11: 扩展性架构

**用户故事:** 作为开发者，我想要系统具有良好的扩展性，以便未来添加新的语言支持和追溯逻辑。

#### 验收标准

1. THE System SHALL 提供语言解析器的插件接口
2. THE System SHALL 提供跨服务追溯逻辑的扩展点
3. WHEN 添加新的语言解析器，THE System SHALL 无需修改核心追溯逻辑
4. WHEN 添加新的追溯类型（如新的中间件），THE System SHALL 通过扩展接口实现
5. THE System SHALL 将语言解析、调用分析和图生成解耦为独立模块

### 需求 12: 错误处理和日志

**用户故事:** 作为开发者，我想要系统提供清晰的错误信息和日志，以便调试和监控分析过程。

#### 验收标准

1. WHEN 输入路径不存在，THE System SHALL 返回明确的错误信息
2. WHEN 解析失败，THE System SHALL 记录失败的文件路径和错误原因
3. THE System SHALL 提供详细的日志级别（DEBUG、INFO、WARN、ERROR）
4. WHEN 分析过程中遇到警告，THE System SHALL 继续处理并在最后汇总所有警告
5. THE System SHALL 在分析完成后输出统计信息（处理的文件数、识别的方法数、追溯的链路数等）

### 需求 13: 性能和可扩展性

**用户故事:** 作为开发者，我想要系统能够高效处理大型代码库，以便在实际项目中使用。

#### 验收标准

1. THE System SHALL 支持并行解析多个源文件
2. THE System SHALL 缓存解析结果以避免重复解析
3. WHEN 处理大型代码库（超过 10000 个文件），THE System SHALL 在合理时间内完成分析（少于 5 分钟）
4. THE System SHALL 提供进度指示器显示分析进度
5. WHEN 内存使用超过阈值，THE System SHALL 采用流式处理或分批处理策略
