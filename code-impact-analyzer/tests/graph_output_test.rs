use code_impact_analyzer::impact_tracer::{ImpactGraph, ImpactNode, EdgeType, Direction};
use code_impact_analyzer::types::HttpMethod;

#[test]
fn test_graph_output_formats_integration() {
    let mut graph = ImpactGraph::new();
    
    // 创建一个包含多种节点类型的图
    let method_a = ImpactNode::method("com.example.ServiceA::processRequest".to_string());
    let http_endpoint = ImpactNode::http_endpoint(HttpMethod::POST, "/api/process".to_string());
    let kafka_topic = ImpactNode::kafka_topic("request-events".to_string());
    let db_table = ImpactNode::database_table("requests".to_string());
    let redis_key = ImpactNode::redis_prefix("cache:request:*".to_string());
    
    graph.add_node(method_a);
    graph.add_node(http_endpoint);
    graph.add_node(kafka_topic);
    graph.add_node(db_table);
    graph.add_node(redis_key);
    
    // 添加边
    graph.add_edge(
        "method:com.example.ServiceA::processRequest",
        "http:POST:/api/process",
        EdgeType::HttpCall,
        Direction::Downstream,
    );
    
    graph.add_edge(
        "method:com.example.ServiceA::processRequest",
        "kafka:request-events",
        EdgeType::KafkaProduceConsume,
        Direction::Downstream,
    );
    
    graph.add_edge(
        "method:com.example.ServiceA::processRequest",
        "db:requests",
        EdgeType::DatabaseReadWrite,
        Direction::Downstream,
    );
    
    graph.add_edge(
        "redis:cache:request:*",
        "method:com.example.ServiceA::processRequest",
        EdgeType::RedisReadWrite,
        Direction::Upstream,
    );
    
    // 测试 DOT 输出
    let dot_output = graph.to_dot();
    assert!(dot_output.contains("digraph"));
    assert!(dot_output.contains("com.example.ServiceA::processRequest"));
    assert!(dot_output.contains("/api/process"));
    assert!(dot_output.contains("request-events"));
    assert!(dot_output.contains("requests"));
    assert!(dot_output.contains("cache:request:*"));
    println!("DOT Output:\n{}\n", dot_output);
    
    // 测试 JSON 输出
    let json_output = graph.to_json().expect("Failed to generate JSON");
    assert!(json_output.contains("nodes"));
    assert!(json_output.contains("edges"));
    
    let parsed: serde_json::Value = serde_json::from_str(&json_output)
        .expect("Failed to parse JSON");
    assert_eq!(parsed["node_count"], 5);
    assert_eq!(parsed["edge_count"], 4);
    println!("JSON Output:\n{}\n", json_output);
    
    // 测试循环检测（这个图没有循环）
    let cycles = graph.detect_cycles();
    assert_eq!(cycles.len(), 0);
    println!("Cycles detected: {}", cycles.len());
}

#[test]
fn test_cycle_detection_integration() {
    let mut graph = ImpactGraph::new();
    
    // 创建一个包含循环的图: A -> B -> C -> A
    let node_a = ImpactNode::method("com.example.A::methodA".to_string());
    let node_b = ImpactNode::method("com.example.B::methodB".to_string());
    let node_c = ImpactNode::method("com.example.C::methodC".to_string());
    
    graph.add_node(node_a);
    graph.add_node(node_b);
    graph.add_node(node_c);
    
    graph.add_edge("method:com.example.A::methodA", "method:com.example.B::methodB", 
                   EdgeType::MethodCall, Direction::Downstream);
    graph.add_edge("method:com.example.B::methodB", "method:com.example.C::methodC", 
                   EdgeType::MethodCall, Direction::Downstream);
    graph.add_edge("method:com.example.C::methodC", "method:com.example.A::methodA", 
                   EdgeType::MethodCall, Direction::Downstream);
    
    // 检测循环
    let cycles = graph.detect_cycles();
    
    assert_eq!(cycles.len(), 1, "Should detect exactly one cycle");
    assert_eq!(cycles[0].len(), 3, "Cycle should contain 3 nodes");
    
    // 验证循环包含所有三个节点
    assert!(cycles[0].contains(&"method:com.example.A::methodA".to_string()));
    assert!(cycles[0].contains(&"method:com.example.B::methodB".to_string()));
    assert!(cycles[0].contains(&"method:com.example.C::methodC".to_string()));
    
    println!("Detected cycle: {:?}", cycles[0]);
    
    // 测试 JSON 输出仍然有效
    let json_output = graph.to_json().expect("Failed to generate JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json_output)
        .expect("Failed to parse JSON");
    assert_eq!(parsed["node_count"], 3);
    assert_eq!(parsed["edge_count"], 3);
}
