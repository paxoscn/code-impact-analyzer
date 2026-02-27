use std::path::{Path, PathBuf};
use std::time::Instant;
use crate::errors::{AnalysisError, ParseError};
use crate::patch_parser::{PatchParser, FileChange};
use crate::code_index::CodeIndex;
use crate::impact_tracer::{ImpactTracer, TraceConfig, ImpactGraph};
use crate::language_parser::LanguageParser;
use crate::java_parser::JavaParser;
use crate::rust_parser::RustParser;
use crate::config_parser::{ConfigParser, XmlConfigParser, YamlConfigParser};
use crate::index_storage::IndexStorage;

/// 分析统计信息
#[derive(Debug, Clone)]
pub struct AnalysisStatistics {
    /// 处理的文件总数
    pub total_files: usize,
    /// 成功解析的文件数
    pub parsed_files: usize,
    /// 解析失败的文件数
    pub failed_files: usize,
    /// 识别的方法总数
    pub total_methods: usize,
    /// 追溯的调用链路数
    pub traced_chains: usize,
    /// 分析耗时（毫秒）
    pub duration_ms: u128,
}

impl AnalysisStatistics {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self {
            total_files: 0,
            parsed_files: 0,
            failed_files: 0,
            total_methods: 0,
            traced_chains: 0,
            duration_ms: 0,
        }
    }
}

impl Default for AnalysisStatistics {
    fn default() -> Self {
        Self::new()
    }
}

/// 分析结果
#[derive(Debug)]
pub struct AnalysisResult {
    /// 影响图
    pub impact_graph: ImpactGraph,
    /// 统计信息
    pub statistics: AnalysisStatistics,
    /// 警告列表
    pub warnings: Vec<String>,
    /// 错误列表
    pub errors: Vec<String>,
}

/// 分析编排器
/// 
/// 协调整个分析流程：解析 patch -> 构建索引 -> 追溯影响 -> 生成图
pub struct AnalysisOrchestrator {
    /// 工作空间路径
    workspace_path: PathBuf,
    /// 追溯配置
    trace_config: TraceConfig,
    /// 语言解析器列表
    parsers: Vec<Box<dyn LanguageParser>>,
    /// 配置解析器列表
    config_parsers: Vec<Box<dyn ConfigParser>>,
    /// 索引存储管理器
    index_storage: IndexStorage,
    /// 警告列表
    warnings: Vec<String>,
    /// 错误列表
    errors: Vec<String>,
    /// 是否强制重建索引
    force_rebuild: bool,
}

impl AnalysisOrchestrator {
    /// 创建新的分析编排器
    /// 
    /// # Arguments
    /// * `workspace_path` - 工作空间根目录路径
    /// * `trace_config` - 追溯配置
    /// 
    /// # Returns
    /// * `Result<Self, AnalysisError>` - 分析编排器实例或错误
    pub fn new(workspace_path: PathBuf, trace_config: TraceConfig) -> Result<Self, AnalysisError> {
        // 初始化语言解析器
        let mut parsers: Vec<Box<dyn LanguageParser>> = Vec::new();
        
        // 尝试创建 JavaParser
        match JavaParser::new() {
            Ok(parser) => parsers.push(Box::new(parser)),
            Err(e) => {
                log::warn!("Failed to initialize JavaParser: {}", e);
            }
        }
        
        // 尝试创建 RustParser
        match RustParser::new() {
            Ok(parser) => parsers.push(Box::new(parser)),
            Err(e) => {
                log::warn!("Failed to initialize RustParser: {}", e);
            }
        }
        
        // 初始化配置解析器
        let config_parsers: Vec<Box<dyn ConfigParser>> = vec![
            Box::new(XmlConfigParser),
            Box::new(YamlConfigParser),
        ];
        
        // 初始化索引存储管理器
        let index_storage = IndexStorage::new(workspace_path.clone());
        
        Ok(Self {
            workspace_path,
            trace_config,
            parsers,
            config_parsers,
            index_storage,
            warnings: Vec::new(),
            errors: Vec::new(),
            force_rebuild: false,
        })
    }
    
    /// 设置是否强制重建索引
    pub fn set_force_rebuild(&mut self, force: bool) {
        self.force_rebuild = force;
    }
    
