# 设计文档

## 概述

代码影响分析工具是一个静态分析系统，用于追踪 Git patch 文件中代码变更的完整影响链路。系统采用模块化架构，支持多语言解析、跨服务追溯和图形化可视化。

核心工作流程：
1. 解析 Git patch 文件，识别变更的文件和方法
2. 解析 workspace 中的所有源代码，构建调用图
3. 从变更点开始，双向追溯调用链（上游和下游）
4. 跨服务边界追溯（HTTP、Kafka、数据库、Redis）
5. 生成影响图并输出为可视化格式

## 架构

系统采用分层架构，主要模块包括：

```
┌─────────────────────────────────────────────────────────┐
│                     CLI Interface                        │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                   Analysis Orchestrator                  │
│  - 协调整个分析流程                                        │
│  - 管理解析器和追溯器                                      │
└─────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
┌───────▼────────┐ ┌──────▼──────┐ ┌───────▼────────┐
│  Patch Parser  │ │   Language  │ │ Config Parser  │
│                │ │   Parsers   │ │                │
│ - Git diff     │ │ - Java      │ │ - XML          │
│   解析         │ │ - Rust      │ │ - YAML         │
└────────────────┘ │ - 扩展接口  │ └────────────────┘
                   └─────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                   Code Index Builder                     │
│  - 构建全局符号表                                          │
│  - 方法调用关系索引                                        │
│  - 跨服务资源索引（HTTP/Kafka/DB/Redis）                  │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                   Impact Tracer                          │
│  - 方法级调用链追溯                                        │
│  - 跨服务边界追溯                                          │
│  - 循环检测                                               │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                   Graph Generator                        │
│  - 构建影响图                                             │
│  - DOT 格式输出                                           │
│  - 节点和边的类型标注                                      │
└─────────────────────────────────────────────────────────┘
```

### 关键设计决策

1. **使用 tree-sitter 进行多语言解析**: tree-sitter 提供统一的解析接口和高质量的语言语法，支持增量解析
2. **使用 petgraph 构建调用图**: petgraph 是 Rust 生态中成熟的图库，支持 DOT 格式输出
3. **使用 gitpatch 解析 patch 文件**: 专门用于解析 Git unified diff 格式
4. **插件化语言解析器**: 通过 trait 定义统一接口，便于扩展新语言

## 组件和接口

### 1. Patch Parser 模块

**职责**: 解析 Git patch 文件，提取变更信息

**接口**:
```rust
pub struct PatchParser;

pub struct FileChange {
    pub file_path: String,
    pub change_type: ChangeType,
    pub hunks: Vec<Hunk>,
}

pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<HunkLine>,
}

impl PatchParser {
    pub fn parse_patch_file(path: &Path) -> Result<Vec<FileChange>, ParseError>;
    pub fn extract_modified_methods(
        &self,
        file_change: &FileChange,
        source_content: &str,
        language: Language,
    ) -> Result<Vec<MethodLocation>, ParseError>;
}
```

**依赖**: gitpatch crate

### 2. Language Parser 模块

**职责**: 提供统一的多语言源代码解析接口

**接口**:
```rust
pub trait LanguageParser: Send + Sync {
    fn language_name(&self) -> &str;
    fn file_extensions(&self) -> &[&str];
    fn parse_file(&self, content: &str, file_path: &Path) -> Result<ParsedFile, ParseError>;
}

pub struct ParsedFile {
    pub file_path: PathBuf,
    pub language: String,
    pub classes: Vec<ClassInfo>,
    pub functions: Vec<FunctionInfo>,
    pub imports: Vec<Import>,
}

pub struct ClassInfo {
    pub name: String,
    pub methods: Vec<MethodInfo>,
    pub line_range: (usize, usize),
}

pub struct MethodInfo {
    pub name: String,
    pub full_qualified_name: String,
    pub line_range: (usize, usize),
    pub calls: Vec<MethodCall>,
    pub http_annotations: Option<HttpAnnotation>,
    pub kafka_operations: Vec<KafkaOperation>,
    pub db_operations: Vec<DbOperation>,
    pub redis_operations: Vec<RedisOperation>,
}

pub struct MethodCall {
    pub target: String,
    pub line: usize,
}
```

