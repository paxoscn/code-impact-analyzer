/// 并行处理集成测试
/// 
/// 验证 rayon 并行处理功能正常工作

use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::java_parser::JavaParser;
use code_impact_analyzer::rust_parser::RustParser;
use code_impact_analyzer::language_parser::LanguageParser;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_parallel_indexing_multiple_files() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path();
    
    // 创建多个 Java 文件
    for i in 0..10 {
        let file_path = workspace_path.join(format!("TestClass{}.java", i));
        let content = format!(
            r#"
            package com.example;
            
            public class TestClass{} {{
                public void method{}() {{
                    System.out.println("Method {}");
                }}
                
                public void callOther() {{
                    method{}();
                }}
            }}
            "#,
            i, i, i, i
        );
        fs::write(&file_path, content).unwrap();
    }
    
    // 创建多个 Rust 文件
    for i in 0..10 {
        let file_path = workspace_path.join(format!("test_module{}.rs", i));
        let content = format!(
            r#"
            pub fn function_{}() {{
                println!("Function {}");
            }}
            
            pub fn call_other() {{
                function_{}();
            }}
            "#,
            i, i, i
        );
        fs::write(&file_path, content).unwrap();
    }
    
    // 创建解析器
    let parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(JavaParser::new().unwrap()),
        Box::new(RustParser::new().unwrap()),
    ];
    
    // 构建索引（使用并行处理）
    let mut index = CodeIndex::new();
    let result = index.index_workspace(workspace_path, &parsers);
    
    // 验证索引构建成功
    assert!(result.is_ok(), "Index building should succeed");
    
    // 验证所有方法都被索引
    let mut method_count = 0;
    for (_, _) in index.methods() {
        method_count += 1;
    }
    
    // 应该有至少 20 个方法/函数被索引（每个文件至少 1 个）
    assert!(method_count >= 20, "Should have indexed at least 20 methods/functions, got {}", method_count);
}

#[test]
fn test_parallel_indexing_with_errors() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path();
    
    // 创建一些有效的文件
    for i in 0..5 {
        let file_path = workspace_path.join(format!("Valid{}.java", i));
        let content = format!(
            r#"
            package com.example;
            
            public class Valid{} {{
                public void method{}() {{}}
            }}
            "#,
            i, i
        );
        fs::write(&file_path, content).unwrap();
    }
    
    // 创建一些无效的文件（语法错误）
    for i in 0..5 {
        let file_path = workspace_path.join(format!("Invalid{}.java", i));
        let content = "this is not valid Java code { { {";
        fs::write(&file_path, content).unwrap();
    }
    
    // 创建解析器
    let parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(JavaParser::new().unwrap()),
    ];
    
    // 构建索引（应该继续处理有效文件，跳过无效文件）
    let mut index = CodeIndex::new();
    let result = index.index_workspace(workspace_path, &parsers);
    
    // 验证索引构建成功（尽管有错误）
    assert!(result.is_ok(), "Index building should succeed even with some invalid files");
    
    // 验证有效文件被索引
    let mut method_count = 0;
    for (_, _) in index.methods() {
        method_count += 1;
    }
    
    // 应该至少有 5 个方法（来自有效文件）
    assert!(method_count >= 5, "Should have indexed at least 5 methods from valid files");
}

#[test]
fn test_parallel_indexing_preserves_correctness() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path();
    
    // 创建多个简单的 Java 类
    for i in 0..5 {
        let class_file = workspace_path.join(format!("Class{}.java", i));
        fs::write(&class_file, format!(r#"
            package com.example;
            
            public class Class{} {{
                public void method{}() {{
                    System.out.println("Method {}");
                }}
            }}
        "#, i, i, i)).unwrap();
    }
    
    // 创建解析器
    let parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(JavaParser::new().unwrap()),
    ];
    
    // 构建索引
    let mut index = CodeIndex::new();
    index.index_workspace(workspace_path, &parsers).unwrap();
    
    // 验证方法数量
    let mut method_count = 0;
    for (_, _) in index.methods() {
        method_count += 1;
    }
    assert!(method_count >= 5, "Should have indexed at least 5 methods, got {}", method_count);
}

#[test]
fn test_parallel_indexing_empty_workspace() {
    // 创建空的临时目录
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path();
    
    // 创建解析器
    let parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(JavaParser::new().unwrap()),
        Box::new(RustParser::new().unwrap()),
    ];
    
    // 构建索引
    let mut index = CodeIndex::new();
    let result = index.index_workspace(workspace_path, &parsers);
    
    // 验证索引构建成功
    assert!(result.is_ok(), "Index building should succeed for empty workspace");
    
    // 验证没有方法被索引
    let mut method_count = 0;
    for (_, _) in index.methods() {
        method_count += 1;
    }
    
    assert_eq!(method_count, 0, "Should have no methods in empty workspace");
}
