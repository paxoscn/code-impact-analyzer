use std::path::{Path, PathBuf};
use std::fs;
use rayon::prelude::*;
use indicatif::{ProgressBar, ProgressStyle, ParallelProgressIterator};
use rustc_hash::FxHashMap;
use crate::errors::IndexError;
use crate::language_parser::{LanguageParser, LanguageDetector, ParsedFile, MethodInfo, FunctionInfo};
use crate::types::{HttpAnnotation, HttpEndpoint};

/// 代码索引
/// 
/// 构建全局代码索引，支持快速查询方法调用关系和跨服务资源
/// 
/// 使用 FxHashMap 替代标准 HashMap 以提升性能（约 20-30% 提升）
pub struct CodeIndex {
    /// 方法信息映射: qualified_name -> MethodInfo
    methods: FxHashMap<String, MethodInfo>,
    
    /// 方法调用映射: caller -> [callees]
    method_calls: FxHashMap<String, Vec<String>>,
    
    /// 反向调用映射: callee -> [callers]
    reverse_calls: FxHashMap<String, Vec<String>>,
    
    /// HTTP 提供者映射: endpoint -> provider_method
    http_providers: FxHashMap<HttpEndpoint, String>,
    
    /// HTTP 消费者映射: endpoint -> [consumer_methods]
    http_consumers: FxHashMap<HttpEndpoint, Vec<String>>,
    
    /// Kafka 生产者映射: topic -> [producer_methods]
    kafka_producers: FxHashMap<String, Vec<String>>,
    
    /// Kafka 消费者映射: topic -> [consumer_methods]
    kafka_consumers: FxHashMap<String, Vec<String>>,
    
    /// 数据库写入者映射: table -> [writer_methods]
    db_writers: FxHashMap<String, Vec<String>>,
    
    /// 数据库读取者映射: table -> [reader_methods]
    db_readers: FxHashMap<String, Vec<String>>,
    
    /// Redis 写入者映射: key_prefix -> [writer_methods]
    redis_writers: FxHashMap<String, Vec<String>>,
    
    /// Redis 读取者映射: key_prefix -> [reader_methods]
    redis_readers: FxHashMap<String, Vec<String>>,
    
    /// 配置关联映射: 配置值 -> 使用该配置的方法列表
    /// 用于追踪从配置文件中读取的值在代码中的使用
    config_associations: FxHashMap<String, Vec<String>>,
    
    /// 接口到实现类的映射: interface_name -> [implementation_class_names]
    interface_implementations: FxHashMap<String, Vec<String>>,
    
    /// 实现类到接口的映射: implementation_class_name -> [interface_names]
    class_interfaces: FxHashMap<String, Vec<String>>,
    
    /// 子类到父类的映射: child_class_name -> parent_class_name
    class_inheritance: FxHashMap<String, String>,
    
    /// 父类到子类的映射: parent_class_name -> [child_class_names]
    parent_children: FxHashMap<String, Vec<String>>,
}

impl CodeIndex {
    /// 创建新的代码索引
    pub fn new() -> Self {
        Self {
            methods: FxHashMap::default(),
            method_calls: FxHashMap::default(),
            reverse_calls: FxHashMap::default(),
            http_providers: FxHashMap::default(),
            http_consumers: FxHashMap::default(),
            kafka_producers: FxHashMap::default(),
            kafka_consumers: FxHashMap::default(),
            db_writers: FxHashMap::default(),
            db_readers: FxHashMap::default(),
            redis_writers: FxHashMap::default(),
            redis_readers: FxHashMap::default(),
            config_associations: FxHashMap::default(),
            interface_implementations: FxHashMap::default(),
            class_interfaces: FxHashMap::default(),
            class_inheritance: FxHashMap::default(),
            parent_children: FxHashMap::default(),
        }
    }
    