**实现**:
- `JavaParser`: 使用 tree-sitter-java
- `RustParser`: 使用 tree-sitter-rust
- 扩展点：实现 `LanguageParser` trait 添加新语言

### 3. Config Parser 模块

**职责**: 解析 XML 和 YAML 配置文件，提取资源配置

**接口**:
```rust
pub trait ConfigParser: Send + Sync {
    fn parse(&self, content: &str) -> Result<ConfigData, ParseError>;
}

pub struct ConfigData {
    pub http_endpoints: Vec<HttpEndpoint>,
    pub kafka_topics: Vec<String>,
    pub db_tables: Vec<String>,
    pub redis_prefixes: Vec<String>,
}

pub struct XmlConfigParser;
pub struct YamlConfigParser;
```

**依赖**: 
- quick-xml crate (XML 解析)
- serde_yaml crate (YAML 解析)

### 4. Code Index Builder 模块

**职责**: 构建全局代码索引，支持快速查询

**接口**:
```rust
pub struct CodeIndex {
    methods: HashMap<String, MethodInfo>,
    method_calls: HashMap<String, Vec<String>>,
    reverse_calls: HashMap<String, Vec<String>>,
    http_providers: HashMap<HttpEndpoint, String>,
    http_consumers: HashMap<HttpEndpoint, Vec<String>>,
    kafka_producers: HashMap<String, Vec<String>>,
    kafka_consumers: HashMap<String, Vec<String>>,
    db_writers: HashMap<String, Vec<String>>,
    db_readers: HashMap<String, Vec<String>>,
    redis_writers: HashMap<String, Vec<String>>,
    redis_readers: HashMap<String, Vec<String>>,
}

impl CodeIndex {
    pub fn new() -> Self;
    pub fn index_workspace(&mut self, workspace_path: &Path, parsers: &[Box<dyn LanguageParser>]) -> Result<(), IndexError>;
    pub fn find_method(&self, qualified_name: &str) -> Option<&MethodInfo>;
    pub fn find_callers(&self, method: &str) -> Vec<&str>;
    pub fn find_callees(&self, method: &str) -> Vec<&str>;
    pub fn find_http_consumers(&self, endpoint: &HttpEndpoint) -> Vec<&str>;
    pub fn find_kafka_consumers(&self, topic: &str) -> Vec<&str>;
    pub fn find_db_readers(&self, table: &str) -> Vec<&str>;
    pub fn find_redis_readers(&self, prefix: &str) -> Vec<&str>;
}
```

### 5. Impact Tracer 模块

**职责**: 从变更点追溯完整的影响链路

**接口**:
```rust
pub struct ImpactTracer<'a> {
    index: &'a CodeIndex,
    max_depth: usize,
}

pub struct TraceConfig {
    pub max_depth: usize,
    pub trace_upstream: bool,
    pub trace_downstream: bool,
    pub trace_cross_service: bool,
}

pub struct ImpactNode {
    pub id: String,
    pub node_type: NodeType,
    pub metadata: NodeMetadata,
}

pub enum NodeType {
    Method { qualified_name: String },
    HttpEndpoint { path: String, method: String },
    KafkaTopic { name: String },
    DatabaseTable { name: String },
    RedisPrefix { prefix: String },
}

pub struct ImpactEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub direction: Direction,
}

pub enum EdgeType {
    MethodCall,
    HttpCall,
    KafkaProduceConsume,
    DatabaseReadWrite,
    RedisReadWrite,
}

pub enum Direction {
    Upstream,
    Downstream,
}

impl<'a> ImpactTracer<'a> {
    pub fn new(index: &'a CodeIndex, config: TraceConfig) -> Self;
    pub fn trace_impact(&self, changed_methods: &[String]) -> Result<ImpactGraph, TraceError>;
    fn trace_method_upstream(&self, method: &str, depth: usize, visited: &mut HashSet<String>, graph: &mut ImpactGraph);
    fn trace_method_downstream(&self, method: &str, depth: usize, visited: &mut HashSet<String>, graph: &mut ImpactGraph);
    fn trace_cross_service(&self, node: &ImpactNode, visited: &mut HashSet<String>, graph: &mut ImpactGraph);
}
```

