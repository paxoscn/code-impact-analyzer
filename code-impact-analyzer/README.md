# 代码影响分析工具

一个用于分析 Git patch 文件对代码库影响的静态分析工具。通过解析源代码和配置文件，追踪完整的调用链路，包括方法调用、HTTP 接口、数据库访问、消息队列和缓存操作，帮助开发者理解代码变更的影响范围。

## 功能特性

### 核心功能

- **Git Patch 解析**: 解析 Git unified diff 格式的补丁文件，识别变更的文件和方法
- **多语言支持**: 支持 Java 和 Rust 源代码解析，可扩展支持更多语言
- **方法级调用链追溯**: 双向追溯方法的上游调用者和下游被调用者
- **跨服务边界追溯**: 追踪服务间的依赖关系
  - HTTP 接口的提供者和消费者
  - Kafka 消息队列的生产者和消费者
  - 数据库表的读写操作
  - Redis 缓存键的读写操作
- **配置文件解析**: 支持 XML 和 YAML 配置文件，提取接口地址、Topic 名称等
- **影响图可视化**: 生成 DOT、JSON 等格式的影响图，支持图形化展示

### 技术特点

- 使用 tree-sitter 进行高质量的多语言源代码解析
- 基于 petgraph 构建调用图，支持循环检测
- 并行处理大型代码库，提升分析性能
- 解析结果缓存，避免重复解析
- 完善的错误处理和日志记录
- **智能过滤外部库调用**: 自动忽略找不到源代码的外部库方法（如 JDK、标准库等），只追溯项目内部的调用链

## 安装

### 前置要求

- Rust 1.70 或更高版本
- Cargo 包管理器

### 从源码构建

```bash
# 克隆仓库
git clone <repository-url>
cd code-impact-analyzer

# 构建项目
cargo build --release

# 可执行文件位于 target/release/code-impact-analyzer
```

### 安装到系统

```bash
cargo install --path .
```

## 使用方法

### 基本用法

```bash
code-impact-analyzer --workspace <工作空间路径> --diff <patch目录路径>
cargo run -- --workspace /Users/lindagao/Workspace/javadiff/examples/single-call --diff /Users/lindagao/Workspace/javadiff/examples/single-call/patches
cargo run -- --workspace /Users/lindagao/Workspace/test/ws --diff /Users/lindagao/Workspace/javadiff/examples/single-call/patches
```

### 命令行参数

- `--workspace <PATH>`: 包含多个项目源代码的工作空间根目录（必需）
- `--diff <PATH>`: Git patch 文件目录路径，包含以项目命名的多个 .patch 文件（必需）
  - 目录中的每个 .patch 文件应以对应的项目名命名，例如 `project_a.patch` 对应 workspace 中的 `project_a` 项目
  - 工具会自动扫描目录中的所有 .patch 文件并逐个解析
  - 也支持传入单个 .patch 文件路径以保持向后兼容
- `--output-format <FORMAT>`: 输出格式，可选值：`dot`（默认）、`json`、`mermaid`
- `--max-depth <N>`: 追溯的最大深度，默认为 10
- `--log-level <LEVEL>`: 日志级别，可选值：`debug`、`info`（默认）、`warn`、`error`
- `--output <PATH>`: 输出文件路径，默认输出到标准输出

### 使用示例

#### 示例 1: 分析 patch 目录中的所有文件

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --output impact-graph.dot
```

假设 workspace 结构如下：
```
workspace/
├── project_a/
├── project_b/
└── project_c/
```

patches 目录结构如下：
```
patches/
├── project_a.patch
├── project_b.patch
└── project_c.patch
```

工具会自动解析所有 .patch 文件，并分析它们对整个 workspace 的影响。

#### 示例 2: 分析单个 patch 文件（向后兼容）

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/changes.patch \
  --output impact-graph.dot
```

#### 示例 3: 生成 JSON 格式输出

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --output-format json \
  --output impact-graph.json
```

#### 示例 4: 限制追溯深度并启用详细日志

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --max-depth 5 \
  --log-level debug
```

#### 示例 5: 生成 Mermaid 格式用于文档

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --output-format mermaid \
  --output impact-graph.mmd
```

## 输出格式

### DOT 格式

DOT 是 Graphviz 的图描述语言，可以使用 Graphviz 工具渲染为图像。

**节点类型**:
- 方法节点: 矩形，标注完全限定名
- HTTP 接口: 圆角矩形，标注 HTTP 方法和路径
- Kafka Topic: 菱形，标注 Topic 名称
- 数据库表: 圆柱形，标注表名
- Redis 键: 椭圆形，标注键前缀

**边类型**:
- 实线箭头: 方法调用
- 虚线箭头: HTTP 调用
- 点线箭头: 消息队列
- 双线箭头: 数据库读写
- 波浪线箭头: Redis 读写

**可视化示例**:

```bash
# 使用 Graphviz 渲染为 PNG 图像
dot -Tpng impact-graph.dot -o impact-graph.png

