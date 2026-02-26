use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::config_parser::{ConfigData, XmlConfigParser, YamlConfigParser, ConfigParser};
use code_impact_analyzer::language_parser::MethodInfo;
use code_impact_analyzer::types::{
    HttpAnnotation, HttpEndpoint, HttpMethod, KafkaOperation, KafkaOpType,
    DbOperation, DbOpType, RedisOperation, RedisOpType,
};
use code_impact_analyzer::language_parser::MethodCall;

/// 测试 HTTP 端点配置关联
#[test]
fn test_http_endpoint_config_association() {
    let mut index = CodeIndex::new();
    
    // 添加 HTTP 提供者
    let provider = MethodInfo {
        name: "getUserById".to_string(),
        full_qualified_name: "com.example.api.UserController::getUserById".to_string(),
        line_range: (10, 25),
        calls: vec![],
        http_annotations: Some(HttpAnnotation {
            method: HttpMethod::GET,
            path: "/api/v1/users/{id}".to_string(),
            path_params: vec!["id".to_string()],
        }),
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 添加 HTTP 消费者（调用该接口的客户端代码）
    let consumer = MethodInfo {
        name: "fetchUserData".to_string(),
        full_qualified_name: "com.example.client.UserClient::fetchUserData".to_string(),
        line_range: (30, 45),
        calls: vec![
            MethodCall {
                target: "RestTemplate.getForObject(/api/v1/users)".to_string(),
                line: 35,
            },
        ],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    index.test_index_method(&provider).unwrap();
    index.test_index_method(&consumer).unwrap();
    
    // 从配置文件解析 HTTP 端点
    let xml_config = r#"
        <configuration>
            <services>
                <user-service>
                    <api-url>http://localhost:8080/api/v1/users/{id}</api-url>
                </user-service>
            </services>
        </configuration>
    "#;
    
    let parser = XmlConfigParser;
    let config_data = parser.parse(xml_config).unwrap();
    
    // 关联配置到代码
    index.associate_config_data(&config_data);
    
    // 验证 HTTP 消费者已被正确关联
    let endpoint = HttpEndpoint {
        method: HttpMethod::GET,
        path_pattern: "/api/v1/users/{id}".to_string(),
    };
    
    let consumers = index.find_http_consumers(&endpoint);
    assert_eq!(consumers.len(), 1);
    assert!(consumers.contains(&"com.example.client.UserClient::fetchUserData"));
    
    // 验证配置关联
    let associated = index.find_config_associations("http:GET:/api/v1/users/{id}");
    assert_eq!(associated.len(), 1);
    assert!(associated.contains(&"com.example.client.UserClient::fetchUserData"));
}

/// 测试 Kafka Topic 配置关联
#[test]
fn test_kafka_topic_config_association() {
    let mut index = CodeIndex::new();
    
    // 添加 Kafka 生产者
    let producer = MethodInfo {
        name: "publishUserEvent".to_string(),
        full_qualified_name: "com.example.events.UserEventPublisher::publishUserEvent".to_string(),
        line_range: (10, 25),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![
            KafkaOperation {
                operation_type: KafkaOpType::Produce,
                topic: "user-lifecycle-events".to_string(),
                line: 15,
            },
        ],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    // 添加 Kafka 消费者
    let consumer = MethodInfo {
        name: "handleUserEvent".to_string(),
        full_qualified_name: "com.example.handlers.UserEventHandler::handleUserEvent".to_string(),
        line_range: (30, 50),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![
            KafkaOperation {
                operation_type: KafkaOpType::Consume,
                topic: "user-lifecycle-events".to_string(),
                line: 35,
            },
        ],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    index.test_index_method(&producer).unwrap();
    index.test_index_method(&consumer).unwrap();
    
    // 从 YAML 配置文件解析 Kafka Topic
    let yaml_config = r#"
        kafka:
          topics:
            - user-lifecycle-events
            - order-events
          bootstrap-servers: localhost:9092
    "#;
    
    let parser = YamlConfigParser;
    let config_data = parser.parse(yaml_config).unwrap();
    
    // 关联配置到代码
    index.associate_config_data(&config_data);
    
    // 验证配置关联
    let associated = index.find_config_associations("kafka:topic:user-lifecycle-events");
    assert_eq!(associated.len(), 2);
    assert!(associated.contains(&"com.example.events.UserEventPublisher::publishUserEvent"));
    assert!(associated.contains(&"com.example.handlers.UserEventHandler::handleUserEvent"));
}

/// 测试数据库表配置关联
#[test]
fn test_database_table_config_association() {
    let mut index = CodeIndex::new();
    
    // 添加数据库读取者
    let reader = MethodInfo {
        name: "findUserById".to_string(),
        full_qualified_name: "com.example.repository.UserRepository::findUserById".to_string(),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![
            DbOperation {
                operation_type: DbOpType::Select,
                table: "users".to_string(),
                line: 15,
            },
        ],
        redis_operations: vec![],
    };
    
    // 添加数据库写入者
    let writer = MethodInfo {
        name: "saveUser".to_string(),
        full_qualified_name: "com.example.repository.UserRepository::saveUser".to_string(),
        line_range: (25, 35),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![
            DbOperation {
                operation_type: DbOpType::Insert,
                table: "users".to_string(),
                line: 30,
            },
        ],
        redis_operations: vec![],
    };
    
    let updater = MethodInfo {
        name: "updateUser".to_string(),
        full_qualified_name: "com.example.repository.UserRepository::updateUser".to_string(),
        line_range: (40, 50),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![
            DbOperation {
                operation_type: DbOpType::Update,
                table: "users".to_string(),
                line: 45,
            },
        ],
        redis_operations: vec![],
    };
    
    index.test_index_method(&reader).unwrap();
    index.test_index_method(&writer).unwrap();
    index.test_index_method(&updater).unwrap();
    
    // 从 XML 配置文件解析数据库表
    let xml_config = r#"
        <database>
            <entities>
                <table>users</table>
                <table>orders</table>
            </entities>
        </database>
    "#;
    
    let parser = XmlConfigParser;
    let config_data = parser.parse(xml_config).unwrap();
    
    // 关联配置到代码
    index.associate_config_data(&config_data);
    
    // 验证配置关联
    let associated = index.find_config_associations("db:table:users");
    assert_eq!(associated.len(), 3);
    assert!(associated.contains(&"com.example.repository.UserRepository::findUserById"));
    assert!(associated.contains(&"com.example.repository.UserRepository::saveUser"));
    assert!(associated.contains(&"com.example.repository.UserRepository::updateUser"));
}

/// 测试 Redis 键前缀配置关联
#[test]
fn test_redis_prefix_config_association() {
    let mut index = CodeIndex::new();
    
    // 添加 Redis 读取者
    let reader = MethodInfo {
        name: "getUserFromCache".to_string(),
        full_qualified_name: "com.example.cache.UserCache::getUserFromCache".to_string(),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![
            RedisOperation {
                operation_type: RedisOpType::Get,
                key_pattern: "user:profile:*".to_string(),
                line: 15,
            },
        ],
    };
    
    // 添加 Redis 写入者
    let writer = MethodInfo {
        name: "cacheUser".to_string(),
        full_qualified_name: "com.example.cache.UserCache::cacheUser".to_string(),
        line_range: (25, 35),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![],
        db_operations: vec![],
        redis_operations: vec![
            RedisOperation {
                operation_type: RedisOpType::Set,
                key_pattern: "user:profile:*".to_string(),
                line: 30,
            },
        ],
    };
    
    index.test_index_method(&reader).unwrap();
    index.test_index_method(&writer).unwrap();
    
    // 从 YAML 配置文件解析 Redis 键前缀
    let yaml_config = r#"
        redis:
          cache:
            user-profile-key: "user:profile:*"
            session-key: "session:*"
    "#;
    
    let parser = YamlConfigParser;
    let config_data = parser.parse(yaml_config).unwrap();
    
    // 关联配置到代码
    index.associate_config_data(&config_data);
    
    // 验证配置关联
    let associated = index.find_config_associations("redis:key:user:profile:*");
    assert_eq!(associated.len(), 2);
    assert!(associated.contains(&"com.example.cache.UserCache::getUserFromCache"));
    assert!(associated.contains(&"com.example.cache.UserCache::cacheUser"));
}

/// 测试混合配置关联（多种资源类型）
#[test]
fn test_mixed_config_association() {
    let mut index = CodeIndex::new();
    
    // 添加一个复杂的服务方法，使用多种资源
    let service_method = MethodInfo {
        name: "processUserRegistration".to_string(),
        full_qualified_name: "com.example.service.UserService::processUserRegistration".to_string(),
        line_range: (10, 50),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![
            KafkaOperation {
                operation_type: KafkaOpType::Produce,
                topic: "user-registered".to_string(),
                line: 20,
            },
        ],
        db_operations: vec![
            DbOperation {
                operation_type: DbOpType::Insert,
                table: "users".to_string(),
                line: 25,
            },
        ],
        redis_operations: vec![
            RedisOperation {
                operation_type: RedisOpType::Set,
                key_pattern: "user:session:*".to_string(),
                line: 30,
            },
        ],
    };
    
    index.test_index_method(&service_method).unwrap();
    
    // 从配置文件解析多种资源
    let yaml_config = r#"
        kafka:
          topics:
            - user-registered
            - user-updated
        database:
          tables:
            - users
            - orders
        redis:
          keys:
            - "user:session:*"
            - "user:profile:*"
    "#;
    
    let parser = YamlConfigParser;
    let config_data = parser.parse(yaml_config).unwrap();
    
    // 关联配置到代码
    index.associate_config_data(&config_data);
    
    // 验证 Kafka 关联
    let kafka_associated = index.find_config_associations("kafka:topic:user-registered");
    assert_eq!(kafka_associated.len(), 1);
    assert!(kafka_associated.contains(&"com.example.service.UserService::processUserRegistration"));
    
    // 验证数据库关联
    let db_associated = index.find_config_associations("db:table:users");
    assert_eq!(db_associated.len(), 1);
    assert!(db_associated.contains(&"com.example.service.UserService::processUserRegistration"));
    
    // 验证 Redis 关联
    let redis_associated = index.find_config_associations("redis:key:user:session:*");
    assert_eq!(redis_associated.len(), 1);
    assert!(redis_associated.contains(&"com.example.service.UserService::processUserRegistration"));
}

/// 测试配置关联的去重功能
#[test]
fn test_config_association_deduplication() {
    let mut index = CodeIndex::new();
    
    // 添加使用相同 topic 的多个方法
    let producer1 = MethodInfo {
        name: "sendEvent1".to_string(),
        full_qualified_name: "com.example.Producer1::sendEvent1".to_string(),
        line_range: (10, 20),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![
            KafkaOperation {
                operation_type: KafkaOpType::Produce,
                topic: "events".to_string(),
                line: 15,
            },
        ],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    let producer2 = MethodInfo {
        name: "sendEvent2".to_string(),
        full_qualified_name: "com.example.Producer2::sendEvent2".to_string(),
        line_range: (30, 40),
        calls: vec![],
        http_annotations: None,
        kafka_operations: vec![
            KafkaOperation {
                operation_type: KafkaOpType::Produce,
                topic: "events".to_string(),
                line: 35,
            },
        ],
        db_operations: vec![],
        redis_operations: vec![],
    };
    
    index.test_index_method(&producer1).unwrap();
    index.test_index_method(&producer2).unwrap();
    
    // 创建配置数据
    let mut config_data = ConfigData::default();
    config_data.kafka_topics.push("events".to_string());
    
    // 关联配置到代码
    index.associate_config_data(&config_data);
    
    // 验证关联（应该去重）
    let associated = index.find_config_associations("kafka:topic:events");
    assert_eq!(associated.len(), 2);
    assert!(associated.contains(&"com.example.Producer1::sendEvent1"));
    assert!(associated.contains(&"com.example.Producer2::sendEvent2"));
}