### 6. Graph Generator 模块

**职责**: 生成可视化图形输出

**接口**:
```rust
pub struct ImpactGraph {
    graph: DiGraph<ImpactNode, ImpactEdge>,
    node_map: HashMap<String, NodeIndex>,
}

impl ImpactGraph {
    pub fn new() -> Self;
    pub fn add_node(&mut self, node: ImpactNode) -> NodeIndex;
    pub fn add_edge(&mut self, from: &str, to: &str, edge: ImpactEdge);
    pub fn to_dot(&self) -> String;
    pub fn to_json(&self) -> Result<String, serde_json::Error>;
    pub fn detect_cycles(&self) -> Vec<Vec<String>>;
}
```

**依赖**: petgraph crate

### 7. CLI Interface

**职责**: 提供命令行接口

**接口**:
```rust
pub struct CliArgs {
    pub workspace_path: PathBuf,
    pub diff_path: PathBuf,
    pub output_format: OutputFormat,
    pub max_depth: usize,
    pub log_level: LogLevel,
}

pub enum OutputFormat {
    Dot,
    Json,
    Mermaid,
}

pub fn run(args: CliArgs) -> Result<(), AnalysisError>;
```

**依赖**: clap crate

## 数据模型

### 核心数据结构

```rust
// 方法定位
pub struct MethodLocation {
    pub file_path: PathBuf,
    pub qualified_name: String,
    pub line_start: usize,
    pub line_end: usize,
}

// HTTP 注解
pub struct HttpAnnotation {
    pub method: HttpMethod,
    pub path: String,
    pub path_params: Vec<String>,
}

pub enum HttpMethod {
    GET, POST, PUT, DELETE, PATCH,
}

// HTTP 端点
pub struct HttpEndpoint {
    pub method: HttpMethod,
    pub path_pattern: String,
}

// Kafka 操作
pub struct KafkaOperation {
    pub operation_type: KafkaOpType,
    pub topic: String,
    pub line: usize,
}

pub enum KafkaOpType {
    Produce,
    Consume,
}

// 数据库操作
pub struct DbOperation {
    pub operation_type: DbOpType,
    pub table: String,
    pub line: usize,
}

pub enum DbOpType {
    Select,
    Insert,
    Update,
    Delete,
}

// Redis 操作
pub struct RedisOperation {
    pub operation_type: RedisOpType,
    pub key_pattern: String,
    pub line: usize,
}

pub enum RedisOpType {
    Get,
    Set,
    Delete,
}

// 导入声明
pub struct Import {
    pub module: String,
    pub items: Vec<String>,
}
```

### 索引数据结构

```rust
// 全局索引使用 HashMap 实现 O(1) 查询
// 键设计：
// - 方法: "project_name::package.Class::method"
// - HTTP: "GET:/api/users/{id}"
// - Kafka: "topic:user-events"
// - DB: "table:users"
// - Redis: "prefix:user:*"
```

## 正确性属性

*属性是一个特征或行为，应该在系统的所有有效执行中保持为真——本质上是关于系统应该做什么的形式化陈述。属性作为人类可读规范和机器可验证正确性保证之间的桥梁。*


### Patch 解析属性

**属性 1: Patch 文件路径提取完整性**

*对于任意*有效的 Git patch 文件，解析后提取的文件路径集合应该与 patch 中声明的文件路径集合完全一致

**验证需求: 1.1**

**属性 2: 方法变更识别准确性**