    /// 执行完整的分析流程
    /// 
    /// # Arguments
    /// * `patch_dir` - Git patch 文件目录路径
    /// 
    /// # Returns
    /// * `Ok(AnalysisResult)` - 分析结果
    /// * `Err(AnalysisError)` - 分析错误
    pub fn analyze(&mut self, patch_dir: &Path) -> Result<AnalysisResult, AnalysisError> {
        let start_time = Instant::now();
        
        log::info!("Starting code impact analysis");
        log::info!("Workspace: {:?}", self.workspace_path);
        log::info!("Patch directory: {:?}", patch_dir);
        
        // 清空之前的警告和错误
        self.warnings.clear();
        self.errors.clear();
        
        // 步骤 1: 解析 patch 目录中的所有文件
        log::info!("Step 1: Parsing patch files from directory");
        let file_changes = self.parse_patches_from_directory(patch_dir)?;
        log::info!("Found {} file changes", file_changes.len());
        
        // 步骤 2: 构建代码索引
        log::info!("Step 2: Building code index");
        let code_index = self.build_index()?;
        log::info!("Index built successfully");
        
        // 步骤 3: 从 patch 中提取变更的方法
        log::info!("Step 3: Extracting changed methods from patch");
        let changed_methods = self.extract_changed_methods(&file_changes, &code_index)?;
        log::info!("Found {} changed methods", changed_methods.len());
        
        // 步骤 4: 追溯影响
        log::info!("Step 4: Tracing impact");
        let impact_graph = self.trace_impact(&changed_methods, &code_index)?;
        log::info!("Impact graph generated with {} nodes and {} edges", 
                   impact_graph.node_count(), impact_graph.edge_count());
        
        // 步骤 5: 收集统计信息
        let duration_ms = start_time.elapsed().as_millis();
        let statistics = AnalysisStatistics {
            total_files: file_changes.len(),
            parsed_files: file_changes.len() - self.errors.len(),
            failed_files: self.errors.len(),
            total_methods: changed_methods.len(),
            traced_chains: impact_graph.edge_count(),
            duration_ms,
        };
        
        log::info!("Analysis completed in {} ms", duration_ms);
        log::info!("Statistics: {:?}", statistics);
        
        // 返回分析结果
        Ok(AnalysisResult {
            impact_graph,
            statistics,
            warnings: self.warnings.clone(),
            errors: self.errors.clone(),
        })
    }
    
