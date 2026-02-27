use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::impact_tracer::{ImpactTracer, TraceConfig, NodeType, EdgeType};
use code_impact_analyzer::language_parser::{MethodInfo, MethodCall};
use code_impact_analyzer::types::{
    HttpAnnotation, HttpMethod, KafkaOperation, KafkaOpType,
    DbOperation, DbOpType, RedisOperation, RedisOpType,
};

/// 测试 HTTP 接口双向追溯
#[test]
fn test_http_bidirectional_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建 HTTP 提供者方法
    let provider = MethodInfo {
        name: "getUser".to_string(),
        full_qualified_name: "com.example.UserController::getUser".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::GET,
            path: "/api/users/{id}".to_string(),
            path_params: vec!["id".to_string()],
            is_feign_client: false,
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 索引提供者
    index.test_index_method(&provider).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 追溯影响
    let result = tracer.trace_impact(&["com.example.UserController::getUser".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含方法节点
    assert!(graph.get_node("method:com.example.UserController::getUser").is_some());
    
    // 验证包含 HTTP 端点节点
    assert!(graph.get_node("http:GET:/api/users/{id}").is_some());
    
    // 验证节点类型
    let http_node = graph.get_node("http:GET:/api/users/{id}").unwrap();
    assert!(matches!(http_node.node_type, NodeType::HttpEndpoint { .. }));
    
    // 验证边的存在
    let has_http_edge = graph.edges().any(|edge| {
        edge.from == "method:com.example.UserController::getUser"
            && edge.to == "http:GET:/api/users/{id}"
            && edge.edge_type == EdgeType::HttpCall
    });
    assert!(has_http_edge);
}

/// 测试 Kafka Topic 双向追溯 - 生产者到消费者
#[test]
fn test_kafka_producer_to_consumer_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建 Kafka 生产者方法
    let producer = MethodInfo {
        name: "sendEvent".to_string(),
        full_qualified_name: "com.example.EventProducer::sendEvent".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![KafkaOperation {
            operation_type: KafkaOpType::Produce,
            topic: "user-events".to_string(),
            line: 15,
        }],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 创建 Kafka 消费者方法
    let consumer = MethodInfo {
        name: "handleEvent".to_string(),
        full_qualified_name: "com.example.EventConsumer::handleEvent".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![KafkaOperation {
            operation_type: KafkaOpType::Consume,
            topic: "user-events".to_string(),
            line: 35,
        }],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 索引生产者和消费者
    index.test_index_method(&producer).unwrap();
    index.test_index_method(&consumer).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 从生产者开始追溯
    let result = tracer.trace_impact(&["com.example.EventProducer::sendEvent".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含生产者节点
    assert!(graph.get_node("method:com.example.EventProducer::sendEvent").is_some());
    
    // 验证包含 Kafka Topic 节点
    assert!(graph.get_node("kafka:user-events").is_some());
    
    // 验证包含消费者节点
    assert!(graph.get_node("method:com.example.EventConsumer::handleEvent").is_some());
    
    // 验证节点类型
    let kafka_node = graph.get_node("kafka:user-events").unwrap();
    assert!(matches!(kafka_node.node_type, NodeType::KafkaTopic { .. }));
    
    // 验证边：producer -> topic
    let has_producer_edge = graph.edges().any(|edge| {
        edge.from == "method:com.example.EventProducer::sendEvent"
            && edge.to == "kafka:user-events"
            && edge.edge_type == EdgeType::KafkaProduceConsume
    });
    assert!(has_producer_edge);
    
    // 验证边：topic -> consumer
    let has_consumer_edge = graph.edges().any(|edge| {
        edge.from == "kafka:user-events"
            && edge.to == "method:com.example.EventConsumer::handleEvent"
            && edge.edge_type == EdgeType::KafkaProduceConsume
    });
    assert!(has_consumer_edge);
}

/// 测试 Kafka Topic 双向追溯 - 消费者到生产者
#[test]
fn test_kafka_consumer_to_producer_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建 Kafka 生产者方法
    let producer = MethodInfo {
        name: "sendEvent".to_string(),
        full_qualified_name: "com.example.EventProducer::sendEvent".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![KafkaOperation {
            operation_type: KafkaOpType::Produce,
            topic: "order-events".to_string(),
            line: 15,
        }],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 创建 Kafka 消费者方法
    let consumer = MethodInfo {
        name: "processOrder".to_string(),
        full_qualified_name: "com.example.OrderProcessor::processOrder".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![KafkaOperation {
            operation_type: KafkaOpType::Consume,
            topic: "order-events".to_string(),
            line: 35,
        }],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 索引生产者和消费者
    index.test_index_method(&producer).unwrap();
    index.test_index_method(&consumer).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 从消费者开始追溯（上游）
    let result = tracer.trace_impact(&["com.example.OrderProcessor::processOrder".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含消费者节点
    assert!(graph.get_node("method:com.example.OrderProcessor::processOrder").is_some());
    
    // 验证包含 Kafka Topic 节点
    assert!(graph.get_node("kafka:order-events").is_some());
    
    // 验证包含生产者节点
    assert!(graph.get_node("method:com.example.EventProducer::sendEvent").is_some());
}

/// 测试数据库表双向追溯 - 写入者到读取者
#[test]
fn test_database_writer_to_reader_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建数据库写入者方法
    let writer = MethodInfo {
        name: "saveUser".to_string(),
        full_qualified_name: "com.example.UserRepository::saveUser".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![DbOperation {
            operation_type: DbOpType::Insert,
            table: "users".to_string(),
            line: 15,
        }],
        redis_operations: vec![],
    };
    
    // 创建数据库读取者方法
    let reader = MethodInfo {
        name: "findUser".to_string(),
        full_qualified_name: "com.example.UserRepository::findUser".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![DbOperation {
            operation_type: DbOpType::Select,
            table: "users".to_string(),
            line: 35,
        }],
        redis_operations: vec![],
    };
    
    // 索引写入者和读取者
    index.test_index_method(&writer).unwrap();
    index.test_index_method(&reader).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 从写入者开始追溯
    let result = tracer.trace_impact(&["com.example.UserRepository::saveUser".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含写入者节点
    assert!(graph.get_node("method:com.example.UserRepository::saveUser").is_some());
    
    // 验证包含数据库表节点
    assert!(graph.get_node("db:users").is_some());
    
    // 验证包含读取者节点
    assert!(graph.get_node("method:com.example.UserRepository::findUser").is_some());
    
    // 验证节点类型
    let db_node = graph.get_node("db:users").unwrap();
    assert!(matches!(db_node.node_type, NodeType::DatabaseTable { .. }));
    
    // 验证边：writer -> table
    let has_writer_edge = graph.edges().any(|edge| {
        edge.from == "method:com.example.UserRepository::saveUser"
            && edge.to == "db:users"
            && edge.edge_type == EdgeType::DatabaseReadWrite
    });
    assert!(has_writer_edge);
    
    // 验证边：table -> reader
    let has_reader_edge = graph.edges().any(|edge| {
        edge.from == "db:users"
            && edge.to == "method:com.example.UserRepository::findUser"
            && edge.edge_type == EdgeType::DatabaseReadWrite
    });
    assert!(has_reader_edge);
}

/// 测试数据库表双向追溯 - 读取者到写入者
#[test]
fn test_database_reader_to_writer_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建数据库写入者方法
    let writer = MethodInfo {
        name: "updateOrder".to_string(),
        full_qualified_name: "com.example.OrderRepository::updateOrder".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![DbOperation {
            operation_type: DbOpType::Update,
            table: "orders".to_string(),
            line: 15,
        }],
        redis_operations: vec![],
    };
    
    // 创建数据库读取者方法
    let reader = MethodInfo {
        name: "getOrder".to_string(),
        full_qualified_name: "com.example.OrderRepository::getOrder".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![DbOperation {
            operation_type: DbOpType::Select,
            table: "orders".to_string(),
            line: 35,
        }],
        redis_operations: vec![],
    };
    
    // 索引写入者和读取者
    index.test_index_method(&writer).unwrap();
    index.test_index_method(&reader).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 从读取者开始追溯（上游）
    let result = tracer.trace_impact(&["com.example.OrderRepository::getOrder".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含读取者节点
    assert!(graph.get_node("method:com.example.OrderRepository::getOrder").is_some());
    
    // 验证包含数据库表节点
    assert!(graph.get_node("db:orders").is_some());
    
    // 验证包含写入者节点
    assert!(graph.get_node("method:com.example.OrderRepository::updateOrder").is_some());
}

/// 测试 Redis 键双向追溯 - 写入者到读取者
#[test]
fn test_redis_writer_to_reader_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建 Redis 写入者方法
    let writer = MethodInfo {
        name: "cacheSession".to_string(),
        full_qualified_name: "com.example.SessionCache::cacheSession".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![RedisOperation {
            operation_type: RedisOpType::Set,
            key_pattern: "session:*".to_string(),
            line: 15,
        }],
    };
    
    // 创建 Redis 读取者方法
    let reader = MethodInfo {
        name: "getSession".to_string(),
        full_qualified_name: "com.example.SessionCache::getSession".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![RedisOperation {
            operation_type: RedisOpType::Get,
            key_pattern: "session:*".to_string(),
            line: 35,
        }],
    };
    
    // 索引写入者和读取者
    index.test_index_method(&writer).unwrap();
    index.test_index_method(&reader).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 从写入者开始追溯
    let result = tracer.trace_impact(&["com.example.SessionCache::cacheSession".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含写入者节点
    assert!(graph.get_node("method:com.example.SessionCache::cacheSession").is_some());
    
    // 验证包含 Redis 键节点
    assert!(graph.get_node("redis:session:*").is_some());
    
    // 验证包含读取者节点
    assert!(graph.get_node("method:com.example.SessionCache::getSession").is_some());
    
    // 验证节点类型
    let redis_node = graph.get_node("redis:session:*").unwrap();
    assert!(matches!(redis_node.node_type, NodeType::RedisPrefix { .. }));
    
    // 验证边：writer -> redis
    let has_writer_edge = graph.edges().any(|edge| {
        edge.from == "method:com.example.SessionCache::cacheSession"
            && edge.to == "redis:session:*"
            && edge.edge_type == EdgeType::RedisReadWrite
    });
    assert!(has_writer_edge);
    
    // 验证边：redis -> reader
    let has_reader_edge = graph.edges().any(|edge| {
        edge.from == "redis:session:*"
            && edge.to == "method:com.example.SessionCache::getSession"
            && edge.edge_type == EdgeType::RedisReadWrite
    });
    assert!(has_reader_edge);
}

/// 测试 Redis 键双向追溯 - 读取者到写入者
#[test]
fn test_redis_reader_to_writer_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建 Redis 写入者方法
    let writer = MethodInfo {
        name: "cacheUser".to_string(),
        full_qualified_name: "com.example.UserCache::cacheUser".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![RedisOperation {
            operation_type: RedisOpType::Set,
            key_pattern: "user:*".to_string(),
            line: 15,
        }],
    };
    
    // 创建 Redis 读取者方法
    let reader = MethodInfo {
        name: "getUser".to_string(),
        full_qualified_name: "com.example.UserCache::getUser".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![RedisOperation {
            operation_type: RedisOpType::Get,
            key_pattern: "user:*".to_string(),
            line: 35,
        }],
    };
    
    // 索引写入者和读取者
    index.test_index_method(&writer).unwrap();
    index.test_index_method(&reader).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 从读取者开始追溯（上游）
    let result = tracer.trace_impact(&["com.example.UserCache::getUser".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含读取者节点
    assert!(graph.get_node("method:com.example.UserCache::getUser").is_some());
    
    // 验证包含 Redis 键节点
    assert!(graph.get_node("redis:user:*").is_some());
    
    // 验证包含写入者节点
    assert!(graph.get_node("method:com.example.UserCache::cacheUser").is_some());
}

/// 测试复杂的跨服务追溯场景
#[test]
fn test_complex_cross_service_tracing() {
    let mut index = CodeIndex::new();
    
    // 创建一个复杂的调用链：
    // HTTP -> Method1 -> Kafka Producer -> Kafka Consumer -> DB Writer -> DB Reader -> Redis Writer -> Redis Reader
    
    let http_handler = MethodInfo {
        name: "handleRequest".to_string(),
        full_qualified_name: "com.example.Controller::handleRequest".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (10, 20),
        calls: vec![MethodCall {
            target: "com.example.Service::processRequest".to_string(),
            line: 15,
        }],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::POST,
            path: "/api/process".to_string(),
            path_params: vec![],
            is_feign_client: false,
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    let service_method = MethodInfo {
        name: "processRequest".to_string(),
        full_qualified_name: "com.example.Service::processRequest".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![KafkaOperation {
            operation_type: KafkaOpType::Produce,
            topic: "process-events".to_string(),
            line: 35,
        }],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    let kafka_consumer = MethodInfo {
        name: "handleEvent".to_string(),
        full_qualified_name: "com.example.EventHandler::handleEvent".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (50, 60),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![KafkaOperation {
            operation_type: KafkaOpType::Consume,
            topic: "process-events".to_string(),
            line: 55,
        }],
        db_operations: vec![DbOperation {
            operation_type: DbOpType::Insert,
            table: "events".to_string(),
            line: 58,
        }],
        redis_operations: vec![],
    };
    
    let db_reader = MethodInfo {
        name: "queryEvents".to_string(),
        full_qualified_name: "com.example.EventQuery::queryEvents".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (70, 80),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![DbOperation {
            operation_type: DbOpType::Select,
            table: "events".to_string(),
            line: 75,
        }],
        redis_operations: vec![RedisOperation {
            operation_type: RedisOpType::Set,
            key_pattern: "event:*".to_string(),
            line: 78,
        }],
    };
    
    let redis_reader = MethodInfo {
        name: "getCachedEvent".to_string(),
        full_qualified_name: "com.example.EventCache::getCachedEvent".to_string(),
        file_path: std::path::PathBuf::from("test.java"),
        line_range: (90, 100),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![RedisOperation {
            operation_type: RedisOpType::Get,
            key_pattern: "event:*".to_string(),
            line: 95,
        }],
    };
    
    // 索引所有方法
    index.test_index_method(&http_handler).unwrap();
    index.test_index_method(&service_method).unwrap();
    index.test_index_method(&kafka_consumer).unwrap();
    index.test_index_method(&db_reader).unwrap();
    index.test_index_method(&redis_reader).unwrap();
    
    // 创建追溯器
    let config = TraceConfig::default();
    let tracer = ImpactTracer::new(&index, config);
    
    // 从 HTTP 处理器开始追溯
    let result = tracer.trace_impact(&["com.example.Controller::handleRequest".to_string()]);
    assert!(result.is_ok());
    
    let graph = result.unwrap();
    
    // 验证包含所有关键节点
    assert!(graph.get_node("method:com.example.Controller::handleRequest").is_some());
    assert!(graph.get_node("http:POST:/api/process").is_some());
    assert!(graph.get_node("method:com.example.Service::processRequest").is_some());
    assert!(graph.get_node("kafka:process-events").is_some());
    assert!(graph.get_node("method:com.example.EventHandler::handleEvent").is_some());
    assert!(graph.get_node("db:events").is_some());
    assert!(graph.get_node("method:com.example.EventQuery::queryEvents").is_some());
    assert!(graph.get_node("redis:event:*").is_some());
    assert!(graph.get_node("method:com.example.EventCache::getCachedEvent").is_some());
    
    // 验证至少有多个边类型
    let has_method_call = graph.edges().any(|e| e.edge_type == EdgeType::MethodCall);
    let has_http_call = graph.edges().any(|e| e.edge_type == EdgeType::HttpCall);
    let has_kafka = graph.edges().any(|e| e.edge_type == EdgeType::KafkaProduceConsume);
    let has_db = graph.edges().any(|e| e.edge_type == EdgeType::DatabaseReadWrite);
    let has_redis = graph.edges().any(|e| e.edge_type == EdgeType::RedisReadWrite);
    
    assert!(has_method_call);
    assert!(has_http_call);
    assert!(has_kafka);
    assert!(has_db);
    assert!(has_redis);
}