*对于任意*包含方法变更的 patch 文件和对应的源文件，解析后识别的被修改方法应该与 hunk 覆盖的行范围内的方法定义一致

**验证需求: 1.2**

**属性 3: 无效 patch 错误处理**

*对于任意*格式无效的 patch 文件，解析应该返回错误结果而不是崩溃或返回不正确的数据

**验证需求: 1.3**

**属性 4: Patch 解析往返一致性**

*对于任意*有效的 patch 数据结构，将其序列化为 unified diff 格式后再解析，应该得到等价的数据结构

**验证需求: 1.5**

### 语言解析属性

**属性 5: Java 代码结构提取完整性**

*对于任意*有效的 Java 源文件，解析后应该提取所有类定义、方法定义和方法内的调用语句

**验证需求: 2.1**

**属性 6: Rust 代码结构提取完整性**

*对于任意*有效的 Rust 源文件，解析后应该提取所有模块、函数定义和函数内的调用语句

**验证需求: 2.2**

**属性 7: 语言识别准确性**

*对于任意*源文件，基于文件扩展名或内容识别的语言类型应该与该文件的实际语言类型一致

**验证需求: 2.4**

**属性 8: 语法错误容错性**

*对于任意*包含语法错误的源文件，解析器应该返回错误结果并报告错误位置，而不是崩溃

**验证需求: 2.5**

### 调用链追溯属性

**属性 9: 上游方法查找完整性**

*对于任意*方法 M 和调用图 G，查找 M 的上游方法应该返回 G 中所有直接调用 M 的方法

**验证需求: 3.1**

**属性 10: 下游方法查找完整性**

*对于任意*方法 M 和调用图 G，查找 M 的下游方法应该返回 G 中 M 直接调用的所有方法

**验证需求: 3.2**

**属性 11: 递归上游追溯传递性**

*对于任意*方法 M 和调用图 G，如果方法 A 调用方法 B，方法 B 调用方法 M，则递归上游追溯应该同时包含 A 和 B

**验证需求: 3.3**

**属性 12: 递归下游追溯传递性**

*对于任意*方法 M 和调用图 G，如果方法 M 调用方法 A，方法 A 调用方法 B，则递归下游追溯应该同时包含 A 和 B

**验证需求: 3.4**

**属性 13: 循环调用检测终止性**

*对于任意*包含循环的调用图，追溯过程应该检测到循环并终止该分支，不会无限递归

**验证需求: 3.5**

**属性 14: 深度限制有效性**

*对于任意*调用图和最大深度 N，追溯结果中任意路径的长度不应超过 N

**验证需求: 3.6**

### HTTP 跨服务追溯属性

**属性 15: HTTP 接口信息提取准确性**

*对于任意*带有 HTTP 注解的方法，提取的 HTTP 方法和路径应该与注解中声明的一致

**验证需求: 4.1**

**属性 16: HTTP 双向追溯完整性**

*对于任意*HTTP 接口端点，如果服务 A 提供该接口且服务 B 调用该接口，则从 A 追溯应该找到 B，从 B 追溯应该找到 A

**验证需求: 4.2, 4.3**

**属性 17: HTTP 路径模式匹配正确性**

*对于任意*带路径参数的 HTTP 端点模式（如 /api/users/{id}），应该匹配所有符合该模式的具体 URL（如 /api/users/123）

**验证需求: 4.4**

**属性 18: HTTP 框架注解识别覆盖性**

*对于任意*支持的 HTTP 框架注解（Spring、Axum 等），解析器应该正确识别并提取接口信息

**验证需求: 4.5**

### Kafka 追溯属性

**属性 19: Kafka Topic 识别准确性**

*对于任意*包含 Kafka 生产或消费操作的方法，提取的 Topic 名称应该与代码中使用的 Topic 一致

**验证需求: 5.1, 5.3**

**属性 20: Kafka 双向追溯完整性**

