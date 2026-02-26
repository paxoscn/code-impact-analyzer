use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// 代码影响分析工具 - 分析 Git patch 文件对代码库的影响
#[derive(Parser, Debug)]
#[command(name = "code-impact-analyzer")]
#[command(version = "0.1.0")]
#[command(about = "分析 Git patch 文件对代码库的影响", long_about = None)]
pub struct CliArgs {
    /// Workspace 根目录路径，包含多个项目源代码
    #[arg(short = 'w', long = "workspace", value_name = "PATH")]
    pub workspace_path: PathBuf,

    /// Git diff 补丁文件路径
    #[arg(short = 'd', long = "diff", value_name = "PATH")]
    pub diff_path: PathBuf,

    /// 输出格式：dot, json, 或 mermaid
    #[arg(short = 'o', long = "output-format", value_enum, default_value = "dot")]
    pub output_format: OutputFormat,

    /// 追溯的最大深度，防止无限递归
    #[arg(short = 'm', long = "max-depth", default_value = "10")]
    pub max_depth: usize,

    /// 日志级别：trace, debug, info, warn, error
    #[arg(short = 'l', long = "log-level", value_enum, default_value = "info")]
    pub log_level: LogLevel,
}

/// 输出格式枚举
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Graphviz DOT 格式
    Dot,
    /// JSON 格式
    Json,
    /// Mermaid 图表格式
    Mermaid,
}

/// 日志级别枚举
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogLevel {
    /// 跟踪级别（最详细）
    Trace,
    /// 调试级别
    Debug,
    /// 信息级别
    Info,
    /// 警告级别
    Warn,
    /// 错误级别
    Error,
}

impl LogLevel {
    /// 转换为 env_logger 的过滤器字符串
    pub fn to_filter_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_args_parsing() {
        // 测试基本参数解析
        let args = CliArgs::parse_from(&[
            "code-impact-analyzer",
            "--workspace", "/path/to/workspace",
            "--diff", "/path/to/patch.diff",
        ]);

        assert_eq!(args.workspace_path, PathBuf::from("/path/to/workspace"));
        assert_eq!(args.diff_path, PathBuf::from("/path/to/patch.diff"));
        assert!(matches!(args.output_format, OutputFormat::Dot));
        assert_eq!(args.max_depth, 10);
        assert!(matches!(args.log_level, LogLevel::Info));
    }

    #[test]
    fn test_cli_args_with_all_options() {
        // 测试所有参数
        let args = CliArgs::parse_from(&[
            "code-impact-analyzer",
            "-w", "/workspace",
            "-d", "/patch.diff",
            "-o", "json",
            "-m", "5",
            "-l", "debug",
        ]);

        assert_eq!(args.workspace_path, PathBuf::from("/workspace"));
        assert_eq!(args.diff_path, PathBuf::from("/patch.diff"));
        assert!(matches!(args.output_format, OutputFormat::Json));
        assert_eq!(args.max_depth, 5);
        assert!(matches!(args.log_level, LogLevel::Debug));
    }

    #[test]
    fn test_output_format_variants() {
        // 测试 DOT 格式
        let args = CliArgs::parse_from(&[
            "code-impact-analyzer",
            "-w", "/workspace",
            "-d", "/patch.diff",
            "-o", "dot",
        ]);
        assert!(matches!(args.output_format, OutputFormat::Dot));

        // 测试 JSON 格式
        let args = CliArgs::parse_from(&[
            "code-impact-analyzer",
            "-w", "/workspace",
            "-d", "/patch.diff",
            "-o", "json",
        ]);
        assert!(matches!(args.output_format, OutputFormat::Json));

        // 测试 Mermaid 格式
        let args = CliArgs::parse_from(&[
            "code-impact-analyzer",
            "-w", "/workspace",
            "-d", "/patch.diff",
            "-o", "mermaid",
        ]);
        assert!(matches!(args.output_format, OutputFormat::Mermaid));
    }

    #[test]
    fn test_log_level_variants() {
        // 测试所有日志级别
        let levels = vec![
            ("trace", LogLevel::Trace),
            ("debug", LogLevel::Debug),
            ("info", LogLevel::Info),
            ("warn", LogLevel::Warn),
            ("error", LogLevel::Error),
        ];

        for (level_str, _expected) in levels {
            let args = CliArgs::parse_from(&[
                "code-impact-analyzer",
                "-w", "/workspace",
                "-d", "/patch.diff",
                "-l", level_str,
            ]);
            // 验证日志级别被正确解析（通过 Debug 格式比较）
            let level_debug = format!("{:?}", args.log_level);
            assert!(level_debug.to_lowercase().contains(level_str));
        }
    }

    #[test]
    fn test_log_level_to_filter_str() {
        assert_eq!(LogLevel::Trace.to_filter_str(), "trace");
        assert_eq!(LogLevel::Debug.to_filter_str(), "debug");
        assert_eq!(LogLevel::Info.to_filter_str(), "info");
        assert_eq!(LogLevel::Warn.to_filter_str(), "warn");
        assert_eq!(LogLevel::Error.to_filter_str(), "error");
    }

    #[test]
    fn test_max_depth_parsing() {
        // 测试自定义深度
        let args = CliArgs::parse_from(&[
            "code-impact-analyzer",
            "-w", "/workspace",
            "-d", "/patch.diff",
            "-m", "20",
        ]);
        assert_eq!(args.max_depth, 20);
    }

    #[test]
    fn test_cli_help_generation() {
        // 确保帮助信息可以生成（不会 panic）
        let mut cmd = CliArgs::command();
        let _ = cmd.render_help();
    }

    #[test]
    fn test_required_arguments() {
        // 测试缺少必需参数时的行为
        let result = CliArgs::try_parse_from(&[
            "code-impact-analyzer",
            "-w", "/workspace",
            // 缺少 --diff 参数
        ]);
        assert!(result.is_err());

        let result = CliArgs::try_parse_from(&[
            "code-impact-analyzer",
            "-d", "/patch.diff",
            // 缺少 --workspace 参数
        ]);
        assert!(result.is_err());
    }
}
