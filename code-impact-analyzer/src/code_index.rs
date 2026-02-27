use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use crate::errors::IndexError;
use crate::language_parser::{LanguageParser, LanguageDetector, ParsedFile, MethodInfo, FunctionInfo};
use crate::types::{HttpAnnotation, HttpEndpoint};
use crate::parse_cache::ParseCache;

/// 代码索引
/// 
/// 构建全局代码索引，支持快速查询方法调用关系和跨服务资源
pub struct CodeIndex {
    /// 方法信息映射: qualified_name -> MethodInfo
    methods: HashMap<String, MethodInfo>,
    
    /// 方法调用映射: caller -> [callees]
    method_calls: HashMap<String, Vec<String>>,
    
    /// 反向调用映射: callee -> [callers]
    reverse_calls: HashMap<String, Vec<String>>,
    
    /// HTTP 提供者映射: endpoint -> provider_method
    http_providers: HashMap<HttpEndpoint, String>,
    
    /// HTTP 消费者映射: endpoint -> [consumer_methods]
    http_consumers: HashMap<HttpEndpoint, Vec<String>>,
    
    /// Kafka 生产者映射: topic -> [producer_methods]
    kafka_producers: HashMap<String, Vec<String>>,
    
    /// Kafka 消费者映射: topic -> [consumer_methods]
    kafka_consumers: HashMap<String, Vec<String>>,
    
    /// 数据库写入者映射: table -> [writer_methods]
    db_writers: HashMap<String, Vec<String>>,
    
    /// 数据库读取者映射: table -> [reader_methods]
    db_readers: HashMap<String, Vec<String>>,
    
    /// Redis 写入者映射: key_prefix -> [writer_methods]
    redis_writers: HashMap<String, Vec<String>>,
    
    /// Redis 读取者映射: key_prefix -> [reader_methods]
    redis_readers: HashMap<String, Vec<String>>,
    
    /// 配置关联映射: 配置值 -> 使用该配置的方法列表
    /// 用于追踪从配置文件中读取的值在代码中的使用
    config_associations: HashMap<String, Vec<String>>,
    
    /// 接口到实现类的映射: interface_name -> [implementation_class_names]
    interface_implementations: HashMap<String, Vec<String>>,
}