*对于任意*Kafka Topic T，如果方法 P 生产消息到 T 且方法 C 消费 T 的消息，则从 P 追溯应该找到 C，从 C 追溯应该找到 P

**验证需求: 5.2, 5.4**

**属性 21: Kafka Topic 多源提取一致性**

*对于任意*Kafka Topic，无论是从代码硬编码还是配置文件中提取，相同的 Topic 名称应该被识别为同一个 Topic

**验证需求: 5.5**

### 数据库追溯属性

**属性 22: 数据库表和操作类型识别准确性**

*对于任意*包含数据库操作的方法，提取的表名和操作类型（SELECT/INSERT/UPDATE/DELETE）应该与代码中的 SQL 语句或 ORM 调用一致

**验证需求: 6.1, 6.3**

**属性 23: 数据库双向追溯完整性**

*对于任意*数据库表 T，如果方法 W 写入 T 且方法 R 读取 T，则从 W 追溯应该找到 R，从 R 追溯应该找到 W

**验证需求: 6.2, 6.4**

**属性 24: 数据库表多源提取一致性**

*对于任意*数据库表，无论是从 SQL 语句、ORM 代码还是配置文件中提取，相同的表名应该被识别为同一个表

**验证需求: 6.5**

### Redis 追溯属性

**属性 25: Redis 键识别准确性**

*对于任意*包含 Redis 操作的方法，提取的键名或键前缀应该与代码中使用的键一致

**验证需求: 7.1, 7.3**

**属性 26: Redis 双向追溯完整性**

*对于任意*Redis 键前缀 P，如果方法 W 写入 P 且方法 R 读取 P，则从 W 追溯应该找到 R，从 R 追溯应该找到 W

**验证需求: 7.2, 7.4**

**属性 27: Redis 键前缀模式匹配正确性**

*对于任意*Redis 键前缀模式（如 "user:*"），应该匹配所有以该前缀开头的具体键（如 "user:123", "user:456"）

**验证需求: 7.5**

### 搜索功能属性

**属性 28: 项目文件搜索完整性**

*对于任意*项目名称，搜索应该返回该项目目录下的所有源文件路径

**验证需求: 8.1**

**属性 29: 符号搜索完整性**

*对于任意*类名或方法名，搜索应该返回代码库中所有匹配该名称的定义位置

**验证需求: 8.2**

**属性 30: HTTP 接口搜索完整性**

*对于任意*HTTP 端点，搜索应该返回所有提供该接口的方法和所有调用该接口的方法

**验证需求: 8.3**

**属性 31: Kafka Topic 搜索完整性**

*对于任意*Kafka Topic，搜索应该返回所有生产该 Topic 的方法和所有消费该 Topic 的方法

**验证需求: 8.4**

**属性 32: 数据库表搜索完整性**

*对于任意*数据库表，搜索应该返回所有读取该表的方法和所有写入该表的方法

**验证需求: 8.5**

**属性 33: Redis 键搜索完整性**

*对于任意*Redis 键前缀，搜索应该返回所有读取该前缀的方法和所有写入该前缀的方法

**验证需求: 8.6**

**属性 34: 搜索模式匹配正确性**

*对于任意*搜索模式（模糊匹配或正则表达式），搜索结果应该只包含匹配该模式的项，且不遗漏任何匹配项

**验证需求: 8.7**

### 图生成属性

**属性 35: 影响图节点完整性**

*对于任意*影响分析结果，生成的图应该包含所有被追溯到的方法、接口、Topic、表和键作为节点

**验证需求: 9.1**

**属性 36: 节点类型标注正确性**

*对于任意*影响图中的节点，其类型标注应该与该节点代表的实体类型一致

**验证需求: 9.2**

**属性 37: 边标注正确性**

*对于任意*影响图中的边，其方向和类型标注应该与该边代表的调用关系一致

**验证需求: 9.3**

**属性 38: 图格式输出往返一致性**

*对于任意*影响图，将其输出为 DOT 或 JSON 格式后再解析，应该得到等价的图结构