    /// 解析 patch 目录中的所有文件
    fn parse_patches_from_directory(&mut self, patch_dir: &Path) -> Result<Vec<FileChange>, AnalysisError> {
        // 检查路径是否存在
        if !patch_dir.exists() {
            let error_msg = format!("Patch directory does not exist: {:?}", patch_dir);
            self.errors.push(error_msg.clone());
            return Err(AnalysisError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                error_msg,
            )));
        }
        
        // 如果是文件，直接解析（向后兼容）
        if patch_dir.is_file() {
            log::warn!("--diff points to a file instead of directory, parsing single file for backward compatibility");
            return self.parse_patch(patch_dir, None);
        }
        
        // 如果是目录，遍历所有 .patch 文件
        if !patch_dir.is_dir() {
            let error_msg = format!("Patch path is neither a file nor a directory: {:?}", patch_dir);
            self.errors.push(error_msg.clone());
            return Err(AnalysisError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                error_msg,
            )));
        }
        
        let mut all_changes = Vec::new();
        let entries = std::fs::read_dir(patch_dir)
            .map_err(|e| {
                let error_msg = format!("Failed to read patch directory: {}", e);
                self.errors.push(error_msg.clone());
                AnalysisError::IoError(e)
            })?;
        
        let mut patch_files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| AnalysisError::IoError(e))?;
            let path = entry.path();
            
            // 只处理 .patch 文件
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("patch") {
                patch_files.push(path);
            }
        }
        
        if patch_files.is_empty() {
            let warning = format!("No .patch files found in directory: {:?}", patch_dir);
            log::warn!("{}", warning);
            self.warnings.push(warning);
            return Ok(Vec::new());
        }
        
        log::info!("Found {} patch files to process", patch_files.len());
        
        // 解析每个 patch 文件
        for patch_file in patch_files {
            log::info!("Processing patch file: {:?}", patch_file);
            
            // 从文件名提取项目名（去掉 .patch 扩展名）
            let project_name = patch_file
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            
            if let Some(ref name) = project_name {
                log::info!("  - Project name: {}", name);
            }
            
            match self.parse_patch(&patch_file, project_name) {
                Ok(mut changes) => {
                    log::info!("  - Parsed {} file changes from {:?}", changes.len(), patch_file.file_name().unwrap());
                    all_changes.append(&mut changes);
                }
                Err(e) => {
                    let warning = format!("Failed to parse patch file {:?}: {}", patch_file, e);
                    log::warn!("{}", warning);
                    self.warnings.push(warning);
                    // 继续处理其他文件，不中断整个流程
                }
            }
        }
        
        log::info!("Total file changes from all patches: {}", all_changes.len());
        Ok(all_changes)
    }
    
    /// 解析单个 patch 文件
    /// 
    /// # 参数
    /// * `patch_path` - patch 文件路径
    /// * `project_prefix` - 可选的项目名前缀，将添加到文件路径前
    fn parse_patch(&mut self, patch_path: &Path, project_prefix: Option<String>) -> Result<Vec<FileChange>, AnalysisError> {
        match PatchParser::parse_patch_file(patch_path) {
            Ok(mut changes) => {
                // 如果提供了项目前缀，添加到所有文件路径前
                if let Some(prefix) = project_prefix {
                    for change in &mut changes {
                        // 添加项目名作为目录前缀
                        change.file_path = format!("{}/{}", prefix, change.file_path);
                        log::debug!("  - Prefixed file path: {}", change.file_path);
                    }
                }
                Ok(changes)
            }
            Err(e) => {
                let error_msg = format!("Failed to parse patch file: {}", e);
                self.errors.push(error_msg.clone());
                Err(AnalysisError::PatchParseError(e))
            }
        }
    }
    
    /// 构建代码索引
    fn build_index(&mut self) -> Result<CodeIndex, AnalysisError> {
        // 如果强制重建，清除现有索引
        if self.force_rebuild {
            log::info!("Force rebuild enabled, clearing existing index");
            if let Err(e) = self.index_storage.clear_index() {
                log::warn!("Failed to clear index: {}", e);
            }
        }
        
        // 尝试加载现有索引
        if !self.force_rebuild {
            match self.index_storage.load_index() {
                Ok(Some(index)) => {
                    log::info!("Loaded existing index from cache");
                    return Ok(index);
                }
                Ok(None) => {
                    log::info!("No valid index found, building new index");
                }
                Err(e) => {
                    log::warn!("Failed to load index: {}, will rebuild", e);
                }
            }
        }
        
        // 构建新索引
        let mut index = CodeIndex::new();
        
        match index.index_workspace(&self.workspace_path, &self.parsers) {
            Ok(_) => {
                log::info!("Workspace indexed successfully");
                
                // 解析配置文件并关联到代码
                self.parse_and_associate_configs(&mut index);
                
                // 保存索引到磁盘
                if let Err(e) = self.index_storage.save_index(&index) {
                    log::warn!("Failed to save index: {}", e);
                    // 不中断流程，继续使用内存中的索引
                }
                
                Ok(index)
            }
            Err(e) => {
                let error_msg = format!("Failed to build index: {}", e);
                self.errors.push(error_msg.clone());
                Err(AnalysisError::IndexBuildError(e))
            }
        }
    }
    
    /// 解析配置文件并关联到代码
    fn parse_and_associate_configs(&mut self, index: &mut CodeIndex) {
        // 查找所有配置文件
        let config_files = self.find_config_files();
        
        log::info!("Found {} configuration files", config_files.len());
        
        for config_file in config_files {
            if let Err(e) = self.parse_config_file(&config_file, index) {
                let warning = format!("Failed to parse config file {:?}: {}", config_file, e);
                log::warn!("{}", warning);
                self.warnings.push(warning);
            }
        }
    }
    
    /// 查找所有配置文件
    fn find_config_files(&self) -> Vec<PathBuf> {
        let mut config_files = Vec::new();
        
        if let Err(e) = self.collect_config_files(&self.workspace_path, &mut config_files) {
            log::warn!("Failed to collect config files: {}", e);
        }
        
        config_files
    }
    
    /// 递归收集配置文件
    fn collect_config_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
        if !dir.is_dir() {
            return Ok(());
        }
        
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            // 跳过隐藏目录和构建目录
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "target" || name == "build" || name == "node_modules" {
                    continue;
                }
            }
            
            if path.is_dir() {
                self.collect_config_files(&path, files)?;
            } else if self.is_config_file(&path) {
                files.push(path);
            }
        }
        
        Ok(())
    }
    
    /// 判断是否是配置文件
    fn is_config_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(ext, "xml" | "yaml" | "yml")
        } else {
            false
        }
    }
    
    /// 解析单个配置文件
    fn parse_config_file(&mut self, config_path: &Path, index: &mut CodeIndex) -> Result<(), AnalysisError> {
        // 读取文件内容
        let content = std::fs::read_to_string(config_path)
            .map_err(|e| AnalysisError::IoError(e))?;
        
        // 选择合适的配置解析器
        let parser = self.select_config_parser(config_path)
            .ok_or_else(|| AnalysisError::ConfigParseError {
                file: config_path.to_path_buf(),
                error: ParseError::InvalidFormat {
                    message: "No suitable config parser found".to_string(),
                },
            })?;
        
        // 解析配置
        let config_data = parser.parse(&content)
            .map_err(|e| AnalysisError::ConfigParseError {
                file: config_path.to_path_buf(),
                error: e,
            })?;
        
        // 关联配置到代码
        index.associate_config_data(&config_data);
        
        log::debug!("Parsed config file: {:?}", config_path);
        
        Ok(())
    }
    
    /// 选择合适的配置解析器
    fn select_config_parser(&self, path: &Path) -> Option<&Box<dyn ConfigParser>> {
        let ext = path.extension()?.to_str()?;
        
        self.config_parsers.iter().find(|p| {
            match ext {
                "xml" => p.supports_format("xml"),
                "yaml" | "yml" => p.supports_format("yaml"),
                _ => false,
            }
        })
    }
    
    /// 从文件变更中提取变更的方法
    fn extract_changed_methods(
        &mut self,
        file_changes: &[FileChange],
        code_index: &CodeIndex,
    ) -> Result<Vec<String>, AnalysisError> {
        let mut changed_methods = Vec::new();
        
        for file_change in file_changes {
            // 获取文件的完整路径
            let file_path = self.workspace_path.join(&file_change.file_path);
            
            // 如果文件不存在（可能是删除的文件），跳过
            if !file_path.exists() {
                let warning = format!("File does not exist: {:?}", file_path);
                log::warn!("{}", warning);
                self.warnings.push(warning);
                continue;
            }
            
            // 读取文件内容
            let _content = match std::fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    let warning = format!("Failed to read file {:?}: {}", file_path, e);
                    log::warn!("{}", warning);
                    self.warnings.push(warning);
                    continue;
                }
            };
            
            // 从 hunk 中提取变更的行号范围
            let mut modified_line_ranges = Vec::new();
            for hunk in &file_change.hunks {
                let start = hunk.new_start;
                let end = hunk.new_start + hunk.new_lines;
                modified_line_ranges.push((start, end));
            }
            
            // 查找这些行范围内的方法
            // 遍历索引中的所有方法，检查是否属于当前文件且在变更范围内
            for (method_name, method_info) in code_index.methods() {
                // 首先检查方法是否属于当前文件
                // 通过比较 file_path 来判断
                if method_info.file_path != file_path {
                    continue;
                }
                
                // 检查方法的行范围是否与变更范围重叠
                let method_start = method_info.line_range.0;
                let method_end = method_info.line_range.1;
                
                for (change_start, change_end) in &modified_line_ranges {
                    // 检查是否有重叠
                    if method_start <= *change_end && method_end >= *change_start {
                        changed_methods.push(method_name.clone());
                        log::debug!("Found changed method: {} in file {:?}", method_name, file_path);
                        break;
                    }
                }
            }
        }
        
        // 去重
        changed_methods.sort();
        changed_methods.dedup();
        
        Ok(changed_methods)
    }
    
    /// 追溯影响
    fn trace_impact(
        &mut self,
        changed_methods: &[String],
        code_index: &CodeIndex,
    ) -> Result<ImpactGraph, AnalysisError> {
        let tracer = ImpactTracer::new(code_index, self.trace_config.clone());
        
        match tracer.trace_impact(changed_methods) {
            Ok(graph) => Ok(graph),
            Err(e) => {
                let error_msg = format!("Failed to trace impact: {}", e);
                self.errors.push(error_msg.clone());
                Err(AnalysisError::TraceError(e))
            }
        }
    }
    
    /// 获取警告列表
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    
    /// 获取错误列表
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use std::io::Write;
    
    #[test]
    fn test_analysis_statistics_creation() {
        let stats = AnalysisStatistics::new();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.parsed_files, 0);
        assert_eq!(stats.failed_files, 0);
        assert_eq!(stats.total_methods, 0);
        assert_eq!(stats.traced_chains, 0);
        assert_eq!(stats.duration_ms, 0);
    }
    
    #[test]
    fn test_orchestrator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        
        let orchestrator = AnalysisOrchestrator::new(workspace_path.clone(), trace_config).unwrap();
        
        assert_eq!(orchestrator.workspace_path, workspace_path);
        assert_eq!(orchestrator.warnings.len(), 0);
        assert_eq!(orchestrator.errors.len(), 0);
    }
    
    #[test]
    fn test_is_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        assert!(orchestrator.is_config_file(Path::new("config.xml")));
        assert!(orchestrator.is_config_file(Path::new("config.yaml")));
        assert!(orchestrator.is_config_file(Path::new("config.yml")));
        assert!(!orchestrator.is_config_file(Path::new("config.txt")));
        assert!(!orchestrator.is_config_file(Path::new("config.rs")));
    }
    
    #[test]
    fn test_parse_patch_with_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        // 创建一个无效的 patch 文件
        let patch_path = temp_dir.path().join("invalid.patch");
        let mut file = fs::File::create(&patch_path).unwrap();
        file.write_all(b"not a valid patch").unwrap();
        
        // 尝试解析
        let result = orchestrator.parse_patch(&patch_path, None);
        
        // 应该返回错误
        assert!(result.is_err());
        assert_eq!(orchestrator.errors.len(), 1);
    }
    
    #[test]
    fn test_parse_patches_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        // 创建 patches 目录
        let patches_dir = temp_dir.path().join("patches");
        fs::create_dir(&patches_dir).unwrap();
        
        // 创建多个 patch 文件
        let patch1_content = r#"diff --git a/file1.txt b/file1.txt
