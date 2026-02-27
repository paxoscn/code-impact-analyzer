use std::path::{Path, PathBuf};
use crate::errors::ParseError;
use crate::types::*;
use serde::{Deserialize, Serialize};

/// 语言解析器 trait
/// 
/// 提供统一的多语言源代码解析接口
pub trait LanguageParser: Send + Sync {
    /// 返回语言名称
    fn language_name(&self) -> &str;
    
    /// 返回支持的文件扩展名列表
    fn file_extensions(&self) -> &[&str];
    
    /// 解析源文件
    fn parse_file(&self, content: &str, file_path: &Path) -> Result<ParsedFile, ParseError>;
}

/// 解析后的文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFile {
    pub file_path: PathBuf,
    pub language: String,
    pub classes: Vec<ClassInfo>,
    pub functions: Vec<FunctionInfo>,
    pub imports: Vec<Import>,
}

/// 类信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassInfo {
    pub name: String,
    pub methods: Vec<MethodInfo>,
    pub line_range: (usize, usize),
    /// 是否是接口
    pub is_interface: bool,
    /// 实现的接口列表（完整类名）
    pub implements: Vec<String>,
}

/// 方法信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodInfo {
    pub name: String,
    pub full_qualified_name: String,
    pub file_path: PathBuf,
    pub line_range: (usize, usize),
    pub calls: Vec<MethodCall>,
    pub http_annotations: Option<HttpAnnotation>,
    pub kafka_operations: Vec<KafkaOperation>,
    pub db_operations: Vec<DbOperation>,
    pub redis_operations: Vec<RedisOperation>,
}

/// 函数信息（用于非面向对象语言如 Rust）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub full_qualified_name: String,
    pub file_path: PathBuf,
    pub line_range: (usize, usize),
    pub calls: Vec<MethodCall>,
    pub http_annotations: Option<HttpAnnotation>,
    pub kafka_operations: Vec<KafkaOperation>,
    pub db_operations: Vec<DbOperation>,
    pub redis_operations: Vec<RedisOperation>,
}

/// 方法调用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodCall {
    pub target: String,
    pub line: usize,
}

/// 语言识别器
/// 
/// 基于文件扩展名识别编程语言类型
pub struct LanguageDetector;

impl LanguageDetector {
    /// 根据文件路径识别语言类型
    /// 
    /// # Arguments
    /// * `file_path` - 文件路径
    /// 
    /// # Returns
    /// * `Some(language_name)` - 如果识别成功
    /// * `None` - 如果无法识别
    pub fn detect_language(file_path: &Path) -> Option<&'static str> {
        file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext {
                "java" => Some("java"),
                "rs" => Some("rust"),
                _ => None,
            })
    }
    
    /// 检查文件是否为支持的语言
    pub fn is_supported(file_path: &Path) -> bool {
        Self::detect_language(file_path).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_java() {
        let path = Path::new("src/main/java/Example.java");
        assert_eq!(LanguageDetector::detect_language(path), Some("java"));
        assert!(LanguageDetector::is_supported(path));
    }
    
    #[test]
    fn test_detect_rust() {
        let path = Path::new("src/lib.rs");
        assert_eq!(LanguageDetector::detect_language(path), Some("rust"));
        assert!(LanguageDetector::is_supported(path));
    }
    
    #[test]
    fn test_detect_unsupported() {
        let path = Path::new("README.md");
        assert_eq!(LanguageDetector::detect_language(path), None);
        assert!(!LanguageDetector::is_supported(path));
    }
    
    #[test]
    fn test_detect_no_extension() {
        let path = Path::new("Makefile");
        assert_eq!(LanguageDetector::detect_language(path), None);
        assert!(!LanguageDetector::is_supported(path));
    }
}