impl CodeIndex {
    /// 创建新的代码索引
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            method_calls: HashMap::new(),
            reverse_calls: HashMap::new(),
            http_providers: HashMap::new(),
            http_consumers: HashMap::new(),
            kafka_producers: HashMap::new(),
            kafka_consumers: HashMap::new(),
            db_writers: HashMap::new(),
            db_readers: HashMap::new(),
            redis_writers: HashMap::new(),
            redis_readers: HashMap::new(),
            config_associations: HashMap::new(),
            interface_implementations: HashMap::new(),
        }
    }
    
    /// 索引整个工作空间
    /// 
    /// # Arguments
    /// * `workspace_path` - 工作空间根目录路径
    /// * `parsers` - 语言解析器列表
    /// 
    /// # Returns
    /// * `Ok(())` - 索引构建成功
    /// * `Err(IndexError)` - 索引构建失败
    pub fn index_workspace(
        &mut self,
        workspace_path: &Path,
        parsers: &[Box<dyn LanguageParser>],
    ) -> Result<(), IndexError> {
        // 遍历工作空间中的所有文件
        let source_files = self.collect_source_files(workspace_path)?;
        
        // 创建线程安全的解析缓存
        let cache = Arc::new(Mutex::new(ParseCache::new()));
        
        // 使用 rayon 并行解析所有源文件
        let parsed_files: Vec<ParsedFile> = source_files
            .par_iter()
            .filter_map(|file_path| {
                match self.parse_file_with_cache(file_path, parsers, &cache) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        // 记录错误但继续处理其他文件
                        eprintln!("Warning: Failed to parse {}: {}", file_path.display(), e);
                        None
                    }
                }
            })
            .collect();
        
        // 串行构建索引（确保线程安全）
        for parsed_file in parsed_files {
            if let Err(e) = self.index_parsed_file(parsed_file) {
                eprintln!("Warning: Failed to index parsed file: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// 使用缓存解析单个文件
    /// 
    /// 此方法设计为线程安全，可以在多个线程中并行调用
    fn parse_file_with_cache(
        &self,
        file_path: &Path,
        parsers: &[Box<dyn LanguageParser>],
        cache: &Arc<Mutex<ParseCache>>,
    ) -> Result<ParsedFile, IndexError> {
        // 尝试从缓存获取或解析
        let mut cache_guard = cache.lock().unwrap();
        
        cache_guard.get_or_parse(file_path, |path| {
            // 读取文件内容
            let content = fs::read_to_string(path)
                .map_err(|e| crate::errors::ParseError::IoError {
                    path: path.to_path_buf(),
                    error: e.to_string(),
                })?;
            
            // 选择合适的解析器
            let parser = self.select_parser(path, parsers)
                .ok_or_else(|| crate::errors::ParseError::UnsupportedLanguage {
                    language: format!("{:?}", path.extension()),
                })?;
            
            // 解析文件
            parser.parse_file(&content, path)
        })
        .map(|parsed| parsed.clone())
        .map_err(|e| IndexError::ParseError {
            file: file_path.to_path_buf(),
            error: format!("{:?}", e),
        })
    }
    
    /// 收集工作空间中的所有源文件
    fn collect_source_files(&self, workspace_path: &Path) -> Result<Vec<PathBuf>, IndexError> {
        let mut source_files = Vec::new();
        self.collect_files_recursive(workspace_path, &mut source_files)?;
        Ok(source_files)
    }
    
    /// 递归收集文件
    fn collect_files_recursive(
        &self,
        dir: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), IndexError> {
        if !dir.is_dir() {
            return Ok(());
        }
        
        let entries = fs::read_dir(dir)
            .map_err(|e| IndexError::IoError {
                path: dir.to_path_buf(),
                error: e.to_string(),
            })?;
        
        for entry in entries {
            let entry = entry.map_err(|e| IndexError::IoError {
                path: dir.to_path_buf(),
                error: e.to_string(),
            })?;
            
            let path = entry.path();
            
            // 跳过隐藏目录和常见的构建目录
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "target" || name == "build" || name == "node_modules" {
                    continue;
                }
            }
            
            if path.is_dir() {
                self.collect_files_recursive(&path, files)?;
            } else if LanguageDetector::is_supported(&path) {
                files.push(path);
            }
        }
        
        Ok(())
    }
    
    
    /// 选择合适的语言解析器
    fn select_parser<'a>(
        &self,
        file_path: &Path,
        parsers: &'a [Box<dyn LanguageParser>],
    ) -> Option<&'a Box<dyn LanguageParser>> {
        let language = LanguageDetector::detect_language(file_path)?;
        parsers.iter().find(|p| p.language_name() == language)
    }
    
    /// 索引解析后的文件
    fn index_parsed_file(&mut self, parsed_file: ParsedFile) -> Result<(), IndexError> {
        // 索引类中的方法
        for class in &parsed_file.classes {
            // 索引接口实现关系
            if !class.implements.is_empty() {
                for interface_name in &class.implements {
                    self.interface_implementations
                        .entry(interface_name.clone())
                        .or_insert_with(Vec::new)
                        .push(class.name.clone());
                }
            }
            
            for method in &class.methods {
                self.index_method(method)?;
            }
        }
        
        // 索引独立函数（用于 Rust 等语言）
        for function in &parsed_file.functions {
            self.index_function(function)?;
        }
        
        Ok(())
    }
    
    /// 索引方法信息
    pub fn index_method(&mut self, method: &MethodInfo) -> Result<(), IndexError> {
        let qualified_name = method.full_qualified_name.clone();
        
        // 存储方法信息
        self.methods.insert(qualified_name.clone(), method.clone());
        
        // 构建方法调用索引
        for call in &method.calls {
            // 正向调用: caller -> callee
            self.method_calls
                .entry(qualified_name.clone())
                .or_insert_with(Vec::new)
                .push(call.target.clone());
            
            // 反向调用: callee -> caller
            self.reverse_calls
                .entry(call.target.clone())
                .or_insert_with(Vec::new)
                .push(qualified_name.clone());
        }
        
        // 索引 HTTP 注解
        if let Some(http_annotation) = &method.http_annotations {
            self.index_http_annotation(&qualified_name, http_annotation);
        }
        
        // 索引 Kafka 操作
        for kafka_op in &method.kafka_operations {
            self.index_kafka_operation(&qualified_name, kafka_op);
        }
        
        // 索引数据库操作
        for db_op in &method.db_operations {
            self.index_db_operation(&qualified_name, db_op);
        }
        
        // 索引 Redis 操作
        for redis_op in &method.redis_operations {
            self.index_redis_operation(&qualified_name, redis_op);
        }
        
        Ok(())
    }
    
    /// 索引函数信息（与方法类似，用于非 OOP 语言）
    fn index_function(&mut self, function: &FunctionInfo) -> Result<(), IndexError> {
        // 将函数转换为 MethodInfo 进行存储
        let method_info = MethodInfo {
            name: function.name.clone(),
            full_qualified_name: function.full_qualified_name.clone(),
            file_path: function.file_path.clone(),
            line_range: function.line_range,
            calls: function.calls.clone(),
            http_annotations: function.http_annotations.clone(),
            kafka_operations: function.kafka_operations.clone(),
            db_operations: function.db_operations.clone(),
            redis_operations: function.redis_operations.clone(),
        };
        
        self.index_method(&method_info)
    }
    
    /// 索引 HTTP 注解
    fn index_http_annotation(&mut self, method_name: &str, annotation: &HttpAnnotation) {
        let endpoint = HttpEndpoint {
            method: annotation.method.clone(),
            path_pattern: annotation.path.clone(),
        };
        
        // 根据 is_feign_client 标志判断是提供者还是消费者
        if annotation.is_feign_client {
            // Feign 消费者
            self.http_consumers
                .entry(endpoint)
                .or_insert_with(Vec::new)
                .push(method_name.to_string());
        } else {
            // HTTP 接口提供者
            self.http_providers.insert(endpoint, method_name.to_string());
        }
    }
    
    /// 索引 Kafka 操作
    fn index_kafka_operation(&mut self, method_name: &str, operation: &crate::types::KafkaOperation) {
        use crate::types::KafkaOpType;
        
        match operation.operation_type {
            KafkaOpType::Produce => {
                self.kafka_producers
                    .entry(operation.topic.clone())
                    .or_insert_with(Vec::new)
                    .push(method_name.to_string());
            }
            KafkaOpType::Consume => {
                self.kafka_consumers
                    .entry(operation.topic.clone())
                    .or_insert_with(Vec::new)
                    .push(method_name.to_string());
            }
        }
    }
    
    /// 索引数据库操作
    fn index_db_operation(&mut self, method_name: &str, operation: &crate::types::DbOperation) {
        use crate::types::DbOpType;
        
        match operation.operation_type {
            DbOpType::Select => {
                self.db_readers
                    .entry(operation.table.clone())
                    .or_insert_with(Vec::new)
                    .push(method_name.to_string());
            }
            DbOpType::Insert | DbOpType::Update | DbOpType::Delete => {
                self.db_writers
                    .entry(operation.table.clone())
                    .or_insert_with(Vec::new)
                    .push(method_name.to_string());
            }
        }
    }
    
    /// 索引 Redis 操作
    fn index_redis_operation(&mut self, method_name: &str, operation: &crate::types::RedisOperation) {
        use crate::types::RedisOpType;
        
        match operation.operation_type {
            RedisOpType::Get => {
                self.redis_readers
                    .entry(operation.key_pattern.clone())
                    .or_insert_with(Vec::new)
                    .push(method_name.to_string());
            }
            RedisOpType::Set | RedisOpType::Delete => {
                self.redis_writers
                    .entry(operation.key_pattern.clone())
                    .or_insert_with(Vec::new)
                    .push(method_name.to_string());
            }
        }
    }
    
    /// 查找方法信息
    pub fn find_method(&self, qualified_name: &str) -> Option<&MethodInfo> {
        self.methods.get(qualified_name)
    }
    
    /// 查找调用指定方法的所有方法（上游）
    pub fn find_callers(&self, method: &str) -> Vec<&str> {
        self.reverse_calls
            .get(method)
            .map(|callers| callers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找指定方法调用的所有方法（下游）
    pub fn find_callees(&self, method: &str) -> Vec<&str> {
        self.method_calls
            .get(method)
            .map(|callees| callees.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找 HTTP 端点的提供者
    pub fn find_http_providers(&self, endpoint: &HttpEndpoint) -> Vec<&str> {
        self.http_providers
            .get(endpoint)
            .map(|provider| vec![provider.as_str()])
            .unwrap_or_default()
    }
    
    /// 查找 HTTP 端点的消费者
    pub fn find_http_consumers(&self, endpoint: &HttpEndpoint) -> Vec<&str> {
        self.http_consumers
            .get(endpoint)
            .map(|consumers| consumers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找 Kafka Topic 的消费者
    pub fn find_kafka_consumers(&self, topic: &str) -> Vec<&str> {
        self.kafka_consumers
            .get(topic)
            .map(|consumers| consumers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找 Kafka Topic 的生产者
    pub fn find_kafka_producers(&self, topic: &str) -> Vec<&str> {
        self.kafka_producers
            .get(topic)
            .map(|producers| producers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找数据库表的读取者
    pub fn find_db_readers(&self, table: &str) -> Vec<&str> {
        self.db_readers
            .get(table)
            .map(|readers| readers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找数据库表的写入者
    pub fn find_db_writers(&self, table: &str) -> Vec<&str> {
        self.db_writers
            .get(table)
            .map(|writers| writers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找 Redis 键前缀的读取者
    pub fn find_redis_readers(&self, prefix: &str) -> Vec<&str> {
        self.redis_readers
            .get(prefix)
            .map(|readers| readers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找 Redis 键前缀的写入者
    pub fn find_redis_writers(&self, prefix: &str) -> Vec<&str> {
        self.redis_writers
            .get(prefix)
            .map(|writers| writers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 获取所有方法的迭代器
    /// 
    /// # Returns
    /// 返回一个迭代器，遍历所有 (方法名, 方法信息) 对
    pub fn methods(&self) -> impl Iterator<Item = (&String, &MethodInfo)> {
        self.methods.iter()
    }
    
    /// 关联配置数据到代码
    /// 
    /// 此方法将配置文件中的值与代码中的引用关联起来
    /// 
    /// # Arguments
    /// * `config_data` - 从配置文件解析的配置数据
    pub fn associate_config_data(&mut self, config_data: &crate::config_parser::ConfigData) {
        // 关联 HTTP 端点配置
        for endpoint in &config_data.http_endpoints {
            self.associate_http_endpoint(endpoint);
        }
        
        // 关联 Kafka Topic 配置
        for topic in &config_data.kafka_topics {
            self.associate_kafka_topic(topic);
        }
        
        // 关联数据库表配置
        for table in &config_data.db_tables {
            self.associate_db_table(table);
        }
        
        // 关联 Redis 键前缀配置
        for prefix in &config_data.redis_prefixes {
            self.associate_redis_prefix(prefix);
        }
    }
    
    /// 关联 HTTP 端点配置到代码
    /// 
    /// 查找所有可能调用该端点的方法，并建立关联
    fn associate_http_endpoint(&mut self, endpoint: &HttpEndpoint) {
        // 查找所有方法，检查是否有 HTTP 客户端调用匹配该端点
        let mut consumers = Vec::new();
        
        for (method_name, method_info) in &self.methods {
            // 检查方法调用中是否包含 HTTP 客户端调用
            if self.method_contains_http_call(method_info, endpoint) {
                consumers.push(method_name.clone());
            }
        }
        
        // 如果找到消费者，添加到索引
        if !consumers.is_empty() {
            self.http_consumers
                .entry(endpoint.clone())
                .or_insert_with(Vec::new)
                .extend(consumers.clone());
            
            // 记录配置关联
            let config_key = format!("http:{}:{}", endpoint.method_str(), endpoint.path_pattern);
            self.config_associations
                .entry(config_key)
                .or_insert_with(Vec::new)
                .extend(consumers);
        }
    }
    
    /// 检查方法是否包含对指定 HTTP 端点的调用
    fn method_contains_http_call(&self, method_info: &MethodInfo, endpoint: &HttpEndpoint) -> bool {
        // 检查方法调用中是否有匹配的 HTTP 客户端调用
        // 这里简化处理：检查调用目标是否包含 HTTP 客户端相关的方法名
        for call in &method_info.calls {
            let target_lower = call.target.to_lowercase();
            
            // 检查是否是 HTTP 客户端调用
            if target_lower.contains("httpclient") 
                || target_lower.contains("resttemplate")
                || target_lower.contains("webclient")
                || target_lower.contains("http::get")
                || target_lower.contains("http::post")
                || target_lower.contains("reqwest")
                || target_lower.contains("hyper") {
                
                // 进一步检查路径是否匹配
                // 这里简化处理：检查端点路径是否出现在方法的某个位置
                // 实际应用中可能需要更复杂的匹配逻辑
                if self.path_matches(&endpoint.path_pattern, &call.target) {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// 检查路径是否匹配
    /// 
    /// 支持路径参数匹配，例如 /api/users/{id} 匹配 /api/users/123
    fn path_matches(&self, pattern: &str, target: &str) -> bool {
        // 简化实现：检查目标字符串是否包含路径模式的主要部分
        // 移除路径参数占位符进行匹配
        let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
        
        for part in pattern_parts {
            // 跳过路径参数
            if part.starts_with('{') && part.ends_with('}') {
                continue;
            }
            
            // 检查目标是否包含该路径部分
            if !target.to_lowercase().contains(&part.to_lowercase()) {
                return false;
            }
        }
        
        true
    }
    
    /// 关联 Kafka Topic 配置到代码
    fn associate_kafka_topic(&mut self, topic: &str) {
        // 查找所有使用该 topic 的生产者和消费者
        let mut associated_methods = Vec::new();
        
        if let Some(producers) = self.kafka_producers.get(topic) {
            associated_methods.extend(producers.clone());
        }
        
        if let Some(consumers) = self.kafka_consumers.get(topic) {
            associated_methods.extend(consumers.clone());
        }
        
        // 记录配置关联
        if !associated_methods.is_empty() {
            let config_key = format!("kafka:topic:{}", topic);
            self.config_associations
                .entry(config_key)
                .or_insert_with(Vec::new)
                .extend(associated_methods);
        }
    }
    
    /// 关联数据库表配置到代码
    fn associate_db_table(&mut self, table: &str) {
        // 查找所有访问该表的读取者和写入者
        let mut associated_methods = Vec::new();
        
        if let Some(readers) = self.db_readers.get(table) {
            associated_methods.extend(readers.clone());
        }
        
        if let Some(writers) = self.db_writers.get(table) {
            associated_methods.extend(writers.clone());
        }
        
        // 记录配置关联
        if !associated_methods.is_empty() {
            let config_key = format!("db:table:{}", table);
            self.config_associations
                .entry(config_key)
                .or_insert_with(Vec::new)
                .extend(associated_methods);
        }
    }
    
    /// 关联 Redis 键前缀配置到代码
    fn associate_redis_prefix(&mut self, prefix: &str) {
        // 查找所有使用该前缀的读取者和写入者
        let mut associated_methods = std::collections::HashSet::new();
        
        // 精确匹配
        if let Some(readers) = self.redis_readers.get(prefix) {
            for reader in readers {
                associated_methods.insert(reader.clone());
            }
        }
        
        if let Some(writers) = self.redis_writers.get(prefix) {
            for writer in writers {
                associated_methods.insert(writer.clone());
            }
        }
        
        // 模糊匹配：查找所有可能匹配的键前缀
        for (key, readers) in &self.redis_readers {
            if key != prefix && self.redis_key_matches(prefix, key) {
                for reader in readers {
                    associated_methods.insert(reader.clone());
                }
            }
        }
        
        for (key, writers) in &self.redis_writers {
            if key != prefix && self.redis_key_matches(prefix, key) {
                for writer in writers {
                    associated_methods.insert(writer.clone());
                }
            }
        }
        
        // 记录配置关联
        if !associated_methods.is_empty() {
            let config_key = format!("redis:key:{}", prefix);
            self.config_associations
                .entry(config_key)
                .or_insert_with(Vec::new)
                .extend(associated_methods);
        }
    }
    
    /// 检查 Redis 键是否匹配
    /// 
    /// 支持通配符匹配，例如 user:* 匹配 user:123
    fn redis_key_matches(&self, pattern: &str, key: &str) -> bool {
        if pattern == key {
            return true;
        }
        
        // 处理通配符
        if pattern.contains('*') {
            let prefix = pattern.trim_end_matches('*');
            return key.starts_with(prefix);
        }
        
        if key.contains('*') {
            let prefix = key.trim_end_matches('*');
            return pattern.starts_with(prefix);
        }
        
        false
    }
    
    /// 查找与配置关联的方法
    /// 
    /// # Arguments
    /// * `config_key` - 配置键，格式如 "http:GET:/api/users" 或 "kafka:topic:user-events"
    /// 
    /// # Returns
    /// 与该配置关联的方法列表
    pub fn find_config_associations(&self, config_key: &str) -> Vec<&str> {
        self.config_associations
            .get(config_key)
            .map(|methods| methods.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 查找接口的所有实现类
    /// 
    /// # Arguments
    /// * `interface_name` - 接口的完整类名
    /// 
    /// # Returns
    /// 实现该接口的所有类的完整类名列表
    pub fn find_interface_implementations(&self, interface_name: &str) -> Vec<&str> {
        self.interface_implementations
            .get(interface_name)
            .map(|impls| impls.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    
    /// 解析方法调用目标，如果是接口且只有一个实现类，则返回实现类的方法
    /// 
    /// # Arguments
    /// * `method_call_target` - 方法调用目标（格式：ClassName::methodName）
    /// 
    /// # Returns
    /// 解析后的方法调用目标（如果接口只有一个实现类，返回实现类的方法；否则返回原始目标）
    pub fn resolve_interface_call(&self, method_call_target: &str) -> String {
        // 解析方法调用目标：ClassName::methodName
        if let Some(pos) = method_call_target.rfind("::") {
            let class_name = &method_call_target[..pos];
            let method_name = &method_call_target[pos + 2..];
            
            // 查找该类是否是接口，以及是否只有一个实现类
            let implementations = self.find_interface_implementations(class_name);
            
            if implementations.len() == 1 {
                // 只有一个实现类，用实现类替换接口
                let impl_class = implementations[0];
                return format!("{}::{}", impl_class, method_name);
            }
        }
        
        // 否则返回原始目标
        method_call_target.to_string()
    }
    
    /// 测试辅助方法：直接索引方法
    /// 
    /// 注意：此方法仅用于测试目的，不应在生产代码中使用
    #[doc(hidden)]
    pub fn test_index_method(&mut self, method: &MethodInfo) -> Result<(), IndexError> {
        self.index_method(method)
    }
    
    /// 测试辅助方法：索引解析后的文件
    /// 
    /// 注意：此方法仅用于测试目的，不应在生产代码中使用
    #[doc(hidden)]
    pub fn test_index_parsed_file(&mut self, parsed_file: ParsedFile) -> Result<(), IndexError> {
        self.index_parsed_file(parsed_file)
    }
}

impl Default for CodeIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_parser::MethodCall;
    use crate::types::{KafkaOpType, DbOpType, RedisOpType};
    
    #[test]
    fn test_new_code_index() {
        let index = CodeIndex::new();
        assert_eq!(index.methods.len(), 0);
        assert_eq!(index.method_calls.len(), 0);
    }
    
    #[test]
    fn test_index_method_with_calls() {
        let mut index = CodeIndex::new();
        
        let method = MethodInfo {
            name: "foo".to_string(),
            full_qualified_name: "com.example.Foo::foo".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![
                MethodCall {
                    target: "com.example.Bar::bar".to_string(),
                    line: 15,
                },
            ],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&method).unwrap();
        
        // 验证方法已索引
        assert!(index.find_method("com.example.Foo::foo").is_some());
        
        // 验证正向调用
        let callees = index.find_callees("com.example.Foo::foo");
        assert_eq!(callees, vec!["com.example.Bar::bar"]);
        
        // 验证反向调用
        let callers = index.find_callers("com.example.Bar::bar");
        assert_eq!(callers, vec!["com.example.Foo::foo"]);
    }
    
    #[test]
    fn test_index_kafka_operations() {
        let mut index = CodeIndex::new();
        
        let producer_method = MethodInfo {
            name: "sendMessage".to_string(),
            full_qualified_name: "com.example.Producer::sendMessage".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![
                crate::types::KafkaOperation {
                    operation_type: KafkaOpType::Produce,
                    topic: "user-events".to_string(),
                    line: 15,
                },
            ],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&producer_method).unwrap();
        
        let producers = index.find_kafka_producers("user-events");
        assert_eq!(producers, vec!["com.example.Producer::sendMessage"]);
    }
    
    #[test]
    fn test_index_db_operations() {
        let mut index = CodeIndex::new();
        
        let method = MethodInfo {
            name: "getUser".to_string(),
            full_qualified_name: "com.example.UserDao::getUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![
                crate::types::DbOperation {
                    operation_type: DbOpType::Select,
                    table: "users".to_string(),
                    line: 15,
                },
            ],
            redis_operations: vec![],
        };
        
        index.index_method(&method).unwrap();
        
        let readers = index.find_db_readers("users");
        assert_eq!(readers, vec!["com.example.UserDao::getUser"]);
    }
    
    #[test]
    fn test_index_redis_operations() {
        let mut index = CodeIndex::new();
        
        let method = MethodInfo {
            name: "cacheUser".to_string(),
            full_qualified_name: "com.example.UserCache::cacheUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![
                crate::types::RedisOperation {
                    operation_type: RedisOpType::Set,
                    key_pattern: "user:*".to_string(),
                    line: 15,
                },
            ],
        };
        
        index.index_method(&method).unwrap();
        
        let writers = index.find_redis_writers("user:*");
        assert_eq!(writers, vec!["com.example.UserCache::cacheUser"]);
    }
    
    #[test]
    fn test_find_method_query() {
        let mut index = CodeIndex::new();
        
        let method = MethodInfo {
            name: "testMethod".to_string(),
            full_qualified_name: "com.example.Test::testMethod".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (5, 15),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&method).unwrap();
        
        // 测试查找存在的方法
        let found = index.find_method("com.example.Test::testMethod");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "testMethod");
        
        // 测试查找不存在的方法
        let not_found = index.find_method("com.example.NonExistent::method");
        assert!(not_found.is_none());
    }
    
    #[test]
    fn test_find_callers_and_callees() {
        let mut index = CodeIndex::new();
        
        // 创建调用链: methodA -> methodB -> methodC
        let method_a = MethodInfo {
            name: "methodA".to_string(),
            full_qualified_name: "com.example.A::methodA".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (1, 10),
            calls: vec![
                MethodCall {
                    target: "com.example.B::methodB".to_string(),
                    line: 5,
                },
            ],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        let method_b = MethodInfo {
            name: "methodB".to_string(),
            full_qualified_name: "com.example.B::methodB".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (1, 10),
            calls: vec![
                MethodCall {
                    target: "com.example.C::methodC".to_string(),
                    line: 5,
                },
            ],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        let method_c = MethodInfo {
            name: "methodC".to_string(),
            full_qualified_name: "com.example.C::methodC".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (1, 10),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&method_a).unwrap();
        index.index_method(&method_b).unwrap();
        index.index_method(&method_c).unwrap();
        
        // 测试 find_callees
        let callees_a = index.find_callees("com.example.A::methodA");
        assert_eq!(callees_a, vec!["com.example.B::methodB"]);
        
        let callees_b = index.find_callees("com.example.B::methodB");
        assert_eq!(callees_b, vec!["com.example.C::methodC"]);
        
        let callees_c = index.find_callees("com.example.C::methodC");
        assert!(callees_c.is_empty());
        
        // 测试 find_callers
        let callers_a = index.find_callers("com.example.A::methodA");
        assert!(callers_a.is_empty());
        
        let callers_b = index.find_callers("com.example.B::methodB");
        assert_eq!(callers_b, vec!["com.example.A::methodA"]);
        
        let callers_c = index.find_callers("com.example.C::methodC");
        assert_eq!(callers_c, vec!["com.example.B::methodB"]);
    }
    
    #[test]
    fn test_http_provider_and_consumer_queries() {
        let mut index = CodeIndex::new();
        
        use crate::types::HttpMethod;
        
        let provider = MethodInfo {
            name: "getUser".to_string(),
            full_qualified_name: "com.example.UserController::getUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: Some(HttpAnnotation {
                method: HttpMethod::GET,
                path: "/api/users/{id}".to_string(),
                path_params: vec!["id".to_string()],
                is_feign_client: false,
            }),
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&provider).unwrap();
        
        let endpoint = HttpEndpoint {
            method: HttpMethod::GET,
            path_pattern: "/api/users/{id}".to_string(),
        };
        
        // 测试查找 HTTP 消费者（当前为空，因为没有索引消费者）
        let consumers = index.find_http_consumers(&endpoint);
        assert!(consumers.is_empty());
        
        // 验证提供者已被索引
        assert!(index.http_providers.contains_key(&endpoint));
    }
    
    #[test]
    fn test_kafka_producer_and_consumer_queries() {
        let mut index = CodeIndex::new();
        
        let producer = MethodInfo {
            name: "sendEvent".to_string(),
            full_qualified_name: "com.example.EventProducer::sendEvent".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![
                crate::types::KafkaOperation {
                    operation_type: KafkaOpType::Produce,
                    topic: "order-events".to_string(),
                    line: 15,
                },
            ],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        let consumer = MethodInfo {
            name: "handleEvent".to_string(),
            full_qualified_name: "com.example.EventConsumer::handleEvent".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (30, 40),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![
                crate::types::KafkaOperation {
                    operation_type: KafkaOpType::Consume,
                    topic: "order-events".to_string(),
                    line: 35,
                },
            ],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&producer).unwrap();
        index.index_method(&consumer).unwrap();
        
        // 测试查找生产者
        let producers = index.find_kafka_producers("order-events");
        assert_eq!(producers, vec!["com.example.EventProducer::sendEvent"]);
        
        // 测试查找消费者
        let consumers = index.find_kafka_consumers("order-events");
        assert_eq!(consumers, vec!["com.example.EventConsumer::handleEvent"]);
        
        // 测试查找不存在的 topic
        let no_producers = index.find_kafka_producers("non-existent-topic");
        assert!(no_producers.is_empty());
    }
    
    #[test]
    fn test_db_reader_and_writer_queries() {
        let mut index = CodeIndex::new();
        
        let reader = MethodInfo {
            name: "findUser".to_string(),
            full_qualified_name: "com.example.UserRepository::findUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![
                crate::types::DbOperation {
                    operation_type: DbOpType::Select,
                    table: "users".to_string(),
                    line: 15,
                },
            ],
            redis_operations: vec![],
        };
        
        let writer = MethodInfo {
            name: "saveUser".to_string(),
            full_qualified_name: "com.example.UserRepository::saveUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (30, 40),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![
                crate::types::DbOperation {
                    operation_type: DbOpType::Insert,
                    table: "users".to_string(),
                    line: 35,
                },
            ],
            redis_operations: vec![],
        };
        
        index.index_method(&reader).unwrap();
        index.index_method(&writer).unwrap();
        
        // 测试查找读取者
        let readers = index.find_db_readers("users");
        assert_eq!(readers, vec!["com.example.UserRepository::findUser"]);
        
        // 测试查找写入者
        let writers = index.find_db_writers("users");
        assert_eq!(writers, vec!["com.example.UserRepository::saveUser"]);
        
        // 测试查找不存在的表
        let no_readers = index.find_db_readers("non_existent_table");
        assert!(no_readers.is_empty());
    }
    
    #[test]
    fn test_redis_reader_and_writer_queries() {
        let mut index = CodeIndex::new();
        
        let reader = MethodInfo {
            name: "getCache".to_string(),
            full_qualified_name: "com.example.CacheService::getCache".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![
                crate::types::RedisOperation {
                    operation_type: RedisOpType::Get,
                    key_pattern: "session:*".to_string(),
                    line: 15,
                },
            ],
        };
        
        let writer = MethodInfo {
            name: "setCache".to_string(),
            full_qualified_name: "com.example.CacheService::setCache".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (30, 40),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![
                crate::types::RedisOperation {
                    operation_type: RedisOpType::Set,
                    key_pattern: "session:*".to_string(),
                    line: 35,
                },
            ],
        };
        
        index.index_method(&reader).unwrap();
        index.index_method(&writer).unwrap();
        
        // 测试查找读取者
        let readers = index.find_redis_readers("session:*");
        assert_eq!(readers, vec!["com.example.CacheService::getCache"]);
        
        // 测试查找写入者
        let writers = index.find_redis_writers("session:*");
        assert_eq!(writers, vec!["com.example.CacheService::setCache"]);
        
        // 测试查找不存在的键前缀
        let no_readers = index.find_redis_readers("non_existent:*");
        assert!(no_readers.is_empty());
    }
    
    #[test]
    fn test_multiple_callers_and_callees() {
        let mut index = CodeIndex::new();
        
        // 创建多个方法调用同一个方法的场景
        let method_a = MethodInfo {
            name: "methodA".to_string(),
            full_qualified_name: "com.example.A::methodA".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (1, 10),
            calls: vec![
                MethodCall {
                    target: "com.example.Common::shared".to_string(),
                    line: 5,
                },
            ],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        let method_b = MethodInfo {
            name: "methodB".to_string(),
            full_qualified_name: "com.example.B::methodB".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (1, 10),
            calls: vec![
                MethodCall {
                    target: "com.example.Common::shared".to_string(),
                    line: 5,
                },
            ],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&method_a).unwrap();
        index.index_method(&method_b).unwrap();
        
        // 测试多个调用者
        let callers = index.find_callers("com.example.Common::shared");
        assert_eq!(callers.len(), 2);
        assert!(callers.contains(&"com.example.A::methodA"));
        assert!(callers.contains(&"com.example.B::methodB"));
    }
    
    #[test]
    fn test_associate_kafka_topic_config() {
        let mut index = CodeIndex::new();
        
        // 添加 Kafka 生产者和消费者
        let producer = MethodInfo {
            name: "sendEvent".to_string(),
            full_qualified_name: "com.example.Producer::sendEvent".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![
                crate::types::KafkaOperation {
                    operation_type: KafkaOpType::Produce,
                    topic: "user-events".to_string(),
                    line: 15,
                },
            ],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        let consumer = MethodInfo {
            name: "handleEvent".to_string(),
            full_qualified_name: "com.example.Consumer::handleEvent".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (30, 40),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![
                crate::types::KafkaOperation {
                    operation_type: KafkaOpType::Consume,
                    topic: "user-events".to_string(),
                    line: 35,
                },
            ],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&producer).unwrap();
        index.index_method(&consumer).unwrap();
        
        // 创建配置数据
        let mut config_data = crate::config_parser::ConfigData::default();
        config_data.kafka_topics.push("user-events".to_string());
        
        // 关联配置
        index.associate_config_data(&config_data);
        
        // 验证关联
        let associated = index.find_config_associations("kafka:topic:user-events");
        assert_eq!(associated.len(), 2);
        assert!(associated.contains(&"com.example.Producer::sendEvent"));
        assert!(associated.contains(&"com.example.Consumer::handleEvent"));
    }
    
    #[test]
    fn test_associate_db_table_config() {
        let mut index = CodeIndex::new();
        
        // 添加数据库读取者和写入者
        let reader = MethodInfo {
            name: "getUser".to_string(),
            full_qualified_name: "com.example.UserDao::getUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![
                crate::types::DbOperation {
                    operation_type: DbOpType::Select,
                    table: "users".to_string(),
                    line: 15,
                },
            ],
            redis_operations: vec![],
        };
        
        let writer = MethodInfo {
            name: "saveUser".to_string(),
            full_qualified_name: "com.example.UserDao::saveUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (30, 40),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![
                crate::types::DbOperation {
                    operation_type: DbOpType::Insert,
                    table: "users".to_string(),
                    line: 35,
                },
            ],
            redis_operations: vec![],
        };
        
        index.index_method(&reader).unwrap();
        index.index_method(&writer).unwrap();
        
        // 创建配置数据
        let mut config_data = crate::config_parser::ConfigData::default();
        config_data.db_tables.push("users".to_string());
        
        // 关联配置
        index.associate_config_data(&config_data);
        
        // 验证关联
        let associated = index.find_config_associations("db:table:users");
        assert_eq!(associated.len(), 2);
        assert!(associated.contains(&"com.example.UserDao::getUser"));
        assert!(associated.contains(&"com.example.UserDao::saveUser"));
    }
    
    #[test]
    fn test_associate_redis_prefix_config() {
        let mut index = CodeIndex::new();
        
        // 添加 Redis 读取者和写入者
        let reader = MethodInfo {
            name: "getCache".to_string(),
            full_qualified_name: "com.example.Cache::getCache".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![
                crate::types::RedisOperation {
                    operation_type: RedisOpType::Get,
                    key_pattern: "user:*".to_string(),
                    line: 15,
                },
            ],
        };
        
        let writer = MethodInfo {
            name: "setCache".to_string(),
            full_qualified_name: "com.example.Cache::setCache".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (30, 40),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![
                crate::types::RedisOperation {
                    operation_type: RedisOpType::Set,
                    key_pattern: "user:*".to_string(),
                    line: 35,
                },
            ],
        };
        
        index.index_method(&reader).unwrap();
        index.index_method(&writer).unwrap();
        
        // 创建配置数据
        let mut config_data = crate::config_parser::ConfigData::default();
        config_data.redis_prefixes.push("user:*".to_string());
        
        // 关联配置
        index.associate_config_data(&config_data);
        
        // 验证关联
        let associated = index.find_config_associations("redis:key:user:*");
        assert_eq!(associated.len(), 2);
        assert!(associated.contains(&"com.example.Cache::getCache"));
        assert!(associated.contains(&"com.example.Cache::setCache"));
    }
    
    #[test]
    fn test_redis_key_pattern_matching() {
        let index = CodeIndex::new();
        
        // 测试精确匹配
        assert!(index.redis_key_matches("user:123", "user:123"));
        
        // 测试通配符匹配
        assert!(index.redis_key_matches("user:*", "user:123"));
        assert!(index.redis_key_matches("user:*", "user:456"));
        assert!(index.redis_key_matches("user:123", "user:*"));
        
        // 测试不匹配
        assert!(!index.redis_key_matches("user:*", "order:123"));
        assert!(!index.redis_key_matches("session:*", "user:123"));
    }
    
    #[test]
    fn test_http_path_matching() {
        let index = CodeIndex::new();
        
        // 测试简单路径匹配
        assert!(index.path_matches("/api/users", "RestTemplate.get(/api/users)"));
        
        // 测试带参数的路径匹配
        assert!(index.path_matches("/api/users/{id}", "RestTemplate.get(/api/users/123)"));
        
        // 测试多级路径匹配
        assert!(index.path_matches("/api/v1/users/{id}/orders", 
                                   "HttpClient.get(/api/v1/users/123/orders)"));
        
        // 测试不匹配
        assert!(!index.path_matches("/api/users", "RestTemplate.get(/api/orders)"));
    }
    
    #[test]
    fn test_associate_http_endpoint_config() {
        let mut index = CodeIndex::new();
        
        use crate::types::HttpMethod;
        
        // 添加 HTTP 提供者
        let provider = MethodInfo {
            name: "getUser".to_string(),
            full_qualified_name: "com.example.UserController::getUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (10, 20),
            calls: vec![],
            http_annotations: Some(HttpAnnotation {
                method: HttpMethod::GET,
                path: "/api/users/{id}".to_string(),
                path_params: vec!["id".to_string()],
                is_feign_client: false,
            }),
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        // 添加 HTTP 消费者（调用该接口的方法）
        let consumer = MethodInfo {
            name: "fetchUser".to_string(),
            full_qualified_name: "com.example.UserClient::fetchUser".to_string(),
            file_path: std::path::PathBuf::from("test.java"),
            line_range: (30, 40),
            calls: vec![
                MethodCall {
                    target: "RestTemplate.get(/api/users)".to_string(),
                    line: 35,
                },
            ],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        index.index_method(&provider).unwrap();
        index.index_method(&consumer).unwrap();
        
        // 创建配置数据
        let mut config_data = crate::config_parser::ConfigData::default();
        config_data.http_endpoints.push(HttpEndpoint {
            method: HttpMethod::GET,
            path_pattern: "/api/users/{id}".to_string(),
        });
        
        // 关联配置
        index.associate_config_data(&config_data);
        
        // 验证 HTTP 消费者已被索引
        let endpoint = HttpEndpoint {
            method: HttpMethod::GET,
            path_pattern: "/api/users/{id}".to_string(),
        };
        let consumers = index.find_http_consumers(&endpoint);
        assert_eq!(consumers.len(), 1);
        assert!(consumers.contains(&"com.example.UserClient::fetchUser"));
        
        // 验证配置关联
        let associated = index.find_config_associations("http:GET:/api/users/{id}");
        assert_eq!(associated.len(), 1);
        assert!(associated.contains(&"com.example.UserClient::fetchUser"));
    }
}