**验证需求: 9.4**

**属性 39: 循环依赖标记正确性**

*对于任意*包含循环的影响图，所有循环路径应该被正确识别并在图中标记

**验证需求: 9.5**

### 配置解析属性

**属性 40: XML 配置提取准确性**

*对于任意*有效的 XML 配置文件，提取的配置项应该与 XML 中声明的配置项一致

**验证需求: 10.1**

**属性 41: YAML 配置提取准确性**

*对于任意*有效的 YAML 配置文件，提取的配置项应该与 YAML 中声明的配置项一致

**验证需求: 10.2**

**属性 42: HTTP 配置关联正确性**

*对于任意*在配置文件中声明的 HTTP 接口地址，如果代码中使用了该配置，则应该建立正确的关联关系

**验证需求: 10.3**

**属性 43: Kafka 配置关联正确性**

*对于任意*在配置文件中声明的 Kafka Topic，如果代码中使用了该配置，则应该建立正确的关联关系

**验证需求: 10.4**

**属性 44: 数据库配置提取准确性**

*对于任意*包含数据库连接信息的配置文件，提取的表名和连接配置应该与配置文件中声明的一致

**验证需求: 10.5**

### 错误处理和统计属性

**属性 45: 解析失败记录完整性**

*对于任意*导致解析失败的文件，系统应该记录该文件的路径和具体错误原因

**验证需求: 12.2**

**属性 46: 警告汇总完整性**

*对于任意*分析过程，如果产生了 N 个警告，则最终汇总应该包含所有 N 个警告且系统继续完成分析

**验证需求: 12.4**

**属性 47: 统计信息准确性**

*对于任意*代码库分析结果，输出的统计信息（文件数、方法数、链路数）应该与实际处理和识别的数量一致

**验证需求: 12.5**

### 性能属性

**属性 48: 缓存一致性**

*对于任意*源文件，第一次解析和第二次从缓存读取应该返回完全相同的解析结果

**验证需求: 13.2**

## 错误处理

### 错误类型

系统定义以下错误类型：

```rust
pub enum AnalysisError {
    PatchParseError(ParseError),
    LanguageParseError { file: PathBuf, error: ParseError },
    ConfigParseError { file: PathBuf, error: ParseError },
    IndexBuildError(IndexError),
    TraceError(TraceError),
    IoError(std::io::Error),
}

pub enum ParseError {
    InvalidFormat { message: String },
    SyntaxError { line: usize, column: usize, message: String },
    UnsupportedLanguage { language: String },
    BinaryFile { path: PathBuf },
}

pub enum IndexError {
    DuplicateSymbol { symbol: String },
    InvalidReference { from: String, to: String },
}

pub enum TraceError {
    MethodNotFound { method: String },
    MaxDepthExceeded { depth: usize },
    CyclicDependency { cycle: Vec<String> },
}
```

### 错误处理策略

1. **解析错误**: 记录错误并继续处理其他文件
2. **索引错误**: 记录警告，允许重复符号（不同项目可能有同名类）
3. **追溯错误**: 记录警告，继续追溯其他分支
4. **IO 错误**: 立即返回错误，终止分析

### 日志策略

- **DEBUG**: 详细的解析和追溯过程
- **INFO**: 主要阶段完成（解析完成、索引构建完成等）
- **WARN**: 非致命错误（解析失败、找不到方法等）
- **ERROR**: 致命错误（IO 错误、配置错误等）

## 测试策略

### 单元测试

单元测试专注于具体示例和边缘情况：

1. **Patch 解析器测试**
   - 测试标准 Git diff 格式解析
   - 测试多文件 patch
   - 测试二进制文件处理
   - 测试无效格式错误处理

2. **语言解析器测试**
   - 测试基本类和方法提取
   - 测试嵌套类和内部类
   - 测试泛型和注解
   - 测试语法错误处理

