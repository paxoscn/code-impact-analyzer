use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::code_index::CodeIndex;
use crate::language_parser::MethodInfo;
use crate::errors::IndexError;

/// 索引格式版本
const INDEX_VERSION: &str = "1.0.0";

/// 索引目录名称
const INDEX_DIR: &str = ".code-impact-analyzer";

/// 索引元数据文件名
const META_FILE: &str = "index.meta.json";

/// 索引数据文件名
const INDEX_FILE: &str = "index.json";

/// 索引元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// 索引格式版本
    pub version: String,
    
    /// 工作空间路径
    pub workspace_path: PathBuf,
    
    /// 创建时间（Unix 时间戳）
    pub created_at: u64,
    
    /// 更新时间（Unix 时间戳）
    pub updated_at: u64,
    
    /// 索引的文件总数
    pub file_count: usize,
    
    /// 索引的方法总数
    pub method_count: usize,
    
    /// 工作空间校验和
    pub checksum: String,
}

impl IndexMetadata {
    /// 创建新的元数据
    pub fn new(workspace_path: PathBuf, file_count: usize, method_count: usize) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let checksum = Self::calculate_checksum(&workspace_path);
        
        Self {
            version: INDEX_VERSION.to_string(),
            workspace_path,
            created_at: now,
            updated_at: now,
            file_count,
            method_count,
            checksum,
        }
    }
    
    /// 计算工作空间校验和
    /// 
    /// 基于工作空间中所有源文件的修改时间计算校验和
    fn calculate_checksum(workspace_path: &Path) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        
        // 遍历工作空间，收集所有源文件的修改时间
        if let Ok(entries) = Self::collect_file_mtimes(workspace_path) {
            for (path, mtime) in entries {
                path.hash(&mut hasher);
                mtime.hash(&mut hasher);
            }
        }
        
        format!("{:x}", hasher.finish())
    }
    
    /// 收集文件修改时间
    fn collect_file_mtimes(dir: &Path) -> Result<Vec<(PathBuf, u64)>, std::io::Error> {
        let mut result = Vec::new();
        
        if !dir.is_dir() {
            return Ok(result);
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            // 跳过隐藏目录和构建目录
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "target" || name == "build" || name == "node_modules" {
                    continue;
                }
            }
            
            if path.is_dir() {
                result.extend(Self::collect_file_mtimes(&path)?);
            } else if Self::is_source_file(&path) {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                            result.push((path, duration.as_secs()));
                        }
                    }
                }
            }
        }
        
        Ok(result)
    }
    
    /// 判断是否是源文件
    fn is_source_file(path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(ext, "java" | "rs" | "kt" | "scala" | "go" | "py" | "js" | "ts")
        } else {
            false
        }
    }
    
    /// 验证元数据是否有效
    pub fn is_valid(&self, workspace_path: &Path) -> bool {
        // 检查版本兼容性
        if !self.is_version_compatible() {
            log::warn!("Index version {} is not compatible with current version {}", 
                      self.version, INDEX_VERSION);
            return false;
        }
        
        // 检查工作空间路径
        if self.workspace_path != workspace_path {
            log::warn!("Workspace path mismatch: expected {:?}, got {:?}", 
                      workspace_path, self.workspace_path);
            return false;
        }
        
        // 检查校验和
        let current_checksum = Self::calculate_checksum(workspace_path);
        if self.checksum != current_checksum {
            log::warn!("Workspace checksum mismatch: index may be outdated");
            return false;
        }
        
        true
    }
    
    /// 检查版本兼容性
    fn is_version_compatible(&self) -> bool {
        // 简单的版本检查：主版本号必须相同
        let current_major = INDEX_VERSION.split('.').next().unwrap_or("0");
        let index_major = self.version.split('.').next().unwrap_or("0");
        
        current_major == index_major
    }
}

