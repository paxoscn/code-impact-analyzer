use std::path::PathBuf;
use std::fmt;

/// 顶层分析错误类型
#[derive(Debug)]
pub enum AnalysisError {
    PatchParseError(ParseError),
    LanguageParseError { file: PathBuf, error: ParseError },
    ConfigParseError { file: PathBuf, error: ParseError },
    IndexBuildError(IndexError),
    TraceError(TraceError),
    IoError(std::io::Error),
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisError::PatchParseError(e) => write!(f, "Patch parse error: {}", e),
            AnalysisError::LanguageParseError { file, error } => {
                write!(f, "Language parse error in {:?}: {}", file, error)
            }
            AnalysisError::ConfigParseError { file, error } => {
                write!(f, "Config parse error in {:?}: {}", file, error)
            }
            AnalysisError::IndexBuildError(e) => write!(f, "Index build error: {}", e),
            AnalysisError::TraceError(e) => write!(f, "Trace error: {}", e),
            AnalysisError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for AnalysisError {}

impl From<std::io::Error> for AnalysisError {
    fn from(error: std::io::Error) -> Self {
        AnalysisError::IoError(error)
    }
}

/// 解析错误类型
#[derive(Debug, Clone)]
pub enum ParseError {
    InvalidFormat { message: String },
    SyntaxError { line: usize, column: usize, message: String },
    UnsupportedLanguage { language: String },
    BinaryFile { path: PathBuf },
    IoError { path: PathBuf, error: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFormat { message } => write!(f, "Invalid format: {}", message),
            ParseError::SyntaxError { line, column, message } => {
                write!(f, "Syntax error at {}:{}: {}", line, column, message)
            }
            ParseError::UnsupportedLanguage { language } => {
                write!(f, "Unsupported language: {}", language)
            }
            ParseError::BinaryFile { path } => write!(f, "Binary file: {:?}", path),
            ParseError::IoError { path, error } => write!(f, "IO error for {:?}: {}", path, error),
        }
    }
}

impl std::error::Error for ParseError {}

/// 索引构建错误类型
#[derive(Debug, Clone)]
pub enum IndexError {
    DuplicateSymbol { symbol: String },
    InvalidReference { from: String, to: String },
    IoError { path: PathBuf, error: String },
    UnsupportedLanguage { file: PathBuf },
    ParseError { file: PathBuf, error: String },
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::DuplicateSymbol { symbol } => write!(f, "Duplicate symbol: {}", symbol),
            IndexError::InvalidReference { from, to } => {
                write!(f, "Invalid reference from {} to {}", from, to)
            }
            IndexError::IoError { path, error } => {
                write!(f, "IO error for {:?}: {}", path, error)
            }
            IndexError::UnsupportedLanguage { file } => {
                write!(f, "Unsupported language for file: {:?}", file)
            }
            IndexError::ParseError { file, error } => {
                write!(f, "Parse error for {:?}: {}", file, error)
            }
        }
    }
}

impl std::error::Error for IndexError {}

/// 追溯错误类型
#[derive(Debug, Clone)]
pub enum TraceError {
    MethodNotFound { method: String },
    MaxDepthExceeded { depth: usize },
    CyclicDependency { cycle: Vec<String> },
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceError::MethodNotFound { method } => write!(f, "Method not found: {}", method),
            TraceError::MaxDepthExceeded { depth } => {
                write!(f, "Max depth exceeded: {}", depth)
            }
            TraceError::CyclicDependency { cycle } => {
                write!(f, "Cyclic dependency: {}", cycle.join(" -> "))
            }
        }
    }
}

impl std::error::Error for TraceError {}