3. **配置解析器测试**
   - 测试标准 XML/YAML 格式
   - 测试嵌套配置结构
   - 测试无效格式处理

4. **索引构建器测试**
   - 测试单个项目索引
   - 测试多项目索引
   - 测试符号冲突处理

5. **追溯器测试**
   - 测试简单调用链
   - 测试跨服务追溯
   - 测试循环检测
   - 测试深度限制

6. **图生成器测试**
   - 测试 DOT 格式输出
   - 测试 JSON 格式输出
   - 测试循环标记

### 属性测试

属性测试验证系统的通用正确性属性，每个测试运行至少 100 次迭代：

1. **Patch 解析属性测试**
   - 属性 1-4: 使用随机生成的 patch 文件
   - 标签: **Feature: code-impact-analyzer, Property 1-4**

2. **语言解析属性测试**
   - 属性 5-8: 使用随机生成的源代码
   - 标签: **Feature: code-impact-analyzer, Property 5-8**

3. **调用链追溯属性测试**
   - 属性 9-14: 使用随机生成的调用图
   - 标签: **Feature: code-impact-analyzer, Property 9-14**

4. **跨服务追溯属性测试**
   - 属性 15-27: 使用随机生成的多服务代码库
   - 标签: **Feature: code-impact-analyzer, Property 15-27**

5. **搜索功能属性测试**
   - 属性 28-34: 使用随机生成的代码库和搜索查询
   - 标签: **Feature: code-impact-analyzer, Property 28-34**

6. **图生成属性测试**
   - 属性 35-39: 使用随机生成的影响分析结果
   - 标签: **Feature: code-impact-analyzer, Property 35-39**

7. **配置解析属性测试**
   - 属性 40-44: 使用随机生成的配置文件
   - 标签: **Feature: code-impact-analyzer, Property 40-44**

8. **错误处理属性测试**
   - 属性 45-47: 使用随机生成的错误场景
   - 标签: **Feature: code-impact-analyzer, Property 45-47**

9. **性能属性测试**
   - 属性 48: 使用随机生成的源文件
   - 标签: **Feature: code-impact-analyzer, Property 48**

### 测试工具

- **属性测试框架**: proptest crate (Rust 的属性测试库)
- **单元测试框架**: Rust 内置 test 框架
- **测试配置**: 每个属性测试至少 100 次迭代
- **随机数据生成**: proptest 的 Arbitrary trait 实现

### 集成测试

1. **端到端测试**: 使用真实的开源项目作为测试数据
2. **性能测试**: 测试大型代码库（10000+ 文件）的处理时间
3. **跨语言测试**: 测试包含 Java 和 Rust 混合项目的分析

## 实现细节

### Java 解析实现

使用 tree-sitter-java 解析 Java 源代码：

1. **类提取**: 查询 `class_declaration` 节点
2. **方法提取**: 查询 `method_declaration` 节点
3. **调用提取**: 查询 `method_invocation` 节点
4. **注解提取**: 查询 `annotation` 节点，识别 Spring 注解（@RestController, @GetMapping 等）
5. **SQL 提取**: 查询字符串字面量，使用正则表达式匹配 SQL 语句

### Rust 解析实现

使用 tree-sitter-rust 解析 Rust 源代码：

1. **函数提取**: 查询 `function_item` 节点
2. **调用提取**: 查询 `call_expression` 节点
3. **宏调用提取**: 查询 `macro_invocation` 节点（用于识别 Axum 路由宏等）
4. **模块提取**: 查询 `mod_item` 节点

### HTTP 接口识别策略

**Java (Spring Framework)**:
- `@RestController` + `@RequestMapping` / `@GetMapping` / `@PostMapping` 等
- 提取路径和 HTTP 方法

**Rust (Axum)**:
- `Router::new().route("/path", get(handler))` 模式
- 提取路径和 HTTP 方法

**HTTP 客户端识别**:
- Java: `RestTemplate`, `HttpClient`, `WebClient`
- Rust: `reqwest`, `hyper`