    /// 索引整个工作空间（两遍策略，支持跨文件类型推断）
    /// 
    /// 第一遍：快速解析所有文件，提取方法签名和返回类型
    /// 第二遍：使用全局返回类型映射重新解析，推断跨文件的方法调用参数类型
    /// 
    /// # Arguments
    /// * `workspace_path` - 工作空间根目录路径
    /// * `parsers` - 语言解析器列表
    /// 
    /// # Returns
    /// * `Ok(())` - 索引构建成功
    /// * `Err(IndexError)` - 索引构建失败
    pub fn index_workspace_two_pass(
        &mut self,
        workspace_path: &Path,
        parsers: &[Box<dyn LanguageParser>],
    ) -> Result<(), IndexError> {
        log::info!("开始两遍索引工作空间...");
        
        // 收集所有源文件
        let source_files = self.collect_source_files(workspace_path)?;
        let total_files = source_files.len();
        
        log::info!("找到 {} 个源文件", total_files);
        
        // ===== 第一遍：快速解析，提取方法签名和返回类型 =====
        log::info!("第一遍：提取方法签名和返回类型...");
        
        let pb1 = ProgressBar::new(total_files as u64);
        pb1.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        pb1.set_message("第一遍解析");
        
        let parsed_files_pass1: Vec<ParsedFile> = source_files
            .par_iter()
            .progress_with(pb1.clone())
            .filter_map(|file_path| {
                match self.parse_file(file_path, parsers) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        log::warn!("解析失败 {}: {}", file_path.display(), e);
                        None
                    }
                }
            })
            .collect();
        
        pb1.finish_with_message(format!("第一遍完成：{}/{} 个文件", parsed_files_pass1.len(), total_files));
        
        // 构建全局返回类型映射和全局类索引
        log::info!("构建全局返回类型映射和类索引...");
        let mut global_return_types = rustc_hash::FxHashMap::default();
        let mut global_class_index = rustc_hash::FxHashMap::default();
        
        for parsed_file in &parsed_files_pass1 {
            for class in &parsed_file.classes {
                // 添加类到全局索引
                global_class_index.insert(class.name.clone(), class.name.clone());
                
                for method in &class.methods {
                    if let Some(return_type) = &method.return_type {
                        global_return_types.insert(method.full_qualified_name.clone(), return_type.clone());
                    }
                }
            }
        }
        
        log::info!("全局返回类型映射包含 {} 个方法", global_return_types.len());
        log::info!("全局类索引包含 {} 个类", global_class_index.len());
        
        // ===== 第二遍：使用全局返回类型映射重新解析 =====
        log::info!("第二遍：使用全局类型信息重新解析...");
        
        let pb2 = ProgressBar::new(total_files as u64);
        pb2.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        pb2.set_message("第二遍解析");
        
        // 找到 Java 解析器
        let java_parser = parsers.iter()
            .find(|p| p.language_name() == "java")
            .ok_or_else(|| IndexError::UnsupportedLanguage {
                file: workspace_path.to_path_buf(),
            })?;
        
        // 将 Box<dyn LanguageParser> 转换为 &JavaParser
        let java_parser_ref = java_parser.as_ref() as *const dyn LanguageParser as *const crate::java_parser::JavaParser;
        let java_parser_concrete = unsafe { &*java_parser_ref };
        
        let parsed_files_pass2: Vec<ParsedFile> = source_files
            .par_iter()
            .progress_with(pb2.clone())
            .filter_map(|file_path| {
                // 只对 Java 文件使用两遍解析
                if file_path.extension().and_then(|e| e.to_str()) == Some("java") {
                    // 读取文件内容
                    let content = match std::fs::read_to_string(file_path) {
                        Ok(c) => c,
                        Err(e) => {
                            log::warn!("读取文件失败 {}: {}", file_path.display(), e);
                            return None;
                        }
                    };
                    
                    // 使用全局返回类型映射和类索引解析
                    match java_parser_concrete.parse_file_with_global_types_and_classes(&content, file_path, &global_return_types, &global_class_index) {
                        Ok(parsed) => Some(parsed),
                        Err(e) => {
                            log::warn!("第二遍解析失败 {}: {:?}", file_path.display(), e);
                            None
                        }
                    }
                } else {
                    // 非 Java 文件使用第一遍的结果
                    parsed_files_pass1.iter()
                        .find(|p| p.file_path == *file_path)
                        .cloned()
                }
            })
            .collect();
        
        pb2.finish_with_message(format!("第二遍完成：{}/{} 个文件", parsed_files_pass2.len(), total_files));
        
        // ===== 构建索引 =====
        log::info!("构建最终索引...");
        
        let index_pb = ProgressBar::new(parsed_files_pass2.len() as u64);
        index_pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        index_pb.set_message("构建索引");
        
        for parsed_file in parsed_files_pass2 {
            if let Err(e) = self.index_parsed_file(parsed_file) {
                log::warn!("索引文件失败: {}", e);
            }
            index_pb.inc(1);
        }
        
        index_pb.finish_with_message("索引构建完成");
        
        // ===== 传播继承的成员 =====
        log::info!("传播继承的成员...");
        self.propagate_inherited_members();
        
        // ===== 传播接口的HTTP注解 =====
        log::info!("传播接口的HTTP注解...");
        self.propagate_interface_http_annotations();
        
        // ===== 传播多态调用 =====
        log::info!("传播多态调用...");
        self.propagate_polymorphic_calls();
        
        log::info!("两遍索引完成：");
        log::info!("  - 方法总数: {}", self.methods.len());
        log::info!("  - 方法调用关系: {}", self.method_calls.len());
        log::info!("  - HTTP 提供者: {}", self.http_providers.len());
        log::info!("  - HTTP 消费者: {}", self.http_consumers.len());
        log::info!("  - Kafka 生产者: {}", self.kafka_producers.len());
        log::info!("  - Kafka 消费者: {}", self.kafka_consumers.len());
        log::info!("  - 接口实现关系: {}", self.interface_implementations.len());
        log::info!("  - 继承关系: {}", self.class_inheritance.len());
        
        Ok(())
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
        log::info!("开始收集源文件...");
        
        // 遍历工作空间中的所有文件
        let source_files = self.collect_source_files(workspace_path)?;
        let total_files = source_files.len();
        
        log::info!("找到 {} 个源文件，开始并行解析...", total_files);
        
        // 创建进度条
        let pb = ProgressBar::new(total_files as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        pb.set_message("解析源文件");
        
        // 使用 rayon 并行解析所有源文件，并显示进度
        let parsed_files: Vec<ParsedFile> = source_files
            .par_iter()
            .progress_with(pb.clone())
            .filter_map(|file_path| {
                match self.parse_file(file_path, parsers) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        // 记录错误但继续处理其他文件
                        log::warn!("解析失败 {}: {}", file_path.display(), e);
                        None
                    }
                }
            })
            .collect();
        
        pb.finish_with_message(format!("解析完成：{}/{} 个文件", parsed_files.len(), total_files));
        
        // 创建索引构建进度条
        let index_pb = ProgressBar::new(parsed_files.len() as u64);
        index_pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        index_pb.set_message("构建索引");
        
        log::info!("开始构建索引，处理 {} 个已解析文件...", parsed_files.len());
        
        // 串行构建索引（确保线程安全）
        for parsed_file in parsed_files {
            if let Err(e) = self.index_parsed_file(parsed_file) {
                log::warn!("索引文件失败: {}", e);
            }
            index_pb.inc(1);
        }
        
        index_pb.finish_with_message("索引构建完成");
        
        // 传播继承的成员
        log::info!("传播继承的成员...");
        self.propagate_inherited_members();
        
        // 传播多态调用
        log::info!("传播多态调用...");
        self.propagate_polymorphic_calls();
        
        log::info!("索引构建完成：");
        log::info!("  - 方法总数: {}", self.methods.len());
        log::info!("  - 方法调用关系: {}", self.method_calls.len());
        log::info!("  - HTTP 提供者: {}", self.http_providers.len());
        log::info!("  - HTTP 消费者: {}", self.http_consumers.len());
        log::info!("  - Kafka 生产者: {}", self.kafka_producers.len());
        log::info!("  - Kafka 消费者: {}", self.kafka_consumers.len());
        log::info!("  - 接口实现关系: {}", self.interface_implementations.len());
        log::info!("  - 继承关系: {}", self.class_inheritance.len());
        
        Ok(())
    }
    
    /// 索引单个项目（两遍策略，支持跨文件类型推断）
    /// 
    /// # Arguments
    /// * `project_path` - 项目目录路径
    /// * `parsers` - 语言解析器列表
    /// 
    /// # Returns
    /// * `Ok(())` - 索引构建成功
    /// * `Err(IndexError)` - 索引构建失败
    pub fn index_project_two_pass(
        &mut self,
        project_path: &Path,
        parsers: &[Box<dyn LanguageParser>],
    ) -> Result<(), IndexError> {
        log::info!("开始两遍索引项目: {}", project_path.display());
        
        // 收集项目中的源文件
        let source_files = self.collect_source_files(project_path)?;
        let total_files = source_files.len();
        
        if total_files == 0 {
            log::warn!("项目中没有找到源文件: {}", project_path.display());
            return Ok(());
        }
        
        log::info!("找到 {} 个源文件", total_files);
        
        // ===== 第一遍：快速解析，提取方法签名和返回类型 =====
        log::info!("第一遍：提取方法签名和返回类型...");
        
        let pb1 = ProgressBar::new(total_files as u64);
        pb1.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        pb1.set_message("第一遍解析");
        
        let parsed_files_pass1: Vec<ParsedFile> = source_files
            .par_iter()
            .progress_with(pb1.clone())
            .filter_map(|file_path| {
                match self.parse_file(file_path, parsers) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        log::warn!("解析失败 {}: {}", file_path.display(), e);
                        None
                    }
                }
            })
            .collect();
        
        pb1.finish_with_message(format!("第一遍完成：{}/{} 个文件", parsed_files_pass1.len(), total_files));
        
        // 构建全局返回类型映射和全局类索引
        log::info!("构建全局返回类型映射和类索引...");
        let mut global_return_types = rustc_hash::FxHashMap::default();
        let mut global_class_index = rustc_hash::FxHashMap::default();
        
        for parsed_file in &parsed_files_pass1 {
            for class in &parsed_file.classes {
                // 添加类到全局索引
                global_class_index.insert(class.name.clone(), class.name.clone());
                
                for method in &class.methods {
                    if let Some(return_type) = &method.return_type {
                        global_return_types.insert(method.full_qualified_name.clone(), return_type.clone());
                    }
                }
            }
        }
        
        log::info!("全局返回类型映射包含 {} 个方法", global_return_types.len());
        log::info!("全局类索引包含 {} 个类", global_class_index.len());
        
        // ===== 第二遍：使用全局返回类型映射重新解析 =====
        log::info!("第二遍：使用全局类型信息重新解析...");
        
        let pb2 = ProgressBar::new(total_files as u64);
        pb2.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        pb2.set_message("第二遍解析");
        
        // 找到 Java 解析器
        let java_parser = parsers.iter()
            .find(|p| p.language_name() == "java")
            .ok_or_else(|| IndexError::UnsupportedLanguage {
                file: project_path.to_path_buf(),
            })?;
        
        // 将 Box<dyn LanguageParser> 转换为 &JavaParser
        let java_parser_ref = java_parser.as_ref() as *const dyn LanguageParser as *const crate::java_parser::JavaParser;
        let java_parser_concrete = unsafe { &*java_parser_ref };
        
        let parsed_files_pass2: Vec<ParsedFile> = source_files
            .par_iter()
            .progress_with(pb2.clone())
            .filter_map(|file_path| {
                // 只对 Java 文件使用两遍解析
                if file_path.extension().and_then(|e| e.to_str()) == Some("java") {
                    // 读取文件内容
                    let content = match std::fs::read_to_string(file_path) {
                        Ok(c) => c,
                        Err(e) => {
                            log::warn!("读取文件失败 {}: {}", file_path.display(), e);
                            return None;
                        }
                    };
                    
                    // 使用全局返回类型映射和类索引解析
                    match java_parser_concrete.parse_file_with_global_types_and_classes(&content, file_path, &global_return_types, &global_class_index) {
                        Ok(parsed) => Some(parsed),
                        Err(e) => {
                            log::warn!("第二遍解析失败 {}: {:?}", file_path.display(), e);
                            None
                        }
                    }
                } else {
                    // 非 Java 文件使用第一遍的结果
                    parsed_files_pass1.iter()
                        .find(|p| p.file_path == *file_path)
                        .cloned()
                }
            })
            .collect();
        
        pb2.finish_with_message(format!("第二遍完成：{}/{} 个文件", parsed_files_pass2.len(), total_files));
        
        // ===== 构建索引 =====
        log::info!("构建最终索引...");
        
        let index_pb = ProgressBar::new(parsed_files_pass2.len() as u64);
        index_pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        index_pb.set_message("构建索引");
        
        for parsed_file in parsed_files_pass2 {
            if let Err(e) = self.index_parsed_file(parsed_file) {
                log::warn!("索引文件失败: {}", e);
            }
            index_pb.inc(1);
        }
        
        index_pb.finish_with_message("索引构建完成");
        
        // ===== 传播继承的成员 =====
        log::info!("传播继承的成员...");
        self.propagate_inherited_members();
        
        // ===== 传播接口的HTTP注解 =====
        log::info!("传播接口的HTTP注解...");
        self.propagate_interface_http_annotations();
        
        // ===== 传播多态调用 =====
        log::info!("传播多态调用...");
        self.propagate_polymorphic_calls();
        
        log::info!("项目两遍索引完成: {}", project_path.display());
        log::info!("  - 方法总数: {}", self.methods.len());
        log::info!("  - 方法调用关系: {}", self.method_calls.len());
        log::info!("  - HTTP 提供者: {}", self.http_providers.len());
        log::info!("  - HTTP 消费者: {}", self.http_consumers.len());
        log::info!("  - Kafka 生产者: {}", self.kafka_producers.len());
        log::info!("  - Kafka 消费者: {}", self.kafka_consumers.len());
        log::info!("  - 接口实现关系: {}", self.interface_implementations.len());
        
        Ok(())
    }
    
    /// 索引单个项目
    /// 
    /// # Arguments
    /// * `project_path` - 项目目录路径
    /// * `parsers` - 语言解析器列表
    /// 
    /// # Returns
    /// * `Ok(())` - 索引构建成功
    /// * `Err(IndexError)` - 索引构建失败
    pub fn index_project(
        &mut self,
        project_path: &Path,
        parsers: &[Box<dyn LanguageParser>],
    ) -> Result<(), IndexError> {
        log::info!("开始索引项目: {}", project_path.display());
        
        // 收集项目中的源文件
        let source_files = self.collect_source_files(project_path)?;
        let total_files = source_files.len();
        
        if total_files == 0 {
            log::warn!("项目中没有找到源文件: {}", project_path.display());
            return Ok(());
        }
        
        log::info!("找到 {} 个源文件，开始并行解析...", total_files);
        
        // 创建进度条
        let pb = ProgressBar::new(total_files as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        pb.set_message(format!("解析 {}", project_path.file_name().unwrap_or_default().to_string_lossy()));
        
        // 使用 rayon 并行解析所有源文件
        let parsed_files: Vec<ParsedFile> = source_files
            .par_iter()
            .progress_with(pb.clone())
            .filter_map(|file_path| {
                match self.parse_file(file_path, parsers) {
                    Ok(parsed) => {
                        // println!("parsed: {:?}", file_path);
                        Some(parsed)
                    },
                    Err(e) => {
                        log::warn!("解析失败 {}: {}", file_path.display(), e);
                        None
                    }
                }
            })
            .collect();
        
        log::info!("111");
        pb.finish_with_message(format!("解析完成：{}/{} 个文件", parsed_files.len(), total_files));
        
        // 创建索引构建进度条
        let index_pb = ProgressBar::new(parsed_files.len() as u64);
        index_pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-")
        );
        index_pb.set_message("构建索引");
        
        log::info!("开始构建索引，处理 {} 个已解析文件...", parsed_files.len());
        
        // 串行构建索引
        for parsed_file in parsed_files {
            if let Err(e) = self.index_parsed_file(parsed_file) {
                log::warn!("索引文件失败: {}", e);
            }
            index_pb.inc(1);
        }
        
        index_pb.finish_with_message("索引构建完成");
        
        log::info!("项目索引完成: {}", project_path.display());
        log::info!("  - 方法总数: {}", self.methods.len());
        log::info!("  - 方法调用关系: {}", self.method_calls.len());
        log::info!("  - HTTP 提供者: {}", self.http_providers.len());
        log::info!("  - HTTP 消费者: {}", self.http_consumers.len());
        log::info!("  - Kafka 生产者: {}", self.kafka_producers.len());
        log::info!("  - Kafka 消费者: {}", self.kafka_consumers.len());
        log::info!("  - 接口实现关系: {}", self.interface_implementations.len());
        
        Ok(())
    }
    
    /// 解析单个文件
    /// 
    /// 此方法设计为线程安全，可以在多个线程中并行调用
    fn parse_file(
        &self,
        file_path: &Path,
        parsers: &[Box<dyn LanguageParser>],
    ) -> Result<ParsedFile, IndexError> {
        // println!("parsing: {:?}", file_path);
        // 读取文件内容
        let content = fs::read_to_string(file_path)
            .map_err(|e| IndexError::IoError {
                path: file_path.to_path_buf(),
                error: e.to_string(),
            })?;
        
        // 选择合适的解析器
        let parser = self.select_parser(file_path, parsers)
            .ok_or_else(|| IndexError::UnsupportedLanguage {
                file: file_path.to_path_buf(),
            })?;
        
        // 解析文件
        parser.parse_file(&content, file_path)
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
                    // 正向映射：接口 -> 实现类
                    self.interface_implementations
                        .entry(interface_name.clone())
                        .or_insert_with(Vec::new)
                        .push(class.name.clone());
                    
                    // 反向映射：实现类 -> 接口
                    self.class_interfaces
                        .entry(class.name.clone())
                        .or_insert_with(Vec::new)
                        .push(interface_name.clone());
                }
            }
            
            // 索引继承关系
            if let Some(parent_class) = &class.extends {
                // 子类 -> 父类映射
                self.class_inheritance.insert(class.name.clone(), parent_class.clone());
                
                // 父类 -> 子类映射
                self.parent_children
                    .entry(parent_class.clone())
                    .or_insert_with(Vec::new)
                    .push(class.name.clone());
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
            return_type: function.return_type.clone(),
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
    
    /// 查找方法的返回类型
    /// 
    /// # Arguments
    /// * `method_signature` - 方法的完整签名（如 "com.example.UserService::getUser(String)"）
    /// 
    /// # Returns
    /// * `Some(return_type)` - 如果找到方法且有返回类型
    /// * `None` - 如果方法不存在或没有返回类型信息
    pub fn find_method_return_type(&self, method_signature: &str) -> Option<&str> {
        self.methods
            .get(method_signature)
            .and_then(|method| method.return_type.as_deref())
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
    
    /// 获取所有接口实现映射
    /// 
    /// # Returns
    /// 返回一个迭代器，遍历所有 (接口名, 实现类列表) 对
    pub fn interface_implementations(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.interface_implementations.iter()
    }
    
    /// 获取所有类接口映射
    /// 
    /// # Returns
    /// 返回一个迭代器，遍历所有 (类名, 接口列表) 对
    pub fn class_interfaces(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.class_interfaces.iter()
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
    
    /// 查找类实现的所有接口
    /// 
    /// # Arguments
    /// * `class_name` - 类的完整类名
    /// 
    /// # Returns
    /// 该类实现的所有接口的完整类名列表
    pub fn find_class_interfaces(&self, class_name: &str) -> Vec<&str> {
        self.class_interfaces
            .get(class_name)
            .map(|interfaces| interfaces.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
    /// 传播父类的成员到子类
    ///
    /// 当类A继承类B时，A应该能够访问B的所有方法和接口
    /// 这个方法在所有文件索引完成后调用，确保继承链中的所有成员都被正确传播
    /// 传播父类的成员到子类
    /// 
    /// 当类A继承类B时，A应该能够访问B的所有方法和接口
    /// 这个方法在所有文件索引完成后调用，确保继承链中的所有成员都被正确传播
    pub fn propagate_inherited_members(&mut self) {
        // 收集所有需要传播的信息
        let mut propagations = Vec::new();

        for (child_class, _) in &self.class_inheritance {
            // 收集所有祖先类的接口（递归）
            let mut ancestor_interfaces = Vec::new();
            self.collect_all_interfaces(child_class, &mut ancestor_interfaces);

            // 收集所有祖先类的方法（递归）
            let mut ancestor_methods = Vec::new();
            self.collect_all_ancestor_methods(child_class, &mut ancestor_methods);

            propagations.push((child_class.clone(), ancestor_interfaces, ancestor_methods));
        }

        // 应用传播
        for (child_class, ancestor_interfaces, ancestor_methods) in propagations {
            // 传播接口
            for interface in ancestor_interfaces {
                // 添加到子类的接口列表（避免重复）
                let class_interfaces = self.class_interfaces
                    .entry(child_class.clone())
                    .or_insert_with(Vec::new);
                if !class_interfaces.contains(&interface) {
                    class_interfaces.push(interface.clone());
                }

                // 添加到接口的实现类列表（避免重复）
                let interface_impls = self.interface_implementations
                    .entry(interface)
                    .or_insert_with(Vec::new);
                if !interface_impls.contains(&child_class) {
                    interface_impls.push(child_class.clone());
                }
            }

            // 传播方法（为子类创建方法的别名）
            for ancestor_method_name in ancestor_methods {
                if let Some(ancestor_method) = self.methods.get(&ancestor_method_name).cloned() {
                    // 创建子类的方法名
                    // 从 AncestorClass::methodName(params) 转换为 ChildClass::methodName(params)
                    if let Some(pos) = ancestor_method_name.find("::") {
                        let method_signature = &ancestor_method_name[pos..];
                        let child_method_name = format!("{}{}", child_class, method_signature);

                        // 只有当子类没有重写这个方法时才添加
                        if !self.methods.contains_key(&child_method_name) {
                            // 创建一个新的方法信息，指向祖先的方法
                            let mut child_method = ancestor_method.clone();
                            child_method.full_qualified_name = child_method_name.clone();

                            // 索引这个方法
                            let _ = self.index_method(&child_method);
                        }
                    }
                }
            }
        }
    }
    /// 传播接口的HTTP注解到实现类
    ///
    /// 当类实现了接口，且接口方法有HTTP注解时，将注解传播到实现类的同名方法
    pub fn propagate_interface_http_annotations(&mut self) {
        let mut updates = Vec::new();

        // 遍历所有实现了接口的类
        for (class_name, interfaces) in &self.class_interfaces {
            // 遍历该类的所有接口
            for interface_name in interfaces {
                // 查找接口的所有方法
                let interface_methods: Vec<String> = self.methods
                    .keys()
                    .filter(|method_name| method_name.starts_with(&format!("{}::", interface_name)))
                    .cloned()
                    .collect();

                // 遍历接口的每个方法
                for interface_method_name in interface_methods {
                    if let Some(interface_method) = self.methods.get(&interface_method_name) {
                        // 如果接口方法有HTTP注解
                        if let Some(http_annotation) = &interface_method.http_annotations {
                            // 构建实现类的对应方法名
                            // 从 InterfaceName::methodName(params) 转换为 ClassName::methodName(params)
                            if let Some(pos) = interface_method_name.find("::") {
                                let method_signature = &interface_method_name[pos..];
                                let impl_method_name = format!("{}{}", class_name, method_signature);

                                // 检查实现类是否有这个方法
                                if let Some(impl_method) = self.methods.get(&impl_method_name) {
                                    // 如果实现类的方法没有HTTP注解，则从接口继承
                                    if impl_method.http_annotations.is_none() {
                                        updates.push((impl_method_name.clone(), http_annotation.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 应用更新
        for (method_name, http_annotation) in updates {
            if let Some(method) = self.methods.get_mut(&method_name) {
                method.http_annotations = Some(http_annotation.clone());
                
                // 同时更新HTTP提供者索引
                let endpoint = crate::types::HttpEndpoint {
                    method: http_annotation.method.clone(),
                    path_pattern: http_annotation.path.clone(),
                };
                self.http_providers.insert(endpoint, method_name.clone());
            }
        }
    }

    /// 传播多态调用
    ///
    /// 当类 X 调用了 foo(A)，且 A 继承自 B 时，为类 X 增加一个对 foo(B) 的调用
    /// 这样可以正确追踪多态性带来的影响
    pub fn propagate_polymorphic_calls(&mut self) {
        // 收集所有需要添加的多态调用
        let mut polymorphic_calls = Vec::new();

        // 遍历所有方法调用
        for (caller_method, callees) in &self.method_calls {
            for callee_method in callees {
                // 解析被调用方法的签名：ClassName::methodName(ParamType1,ParamType2,...)
                if let Some(polymorphic_callee) = self.find_polymorphic_variant(callee_method) {
                    polymorphic_calls.push((caller_method.clone(), polymorphic_callee));
                }
            }
        }

        // 应用多态调用
        for (caller_method, polymorphic_callee) in polymorphic_calls {
            // 添加到方法调用映射
            self.method_calls
                .entry(caller_method.clone())
                .or_insert_with(Vec::new)
                .push(polymorphic_callee.clone());

            // 添加到反向调用映射
            self.reverse_calls
                .entry(polymorphic_callee)
                .or_insert_with(Vec::new)
                .push(caller_method);
        }
    }

    /// 查找方法的多态变体
    ///
    /// TODO 返回笛卡尔积
    /// 对于方法 ClassName::methodName(ParamType1,ParamType2,...)
    /// 如果 ParamType1 继承自 BaseType1，则返回 ClassName::methodName(BaseType1,ParamType2,...)
    fn find_polymorphic_variant(&self, method_signature: &str) -> Option<String> {
        // 解析方法签名
        let parts: Vec<&str> = method_signature.split("::").collect();
        if parts.len() != 2 {
            return None;
        }

        let class_name = parts[0];
        let method_part = parts[1];

        // 解析方法名和参数
        if let Some(paren_pos) = method_part.find('(') {
            let method_name = &method_part[..paren_pos];
            let params_str = &method_part[paren_pos + 1..];

            // 移除结尾的 ')'
            let params_str = params_str.trim_end_matches(')');

            if params_str.is_empty() {
                // 无参数方法，没有多态变体
                return None;
            }

            // 分割参数类型
            let param_types: Vec<&str> = params_str.split(',').collect();

            // 检查每个参数类型是否有父类
            for (i, param_type) in param_types.iter().enumerate() {
                if let Some(parent_type) = self.class_inheritance.get(*param_type) {
                    // 找到了一个有父类的参数类型，创建多态变体
                    let mut new_param_types = param_types.clone();
                    new_param_types[i] = parent_type;

                    let new_signature = format!(
                        "{}::{}({})",
                        class_name,
                        method_name,
                        new_param_types.join(",")
                    );

                    // 检查这个多态变体是否存在
                    if self.methods.contains_key(&new_signature) {
                        return Some(new_signature);
                    }
                }
            }
        }

        None
    }


    /// 递归收集类的所有祖先方法
    fn collect_all_ancestor_methods(&self, class_name: &str, result: &mut Vec<String>) {
        // 递归处理父类
        if let Some(parent_class) = self.class_inheritance.get(class_name) {
            // 添加父类的方法
            let parent_methods: Vec<String> = self.methods
                .keys()
                .filter(|method_name| {
                    // 方法名格式：ClassName::methodName(params)
                    method_name.starts_with(&format!("{}::", parent_class))
                })
                .cloned()
                .collect();

            for method in parent_methods {
                if !result.contains(&method) {
                    result.push(method);
                }
            }

            // 递归处理祖先
            self.collect_all_ancestor_methods(parent_class, result);
        }
    }

    /// 递归收集类的所有接口（包括继承的接口）
    fn collect_all_interfaces(&self, class_name: &str, result: &mut Vec<String>) {
        // 添加当前类的接口
        if let Some(interfaces) = self.class_interfaces.get(class_name) {
            for interface in interfaces {
                if !result.contains(interface) {
                    result.push(interface.clone());
                }
            }
        }

        // 递归处理父类
        if let Some(parent_class) = self.class_inheritance.get(class_name) {
            self.collect_all_interfaces(parent_class, result);
        }
    }

    /// 查找类的父类
    pub fn find_parent_class(&self, class_name: &str) -> Option<&str> {
        self.class_inheritance.get(class_name).map(|s| s.as_str())
    }

    /// 查找类的所有子类
    pub fn find_child_classes(&self, class_name: &str) -> Vec<&str> {
        self.parent_children
            .get(class_name)
            .map(|children| children.iter().map(|s| s.as_str()).collect())
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
    
    /// 内部方法：直接设置接口实现映射
    /// 
    /// 注意：此方法仅用于索引反序列化，不应在其他地方使用
    #[doc(hidden)]
    pub fn set_interface_implementations(&mut self, interface_implementations: FxHashMap<String, Vec<String>>) {
        self.interface_implementations = interface_implementations;
    }
    
    /// 内部方法：直接设置类接口映射
    /// 
    /// 注意：此方法仅用于索引反序列化，不应在其他地方使用
    #[doc(hidden)]
    pub fn set_class_interfaces(&mut self, class_interfaces: FxHashMap<String, Vec<String>>) {
        self.class_interfaces = class_interfaces;
    }
    /// 获取所有继承关系的迭代器
    pub fn class_inheritance(&self) -> impl Iterator<Item = (&String, &String)> {
        self.class_inheritance.iter()
    }

    /// 获取所有父子关系的迭代器
    pub fn parent_children(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.parent_children.iter()
    }

    /// 内部方法：直接设置继承关系映射
    ///
    /// 注意：此方法仅用于索引反序列化，不应在其他地方使用
    #[doc(hidden)]
    pub fn set_class_inheritance(&mut self, class_inheritance: FxHashMap<String, String>) {
        self.class_inheritance = class_inheritance;
    }

    /// 内部方法：直接设置父子关系映射
    ///
    /// 注意：此方法仅用于索引反序列化，不应在其他地方使用
    #[doc(hidden)]
    pub fn set_parent_children(&mut self, parent_children: FxHashMap<String, Vec<String>>) {
        self.parent_children = parent_children;
    }

    
    /// 测试辅助方法：索引解析后的文件
    /// 
    /// 注意：此方法仅用于测试目的，不应在生产代码中使用
    #[doc(hidden)]
    pub fn test_index_parsed_file(&mut self, parsed_file: ParsedFile) -> Result<(), IndexError> {
        self.index_parsed_file(parsed_file)
    }
    
    /// 合并另一个索引到当前索引
    /// 
    /// 用于将多个项目的索引合并为全局索引
    pub fn merge(&mut self, other: CodeIndex) {
        // 合并方法信息
        for (name, method) in other.methods {
            self.methods.insert(name, method);
        }
        
        // 合并方法调用关系
        for (caller, callees) in other.method_calls {
            self.method_calls.entry(caller)
                .or_insert_with(Vec::new)
                .extend(callees);
        }
        
        // 合并反向调用关系
        for (callee, callers) in other.reverse_calls {
            self.reverse_calls.entry(callee)
                .or_insert_with(Vec::new)
                .extend(callers);
        }
        
        // 合并 HTTP 提供者
        for (endpoint, provider) in other.http_providers {
            self.http_providers.insert(endpoint, provider);
        }
        
        // 合并 HTTP 消费者
        for (endpoint, consumers) in other.http_consumers {
            self.http_consumers.entry(endpoint)
                .or_insert_with(Vec::new)
                .extend(consumers);
        }
        
        // 合并 Kafka 生产者
        for (topic, producers) in other.kafka_producers {
            self.kafka_producers.entry(topic)
                .or_insert_with(Vec::new)
                .extend(producers);
        }
        
        // 合并 Kafka 消费者
        for (topic, consumers) in other.kafka_consumers {
            self.kafka_consumers.entry(topic)
                .or_insert_with(Vec::new)
                .extend(consumers);
        }
        
        // 合并数据库写入者
        for (table, writers) in other.db_writers {
            self.db_writers.entry(table)
                .or_insert_with(Vec::new)
                .extend(writers);
        }
        
        // 合并数据库读取者
        for (table, readers) in other.db_readers {
            self.db_readers.entry(table)
                .or_insert_with(Vec::new)
                .extend(readers);
        }
        
        // 合并 Redis 写入者
        for (prefix, writers) in other.redis_writers {
            self.redis_writers.entry(prefix)
                .or_insert_with(Vec::new)
                .extend(writers);
        }
        
        // 合并 Redis 读取者
        for (prefix, readers) in other.redis_readers {
            self.redis_readers.entry(prefix)
                .or_insert_with(Vec::new)
                .extend(readers);
        }
        
        // 合并配置关联
        for (config_key, methods) in other.config_associations {
            self.config_associations.entry(config_key)
                .or_insert_with(Vec::new)
                .extend(methods);
        }
        
        // 合并接口实现关系
        for (interface, implementations) in other.interface_implementations {
            self.interface_implementations.entry(interface)
                .or_insert_with(Vec::new)
                .extend(implementations);
        }
        
        // 合并类接口关系
        for (class, interfaces) in other.class_interfaces {
            self.class_interfaces.entry(class)
                .or_insert_with(Vec::new)
                .extend(interfaces);
        }
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
            return_type: None,
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
