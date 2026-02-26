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

/// 主分析流程
/// 
/// 连接所有模块，执行完整的代码影响分析流程
pub fn run(args: CliArgs) -> Result<(), AnalysisError> {
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
