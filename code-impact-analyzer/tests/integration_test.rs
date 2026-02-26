use code_impact_analyzer::*;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_main_run_with_invalid_workspace_path() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    
    // 创建一个有效的 patch 文件
    let patch_path = temp_dir.path().join("test.patch");
    let mut patch_file = fs::File::create(&patch_path).unwrap();
    patch_file.write_all(b"diff --git a/test.rs b/test.rs\n").unwrap();
    
    // 使用不存在的 workspace 路径
    let args = CliArgs {
        workspace_path: temp_dir.path().join("nonexistent"),
        diff_path: patch_path,
        output_format: OutputFormat::Dot,
        max_depth: 10,
        log_level: LogLevel::Error,
    };
    
    // 运行应该失败
    let result = code_impact_analyzer::run(args);
    assert!(result.is_err());
}

#[test]
fn test_main_run_with_invalid_diff_path() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    
    // 使用不存在的 diff 文件
    let args = CliArgs {
        workspace_path: temp_dir.path().to_path_buf(),
        diff_path: temp_dir.path().join("nonexistent.patch"),
        output_format: OutputFormat::Dot,
        max_depth: 10,
        log_level: LogLevel::Error,
    };
    
    // 运行应该失败
    let result = code_impact_analyzer::run(args);
    assert!(result.is_err());
}

#[test]
fn test_output_format_selection() {
    // 测试不同的输出格式
    let formats = vec![
        OutputFormat::Dot,
        OutputFormat::Json,
        OutputFormat::Mermaid,
    ];
    
    for format in formats {
        let temp_dir = TempDir::new().unwrap();
        
        // 创建一个简单的 workspace
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        
        // 创建一个简单的 patch 文件
        let patch_path = temp_dir.path().join("test.patch");
        let mut patch_file = fs::File::create(&patch_path).unwrap();
        patch_file.write_all(b"diff --git a/test.rs b/test.rs\nindex 0000000..1111111 100644\n--- a/test.rs\n+++ b/test.rs\n@@ -1,3 +1,4 @@\n fn main() {\n+    println!(\"Hello\");\n }\n").unwrap();
        
        let args = CliArgs {
            workspace_path: workspace,
            diff_path: patch_path,
            output_format: format,
            max_depth: 10,
            log_level: LogLevel::Error,
        };
        
        // 运行分析（可能会失败，但不应该 panic）
        let _ = code_impact_analyzer::run(args);
    }
}
