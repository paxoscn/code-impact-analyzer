use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// 方法定位信息
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MethodLocation {
    pub file_path: PathBuf,
    pub qualified_name: String,
    pub line_start: usize,
    pub line_end: usize,
}

/// HTTP 方法类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

/// HTTP 注解信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpAnnotation {
    pub method: HttpMethod,
    pub path: String,
    pub path_params: Vec<String>,
}

/// Kafka 操作类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KafkaOpType {
    Produce,
    Consume,
}

/// Kafka 操作信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaOperation {
    pub operation_type: KafkaOpType,
    pub topic: String,
    pub line: usize,
}

/// 数据库操作类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbOpType {
    Select,
    Insert,
    Update,
    Delete,
}

/// 数据库操作信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbOperation {
    pub operation_type: DbOpType,
    pub table: String,
    pub line: usize,
}

/// Redis 操作类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedisOpType {
    Get,
    Set,
    Delete,
}

/// Redis 操作信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedisOperation {
    pub operation_type: RedisOpType,
    pub key_pattern: String,
    pub line: usize,
}

/// 导入声明
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub items: Vec<String>,
}

/// HTTP 端点标识
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HttpEndpoint {
    pub method: HttpMethod,
    pub path_pattern: String,
}

impl HttpEndpoint {
    /// 获取 HTTP 方法的字符串表示
    pub fn method_str(&self) -> &str {
        match self.method {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::PATCH => "PATCH",
        }
    }
}
