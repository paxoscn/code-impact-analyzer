use code_impact_analyzer::{CliArgs, run};
use clap::Parser;
use std::process;

fn main() {
    // 解析命令行参数
    let args = CliArgs::parse();
    
    // 初始化日志系统，使用用户指定的日志级别
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(args.log_level.to_filter_str())
    ).init();
    
    log::info!("Code Impact Analyzer v0.1.0");
    log::info!("Workspace path: {:?}", args.workspace_path);
    log::info!("Diff path: {:?}", args.diff_path);
    log::info!("Output format: {:?}", args.output_format);
    log::info!("Max depth: {}", args.max_depth);
    log::info!("Log level: {:?}", args.log_level);
    
    // 执行分析流程
    if let Err(e) = run(args) {
        log::error!("Analysis failed: {}", e);
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}