index 1234567..abcdefg 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
"#;
        
        let patch2_content = r#"diff --git a/file2.txt b/file2.txt
index 2345678..bcdefgh 100644
--- a/file2.txt
+++ b/file2.txt
@@ -1,2 +1,3 @@
 line 1
 line 2
+line 3
"#;
        
        let mut file1 = fs::File::create(patches_dir.join("project_a.patch")).unwrap();
        file1.write_all(patch1_content.as_bytes()).unwrap();
        
        let mut file2 = fs::File::create(patches_dir.join("project_b.patch")).unwrap();
        file2.write_all(patch2_content.as_bytes()).unwrap();
        
        // 解析目录
        let result = orchestrator.parse_patches_from_directory(&patches_dir);
        
        // 应该成功解析两个文件
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
    }
    
    #[test]
    fn test_parse_patches_from_directory_with_non_patch_files() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        // 创建 patches 目录
        let patches_dir = temp_dir.path().join("patches");
        fs::create_dir(&patches_dir).unwrap();
        
        // 创建一个 patch 文件和一个非 patch 文件
        let patch_content = r#"diff --git a/file1.txt b/file1.txt
index 1234567..abcdefg 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
"#;
        
        let mut patch_file = fs::File::create(patches_dir.join("project_a.patch")).unwrap();
        patch_file.write_all(patch_content.as_bytes()).unwrap();
        
        let mut txt_file = fs::File::create(patches_dir.join("readme.txt")).unwrap();
        txt_file.write_all(b"This is not a patch file").unwrap();
        
        // 解析目录
        let result = orchestrator.parse_patches_from_directory(&patches_dir);
        
        // 应该只解析 .patch 文件
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
    }
    
    #[test]
    fn test_parse_patches_from_directory_empty() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        // 创建空的 patches 目录
        let patches_dir = temp_dir.path().join("patches");
        fs::create_dir(&patches_dir).unwrap();
        
        // 解析目录
        let result = orchestrator.parse_patches_from_directory(&patches_dir);
        
        // 应该返回空列表并有警告
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.len(), 0);
        assert_eq!(orchestrator.warnings.len(), 1);
    }
    
    #[test]
    fn test_parse_patches_from_single_file_backward_compatibility() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        // 创建单个 patch 文件
        let patch_content = r#"diff --git a/file1.txt b/file1.txt
