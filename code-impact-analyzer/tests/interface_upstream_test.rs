use code_impact_analyzer::{CodeIndex, ImpactTracer, TraceConfig};
use code_impact_analyzer::language_parser::{MethodInfo, MethodCall, ClassInfo, ParsedFile};
use std::path::PathBuf;

#[test]
fn test_interface_upstream_tracing() {
    // 创建代码索引
    let mut index = CodeIndex::new();
    
    // 场景：
    // 1. 接口 Service 有方法 execute
    // 2. 实现类 ServiceImpl 实现了 Service::execute
    // 3. Controller 调用接口方法 Service::execute
    // 4. 当追溯 ServiceImpl::execute 的上游时，应该找到 Controller
    
    // 定义接口方法
    let interface_method = MethodInfo {
        name: "execute".to_string(),
        full_qualified_name: "com.example.Service::execute".to_string(),
        file_path: PathBuf::from("Service.java"),
        line_range: (10, 12),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 定义实现类方法
    let impl_method = MethodInfo {
        name: "execute".to_string(),
        full_qualified_name: "com.example.ServiceImpl::execute".to_string(),
        file_path: PathBuf::from("ServiceImpl.java"),
        line_range: (20, 30),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 定义 Controller 方法，调用接口方法
    let controller_method = MethodInfo {
        name: "handle".to_string(),
        full_qualified_name: "com.example.Controller::handle".to_string(),
        file_path: PathBuf::from("Controller.java"),
        line_range: (15, 25),
        calls: vec![
            MethodCall {
                target: "com.example.Service::execute".to_string(),
                line: 18,
            }
        ],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 创建接口类
    let interface_class = ClassInfo {
        name: "com.example.Service".to_string(),
        line_range: (5, 15),
        methods: vec![interface_method.clone()],
        is_interface: true,
        implements: vec![],
    };
    
    // 创建实现类
    let impl_class = ClassInfo {
        name: "com.example.ServiceImpl".to_string(),
        line_range: (10, 35),
        methods: vec![impl_method.clone()],
        is_interface: false,
        implements: vec!["com.example.Service".to_string()],
    };
    
    // 创建 Controller 类
    let controller_class = ClassInfo {
        name: "com.example.Controller".to_string(),
        line_range: (10, 30),
        methods: vec![controller_method.clone()],
        is_interface: false,
        implements: vec![],
    };
    
    // 索引所有类
    let interface_file = ParsedFile {
        file_path: PathBuf::from("Service.java"),
        language: "java".to_string(),
        classes: vec![interface_class],
        functions: vec![],
        imports: vec![],
    };
    
    let impl_file = ParsedFile {
        file_path: PathBuf::from("ServiceImpl.java"),
        language: "java".to_string(),
        classes: vec![impl_class],
        functions: vec![],
        imports: vec![],
    };
    
    let controller_file = ParsedFile {
        file_path: PathBuf::from("Controller.java"),
        language: "java".to_string(),
        classes: vec![controller_class],
        functions: vec![],
        imports: vec![],
    };
    
    index.test_index_parsed_file(interface_file).unwrap();
    index.test_index_parsed_file(impl_file).unwrap();
    index.test_index_parsed_file(controller_file).unwrap();
    
    // 验证接口实现关系
    let implementations = index.find_interface_implementations("com.example.Service");
    assert_eq!(implementations, vec!["com.example.ServiceImpl"]);
    
    let interfaces = index.find_class_interfaces("com.example.ServiceImpl");
    assert_eq!(interfaces, vec!["com.example.Service"]);
    
    // 验证调用关系
    let interface_callers = index.find_callers("com.example.Service::execute");
    assert_eq!(interface_callers, vec!["com.example.Controller::handle"]);
    
    let impl_callers = index.find_callers("com.example.ServiceImpl::execute");
    assert_eq!(impl_callers.len(), 0); // 没有直接调用实现类方法
    
    // 追溯实现类方法的上游
    let config = TraceConfig {
        max_depth: 10,
        trace_upstream: true,
        trace_downstream: false,
        trace_cross_service: false,
    };
    
    let tracer = ImpactTracer::new(&index, config);
    let graph = tracer.trace_impact(&["com.example.ServiceImpl::execute".to_string()])
        .expect("追溯失败");
    
    // 验证影响图
    assert_eq!(graph.node_count(), 2); // ServiceImpl::execute 和 Controller::handle
    assert_eq!(graph.edge_count(), 1); // Controller::handle -> ServiceImpl::execute
    
    // 验证节点存在
    let has_controller = graph.nodes().any(|n| n.id.contains("Controller"));
    let has_impl = graph.nodes().any(|n| n.id.contains("ServiceImpl"));
    
    assert!(has_controller, "应该包含 Controller 节点");
    assert!(has_impl, "应该包含 ServiceImpl 节点");
}

#[test]
fn test_multiple_interfaces_upstream_tracing() {
    // 测试一个类实现多个接口的情况
    let mut index = CodeIndex::new();
    
    // 实现类方法
    let impl_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.MultiImpl::process".to_string(),
        file_path: PathBuf::from("MultiImpl.java"),
        line_range: (20, 30),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 接口1方法
    let interface1_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.Interface1::process".to_string(),
        file_path: PathBuf::from("Interface1.java"),
        line_range: (10, 12),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 接口2方法
    let interface2_method = MethodInfo {
        name: "process".to_string(),
        full_qualified_name: "com.example.Interface2::process".to_string(),
        file_path: PathBuf::from("Interface2.java"),
        line_range: (10, 12),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // Caller1 调用 Interface1::process
    let caller1_method = MethodInfo {
        name: "call1".to_string(),
        full_qualified_name: "com.example.Caller1::call1".to_string(),
        file_path: PathBuf::from("Caller1.java"),
        line_range: (15, 25),
        calls: vec![
            MethodCall {
                target: "com.example.Interface1::process".to_string(),
                line: 18,
            }
        ],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // Caller2 调用 Interface2::process
    let caller2_method = MethodInfo {
        name: "call2".to_string(),
        full_qualified_name: "com.example.Caller2::call2".to_string(),
        file_path: PathBuf::from("Caller2.java"),
        line_range: (15, 25),
        calls: vec![
            MethodCall {
                target: "com.example.Interface2::process".to_string(),
                line: 18,
            }
        ],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 创建类
    let impl_class = ClassInfo {
        name: "com.example.MultiImpl".to_string(),
        line_range: (10, 35),
        methods: vec![impl_method.clone()],
        is_interface: false,
        implements: vec![
            "com.example.Interface1".to_string(),
            "com.example.Interface2".to_string(),
        ],
    };
    
    let interface1_class = ClassInfo {
        name: "com.example.Interface1".to_string(),
        line_range: (5, 15),
        methods: vec![interface1_method.clone()],
        is_interface: true,
        implements: vec![],
    };
    
    let interface2_class = ClassInfo {
        name: "com.example.Interface2".to_string(),
        line_range: (5, 15),
        methods: vec![interface2_method.clone()],
        is_interface: true,
        implements: vec![],
    };
    
    let caller1_class = ClassInfo {
        name: "com.example.Caller1".to_string(),
        line_range: (10, 30),
        methods: vec![caller1_method.clone()],
        is_interface: false,
        implements: vec![],
    };
    
    let caller2_class = ClassInfo {
        name: "com.example.Caller2".to_string(),
        line_range: (10, 30),
        methods: vec![caller2_method.clone()],
        is_interface: false,
        implements: vec![],
    };
    
    // 索引所有类
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("MultiImpl.java"),
        language: "java".to_string(),
        classes: vec![impl_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Interface1.java"),
        language: "java".to_string(),
        classes: vec![interface1_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Interface2.java"),
        language: "java".to_string(),
        classes: vec![interface2_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Caller1.java"),
        language: "java".to_string(),
        classes: vec![caller1_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    index.test_index_parsed_file(ParsedFile {
        file_path: PathBuf::from("Caller2.java"),
        language: "java".to_string(),
        classes: vec![caller2_class],
        functions: vec![],
        imports: vec![],
    }).unwrap();
    
    // 验证接口实现关系
    let interfaces = index.find_class_interfaces("com.example.MultiImpl");
    assert_eq!(interfaces.len(), 2);
    assert!(interfaces.contains(&"com.example.Interface1"));
    assert!(interfaces.contains(&"com.example.Interface2"));
    
    // 追溯实现类方法的上游
    let config = TraceConfig {
        max_depth: 10,
        trace_upstream: true,
        trace_downstream: false,
        trace_cross_service: false,
    };
    
    let tracer = ImpactTracer::new(&index, config);
    let graph = tracer.trace_impact(&["com.example.MultiImpl::process".to_string()])
        .expect("追溯失败");
    
    // 验证影响图 - 应该包含两个调用者
    assert_eq!(graph.node_count(), 3); // MultiImpl::process, Caller1::call1, Caller2::call2
    assert_eq!(graph.edge_count(), 2); // 两条边
    
    // 验证节点存在
    let has_caller1 = graph.nodes().any(|n| n.id.contains("Caller1"));
    let has_caller2 = graph.nodes().any(|n| n.id.contains("Caller2"));
    let has_impl = graph.nodes().any(|n| n.id.contains("MultiImpl"));
    
    assert!(has_caller1, "应该包含 Caller1 节点");
    assert!(has_caller2, "应该包含 Caller2 节点");
    assert!(has_impl, "应该包含 MultiImpl 节点");
}