### Kafka 操作识别策略

**生产者识别**:
- Java: `KafkaProducer.send()`, `KafkaTemplate.send()`
- Rust: `FutureProducer.send()`

**消费者识别**:
- Java: `@KafkaListener` 注解, `KafkaConsumer.poll()`
- Rust: `StreamConsumer.recv()`

### 数据库操作识别策略

**SQL 语句识别**:
- 字符串字面量中的 SQL 关键字（SELECT, INSERT, UPDATE, DELETE）
- 提取 FROM/INTO/UPDATE 后的表名

**ORM 识别**:
- Java: JPA 注解 `@Entity`, `@Table`, Repository 方法
- Rust: Diesel ORM 的 `table!` 宏

### Redis 操作识别策略

**操作识别**:
- Java: `RedisTemplate.opsForValue().get/set()`
- Rust: `redis::Commands` trait 方法

**键前缀提取**:
- 从字符串字面量中提取
- 识别常量定义
- 支持通配符模式（如 "user:*"）

### 追溯算法

**深度优先搜索 (DFS)**:
```rust
fn trace_upstream(method: &str, depth: usize, visited: &mut HashSet<String>, graph: &mut ImpactGraph) {
    if depth >= max_depth || visited.contains(method) {
        return;
    }
    visited.insert(method.to_string());
    
    for caller in self.index.find_callers(method) {
        graph.add_edge(caller, method, EdgeType::MethodCall, Direction::Upstream);
        self.trace_upstream(caller, depth + 1, visited, graph);
    }
    
    // 跨服务追溯
    if let Some(method_info) = self.index.find_method(method) {
        self.trace_cross_service_upstream(method_info, visited, graph);
    }
}
```

**循环检测**:
- 使用 visited 集合记录已访问节点
- 检测到已访问节点时标记循环并终止该分支

### 并行处理策略

使用 Rayon crate 实现并行处理：

```rust
use rayon::prelude::*;

// 并行解析文件
let parsed_files: Vec<ParsedFile> = source_files
    .par_iter()
    .filter_map(|file| {
        match parser.parse_file(file) {
            Ok(parsed) => Some(parsed),
            Err(e) => {
                warn!("Failed to parse {}: {}", file.display(), e);
                None
            }
        }
    })
    .collect();
```

### 缓存策略

使用内存缓存避免重复解析：

```rust
use std::collections::HashMap;

pub struct ParseCache {
    cache: HashMap<PathBuf, ParsedFile>,
}

impl ParseCache {
    pub fn get_or_parse<F>(&mut self, path: &Path, parse_fn: F) -> Result<&ParsedFile, ParseError>
    where
        F: FnOnce(&Path) -> Result<ParsedFile, ParseError>,
    {
        if !self.cache.contains_key(path) {
            let parsed = parse_fn(path)?;
            self.cache.insert(path.to_path_buf(), parsed);
        }
        Ok(self.cache.get(path).unwrap())
    }
}
```

## 依赖库

基于研究，系统将使用以下 Rust crates：

- **tree-sitter** (v0.24): 多语言源代码解析 ([docs.rs](https://docs.rs/tree-sitter/))
- **tree-sitter-java**: Java 语法支持
- **tree-sitter-rust**: Rust 语法支持
- **gitpatch** (v0.7): Git patch 文件解析 ([lib.rs](https://lib.rs/crates/gitpatch))
- **petgraph** (v0.6): 图数据结构和 DOT 输出 ([docs.rs](https://docs.rs/petgraph/))
- **quick-xml** (v0.36): XML 配置解析
- **serde_yaml** (v0.9): YAML 配置解析
- **clap** (v4): 命令行参数解析
- **rayon** (v1.10): 并行处理
- **proptest** (v1.5): 属性测试框架
- **regex** (v1.10): 正则表达式匹配
- **log** (v0.4): 日志接口
- **env_logger** (v0.11): 日志实现

*内容根据合规要求进行了改写*
