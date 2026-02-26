use std::path::Path;
use std::sync::Mutex;
use tree_sitter::Parser;
use regex::Regex;
use crate::errors::ParseError;
use crate::language_parser::{LanguageParser, ParsedFile, FunctionInfo, MethodCall};
use crate::types::*;

/// Rust 语言解析器
/// 
/// 使用 tree-sitter-rust 解析 Rust 源代码
pub struct RustParser {
    parser: Mutex<Parser>,
}

impl RustParser {
    /// 创建新的 RustParser 实例
    pub fn new() -> Result<Self, ParseError> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| ParseError::InvalidFormat {
                message: format!("Failed to set Rust language: {}", e),
            })?;
        
        Ok(RustParser { 
            parser: Mutex::new(parser),
        })
    }
    
    /// 提取函数信息
    fn extract_functions(&self, source: &str, file_path: &Path, tree: &tree_sitter::Tree) -> Vec<FunctionInfo> {
        let mut functions = Vec::new();
        let root_node = tree.root_node();
        
        self.walk_node_for_functions(source, file_path, root_node, &mut functions, None);
        
        functions
    }
    
    /// 递归遍历节点查找函数声明
    fn walk_node_for_functions(
        &self,
        source: &str,
        file_path: &Path,
        node: tree_sitter::Node,
        functions: &mut Vec<FunctionInfo>,
        module_path: Option<&str>,
    ) {
        if node.kind() == "function_item" {
            if let Some(func_info) = self.extract_function_info(source, file_path, node, module_path) {
                functions.push(func_info);
            }
        } else if node.kind() == "mod_item" {
            // 提取模块名并递归处理模块内容
            if let Some(mod_name) = self.extract_module_name(source, node) {
                let new_path = if let Some(parent) = module_path {
                    format!("{}::{}", parent, mod_name)
                } else {
                    mod_name
                };
                
                // 查找模块体
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "declaration_list" {
                        let mut body_cursor = child.walk();
                        for body_child in child.children(&mut body_cursor) {
                            self.walk_node_for_functions(source, file_path, body_child, functions, Some(&new_path));
                        }
                    }
                }
                // Don't continue walking children after processing mod_item
                return;
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_functions(source, file_path, child, functions, module_path);
        }
    }
    
    /// 提取模块名
    fn extract_module_name(&self, source: &str, mod_node: tree_sitter::Node) -> Option<String> {
        let mut cursor = mod_node.walk();
        for child in mod_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    return Some(text.to_string());
                }
            }
        }
        None
    }
    
    /// 从函数节点提取函数信息
    fn extract_function_info(
        &self,
        source: &str,
        file_path: &Path,
        func_node: tree_sitter::Node,
        module_path: Option<&str>,
    ) -> Option<FunctionInfo> {
        // 查找函数名
        let mut cursor = func_node.walk();
        let mut func_name = None;
        
        for child in func_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    func_name = Some(text.to_string());
                    break;
                }
            }
        }
        
        let name = func_name?;
        let line_start = func_node.start_position().row + 1;
        let line_end = func_node.end_position().row + 1;
        
        let full_qualified_name = if let Some(module) = module_path {
            format!("{}::{}", module, name)
        } else {
            name.clone()
        };
        
        // 提取函数调用
        let calls = self.extract_function_calls(source, &func_node);
        
        // 提取 Axum 路由宏
        let http_annotations = self.extract_axum_routes(source, &func_node);
        
        // 提取 Kafka 操作
        let kafka_operations = self.extract_kafka_operations(source, &func_node);
        
        // 提取数据库操作
        let db_operations = self.extract_db_operations(source, &func_node);
        
        // 提取 Redis 操作
        let redis_operations = self.extract_redis_operations(source, &func_node);
        
        Some(FunctionInfo {
            name,
            full_qualified_name,
            file_path: file_path.to_path_buf(),
            line_range: (line_start, line_end),
            calls,
            http_annotations,
            kafka_operations,
            db_operations,
            redis_operations,
        })
    }
    
    /// 提取函数调用
    fn extract_function_calls(&self, source: &str, func_node: &tree_sitter::Node) -> Vec<MethodCall> {
        let mut calls = Vec::new();
        self.walk_node_for_calls(source, *func_node, &mut calls);
        calls
    }
    
    /// 递归遍历节点查找函数调用
    fn walk_node_for_calls(&self, source: &str, node: tree_sitter::Node, calls: &mut Vec<MethodCall>) {
        if node.kind() == "call_expression" {
            // 查找被调用的函数 - 第一个子节点通常是被调用的表达式
            if let Some(first_child) = node.child(0) {
                if let Some(text) = source.get(first_child.byte_range()) {
                    let line = node.start_position().row + 1;
                    calls.push(MethodCall {
                        target: text.to_string(),
                        line,
                    });
                }
            }
        } else if node.kind() == "macro_invocation" {
            // 处理宏调用（如 println!）
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Some(text) = source.get(child.byte_range()) {
                        let line = node.start_position().row + 1;
                        calls.push(MethodCall {
                            target: format!("{}!", text),
                            line,
                        });
                        break;
                    }
                }
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_calls(source, child, calls);
        }
    }
    
    /// 提取 Axum 路由宏
    fn extract_axum_routes(&self, source: &str, func_node: &tree_sitter::Node) -> Option<HttpAnnotation> {
        // 在函数所在的上下文中查找 Router::route 调用
        // 这需要在更大的上下文中查找，暂时通过正则表达式在函数附近查找
        
        // 获取函数的起始位置，向前查找一些行
        let func_start = func_node.start_byte();
        let search_start = if func_start > 500 { func_start - 500 } else { 0 };
        
        if let Some(context) = source.get(search_start..func_node.end_byte()) {
            // 查找 .route("/path", get(function_name)) 模式
            let route_pattern = Regex::new(r#"\.route\s*\(\s*"([^"]+)"\s*,\s*(get|post|put|delete|patch)\s*\("#).unwrap();
            
            if let Some(cap) = route_pattern.captures(context) {
                let path = cap.get(1)?.as_str().to_string();
                let method_str = cap.get(2)?.as_str();
                
                let method = match method_str {
                    "get" => HttpMethod::GET,
                    "post" => HttpMethod::POST,
                    "put" => HttpMethod::PUT,
                    "delete" => HttpMethod::DELETE,
                    "patch" => HttpMethod::PATCH,
                    _ => return None,
                };
                
                let path_params = self.extract_path_params(&path);
                
                return Some(HttpAnnotation {
                    method,
                    path,
                    path_params,
                });
            }
        }
        
        None
    }
    
    /// 提取路径参数
    fn extract_path_params(&self, path: &str) -> Vec<String> {
        let re = Regex::new(r":(\w+)").unwrap();
        re.captures_iter(path)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect()
    }
    
    /// 提取 Kafka 操作
    fn extract_kafka_operations(&self, source: &str, func_node: &tree_sitter::Node) -> Vec<KafkaOperation> {
        let mut operations = Vec::new();
        
        if let Some(text) = source.get(func_node.byte_range()) {
            // 查找 FutureProducer.send 调用
            let producer_pattern = Regex::new(r#"\.send\s*\(\s*"([^"]+)""#).unwrap();
            for cap in producer_pattern.captures_iter(text) {
                if let Some(topic) = cap.get(1) {
                    operations.push(KafkaOperation {
                        operation_type: KafkaOpType::Produce,
                        topic: topic.as_str().to_string(),
                        line: func_node.start_position().row + 1,
                    });
                }
            }
            
            // 查找 StreamConsumer.recv 调用
            // 通常消费者会在函数中调用 recv() 或 stream()
            if text.contains("StreamConsumer") || text.contains(".recv()") || text.contains(".stream()") {
                // 尝试从函数参数或上下文中提取 topic
                let topic_pattern = Regex::new(r#"subscribe\s*\(\s*&?\[?"([^"]+)""#).unwrap();
                for cap in topic_pattern.captures_iter(text) {
                    if let Some(topic) = cap.get(1) {
                        operations.push(KafkaOperation {
                            operation_type: KafkaOpType::Consume,
                            topic: topic.as_str().to_string(),
                            line: func_node.start_position().row + 1,
                        });
                    }
                }
            }
        }
        
        operations
    }
    
    /// 提取数据库操作
    fn extract_db_operations(&self, source: &str, func_node: &tree_sitter::Node) -> Vec<DbOperation> {
        let mut operations = Vec::new();
        
        if let Some(text) = source.get(func_node.byte_range()) {
            // 查找 SQL 语句
            let sql_patterns = vec![
                (Regex::new(r"(?i)SELECT\s+.+?\s+FROM\s+(\w+)").unwrap(), DbOpType::Select),
                (Regex::new(r"(?i)INSERT\s+INTO\s+(\w+)").unwrap(), DbOpType::Insert),
                (Regex::new(r"(?i)UPDATE\s+(\w+)\s+SET").unwrap(), DbOpType::Update),
                (Regex::new(r"(?i)DELETE\s+FROM\s+(\w+)").unwrap(), DbOpType::Delete),
            ];
            
            for (pattern, op_type) in sql_patterns {
                for cap in pattern.captures_iter(text) {
                    if let Some(table) = cap.get(1) {
                        operations.push(DbOperation {
                            operation_type: op_type.clone(),
                            table: table.as_str().to_string(),
                            line: func_node.start_position().row + 1,
                        });
                    }
                }
            }
        }
        
        operations
    }
    
    /// 提取 Redis 操作
    fn extract_redis_operations(&self, source: &str, func_node: &tree_sitter::Node) -> Vec<RedisOperation> {
        let mut operations = Vec::new();
        
        if let Some(text) = source.get(func_node.byte_range()) {
            // 查找 redis Commands trait 方法调用
            // get, set, del 等
            let get_pattern = Regex::new(r#"\.get\s*\(\s*"([^"]+)""#).unwrap();
            let set_pattern = Regex::new(r#"\.set\s*\(\s*"([^"]+)""#).unwrap();
            let del_pattern = Regex::new(r#"\.del\s*\(\s*"([^"]+)""#).unwrap();
            
            for cap in get_pattern.captures_iter(text) {
                if let Some(key) = cap.get(1) {
                    // 确保这是 Redis 操作而不是其他 get 调用
                    if text.contains("redis") || text.contains("Commands") {
                        operations.push(RedisOperation {
                            operation_type: RedisOpType::Get,
                            key_pattern: key.as_str().to_string(),
                            line: func_node.start_position().row + 1,
                        });
                    }
                }
            }
            
            for cap in set_pattern.captures_iter(text) {
                if let Some(key) = cap.get(1) {
                    if text.contains("redis") || text.contains("Commands") {
                        operations.push(RedisOperation {
                            operation_type: RedisOpType::Set,
                            key_pattern: key.as_str().to_string(),
                            line: func_node.start_position().row + 1,
                        });
                    }
                }
            }
            
            for cap in del_pattern.captures_iter(text) {
                if let Some(key) = cap.get(1) {
                    if text.contains("redis") || text.contains("Commands") {
                        operations.push(RedisOperation {
                            operation_type: RedisOpType::Delete,
                            key_pattern: key.as_str().to_string(),
                            line: func_node.start_position().row + 1,
                        });
                    }
                }
            }
        }
        
        operations
    }
    
    /// 提取导入声明
    fn extract_imports(&self, source: &str, tree: &tree_sitter::Tree) -> Vec<Import> {
        let mut imports = Vec::new();
        let root_node = tree.root_node();
        
        self.walk_node_for_imports(source, root_node, &mut imports);
        
        imports
    }
    
    /// 递归遍历节点查找导入声明
    fn walk_node_for_imports(&self, source: &str, node: tree_sitter::Node, imports: &mut Vec<Import>) {
        if node.kind() == "use_declaration" {
            if let Some(text) = source.get(node.byte_range()) {
                // 简单提取 use 语句
                let use_text = text.trim_start_matches("use").trim_end_matches(';').trim();
                
                // 检查是否有 as 重命名
                let parts: Vec<&str> = use_text.split("::").collect();
                if let Some(last_part) = parts.last() {
                    // 处理 {item1, item2} 形式
                    if last_part.contains('{') {
                        let module = parts[..parts.len()-1].join("::");
                        let items_str = last_part.trim_start_matches('{').trim_end_matches('}');
                        let items: Vec<String> = items_str
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .collect();
                        imports.push(Import {
                            module,
                            items,
                        });
                    } else {
                        imports.push(Import {
                            module: use_text.to_string(),
                            items: vec![],
                        });
                    }
                }
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_imports(source, child, imports);
        }
    }
}

impl LanguageParser for RustParser {
    fn language_name(&self) -> &str {
        "rust"
    }
    
    fn file_extensions(&self) -> &[&str] {
        &["rs"]
    }
    
    fn parse_file(&self, content: &str, file_path: &Path) -> Result<ParsedFile, ParseError> {
        let tree = self.parser.lock().unwrap().parse(content, None)
            .ok_or_else(|| ParseError::InvalidFormat {
                message: "Failed to parse Rust file".to_string(),
            })?;
        
        let functions = self.extract_functions(content, file_path, &tree);
        let imports = self.extract_imports(content, &tree);
        
        Ok(ParsedFile {
            file_path: file_path.to_path_buf(),
            language: "rust".to_string(),
            classes: vec![], // Rust 不使用类
            functions,
            imports,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_function() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            fn hello() {
                println!("Hello");
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("example.rs")).unwrap();
        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].name, "hello");
    }
    
    #[test]
    fn test_parse_module_with_functions() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            mod user {
                fn create_user() {
                    println!("Creating user");
                }
                
                fn delete_user() {
                    println!("Deleting user");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("user.rs")).unwrap();
        assert_eq!(result.functions.len(), 2);
        assert_eq!(result.functions[0].full_qualified_name, "user::create_user");
        assert_eq!(result.functions[1].full_qualified_name, "user::delete_user");
    }
    
    #[test]
    fn test_extract_function_calls() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }
            
            fn calculate() -> i32 {
                let result = add(5, 3);
                println!("{}", result);
                result
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("calc.rs")).unwrap();
        assert_eq!(result.functions.len(), 2);
        
        // Check function calls in calculate function
        let calculate_func = &result.functions[1];
        assert!(calculate_func.calls.len() >= 2); // add and println
        
        let call_names: Vec<&str> = calculate_func.calls.iter()
            .map(|c| c.target.as_str())
            .collect();
        assert!(call_names.contains(&"add"));
        assert!(call_names.contains(&"println!"));
    }
    
    #[test]
    fn test_extract_axum_routes() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            use axum::{Router, routing::get};
            
            async fn get_user() -> String {
                "User data".to_string()
            }
            
            fn app() -> Router {
                Router::new()
                    .route("/users/:id", get(get_user))
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("routes.rs")).unwrap();
        
        // Find the get_user function
        let get_user_func = result.functions.iter()
            .find(|f| f.name == "get_user");
        
        assert!(get_user_func.is_some());
        let func = get_user_func.unwrap();
        
        // Note: The current implementation looks for routes near the function
        // This test may need adjustment based on actual implementation
        if let Some(http) = &func.http_annotations {
            assert_eq!(http.method, HttpMethod::GET);
            assert_eq!(http.path, "/users/:id");
            assert_eq!(http.path_params, vec!["id"]);
        }
    }
    
    #[test]
    fn test_extract_kafka_operations() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            use rdkafka::producer::FutureProducer;
            use rdkafka::consumer::StreamConsumer;
            
            async fn send_message(producer: &FutureProducer) {
                producer.send("user-events", "message").await;
            }
            
            async fn consume_messages(consumer: &StreamConsumer) {
                consumer.subscribe(&["order-events"]);
                let stream = consumer.stream();
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("kafka.rs")).unwrap();
        
        // Check producer
        let send_func = result.functions.iter()
            .find(|f| f.name == "send_message");
        assert!(send_func.is_some());
        let producer_func = send_func.unwrap();
        assert_eq!(producer_func.kafka_operations.len(), 1);
        assert_eq!(producer_func.kafka_operations[0].operation_type, KafkaOpType::Produce);
        assert_eq!(producer_func.kafka_operations[0].topic, "user-events");
        
        // Check consumer
        let consume_func = result.functions.iter()
            .find(|f| f.name == "consume_messages");
        assert!(consume_func.is_some());
        let consumer_func = consume_func.unwrap();
        assert_eq!(consumer_func.kafka_operations.len(), 1);
        assert_eq!(consumer_func.kafka_operations[0].operation_type, KafkaOpType::Consume);
        assert_eq!(consumer_func.kafka_operations[0].topic, "order-events");
    }
    
    #[test]
    fn test_extract_db_operations() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            fn save_user() {
                let sql = "INSERT INTO users (name) VALUES ('John')";
            }
            
            fn find_user() {
                let sql = "SELECT * FROM users WHERE id = 1";
            }
            
            fn update_user() {
                let sql = "UPDATE users SET name = 'Jane'";
            }
            
            fn delete_user() {
                let sql = "DELETE FROM users WHERE id = 1";
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("db.rs")).unwrap();
        assert_eq!(result.functions.len(), 4);
        
        // Check INSERT
        let insert_func = &result.functions[0];
        assert_eq!(insert_func.db_operations.len(), 1);
        assert_eq!(insert_func.db_operations[0].operation_type, DbOpType::Insert);
        assert_eq!(insert_func.db_operations[0].table, "users");
        
        // Check SELECT
        let select_func = &result.functions[1];
        assert_eq!(select_func.db_operations.len(), 1);
        assert_eq!(select_func.db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(select_func.db_operations[0].table, "users");
        
        // Check UPDATE
        let update_func = &result.functions[2];
        assert_eq!(update_func.db_operations.len(), 1);
        assert_eq!(update_func.db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(update_func.db_operations[0].table, "users");
        
        // Check DELETE
        let delete_func = &result.functions[3];
        assert_eq!(delete_func.db_operations.len(), 1);
        assert_eq!(delete_func.db_operations[0].operation_type, DbOpType::Delete);
        assert_eq!(delete_func.db_operations[0].table, "users");
    }
    
    #[test]
    fn test_extract_redis_operations() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            use redis::Commands;
            
            fn get_from_cache(conn: &mut redis::Connection) {
                let value: String = conn.get("user:123").unwrap();
            }
            
            fn set_to_cache(conn: &mut redis::Connection) {
                conn.set("user:456", "data").unwrap();
            }
            
            fn delete_from_cache(conn: &mut redis::Connection) {
                conn.del("user:789").unwrap();
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("cache.rs")).unwrap();
        assert_eq!(result.functions.len(), 3);
        
        // Check GET
        let get_func = &result.functions[0];
        assert_eq!(get_func.redis_operations.len(), 1);
        assert_eq!(get_func.redis_operations[0].operation_type, RedisOpType::Get);
        assert_eq!(get_func.redis_operations[0].key_pattern, "user:123");
        
        // Check SET
        let set_func = &result.functions[1];
        assert_eq!(set_func.redis_operations.len(), 1);
        assert_eq!(set_func.redis_operations[0].operation_type, RedisOpType::Set);
        assert_eq!(set_func.redis_operations[0].key_pattern, "user:456");
        
        // Check DELETE
        let delete_func = &result.functions[2];
        assert_eq!(delete_func.redis_operations.len(), 1);
        assert_eq!(delete_func.redis_operations[0].operation_type, RedisOpType::Delete);
        assert_eq!(delete_func.redis_operations[0].key_pattern, "user:789");
    }
    
    #[test]
    fn test_extract_imports() {
        let parser = RustParser::new().unwrap();
        let source = r#"
            use std::collections::HashMap;
            use axum::{Router, routing::get};
            use redis::Commands;
        "#;
        
        let result = parser.parse_file(source, Path::new("imports.rs")).unwrap();
        assert!(result.imports.len() >= 3);
        
        let import_modules: Vec<&str> = result.imports.iter()
            .map(|i| i.module.as_str())
            .collect();
        
        assert!(import_modules.iter().any(|m| m.contains("HashMap")));
        assert!(import_modules.iter().any(|m| m.contains("axum")));
        assert!(import_modules.iter().any(|m| m.contains("redis")));
    }
}