# 渲染为 SVG 矢量图
dot -Tsvg impact-graph.dot -o impact-graph.svg

# 渲染为 PDF
dot -Tpdf impact-graph.dot -o impact-graph.pdf
```

### JSON 格式

JSON 格式输出包含完整的图结构，便于程序化处理和自定义可视化。

**结构示例**:

```json
{
  "nodes": [
    {
      "id": "com.example.UserService::getUser",
      "type": "Method",
      "metadata": {
        "file": "src/main/java/com/example/UserService.java",
        "line_start": 42,
        "line_end": 58
      }
    },
    {
      "id": "GET:/api/users/{id}",
      "type": "HttpEndpoint",
      "metadata": {
        "method": "GET",
        "path": "/api/users/{id}"
      }
    }
  ],
  "edges": [
    {
      "from": "com.example.UserController::getUser",
      "to": "com.example.UserService::getUser",
      "type": "MethodCall",
      "direction": "Downstream"
    }
  ],
  "cycles": [
    ["methodA", "methodB", "methodC", "methodA"]
  ],
  "statistics": {
    "total_nodes": 156,
    "total_edges": 234,
    "methods": 120,
    "http_endpoints": 15,
    "kafka_topics": 8,
    "db_tables": 10,
    "redis_keys": 3
  }
}
```

### Mermaid 格式

Mermaid 是一种基于文本的图表语言，可以在 Markdown 文档中直接渲染。

**示例输出**:

```mermaid
graph TD
    A[UserController::getUser] --> B[UserService::getUser]
    B --> C[UserRepository::findById]
    C --> D[(users table)]
    B --> E{{user-cache:*}}
    A --> F[/GET:/api/users/{id}/]
```

**在 Markdown 中使用**:

将 Mermaid 输出直接嵌入到 Markdown 文档中，GitHub、GitLab 等平台会自动渲染。

## 工作空间和 Patch 目录结构

### 工作空间结构

工具期望的工作空间结构：

```
workspace/
├── project-a/
│   ├── src/
│   │   └── main/
│   │       └── java/
│   │           └── com/
│   │               └── example/
│   │                   └── ServiceA.java
│   └── config/
│       └── application.yml
├── project-b/
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
└── project-c/
    └── ...
```

### Patch 目录结构

Patch 目录应包含以项目命名的 .patch 文件：

```
patches/
├── project-a.patch    # 对 project-a 的修改
├── project-b.patch    # 对 project-b 的修改
└── project-c.patch    # 对 project-c 的修改
```

每个 .patch 文件应该是标准的 Git unified diff 格式，包含对应项目的所有变更。

**重要**: patch 文件名（去掉 .patch 扩展名）会被用作项目目录前缀。例如：
- `project-a.patch` 中的文件路径 `src/ServiceA.java` 会被解析为 `project-a/src/ServiceA.java`
- `project-b.patch` 中的文件路径 `src/lib.rs` 会被解析为 `project-b/src/lib.rs`

这样可以确保工具能够在 workspace 中正确定位文件。

### Patch 文件内容示例

`project-a.patch` 内容：
```diff
diff --git a/src/ServiceA.java b/src/ServiceA.java
index 1234567..abcdefg 100644
--- a/src/ServiceA.java
+++ b/src/ServiceA.java
@@ -10,7 +10,8 @@ public class ServiceA {
     }
     
     public void processData(String data) {
-        System.out.println("Processing: " + data);
+        System.out.println("Processing data: " + data);
+        validateData(data);
     }
 }
```

工具会自动将文件路径 `src/ServiceA.java` 转换为 `project-a/src/ServiceA.java`，然后在 workspace 中查找该文件。

### 分析流程

工具会自动：
1. 扫描 patch 目录中的所有 .patch 文件
2. 从文件名提取项目名（去掉 .patch 扩展名）
3. 逐个解析每个 patch 文件，提取文件变更信息
4. 为每个文件路径添加项目名前缀
5. 遍历工作空间下的所有项目
6. 根据文件扩展名识别语言类型
7. 解析源代码和配置文件
8. 构建全局调用图和资源索引
9. 追溯所有变更的影响范围

## 支持的框架和库

### Java

- **HTTP 框架**: Spring Boot (`@RestController`, `@GetMapping`, `@PostMapping` 等)
- **HTTP 客户端**: `RestTemplate`, `HttpClient`, `WebClient`
- **Kafka**: `KafkaProducer`, `KafkaTemplate`, `@KafkaListener`
- **数据库**: JPA (`@Entity`, `@Table`), JDBC, MyBatis
- **Redis**: `RedisTemplate`

### Rust

- **HTTP 框架**: Axum (`Router::route`)
- **HTTP 客户端**: `reqwest`, `hyper`
- **Kafka**: `rdkafka` (`FutureProducer`, `StreamConsumer`)
- **数据库**: Diesel ORM, `sqlx`
- **Redis**: `redis` crate (`Commands` trait)

## 配置文件支持

### XML 配置

支持解析 Spring XML 配置、MyBatis 配置等：

```xml
<configuration>
  <kafka>
    <topic>user-events</topic>
  </kafka>
  <database>
    <table>users</table>
  </database>
