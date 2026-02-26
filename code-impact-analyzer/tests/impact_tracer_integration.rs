use code_impact_analyzer::{
    CodeIndex, ImpactTracer, TraceConfig,
};

#[test]
fn test_trace_simple_call_chain() {
    // 创建一个简单的调用链进行测试
    // 由于 CodeIndex 的 index_method 是私有的，我们需要通过文件解析来构建索引
    // 这里我们创建一个空索引并测试基本功能
    
    let index = CodeIndex::new();
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 测试追溯一个不存在的方法（应该只返回该方法本身）
    let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    assert_eq!(graph.node_count(), 1);
    assert!(graph.get_node("method:com.example.Test::test").is_some());
}

#[test]
fn test_trace_with_custom_config() {
    let index = CodeIndex::new();
    
    // 测试自定义配置
    let config = TraceConfig {
        max_depth: 5,
        trace_upstream: true,
        trace_downstream: false,
        trace_cross_service: false,
    };
    
    let tracer = ImpactTracer::new(&index, config);
    let result = tracer.trace_impact(&["com.example.Service::method".to_string()]);
    
    assert!(result.is_ok());
}

#[test]
fn test_trace_multiple_changed_methods() {
    let index = CodeIndex::new();
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 测试追溯多个变更方法
    let changed_methods = vec![
        "com.example.A::methodA".to_string(),
        "com.example.B::methodB".to_string(),
        "com.example.C::methodC".to_string(),
    ];
    
    let result = tracer.trace_impact(&changed_methods);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    // 应该有 3 个节点（3 个变更的方法）
    assert_eq!(graph.node_count(), 3);
}

#[test]
fn test_trace_with_zero_depth() {
    let index = CodeIndex::new();
    let config = TraceConfig {
        max_depth: 0,
        trace_upstream: true,
        trace_downstream: true,
        trace_cross_service: false,
    };
    
    let tracer = ImpactTracer::new(&index, config);
    let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
    
    assert!(result.is_ok());
    let graph = result.unwrap();
    
    // 深度为 0，应该只有初始节点，没有追溯
    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.edge_count(), 0);
}