index 1234567..abcdefg 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
"#;
        
        let patch_path = temp_dir.path().join("single.patch");
        let mut file = fs::File::create(&patch_path).unwrap();
        file.write_all(patch_content.as_bytes()).unwrap();
        
        // 解析单个文件（向后兼容）
        let result = orchestrator.parse_patches_from_directory(&patch_path);
        
        // 应该成功解析
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
    }
    
    #[test]
    fn test_parse_patch_with_project_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        // 创建 patch 文件
        let patch_content = r#"diff --git a/src/ServiceA.java b/src/ServiceA.java
index 1234567..abcdefg 100644
--- a/src/ServiceA.java
+++ b/src/ServiceA.java
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
"#;
        
        let patch_path = temp_dir.path().join("project_a.patch");
        let mut file = fs::File::create(&patch_path).unwrap();
        file.write_all(patch_content.as_bytes()).unwrap();
        
        // 解析时带上项目前缀
        let result = orchestrator.parse_patch(&patch_path, Some("project_a".to_string()));
        
        // 应该成功解析
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        
        // 验证文件路径包含项目前缀
        assert_eq!(changes[0].file_path, "project_a/src/ServiceA.java");
    }
    
    #[test]
    fn test_parse_patches_from_directory_with_project_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        // 创建 patches 目录
        let patches_dir = temp_dir.path().join("patches");
        fs::create_dir(&patches_dir).unwrap();
        
        // 创建多个 patch 文件
        let patch1_content = r#"diff --git a/src/ServiceA.java b/src/ServiceA.java