/// 可序列化的索引数据
#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableIndex {
    /// 方法信息映射
    pub methods: HashMap<String, MethodInfo>,
    
    /// 方法调用映射
    pub method_calls: HashMap<String, Vec<String>>,
    
    /// 反向调用映射
    pub reverse_calls: HashMap<String, Vec<String>>,
    
    /// HTTP 提供者映射
    pub http_providers: HashMap<String, String>,
    
    /// HTTP 消费者映射
    pub http_consumers: HashMap<String, Vec<String>>,
    
    /// Kafka 生产者映射
    pub kafka_producers: HashMap<String, Vec<String>>,
    
    /// Kafka 消费者映射
    pub kafka_consumers: HashMap<String, Vec<String>>,
    
    /// 数据库写入者映射
    pub db_writers: HashMap<String, Vec<String>>,
    
    /// 数据库读取者映射
    pub db_readers: HashMap<String, Vec<String>>,
    
    /// Redis 写入者映射
    pub redis_writers: HashMap<String, Vec<String>>,
    
    /// Redis 读取者映射
    pub redis_readers: HashMap<String, Vec<String>>,
    
    /// 配置关联映射
    pub config_associations: HashMap<String, Vec<String>>,
}

/// 索引存储管理器
pub struct IndexStorage {
    /// 工作空间路径
    workspace_path: PathBuf,
    
    /// 索引目录路径
    index_dir: PathBuf,
}

impl IndexStorage {
    /// 创建新的索引存储管理器
    pub fn new(workspace_path: PathBuf) -> Self {
        let index_dir = workspace_path.join(INDEX_DIR);
        
        Self {
            workspace_path,
            index_dir,
        }
    }
    
    /// 检查索引是否存在
    pub fn index_exists(&self) -> bool {
        self.meta_file_path().exists() && self.index_file_path().exists()
    }
    
    /// 加载索引
    /// 
    /// # Returns
    /// * `Ok(Some(CodeIndex))` - 成功加载索引
    /// * `Ok(None)` - 索引不存在或无效
    /// * `Err(IndexError)` - 加载失败
    pub fn load_index(&self) -> Result<Option<CodeIndex>, IndexError> {
        // 检查索引文件是否存在
        if !self.index_exists() {
            log::info!("Index files not found, will build new index");
            return Ok(None);
        }
        
        // 加载元数据
        let metadata = self.load_metadata()?;
        
        // 验证元数据
        if !metadata.is_valid(&self.workspace_path) {
            log::info!("Index is invalid or outdated, will rebuild");
            return Ok(None);
        }
        
        log::info!("Loading index from {:?}", self.index_dir);
        
        // 加载索引数据
        let serializable = self.load_index_data()?;
        
        // 转换为 CodeIndex
        let code_index = self.deserialize_index(serializable)?;
        
        log::info!("Index loaded successfully: {} methods", metadata.method_count);
        
        Ok(Some(code_index))
    }
    
    /// 保存索引
    pub fn save_index(&self, code_index: &CodeIndex) -> Result<(), IndexError> {
        log::info!("Saving index to {:?}", self.index_dir);
        
        // 创建索引目录
        self.ensure_index_dir()?;
        
        // 序列化索引数据
        let serializable = self.serialize_index(code_index)?;
        
        // 计算统计信息
        let method_count = serializable.methods.len();
        let file_count = serializable.methods.values()
            .map(|m| m.file_path.clone())
            .collect::<std::collections::HashSet<_>>()
            .len();
        
        // 创建元数据
        let metadata = IndexMetadata::new(
            self.workspace_path.clone(),
            file_count,
            method_count,
        );
        
        // 保存元数据
        self.save_metadata(&metadata)?;
        
        // 保存索引数据
        self.save_index_data(&serializable)?;
        
        log::info!("Index saved successfully: {} methods in {} files", 
                  method_count, file_count);
        
        Ok(())
    }
    
    /// 清除索引
    pub fn clear_index(&self) -> Result<(), IndexError> {
        if self.index_dir.exists() {
            fs::remove_dir_all(&self.index_dir)
                .map_err(|e| IndexError::IoError {
                    path: self.index_dir.clone(),
                    error: e.to_string(),
                })?;
            
            log::info!("Index cleared");
        }
        
        Ok(())
    }
    
    /// 获取索引信息
    pub fn get_index_info(&self) -> Result<Option<IndexMetadata>, IndexError> {
        if !self.index_exists() {
            return Ok(None);
        }
        
        let metadata = self.load_metadata()?;
        Ok(Some(metadata))
    }
    
    // ========== 私有辅助方法 ==========
    
    /// 确保索引目录存在
    fn ensure_index_dir(&self) -> Result<(), IndexError> {
        if !self.index_dir.exists() {
            fs::create_dir_all(&self.index_dir)
                .map_err(|e| IndexError::IoError {
                    path: self.index_dir.clone(),
                    error: e.to_string(),
                })?;
        }
        
        Ok(())
    }
    
