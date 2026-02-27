use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::language_parser::MethodInfo;
use code_impact_analyzer::types::{HttpAnnotation, HttpMethod, HttpEndpoint};
use std::path::PathBuf;

#[test]
fn test_http_interface_provider_direction() {
    let mut index = CodeIndex::new();
    
    // 创建一个 HTTP 接口提供者（普通的 @GetMapping）
    let provider = MethodInfo {
        name: "getUser".to_string(),
        full_qualified_name: "com.example.UserController::getUser".to_string(),
        file_path: PathBuf::from("UserController.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::GET,
            path: "md-user-service/api/users/{id}".to_string(),
            path_params: vec!["id".to_string()],
            is_feign_client: false,
            is_feign_client: false,  // 普通 HTTP 接口
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    index.index_method(&provider).unwrap();
    
    // 验证：普通的 HTTP 接口应该被索引为提供者
    let endpoint = HttpEndpoint {
        method: HttpMethod::GET,
        path_pattern: "md-user-service/api/users/{id}".to_string(),
    };
    
    let providers = index.find_http_providers(&endpoint);
    assert_eq!(providers.len(), 1);
    assert!(providers.contains(&"com.example.UserController::getUser"));
    
    // 消费者列表应该为空
    let consumers = index.find_http_consumers(&endpoint);
    assert_eq!(consumers.len(), 0);
}

#[test]
fn test_feign_client_consumer_direction() {
    let mut index = CodeIndex::new();
    
    // 创建一个 Feign 客户端调用（路径格式：service-name/path）
    let consumer = MethodInfo {
        name: "fetchUser".to_string(),
        full_qualified_name: "com.example.UserClient::fetchUser".to_string(),
        file_path: PathBuf::from("UserClient.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::GET,
            path: "user-service/api/users".to_string(),
            path_params: vec![],
            is_feign_client: false,
            is_feign_client: true,  // Feign 调用
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    index.index_method(&consumer).unwrap();
    
    // 验证：Feign 调用应该被索引为消费者
    let endpoint = HttpEndpoint {
        method: HttpMethod::GET,
        path_pattern: "user-service/api/users".to_string(),
    };
    
    let consumers = index.find_http_consumers(&endpoint);
    assert_eq!(consumers.len(), 1);
    assert!(consumers.contains(&"com.example.UserClient::fetchUser"));
    
    // 提供者列表应该为空
    let providers = index.find_http_providers(&endpoint);
    assert_eq!(providers.len(), 0);
}

#[test]
fn test_feign_client_with_base_path() {
    let mut index = CodeIndex::new();
    
    // 创建一个 Feign 客户端调用（带 base path）
    let consumer = MethodInfo {
        name: "getGoodsInfo".to_string(),
        full_qualified_name: "com.hualala.shop.domain.feign.BasicInfoFeign::getGoodsInfo".to_string(),
        file_path: PathBuf::from("BasicInfoFeign.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::POST,
            path: "hll-basic-info-api/hll-basic-info-api/feign/shop/copy/info".to_string(),
            path_params: vec![],
            is_feign_client: false,
            is_feign_client: true,  // Feign 调用
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    index.index_method(&consumer).unwrap();
    
    // 验证：Feign 调用应该被索引为消费者
    let endpoint = HttpEndpoint {
        method: HttpMethod::POST,
        path_pattern: "hll-basic-info-api/hll-basic-info-api/feign/shop/copy/info".to_string(),
    };
    
    let consumers = index.find_http_consumers(&endpoint);
    assert_eq!(consumers.len(), 1);
    assert!(consumers.contains(&"com.hualala.shop.domain.feign.BasicInfoFeign::getGoodsInfo"));
}

#[test]
fn test_http_provider_and_consumer_different_endpoints() {
    let mut index = CodeIndex::new();
    
    // 创建一个 HTTP 接口提供者
    let provider = MethodInfo {
        name: "createUser".to_string(),
        full_qualified_name: "com.example.UserController::createUser".to_string(),
        file_path: PathBuf::from("UserController.java"),
        line_range: (10, 15),
        calls: vec![],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::POST,
            path: "md-user-service/api/users".to_string(),
            path_params: vec![],
            is_feign_client: false,
            is_feign_client: false,  // 普通 HTTP 接口
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 创建一个 Feign 客户端调用
    let consumer = MethodInfo {
        name: "callOrderService".to_string(),
        full_qualified_name: "com.example.OrderClient::callOrderService".to_string(),
        file_path: PathBuf::from("OrderClient.java"),
        line_range: (20, 25),
        calls: vec![],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::GET,
            path: "order-service/api/orders".to_string(),
            path_params: vec![],
            is_feign_client: false,
            is_feign_client: true,  // Feign 调用
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    index.index_method(&provider).unwrap();
    index.index_method(&consumer).unwrap();
    
    // 验证提供者端点
    let provider_endpoint = HttpEndpoint {
        method: HttpMethod::POST,
        path_pattern: "md-user-service/api/users".to_string(),
    };
    
    let providers = index.find_http_providers(&provider_endpoint);
    assert_eq!(providers.len(), 1);
    assert!(providers.contains(&"com.example.UserController::createUser"));
    
    // 验证消费者端点
    let consumer_endpoint = HttpEndpoint {
        method: HttpMethod::GET,
        path_pattern: "order-service/api/orders".to_string(),
    };
    
    let consumers = index.find_http_consumers(&consumer_endpoint);
    assert_eq!(consumers.len(), 1);
    assert!(consumers.contains(&"com.example.OrderClient::callOrderService"));
}
