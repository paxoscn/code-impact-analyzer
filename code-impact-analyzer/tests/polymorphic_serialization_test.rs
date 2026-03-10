use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::index_storage::IndexStorage;
use code_impact_analyzer::language_parser::{ClassInfo, MethodInfo, MethodCall, ParsedFile};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_polymorphic_calls_persist_after_serialization() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建索引
    let mut index = CodeIndex::new();
    
    // 创建继承关系：Dog extends Animal
    let animal_class = ClassInfo {
        name: "com.example.Animal".to_string(),
        line_range: (1, 10),
        methods: vec![],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    let dog_class = ClassInfo {
        name: "com.example.Dog".to_string(),
        line_range: (1, 10),
        methods: vec![],
        is_interface: false,
        implements: vec![],
        extends: Some("com.example.Animal".to_string()),
    };
    
    // 创建方法重载
    let process_animal_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.Service::process(com.example.Animal)".to_string(),
        file_path: PathBuf::from("Service.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: None,
    };
    
    let process_dog_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.Service::process(com.example.Dog)".to_string(),
        file_path: PathBuf::from("Service.java"),
        line_range: (17, 22),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: None,
    };
    
    let service_class = ClassInfo {
        name: "com.example.Service".to_string(),
        line_range: (5, 25),
        methods: vec![process_animal_method.clone(), process_dog_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    // 创建调用者
    let controller_method = MethodInfo {
        name: "handle".to_string(),
        full_qualified_name: "com.example.Controller::handle()".to_string(),
        file_path: PathBuf::from("Controller.java"),
        line_range: (10, 20),
        calls: vec![
            MethodCall {
                target: "com.example.Service::process(com.example.Dog)".to_string(),
                line: 15,
            }
        ],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: None,
    };
    
    let controller_class = ClassInfo {
        name: "com.example.Controller".to_string(),
        line_range: (5, 25),
        methods: vec![controller_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    // 索引所有类
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Animal.java"),
        language: "java".to_string(),
        classes: vec![animal_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Dog.java"),
        language: "java".to_string(),
        classes: vec![dog_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Service.java"),
        language: "java".to_string(),
        classes: vec![service_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Controller.java"),
        language: "java".to_string(),
        classes: vec![controller_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    // 传播继承成员和多态调用
    index.propagate_inherited_members();
    index.propagate_polymorphic_calls();
    
    // 验证多态调用存在
    let callees_before = index.find_callees("com.example.Controller::handle()");
    assert!(
        callees_before.contains(&"com.example.Service::process(com.example.Dog)"),
        "序列化前应该有直接调用"
    );
    assert!(
        callees_before.contains(&"com.example.Service::process(com.example.Animal)"),
        "序列化前应该有多态调用"
    );
    
    // 保存索引
    let storage = IndexStorage::new(workspace_path.clone());
    storage.save_index(&index).expect("保存索引失败");
    
    // 加载索引
    let loaded_index = storage.load_index().expect("加载索引失败").expect("索引不存在");
    
    // 验证多态调用在加载后仍然存在
    let callees_after = loaded_index.find_callees("com.example.Controller::handle()");
    assert!(
        callees_after.contains(&"com.example.Service::process(com.example.Dog)"),
        "反序列化后应该有直接调用"
    );
    assert!(
        callees_after.contains(&"com.example.Service::process(com.example.Animal)"),
        "反序列化后应该有多态调用"
    );
    
    // 验证反向调用关系
    let callers_animal = loaded_index.find_callers("com.example.Service::process(com.example.Animal)");
    assert!(
        callers_animal.contains(&"com.example.Controller::handle()"),
        "反序列化后 process(Animal) 应该被 Controller 调用"
    );
}

#[test]
fn test_inherited_members_persist_after_serialization() {
    // 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_path_buf();
    
    // 创建索引
    let mut index = CodeIndex::new();
    
    // 创建父类
    let parent_method = MethodInfo {
        name: "parentMethod".to_string(),
        full_qualified_name: "com.example.Parent::parentMethod()".to_string(),
        file_path: PathBuf::from("Parent.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: None,
    };
    
    let parent_class = ClassInfo {
        name: "com.example.Parent".to_string(),
        line_range: (5, 20),
        methods: vec![parent_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    // 创建子类
    let child_method = MethodInfo {
        name: "childMethod".to_string(),
        full_qualified_name: "com.example.Child::childMethod()".to_string(),
        file_path: PathBuf::from("Child.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: None,
    };
    
    let child_class = ClassInfo {
        name: "com.example.Child".to_string(),
        line_range: (5, 20),
        methods: vec![child_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: Some("com.example.Parent".to_string()),
    };
    
    // 索引所有类
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Parent.java"),
        language: "java".to_string(),
        classes: vec![parent_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Child.java"),
        language: "java".to_string(),
        classes: vec![child_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    // 传播继承成员
    index.propagate_inherited_members();
    
    // 验证继承的方法存在
    let child_inherited_method = index.find_method("com.example.Child::parentMethod()");
    assert!(child_inherited_method.is_some(), "序列化前子类应该有继承的方法");
    
    // 保存索引
    let storage = IndexStorage::new(workspace_path.clone());
    storage.save_index(&index).expect("保存索引失败");
    
    // 加载索引
    let loaded_index = storage.load_index().expect("加载索引失败").expect("索引不存在");
    
    // 验证继承的方法在加载后仍然存在
    let child_inherited_method_after = loaded_index.find_method("com.example.Child::parentMethod()");
    assert!(
        child_inherited_method_after.is_some(),
        "反序列化后子类应该有继承的方法"
    );
    
    // 验证继承关系
    assert_eq!(
        loaded_index.find_parent_class("com.example.Child"),
        Some("com.example.Parent")
    );
}