    /// 获取元数据文件路径
    fn meta_file_path(&self) -> PathBuf {
        self.index_dir.join(META_FILE)
    }
    
    /// 获取索引文件路径
    fn index_file_path(&self) -> PathBuf {
        self.index_dir.join(INDEX_FILE)
    }
    
    /// 加载元数据
    fn load_metadata(&self) -> Result<IndexMetadata, IndexError> {
        let path = self.meta_file_path();
        let content = fs::read_to_string(&path)
            .map_err(|e| IndexError::IoError {
                path: path.clone(),
                error: e.to_string(),
            })?;
        
        serde_json::from_str(&content)
            .map_err(|e| IndexError::SerializationError {
                message: format!("Failed to parse metadata: {}", e),
            })
    }
    
    /// 保存元数据
    fn save_metadata(&self, metadata: &IndexMetadata) -> Result<(), IndexError> {
        let path = self.meta_file_path();
        let content = serde_json::to_string_pretty(metadata)
            .map_err(|e| IndexError::SerializationError {
                message: format!("Failed to serialize metadata: {}", e),
            })?;
        
        fs::write(&path, content)
            .map_err(|e| IndexError::IoError {
                path: path.clone(),
                error: e.to_string(),
            })
    }
    
    /// 加载索引数据
    fn load_index_data(&self) -> Result<SerializableIndex, IndexError> {
        let path = self.index_file_path();
        let content = fs::read_to_string(&path)
            .map_err(|e| IndexError::IoError {
                path: path.clone(),
                error: e.to_string(),
            })?;
        
        serde_json::from_str(&content)
            .map_err(|e| IndexError::SerializationError {
                message: format!("Failed to parse index data: {}", e),
            })
    }
    
    /// 保存索引数据
    fn save_index_data(&self, data: &SerializableIndex) -> Result<(), IndexError> {
        let path = self.index_file_path();
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| IndexError::SerializationError {
                message: format!("Failed to serialize index data: {}", e),
            })?;
        
