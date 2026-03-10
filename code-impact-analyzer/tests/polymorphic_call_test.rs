use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::language_parser::{ClassInfo, MethodInfo, MethodCall, ParsedFile};
use std::path::PathBuf;

#[test]
fn test_polymorphic_call_propagation() {
    let mut index = CodeIndex::new();
    
    // 创建父类 Animal
    let animal_class = ClassInfo {
        name: "com.example.Animal".to_string(),
        line_range: (1, 10),
        methods: vec![],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    // 创建子类 Dog extends Animal
    let dog_class = ClassInfo {
        name: "com.example.Dog".to_string(),
        line_range: (1, 10),
        methods: vec![],
        is_interface: false,
        implements: vec![],
        extends: Some("com.example.Animal".to_string()),
    };
    
    // 创建 Service 类，有两个重载方法：process(Animal) 和 process(Dog)
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
    
    // 创建 Controller 类，调用 process(Dog)
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
    
    // 验证继承关系
    assert_eq!(
        index.find_parent_class("com.example.Dog"),
        Some("com.example.Animal")
    );
    
    // 传播继承成员和多态调用
    index.propagate_inherited_members();
    index.propagate_polymorphic_calls();
    
    // 验证 Controller::handle 调用了 process(Dog)
    let callees = index.find_callees("com.example.Controller::handle()");
    assert!(
        callees.contains(&"com.example.Service::process(com.example.Dog)"),
        "Controller 应该调用 process(Dog)"
    );
    
    // 验证 Controller::handle 也调用了 process(Animal)（多态调用）
    assert!(
        callees.contains(&"com.example.Service::process(com.example.Animal)"),
        "Controller 应该也调用 process(Animal)（多态）"
    );
    
    // 验证反向调用关系
    let callers_dog = index.find_callers("com.example.Service::process(com.example.Dog)");
    assert!(callers_dog.contains(&"com.example.Controller::handle()"));
    
    let callers_animal = index.find_callers("com.example.Service::process(com.example.Animal)");
    assert!(
        callers_animal.contains(&"com.example.Controller::handle()"),
        "process(Animal) 应该被 Controller 调用（多态）"
    );
}

#[test]
fn test_polymorphic_call_with_multiple_params() {
    let mut index = CodeIndex::new();
    
    // 创建继承关系：Cat extends Animal
    let animal_class = ClassInfo {
        name: "com.example.Animal".to_string(),
        line_range: (1, 10),
        methods: vec![],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    let cat_class = ClassInfo {
        name: "com.example.Cat".to_string(),
        line_range: (1, 10),
        methods: vec![],
        is_interface: false,
        implements: vec![],
        extends: Some("com.example.Animal".to_string()),
    };
    
    // 创建方法：process(String, Cat) 和 process(String, Animal)
    let process_cat_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.Service::process(String,com.example.Cat)".to_string(),
        file_path: PathBuf::from("Service.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: None,
    };
    
    let process_animal_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.Service::process(String,com.example.Animal)".to_string(),
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
        methods: vec![process_cat_method.clone(), process_animal_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    // 创建调用者
    let caller_method = MethodInfo {
        name: "execute".to_string(),
        full_qualified_name: "com.example.Caller::execute()".to_string(),
        file_path: PathBuf::from("Caller.java"),
        line_range: (10, 20),
        calls: vec![
            MethodCall {
                target: "com.example.Service::process(String,com.example.Cat)".to_string(),
                line: 15,
            }
        ],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: None,
    };
    
    let caller_class = ClassInfo {
        name: "com.example.Caller".to_string(),
        line_range: (5, 25),
        methods: vec![caller_method.clone()],
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
        file_path: PathBuf::from("Cat.java"),
        language: "java".to_string(),
        classes: vec![cat_class],
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
        file_path: PathBuf::from("Caller.java"),
        language: "java".to_string(),
        classes: vec![caller_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    // 传播
    index.propagate_inherited_members();
    index.propagate_polymorphic_calls();
    
    // 验证多态调用（第二个参数从 Cat 变为 Animal）
    let callees = index.find_callees("com.example.Caller::execute()");
    assert!(
        callees.contains(&"com.example.Service::process(String,com.example.Cat)"),
        "应该调用 process(String, Cat)"
    );
    assert!(
        callees.contains(&"com.example.Service::process(String,com.example.Animal)"),
        "应该也调用 process(String, Animal)（多态）"
    );
}

#[test]
fn test_no_polymorphic_call_without_parent() {
    let mut index = CodeIndex::new();
    
    // 创建没有继承关系的类
    let dog_class = ClassInfo {
        name: "com.example.Dog".to_string(),
        line_range: (1, 10),
        methods: vec![],
        is_interface: false,
        implements: vec![],
        extends: None, // 没有父类
    };
    
    // 创建方法
    let process_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.Service::process(com.example.Dog)".to_string(),
        file_path: PathBuf::from("Service.java"),
        line_range: (10, 15),
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
        methods: vec![process_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    // 创建调用者
    let caller_method = MethodInfo {
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
        methods: vec![caller_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: None,
    };
    
    // 索引所有类
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
    
    // 传播
    index.propagate_inherited_members();
    index.propagate_polymorphic_calls();
    
    // 验证只有原始调用，没有多态调用
    let callees = index.find_callees("com.example.Controller::handle()");
    assert_eq!(callees.len(), 1, "应该只有一个调用");
    assert!(callees.contains(&"com.example.Service::process(com.example.Dog)"));
}