</configuration>
```

### YAML 配置

支持解析 Spring Boot application.yml 等：

```yaml
kafka:
  topics:
    - user-events
    - order-events
database:
  tables:
    - users
    - orders
```

## 高级功能

### 外部库调用过滤

工具会自动识别并忽略对外部库的方法调用，只追溯项目内部的代码。这包括：

- **Java**: JDK 标准库（如 `System.out.println`、`String.format`、`Objects.requireNonNull` 等）
- **Rust**: 标准库和第三方 crate（如 `println!`、`format!`、`assert!` 等）

**工作原理**:
- 工具只索引工作空间内的源代码文件
- 在追溯调用链时，如果被调用的方法不在索引中，则自动跳过
- 这样可以避免影响图中出现大量无关的外部库节点，保持图的清晰和可读性

**示例**:

假设有以下代码：
```java
public void processData(String data) {
    validateData(data);           // 内部方法，会被追溯
    System.out.println(data);     // 外部库，自动忽略
    String.format("Data: %s", data); // 外部库，自动忽略
}

public void validateData(String data) {
    Objects.requireNonNull(data); // 外部库，自动忽略
}
```

影响图中只会包含 `processData` 和 `validateData` 两个节点，以及它们之间的调用关系。

### 循环依赖检测

工具会自动检测调用链中的循环依赖，并在输出中标记：

```
检测到循环依赖:
  ServiceA::methodX -> ServiceB::methodY -> ServiceC::methodZ -> ServiceA::methodX
```

### 深度限制

使用 `--max-depth` 参数限制追溯深度，避免在大型代码库中追溯过深：

```bash
# 只追溯 3 层调用关系
code-impact-analyzer --workspace . --diff changes.patch --max-depth 3
```

### 并行处理

工具自动使用多核并行处理，加速大型代码库的分析。可以通过环境变量控制线程数：

```bash
# 使用 4 个线程
RAYON_NUM_THREADS=4 code-impact-analyzer --workspace . --diff changes.patch
```

## 性能优化

### 解析缓存

工具会缓存已解析的文件，避免重复解析。缓存在内存中，分析完成后自动释放。

### 大型代码库处理

对于超过 10000 个文件的大型代码库：
- 使用并行处理加速解析
- 限制追溯深度减少计算量
- 使用流式处理避免内存溢出

## 错误处理

### 常见错误

**文件不存在**:
```
Error: Patch file not found: /path/to/changes.patch
```

**工作空间路径无效**:
```
Error: Workspace directory does not exist: /path/to/workspace
```

**解析失败**:
```
Warning: Failed to parse file: src/main/java/Example.java
Reason: Syntax error at line 42, column 15
```

### 日志级别

使用 `--log-level` 控制日志详细程度：

- `error`: 只显示致命错误
- `warn`: 显示警告和错误（推荐）
- `info`: 显示主要阶段信息（默认）
- `debug`: 显示详细的解析和追溯过程

## 输出统计信息

分析完成后，工具会输出统计信息：

```
分析完成！
处理的文件数: 1,234
识别的方法数: 5,678
追溯的链路数: 890
HTTP 接口: 45
Kafka Topics: 12
数据库表: 23
Redis 键前缀: 8
警告数: 3
耗时: 12.5 秒
```

## 故障排除

### 问题: 解析失败过多

**原因**: 代码包含语法错误或使用了不支持的语言特性

**解决方案**:
1. 检查日志中的具体错误信息
2. 确保代码可以正常编译
3. 使用 `--log-level debug` 查看详细信息

### 问题: 追溯结果不完整

**原因**: 深度限制过小或存在动态调用

**解决方案**:
1. 增加 `--max-depth` 参数值
2. 检查是否存在反射调用或动态代理（工具无法追溯）
3. 确保所有相关项目都在工作空间中

### 问题: 分析速度慢

**原因**: 代码库过大或追溯深度过深

**解决方案**:
1. 减小 `--max-depth` 参数值
2. 增加并行线程数 `RAYON_NUM_THREADS`
3. 只分析必要的项目子集

## 扩展开发

### 添加新语言支持

实现 `LanguageParser` trait：

```rust
pub trait LanguageParser: Send + Sync {
    fn language_name(&self) -> &str;
    fn file_extensions(&self) -> &[&str];
    fn parse_file(&self, content: &str, file_path: &Path) 
        -> Result<ParsedFile, ParseError>;
}
```

### 添加新的追溯类型

扩展 `NodeType` 和 `EdgeType` 枚举，实现相应的追溯逻辑。

## 许可证

[待定]

## 贡献

欢迎提交 Issue 和 Pull Request！

## 联系方式

[待定]