        fs::write(&path, content)
            .map_err(|e| IndexError::IoError {
                path: path.clone(),
                error: e.to_string(),
            })
    }
    
    /// 序列化 CodeIndex
    fn serialize_index(&self, code_index: &CodeIndex) -> Result<SerializableIndex, IndexError> {
        // 使用反射访问 CodeIndex 的私有字段
        // 注意：这需要 CodeIndex 提供公共访问方法
        
        let mut methods = HashMap::new();
        let mut method_calls = HashMap::new();
        let mut reverse_calls = HashMap::new();
        
        // 收集方法信息
        for (name, method) in code_index.methods() {
            methods.insert(name.clone(), method.clone());
            
            // 收集方法调用
            let callees = code_index.find_callees(name);
            if !callees.is_empty() {
                method_calls.insert(name.clone(), callees.iter().map(|s| s.to_string()).collect());
            }
            
            // 收集反向调用
            let callers = code_index.find_callers(name);
            if !callers.is_empty() {
                reverse_calls.insert(name.clone(), callers.iter().map(|s| s.to_string()).collect());
            }
        }
        
        // 收集 HTTP 端点信息
        let mut http_providers = HashMap::new();
        let mut http_consumers = HashMap::new();
        
        // 遍历所有方法，查找 HTTP 注解
        for (name, method) in &methods {
            if let Some(http_ann) = &method.http_annotations {
                let key = format!("{}:{}", 
                    match http_ann.method {
                        crate::types::HttpMethod::GET => "GET",
                        crate::types::HttpMethod::POST => "POST",
                        crate::types::HttpMethod::PUT => "PUT",
                        crate::types::HttpMethod::DELETE => "DELETE",
                        crate::types::HttpMethod::PATCH => "PATCH",
                    },
                    http_ann.path
                );
                
                if http_ann.is_feign_client {
                    http_consumers.entry(key).or_insert_with(Vec::new).push(name.clone());
                } else {
                    http_providers.insert(key, name.clone());
                }
            }
        }
        
        // 收集 Kafka 信息
        let mut kafka_producers = HashMap::new();
        let mut kafka_consumers = HashMap::new();
        
        for (name, method) in &methods {
            for kafka_op in &method.kafka_operations {
                match kafka_op.operation_type {
                    crate::types::KafkaOpType::Produce => {
                        kafka_producers.entry(kafka_op.topic.clone())
                            .or_insert_with(Vec::new)
                            .push(name.clone());
                    }
                    crate::types::KafkaOpType::Consume => {
                        kafka_consumers.entry(kafka_op.topic.clone())
                            .or_insert_with(Vec::new)
                            .push(name.clone());
                    }
                }
            }
        }
        
        // 收集数据库信息
        let mut db_writers = HashMap::new();
        let mut db_readers = HashMap::new();
        
        for (name, method) in &methods {
            for db_op in &method.db_operations {
                match db_op.operation_type {
                    crate::types::DbOpType::Select => {
                        db_readers.entry(db_op.table.clone())
                            .or_insert_with(Vec::new)
                            .push(name.clone());
                    }
                    crate::types::DbOpType::Insert | 
                    crate::types::DbOpType::Update | 
                    crate::types::DbOpType::Delete => {
                        db_writers.entry(db_op.table.clone())
                            .or_insert_with(Vec::new)
                            .push(name.clone());
                    }
                }
            }
        }
        
        // 收集 Redis 信息
        let mut redis_writers = HashMap::new();
        let mut redis_readers = HashMap::new();
        
        for (name, method) in &methods {
            for redis_op in &method.redis_operations {
                match redis_op.operation_type {
                    crate::types::RedisOpType::Get => {
                        redis_readers.entry(redis_op.key_pattern.clone())
                            .or_insert_with(Vec::new)
                            .push(name.clone());
                    }
                    crate::types::RedisOpType::Set | 
                    crate::types::RedisOpType::Delete => {
                        redis_writers.entry(redis_op.key_pattern.clone())
                            .or_insert_with(Vec::new)
                            .push(name.clone());
                    }
                }
            }
        }
        
        // 配置关联（暂时为空，需要从 CodeIndex 获取）
        let config_associations = HashMap::new();
        
        Ok(SerializableIndex {
            methods,
            method_calls,
            reverse_calls,
            http_providers,
            http_consumers,
            kafka_producers,
            kafka_consumers,
            db_writers,
            db_readers,
            redis_writers,
            redis_readers,
            config_associations,
        })
    }
    
    /// 反序列化为 CodeIndex
    fn deserialize_index(&self, data: SerializableIndex) -> Result<CodeIndex, IndexError> {
        let mut code_index = CodeIndex::new();
        
        // 重建索引
        for (_, method) in data.methods {
            code_index.test_index_method(&method)
                .map_err(|e| IndexError::SerializationError {
                    message: format!("Failed to rebuild index: {}", e),
                })?;
        }
        
        Ok(code_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_index_metadata_creation() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        
        let metadata = IndexMetadata::new(workspace_path.clone(), 10, 100);
        
        assert_eq!(metadata.version, INDEX_VERSION);
        assert_eq!(metadata.workspace_path, workspace_path);
        assert_eq!(metadata.file_count, 10);
        assert_eq!(metadata.method_count, 100);
        assert!(!metadata.checksum.is_empty());
    }
    
    #[test]
    fn test_index_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        
        let storage = IndexStorage::new(workspace_path.clone());
        
        assert_eq!(storage.workspace_path, workspace_path);
        assert!(!storage.index_exists());
    }
    
    #[test]
    fn test_save_and_load_index() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        
        let storage = IndexStorage::new(workspace_path);
        let code_index = CodeIndex::new();
        
        // 保存索引
        storage.save_index(&code_index).unwrap();
        
        // 验证文件存在
        assert!(storage.index_exists());
        
        // 加载索引
        let loaded = storage.load_index().unwrap();
        assert!(loaded.is_some());
    }
    
    #[test]
    fn test_clear_index() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        
        let storage = IndexStorage::new(workspace_path);
        let code_index = CodeIndex::new();
        
        // 保存索引
        storage.save_index(&code_index).unwrap();
        assert!(storage.index_exists());
        
        // 清除索引
        storage.clear_index().unwrap();
        assert!(!storage.index_exists());
    }
    
    #[test]
    fn test_get_index_info() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        
        let storage = IndexStorage::new(workspace_path);
        
        // 索引不存在时
        let info = storage.get_index_info().unwrap();
        assert!(info.is_none());
        
        // 保存索引后
        let code_index = CodeIndex::new();
        storage.save_index(&code_index).unwrap();
        
        let info = storage.get_index_info().unwrap();
        assert!(info.is_some());
        
        let metadata = info.unwrap();
        assert_eq!(metadata.version, INDEX_VERSION);
    }
}
