use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::language_parser::{ClassInfo, MethodInfo, ParsedFile};
use std::path::PathBuf;

#[test]
fn test_class_inheritance_tracking() {
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
        return_type: Some("String".to_string()),
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
        return_type: Some("String".to_string()),
    };
    
    let child_class = ClassInfo {
        name: "com.example.Child".to_string(),
        line_range: (5, 20),
        methods: vec![child_method.clone()],
        is_interface: false,
        implements: vec![],
        extends: Some("com.example.Parent".to_string()),
    };
    
    // 索引父类和子类
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
    
    // 验证继承关系被正确记录
    assert_eq!(
        index.find_parent_class("com.example.Child"),
        Some("com.example.Parent")
    );
    
    let children = index.find_child_classes("com.example.Parent");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0], "com.example.Child");
    
    // 传播继承的成员
    index.propagate_inherited_members();
    
    // 验证子类可以访问父类的方法
    let child_inherited_method = index.find_method("com.example.Child::parentMethod()");
    assert!(child_inherited_method.is_some(), "子类应该继承父类的方法");
}

#[test]
fn test_interface_inheritance_propagation() {
    let mut index = CodeIndex::new();
    
    // 创建接口
    let interface_method = MethodInfo {
        name: "interfaceMethod".to_string(),
        full_qualified_name: "com.example.MyInterface::interfaceMethod()".to_string(),
        file_path: PathBuf::from("MyInterface.java"),
        line_range: (5, 7),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: Some("void".to_string()),
    };
    
    let interface_class = ClassInfo {
        name: "com.example.MyInterface".to_string(),
        line_range: (3, 8),
        methods: vec![interface_method.clone()],
        is_interface: true,
        implements: vec![],
        extends: None,
    };
    
    // 创建父类（实现接口）
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
        return_type: Some("String".to_string()),
    };
    
    let parent_class = ClassInfo {
        name: "com.example.Parent".to_string(),
        line_range: (5, 20),
        methods: vec![parent_method.clone()],
        is_interface: false,
        implements: vec!["com.example.MyInterface".to_string()],
        extends: None,
    };
    
    // 创建子类（继承父类）
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
        return_type: Some("String".to_string()),
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
        file_path: PathBuf::from("MyInterface.java"),
        language: "java".to_string(),
        classes: vec![interface_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
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
    
    // 传播继承的成员
    index.propagate_inherited_members();
    
    // 验证子类继承了父类实现的接口
    let child_interfaces = index.find_class_interfaces("com.example.Child");
    assert!(
        child_interfaces.contains(&"com.example.MyInterface"),
        "子类应该继承父类实现的接口"
    );
    
    // 验证接口的实现类列表包含子类
    let interface_impls = index.find_interface_implementations("com.example.MyInterface");
    assert!(
        interface_impls.contains(&"com.example.Child"),
        "接口的实现类列表应该包含子类"
    );
}

#[test]
fn test_multi_level_inheritance() {
    let mut index = CodeIndex::new();
    
    // 创建祖父类
    let grandparent_method = MethodInfo {
        name: "grandparentMethod".to_string(),
        full_qualified_name: "com.example.GrandParent::grandparentMethod()".to_string(),
        file_path: PathBuf::from("GrandParent.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
        return_type: Some("String".to_string()),
    };
    
    let grandparent_class = ClassInfo {
        name: "com.example.GrandParent".to_string(),
        line_range: (5, 20),
        methods: vec![grandparent_method.clone()],
        is_interface: false,
        implements: vec!["com.example.Interface1".to_string()],
        extends: None,
    };
    
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
        return_type: Some("String".to_string()),
    };
    
    let parent_class = ClassInfo {
        name: "com.example.Parent".to_string(),
        line_range: (5, 20),
        methods: vec![parent_method.clone()],
        is_interface: false,
        implements: vec!["com.example.Interface2".to_string()],
        extends: Some("com.example.GrandParent".to_string()),
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
        return_type: Some("String".to_string()),
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
        file_path: PathBuf::from("GrandParent.java"),
        language: "java".to_string(),
        classes: vec![grandparent_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
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
    
    // 传播继承的成员
    index.propagate_inherited_members();
    
    // 验证子类继承了所有祖先的接口
    let child_interfaces = index.find_class_interfaces("com.example.Child");
    assert!(
        child_interfaces.contains(&"com.example.Interface1"),
        "子类应该继承祖父类实现的接口"
    );
    assert!(
        child_interfaces.contains(&"com.example.Interface2"),
        "子类应该继承父类实现的接口"
    );
    
    // 验证子类可以访问所有祖先的方法
    assert!(
        index.find_method("com.example.Child::grandparentMethod()").is_some(),
        "子类应该继承祖父类的方法"
    );
    assert!(
        index.find_method("com.example.Child::parentMethod()").is_some(),
        "子类应该继承父类的方法"
    );
}
