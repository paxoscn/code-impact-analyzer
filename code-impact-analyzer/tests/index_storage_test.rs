use code_impact_analyzer::{CodeIndex, IndexStorage, MethodInfo};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_index_lifecycle() {
    // 创建临时工作空间
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建索引存储管理器
    let storage = IndexStorage::new(workspace_path.clone());
    
    // 1. 初始状态：索引不存在
    assert!(!storage.index_exists());
    assert!(storage.get_index_info().unwrap().is_none());
    
    // 2. 创建并保存索引
    let mut code_index = CodeIndex::new();
    
    // 添加一些测试方法
    let method = MethodInfo {
        name: "testMethod".to_string(),
        full_qualified_name: "com.example.Test::testMethod".to_string(),
        file_path: PathBuf::from("Test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    code_index.test_index_method(&method).unwrap();
    
    // 保存索引
    storage.save_index(&code_index).unwrap();
    
    // 3. 验证索引已保存
    assert!(storage.index_exists());
    
    let info = storage.get_index_info().unwrap();
    assert!(info.is_some());
    
    let metadata = info.unwrap();
    assert_eq!(metadata.method_count, 1);
    assert_eq!(metadata.workspace_path, workspace_path);
    
    // 4. 加载索引
    let loaded_index = storage.load_index().unwrap();
    assert!(loaded_index.is_some());
    
    let loaded = loaded_index.unwrap();
    let loaded_method = loaded.find_method("com.example.Test::testMethod");
    assert!(loaded_method.is_some());
    assert_eq!(loaded_method.unwrap().name, "testMethod");
    
    // 5. 清除索引
    storage.clear_index().unwrap();
    assert!(!storage.index_exists());
}

#[test]
fn test_index_validation() {
    // 创建临时工作空间
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建索引存储管理器
    let storage = IndexStorage::new(workspace_path.clone());
    
    // 创建并保存索引
    let code_index = CodeIndex::new();
    storage.save_index(&code_index).unwrap();
    
    // 验证索引有效
    let metadata = storage.get_index_info().unwrap().unwrap();
    assert!(metadata.is_valid(&workspace_path));
    
    // 使用不同的工作空间路径验证
    let other_path = PathBuf::from("/other/path");
    assert!(!metadata.is_valid(&other_path));
}

#[test]
fn test_index_reload_after_save() {
    // 创建临时工作空间
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建索引存储管理器
    let storage = IndexStorage::new(workspace_path.clone());
    
    // 创建索引并添加多个方法
    let mut code_index = CodeIndex::new();
    
    for i in 0..10 {
        let method = MethodInfo {
            name: format!("method{}", i),
            full_qualified_name: format!("com.example.Test::method{}", i),
            file_path: PathBuf::from("Test.java"),
            line_range: (i * 10, i * 10 + 10),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        code_index.test_index_method(&method).unwrap();
    }
    
    // 保存索引
    storage.save_index(&code_index).unwrap();
    
    // 重新加载索引
    let loaded_index = storage.load_index().unwrap().unwrap();
    
    // 验证所有方法都被正确加载
    for i in 0..10 {
        let method_name = format!("com.example.Test::method{}", i);
        let method = loaded_index.find_method(&method_name);
        assert!(method.is_some());
        assert_eq!(method.unwrap().name, format!("method{}", i));
    }
}

#[test]
fn test_multiple_save_and_load_cycles() {
    // 创建临时工作空间
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建索引存储管理器
    let storage = IndexStorage::new(workspace_path.clone());
    
    // 执行多次保存和加载循环
    for cycle in 0..3 {
        let mut code_index = CodeIndex::new();
        
        // 添加方法
        let method = MethodInfo {
            name: format!("method_cycle_{}", cycle),
            full_qualified_name: format!("com.example.Test::method_cycle_{}", cycle),
            file_path: PathBuf::from("Test.java"),
            line_range: (cycle * 10, cycle * 10 + 10),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        code_index.test_index_method(&method).unwrap();
        
        // 保存
        storage.save_index(&code_index).unwrap();
        
        // 加载并验证
        let loaded = storage.load_index().unwrap().unwrap();
        let method_name = format!("com.example.Test::method_cycle_{}", cycle);
        assert!(loaded.find_method(&method_name).is_some());
    }
}

#[test]
fn test_index_info_after_save() {
    // 创建临时工作空间
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建索引存储管理器
    let storage = IndexStorage::new(workspace_path.clone());
    
    // 创建索引
    let mut code_index = CodeIndex::new();
    
    // 添加 5 个方法
    for i in 0..5 {
        let method = MethodInfo {
            name: format!("method{}", i),
            full_qualified_name: format!("com.example.Test::method{}", i),
            file_path: PathBuf::from("Test.java"),
            line_range: (i * 10, i * 10 + 10),
            calls: vec![],
            http_annotations: None,
            kafka_operations: vec![],
            db_operations: vec![],
            redis_operations: vec![],
        };
        
        code_index.test_index_method(&method).unwrap();
    }
    
    // 保存索引
    storage.save_index(&code_index).unwrap();
    
    // 获取索引信息
    let info = storage.get_index_info().unwrap().unwrap();
    
    // 验证信息
    assert_eq!(info.method_count, 5);
    assert_eq!(info.file_count, 1); // 所有方法在同一个文件中
    assert_eq!(info.version, "1.0.0");
    assert_eq!(info.workspace_path, workspace_path);
}
