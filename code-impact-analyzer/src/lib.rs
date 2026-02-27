pub mod types;
pub mod errors;
pub mod patch_parser;
pub mod language_parser;
pub mod java_parser;
pub mod rust_parser;
pub mod config_parser;
pub mod code_index;
pub mod parse_cache;
pub mod impact_tracer;
pub mod orchestrator;
pub mod cli;
pub mod index_storage;

pub use types::*;
pub use errors::*;
pub use patch_parser::*;
pub use language_parser::*;
pub use java_parser::*;
pub use rust_parser::*;
pub use config_parser::*;
pub use code_index::*;
pub use parse_cache::*;
pub use impact_tracer::*;
pub use orchestrator::*;
pub use cli::*;
pub use index_storage::*;

/// 主分析流程
/// 
/// 连接所有模块，执行完整的代码影响分析流程
pub fn run(args: CliArgs) -> Result<(), AnalysisError> {
    // 创建索引存储管理器
    let index_storage = IndexStorage::new(args.workspace_path.clone());
    
    // 处理索引管理命令
    if args.clear_index {
        log::info!("Clearing index...");
        index_storage.clear_index()
            .map_err(|e| AnalysisError::IndexBuildError(e))?;
        println!("Index cleared successfully");
        return Ok(());
    }
    
    if args.index_info {
        log::info!("Retrieving index information...");
        match index_storage.get_index_info()
            .map_err(|e| AnalysisError::IndexBuildError(e))? {
            Some(metadata) => {
                println!("Index Information:");
                println!("  Version: {}", metadata.version);
                println!("  Workspace: {:?}", metadata.workspace_path);
                println!("  Created: {}", format_timestamp(metadata.created_at));
                println!("  Updated: {}", format_timestamp(metadata.updated_at));
                println!("  Files: {}", metadata.file_count);
                println!("  Methods: {}", metadata.method_count);
                println!("  Checksum: {}", metadata.checksum);
            }
            None => {
                println!("No index found");
            }
        }
        return Ok(());
    }
    
    if args.verify_index {
        log::info!("Verifying index...");
        match index_storage.get_index_info()
            .map_err(|e| AnalysisError::IndexBuildError(e))? {
            Some(metadata) => {
                if metadata.is_valid(&args.workspace_path) {
                    println!("Index is valid");
                } else {
                    println!("Index is invalid or outdated");
                }
            }
            None => {
                println!("No index found");
            }
        }
        return Ok(());
    }
    
    // 验证输入路径
    if !args.workspace_path.exists() {
        return Err(AnalysisError::IoError(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Workspace path does not exist: {:?}", args.workspace_path)
            )
        ));
    }
    
    if !args.diff_path.exists() {
        return Err(AnalysisError::IoError(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Diff file does not exist: {:?}", args.diff_path)
            )
        ));
    }
    
    // 创建追溯配置
    let trace_config = TraceConfig {
        max_depth: args.max_depth,
        trace_upstream: true,
        trace_downstream: true,
        trace_cross_service: true,
    };
    
    // 创建分析编排器
    let mut orchestrator = AnalysisOrchestrator::new(
        args.workspace_path.clone(),
        trace_config,
    )?;
    
    // 设置是否强制重建索引
    orchestrator.set_force_rebuild(args.rebuild_index);
    
    // 执行分析
    log::info!("Starting analysis...");
    let result = orchestrator.analyze(&args.diff_path)?;
    
    // 输出警告
    if !result.warnings.is_empty() {
        log::warn!("Analysis completed with {} warnings:", result.warnings.len());
        for warning in &result.warnings {
            log::warn!("  - {}", warning);
        }
    }
    
    // 输出统计信息
    log::info!("Analysis Statistics:");
    log::info!("  Total files: {}", result.statistics.total_files);
    log::info!("  Parsed files: {}", result.statistics.parsed_files);
    log::info!("  Failed files: {}", result.statistics.failed_files);
    log::info!("  Total methods: {}", result.statistics.total_methods);
    log::info!("  Traced chains: {}", result.statistics.traced_chains);
    log::info!("  Duration: {} ms", result.statistics.duration_ms);
    
    // 输出影响图
    output_result(&result, &args)?;
    
    log::info!("Analysis completed successfully");
    Ok(())
}

/// 格式化时间戳
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{UNIX_EPOCH, Duration};
    
    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    
    // 简单格式化（实际应用中可以使用 chrono 库）
    match datetime.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            let days = secs / 86400;
            let hours = (secs % 86400) / 3600;
            let minutes = (secs % 3600) / 60;
            let seconds = secs % 60;
            
            format!("{} days, {:02}:{:02}:{:02}", days, hours, minutes, seconds)
        }
        Err(_) => "Invalid timestamp".to_string(),
    }
}

/// 输出分析结果
fn output_result(
    result: &AnalysisResult,
    args: &CliArgs,
) -> Result<(), AnalysisError> {
    match args.output_format {
        OutputFormat::Dot => {
            let dot_output = result.impact_graph.to_dot();
            println!("{}", dot_output);
        }
        OutputFormat::Json => {
            let json_output = result.impact_graph.to_json()
                .map_err(|e| AnalysisError::IoError(
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                ))?;
            println!("{}", json_output);
        }
        OutputFormat::Mermaid => {
            // Mermaid 格式暂未实现，使用 DOT 格式代替
            log::warn!("Mermaid format not yet implemented, using DOT format instead");
            let dot_output = result.impact_graph.to_dot();
            println!("{}", dot_output);
        }
    }
    
    Ok(())
}
