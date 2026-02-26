use code_impact_analyzer::*;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_end_to_end_simple_analysis() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    
    // 创建一个简单的 workspace
    let workspace = temp_dir.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    
    // 创建一个简单的 Rust 源文件
    let src_file = workspace.join("main.rs");
    let mut file = fs::File::create(&src_file).unwrap();
    file.write_all(b"fn main() {\n    println!(\"Hello, world!\");\n}\n\nfn helper() {\n    println!(\"Helper\");\n}\n").unwrap();
    
    // 创建一个简单的 patch 文件
    let patch_path = temp_dir.path().join("test.patch");
    let mut patch_file = fs::File::create(&patch_path).unwrap();
    patch_file.write_all(b"diff --git a/main.rs b/main.rs\nindex 0000000..1111111 100644\n--- a/main.rs\n+++ b/main.rs\n@@ -1,3 +1,4 @@\n fn main() {\n     println!(\"Hello, world!\");\n+    helper();\n }\n").unwrap();
    
    // 创建 CLI 参数
    let args = CliArgs {
        workspace_path: workspace,
        diff_path: patch_path,
        output_format: OutputFormat::Json,
        max_depth: 10,
        log_level: LogLevel::Error,
    };
    
    // 运行分析
    let result = run(args);
    
    // 分析应该成功完成（即使没有找到任何方法调用）
    assert!(result.is_ok());
}

#[test]
fn test_end_to_end_with_statistics() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    
    // 创建 workspace
    let workspace = temp_dir.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    
    // 创建一个 Java 源文件
    let java_file = workspace.join("Test.java");
    let mut file = fs::File::create(&java_file).unwrap();
    file.write_all(b"public class Test {\n    public void method1() {\n        System.out.println(\"Method 1\");\n    }\n    public void method2() {\n        method1();\n    }\n}\n").unwrap();
    
    // 创建 patch 文件
    let patch_path = temp_dir.path().join("test.patch");
    let mut patch_file = fs::File::create(&patch_path).unwrap();
    patch_file.write_all(b"diff --git a/Test.java b/Test.java\nindex 0000000..1111111 100644\n--- a/Test.java\n+++ b/Test.java\n@@ -1,5 +1,6 @@\n public class Test {\n     public void method1() {\n         System.out.println(\"Method 1\");\n+        // Added comment\n     }\n     public void method2() {\n").unwrap();
    
    // 创建 CLI 参数
    let args = CliArgs {
        workspace_path: workspace,
        diff_path: patch_path,
        output_format: OutputFormat::Dot,
        max_depth: 5,
        log_level: LogLevel::Error,
    };
    
    // 运行分析
    let result = run(args);
    
    // 应该成功
    assert!(result.is_ok());
}

#[test]
fn test_end_to_end_error_handling() {
    // 测试各种错误情况
    let temp_dir = TempDir::new().unwrap();
    
    // 测试 1: 不存在的 workspace
    let args = CliArgs {
        workspace_path: temp_dir.path().join("nonexistent"),
        diff_path: temp_dir.path().join("test.patch"),
        output_format: OutputFormat::Dot,
        max_depth: 10,
        log_level: LogLevel::Error,
    };
    assert!(run(args).is_err());
    
    // 测试 2: 不存在的 patch 文件
    let workspace = temp_dir.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    
    let args = CliArgs {
        workspace_path: workspace,
        diff_path: temp_dir.path().join("nonexistent.patch"),
        output_format: OutputFormat::Dot,
        max_depth: 10,
        log_level: LogLevel::Error,
    };
    assert!(run(args).is_err());
}