index 1234567..abcdefg 100644
--- a/src/ServiceA.java
+++ b/src/ServiceA.java
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
"#;
        
        let patch2_content = r#"diff --git a/src/ServiceB.java b/src/ServiceB.java
index 2345678..bcdefgh 100644
--- a/src/ServiceB.java
+++ b/src/ServiceB.java
@@ -1,2 +1,3 @@
 line 1
 line 2
+line 3
"#;
        
        let mut file1 = fs::File::create(patches_dir.join("project_a.patch")).unwrap();
        file1.write_all(patch1_content.as_bytes()).unwrap();
        
        let mut file2 = fs::File::create(patches_dir.join("project_b.patch")).unwrap();
        file2.write_all(patch2_content.as_bytes()).unwrap();
        
        // 解析目录
        let result = orchestrator.parse_patches_from_directory(&patches_dir);
        
        // 应该成功解析两个文件
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        
        // 验证文件路径包含项目前缀（不依赖顺序）
        let paths: Vec<String> = changes.iter().map(|c| c.file_path.clone()).collect();
        assert!(paths.contains(&"project_a/src/ServiceA.java".to_string()));
        assert!(paths.contains(&"project_b/src/ServiceB.java".to_string()));
    }
    
    #[test]
    fn test_find_config_files() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        
        // 创建一些配置文件
        fs::File::create(temp_dir.path().join("config.xml")).unwrap();
        fs::File::create(temp_dir.path().join("config.yaml")).unwrap();
        fs::File::create(temp_dir.path().join("config.yml")).unwrap();
        fs::File::create(temp_dir.path().join("not_config.txt")).unwrap();
        
        let trace_config = TraceConfig::default();
        let orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        let config_files = orchestrator.find_config_files();
        
        // 应该找到 3 个配置文件
        assert_eq!(config_files.len(), 3);
    }
    
    #[test]
    fn test_collect_config_files_skips_hidden_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        
        // 创建隐藏目录
        let hidden_dir = temp_dir.path().join(".hidden");
        fs::create_dir(&hidden_dir).unwrap();
        fs::File::create(hidden_dir.join("config.xml")).unwrap();
        
        // 创建正常目录
        fs::File::create(temp_dir.path().join("config.yaml")).unwrap();
        
        let trace_config = TraceConfig::default();
        let orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        let config_files = orchestrator.find_config_files();
        
        // 应该只找到正常目录中的配置文件
        assert_eq!(config_files.len(), 1);
    }
    
    #[test]
    fn test_warnings_and_errors_accessors() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();
        let trace_config = TraceConfig::default();
        let mut orchestrator = AnalysisOrchestrator::new(workspace_path, trace_config).unwrap();
        
        orchestrator.warnings.push("Test warning".to_string());
        orchestrator.errors.push("Test error".to_string());
        
        assert_eq!(orchestrator.warnings().len(), 1);
        assert_eq!(orchestrator.errors().len(), 1);
        assert_eq!(orchestrator.warnings()[0], "Test warning");
        assert_eq!(orchestrator.errors()[0], "Test error");
    }
}
