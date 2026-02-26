use std::path::Path;
use std::sync::Mutex;
use tree_sitter::Parser;
use regex::Regex;
use crate::errors::ParseError;
use crate::language_parser::{LanguageParser, ParsedFile, ClassInfo, MethodInfo, MethodCall};
use crate::types::*;

/// Java 语言解析器
/// 
/// 使用 tree-sitter-java 解析 Java 源代码
pub struct JavaParser {
    parser: Mutex<Parser>,
}

impl JavaParser {
    /// 创建新的 JavaParser 实例
    pub fn new() -> Result<Self, ParseError> {
        let mut parser = Parser::new();
        let language = tree_sitter_java::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| ParseError::InvalidFormat {
                message: format!("Failed to set Java language: {}", e),
            })?;
        
        Ok(JavaParser { 
            parser: Mutex::new(parser),
        })
    }
    
    /// 提取类信息
    fn extract_classes(&self, source: &str, tree: &tree_sitter::Tree) -> Vec<ClassInfo> {
        let mut classes = Vec::new();
        let root_node = tree.root_node();
        
        self.walk_node_for_classes(source, root_node, &mut classes);
        
        classes
    }
    
    /// 递归遍历节点查找类声明
    fn walk_node_for_classes(&self, source: &str, node: tree_sitter::Node, classes: &mut Vec<ClassInfo>) {
        if node.kind() == "class_declaration" {
            if let Some(class_info) = self.extract_class_info(source, node) {
                classes.push(class_info);
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_classes(source, child, classes);
        }
    }
    
    /// 从类节点提取类信息
    fn extract_class_info(&self, source: &str, class_node: tree_sitter::Node) -> Option<ClassInfo> {
        // 查找类名
        let mut cursor = class_node.walk();
        let mut class_name = None;
        
        for child in class_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    class_name = Some(text.to_string());
                    break;
                }
            }
        }
        
        let name = class_name?;
        let line_start = class_node.start_position().row + 1;
        let line_end = class_node.end_position().row + 1;
        
        // 提取类中的方法
        let methods = self.extract_methods_from_class(source, &class_node, &name);
        
        Some(ClassInfo {
            name,
            methods,
            line_range: (line_start, line_end),
        })
    }
    
    /// 从类节点中提取方法
    fn extract_methods_from_class(
        &self,
        source: &str,
        class_node: &tree_sitter::Node,
        class_name: &str,
    ) -> Vec<MethodInfo> {
        let mut methods = Vec::new();
        
        // 查找类体
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "class_body" {
                let mut body_cursor = child.walk();
                for body_child in child.children(&mut body_cursor) {
                    if body_child.kind() == "method_declaration" {
                        if let Some(method_info) = self.extract_method_info(source, body_child, class_name) {
                            methods.push(method_info);
                        }
                    }
                }
            }
        }
        
        methods
    }
    
    /// 从方法节点提取方法信息
    fn extract_method_info(
        &self,
        source: &str,
        method_node: tree_sitter::Node,
        class_name: &str,
    ) -> Option<MethodInfo> {
        // 查找方法名
        let mut cursor = method_node.walk();
        let mut method_name = None;
        
        for child in method_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    method_name = Some(text.to_string());
                    break;
                }
            }
        }
        
        let name = method_name?;
        let line_start = method_node.start_position().row + 1;
        let line_end = method_node.end_position().row + 1;
        let full_qualified_name = format!("{}::{}", class_name, name);
        
        // 提取方法调用
        let calls = self.extract_method_calls(source, &method_node);
        
        // 提取 HTTP 注解
        let http_annotations = self.extract_http_annotations(source, &method_node);
        
        // 提取 Kafka 操作
        let kafka_operations = self.extract_kafka_operations(source, &method_node);
        
        // 提取数据库操作
        let db_operations = self.extract_db_operations(source, &method_node);
        
        // 提取 Redis 操作
        let redis_operations = self.extract_redis_operations(source, &method_node);
        
        Some(MethodInfo {
            name,
            full_qualified_name,
            line_range: (line_start, line_end),
            calls,
            http_annotations,
            kafka_operations,
            db_operations,
            redis_operations,
        })
    }
    
    /// 提取方法调用
    fn extract_method_calls(&self, source: &str, method_node: &tree_sitter::Node) -> Vec<MethodCall> {
        let mut calls = Vec::new();
        self.walk_node_for_calls(source, *method_node, &mut calls);
        calls
    }
    
    /// 递归遍历节点查找方法调用
    fn walk_node_for_calls(&self, source: &str, node: tree_sitter::Node, calls: &mut Vec<MethodCall>) {
        if node.kind() == "method_invocation" {
            // 查找方法名
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Some(text) = source.get(child.byte_range()) {
                        let line = node.start_position().row + 1;
                        calls.push(MethodCall {
                            target: text.to_string(),
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
    
    /// 提取 HTTP 注解（Spring Framework）
    fn extract_http_annotations(&self, source: &str, method_node: &tree_sitter::Node) -> Option<HttpAnnotation> {
        // 查找方法节点的 modifiers 子节点
        let mut cursor = method_node.walk();
        for child in method_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                // 在 modifiers 中查找注解
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "marker_annotation" || mod_child.kind() == "annotation" {
                        if let Some(http_ann) = self.parse_http_annotation(source, mod_child) {
                            return Some(http_ann);
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// 解析 HTTP 注解
    fn parse_http_annotation(&self, source: &str, annotation_node: tree_sitter::Node) -> Option<HttpAnnotation> {
        // 获取注解名称
        let mut cursor = annotation_node.walk();
        let mut annotation_name = None;
        let mut annotation_args = None;
        
        for child in annotation_node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "scoped_identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    annotation_name = Some(text.to_string());
                }
            } else if child.kind() == "annotation_argument_list" {
                if let Some(text) = source.get(child.byte_range()) {
                    annotation_args = Some(text.to_string());
                }
            }
        }
        
        let name = annotation_name?;
        
        // 检查是否是 Spring HTTP 注解
        let (method, path) = if name.contains("GetMapping") {
            (HttpMethod::GET, self.extract_path_from_args(&annotation_args))
        } else if name.contains("PostMapping") {
            (HttpMethod::POST, self.extract_path_from_args(&annotation_args))
        } else if name.contains("PutMapping") {
            (HttpMethod::PUT, self.extract_path_from_args(&annotation_args))
        } else if name.contains("DeleteMapping") {
            (HttpMethod::DELETE, self.extract_path_from_args(&annotation_args))
        } else if name.contains("PatchMapping") {
            (HttpMethod::PATCH, self.extract_path_from_args(&annotation_args))
        } else if name.contains("RequestMapping") {
            let method = self.extract_request_method_from_args(&annotation_args).unwrap_or(HttpMethod::GET);
            let path = self.extract_path_from_args(&annotation_args);
            (method, path)
        } else {
            return None;
        };
        
        let path_str = path?;
        let path_params = self.extract_path_params(&path_str);
        
        Some(HttpAnnotation {
            method,
            path: path_str,
            path_params,
        })
    }
    
    /// 从注解参数中提取路径
    fn extract_path_from_args(&self, args: &Option<String>) -> Option<String> {
        if let Some(args_text) = args {
            // 查找字符串字面量
            let re = Regex::new(r#""([^"]+)""#).unwrap();
            if let Some(cap) = re.captures(args_text) {
                return cap.get(1).map(|m| m.as_str().to_string());
            }
        }
        None
    }
    
    /// 从 RequestMapping 参数中提取 HTTP 方法
    fn extract_request_method_from_args(&self, args: &Option<String>) -> Option<HttpMethod> {
        if let Some(args_text) = args {
            if args_text.contains("RequestMethod.GET") {
                return Some(HttpMethod::GET);
            } else if args_text.contains("RequestMethod.POST") {
                return Some(HttpMethod::POST);
            } else if args_text.contains("RequestMethod.PUT") {
                return Some(HttpMethod::PUT);
            } else if args_text.contains("RequestMethod.DELETE") {
                return Some(HttpMethod::DELETE);
            } else if args_text.contains("RequestMethod.PATCH") {
                return Some(HttpMethod::PATCH);
            }
        }
        None
    }
    
    /// 提取路径参数
    fn extract_path_params(&self, path: &str) -> Vec<String> {
        let re = Regex::new(r"\{([^}]+)\}").unwrap();
        re.captures_iter(path)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect()
    }
    
    /// 提取 Kafka 操作
    fn extract_kafka_operations(&self, source: &str, method_node: &tree_sitter::Node) -> Vec<KafkaOperation> {
        let mut operations = Vec::new();
        
        // 查找 @KafkaListener 注解 - 只在方法自己的 modifiers 中查找
        let mut cursor = method_node.walk();
        for child in method_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                if let Some(text) = source.get(child.byte_range()) {
                    if text.contains("@KafkaListener") {
                        let topic_pattern = Regex::new(r#"topics\s*=\s*"([^"]+)""#).unwrap();
                        if let Some(cap) = topic_pattern.captures(text) {
                            if let Some(topic) = cap.get(1) {
                                operations.push(KafkaOperation {
                                    operation_type: KafkaOpType::Consume,
                                    topic: topic.as_str().to_string(),
                                    line: method_node.start_position().row + 1,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        // 查找方法体中的 send 调用
        if let Some(text) = source.get(method_node.byte_range()) {
            let producer_pattern = Regex::new(r#"\.send\s*\(\s*"([^"]+)""#).unwrap();
            for cap in producer_pattern.captures_iter(text) {
                if let Some(topic) = cap.get(1) {
                    operations.push(KafkaOperation {
                        operation_type: KafkaOpType::Produce,
                        topic: topic.as_str().to_string(),
                        line: method_node.start_position().row + 1,
                    });
                }
            }
        }
        
        operations
    }
    
    /// 提取数据库操作
    fn extract_db_operations(&self, source: &str, method_node: &tree_sitter::Node) -> Vec<DbOperation> {
        let mut operations = Vec::new();
        
        if let Some(text) = source.get(method_node.byte_range()) {
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
                            line: method_node.start_position().row + 1,
                        });
                    }
                }
            }
        }
        
        operations
    }
    
    /// 提取 Redis 操作
    fn extract_redis_operations(&self, source: &str, method_node: &tree_sitter::Node) -> Vec<RedisOperation> {
        let mut operations = Vec::new();
        
        if let Some(text) = source.get(method_node.byte_range()) {
            // 查找 RedisTemplate 操作
            let get_pattern = Regex::new(r#"\.opsForValue\(\)\.get\s*\(\s*"([^"]+)""#).unwrap();
            let set_pattern = Regex::new(r#"\.opsForValue\(\)\.set\s*\(\s*"([^"]+)""#).unwrap();
            let delete_pattern = Regex::new(r#"\.delete\s*\(\s*"([^"]+)""#).unwrap();
            
            for cap in get_pattern.captures_iter(text) {
                if let Some(key) = cap.get(1) {
                    operations.push(RedisOperation {
                        operation_type: RedisOpType::Get,
                        key_pattern: key.as_str().to_string(),
                        line: method_node.start_position().row + 1,
                    });
                }
            }
            
            for cap in set_pattern.captures_iter(text) {
                if let Some(key) = cap.get(1) {
                    operations.push(RedisOperation {
                        operation_type: RedisOpType::Set,
                        key_pattern: key.as_str().to_string(),
                        line: method_node.start_position().row + 1,
                    });
                }
            }
            
            for cap in delete_pattern.captures_iter(text) {
                if let Some(key) = cap.get(1) {
                    operations.push(RedisOperation {
                        operation_type: RedisOpType::Delete,
                        key_pattern: key.as_str().to_string(),
                        line: method_node.start_position().row + 1,
                    });
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
        if node.kind() == "import_declaration" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "scoped_identifier" {
                    if let Some(text) = source.get(child.byte_range()) {
                        imports.push(Import {
                            module: text.to_string(),
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

impl LanguageParser for JavaParser {
    fn language_name(&self) -> &str {
        "java"
    }
    
    fn file_extensions(&self) -> &[&str] {
        &["java"]
    }
    
    fn parse_file(&self, content: &str, file_path: &Path) -> Result<ParsedFile, ParseError> {
        let tree = self.parser.lock().unwrap().parse(content, None)
            .ok_or_else(|| ParseError::InvalidFormat {
                message: "Failed to parse Java file".to_string(),
            })?;
        
        let classes = self.extract_classes(content, &tree);
        let imports = self.extract_imports(content, &tree);
        
        Ok(ParsedFile {
            file_path: file_path.to_path_buf(),
            language: "java".to_string(),
            classes,
            functions: vec![], // Java 使用类和方法，不使用顶层函数
            imports,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_class() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            public class Example {
                public void hello() {
                    System.out.println("Hello");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("Example.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].name, "Example");
        assert_eq!(result.classes[0].methods.len(), 1);
        assert_eq!(result.classes[0].methods[0].name, "hello");
    }
    
    #[test]
    fn test_debug_tree_structure() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            @RestController
            public class UserController {
                @GetMapping("/users/{id}")
                public User getUser() {
                    return null;
                }
            }
        "#;
        
        let tree = parser.parser.lock().unwrap().parse(source, None).unwrap();
        let root = tree.root_node();
        
        fn print_tree(node: tree_sitter::Node, source: &str, indent: usize) {
            let indent_str = "  ".repeat(indent);
            let text = source.get(node.byte_range()).unwrap_or("");
            let preview = if text.len() > 50 {
                format!("{}...", &text[..50])
            } else {
                text.to_string()
            };
            eprintln!("{}{} [{}]", indent_str, node.kind(), preview.replace('\n', "\\n"));
            
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                print_tree(child, source, indent + 1);
            }
        }
        
        print_tree(root, source, 0);
    }
    
    #[test]
    fn test_extract_http_annotation() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            @RestController
            public class UserController {
                @GetMapping("/users/{id}")
                public User getUser() {
                    return null;
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("UserController.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        
        // Debug: print method info
        if !result.classes[0].methods.is_empty() {
            let method = &result.classes[0].methods[0];
            eprintln!("Method name: {}", method.name);
            eprintln!("HTTP annotation: {:?}", method.http_annotations);
        }
        
        let method = &result.classes[0].methods[0];
        assert!(method.http_annotations.is_some(), "HTTP annotation should be present");
        
        let http = method.http_annotations.as_ref().unwrap();
        assert_eq!(http.method, HttpMethod::GET);
        assert_eq!(http.path, "/users/{id}");
        assert_eq!(http.path_params, vec!["id"]);
    }
    
    #[test]
    fn test_extract_kafka_operations() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            public class MessageService {
                @KafkaListener(topics = "user-events")
                public void handleMessage(String message) {
                    // process message
                }
                
                public void sendMessage() {
                    kafkaTemplate.send("order-events", "data");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("MessageService.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 2);
        
        // Check consumer
        let consumer_method = &result.classes[0].methods[0];
        eprintln!("Consumer method kafka operations: {:?}", consumer_method.kafka_operations);
        assert_eq!(consumer_method.kafka_operations.len(), 1);
        assert_eq!(consumer_method.kafka_operations[0].operation_type, KafkaOpType::Consume);
        assert_eq!(consumer_method.kafka_operations[0].topic, "user-events");
        
        // Check producer
        let producer_method = &result.classes[0].methods[1];
        eprintln!("Producer method kafka operations: {:?}", producer_method.kafka_operations);
        assert_eq!(producer_method.kafka_operations.len(), 1);
        assert_eq!(producer_method.kafka_operations[0].operation_type, KafkaOpType::Produce);
        assert_eq!(producer_method.kafka_operations[0].topic, "order-events");
    }
    
    #[test]
    fn test_extract_db_operations() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            public class UserRepository {
                public void saveUser() {
                    String sql = "INSERT INTO users (name) VALUES ('John')";
                }
                
                public void findUser() {
                    String sql = "SELECT * FROM users WHERE id = 1";
                }
                
                public void updateUser() {
                    String sql = "UPDATE users SET name = 'Jane'";
                }
                
                public void deleteUser() {
                    String sql = "DELETE FROM users WHERE id = 1";
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("UserRepository.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 4);
        
        // Check INSERT
        let insert_method = &result.classes[0].methods[0];
        assert_eq!(insert_method.db_operations.len(), 1);
        assert_eq!(insert_method.db_operations[0].operation_type, DbOpType::Insert);
        assert_eq!(insert_method.db_operations[0].table, "users");
        
        // Check SELECT
        let select_method = &result.classes[0].methods[1];
        assert_eq!(select_method.db_operations.len(), 1);
        assert_eq!(select_method.db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(select_method.db_operations[0].table, "users");
        
        // Check UPDATE
        let update_method = &result.classes[0].methods[2];
        assert_eq!(update_method.db_operations.len(), 1);
        assert_eq!(update_method.db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(update_method.db_operations[0].table, "users");
        
        // Check DELETE
        let delete_method = &result.classes[0].methods[3];
        assert_eq!(delete_method.db_operations.len(), 1);
        assert_eq!(delete_method.db_operations[0].operation_type, DbOpType::Delete);
        assert_eq!(delete_method.db_operations[0].table, "users");
    }
    
    #[test]
    fn test_extract_redis_operations() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            public class CacheService {
                public void getFromCache() {
                    String value = redisTemplate.opsForValue().get("user:123");
                }
                
                public void setToCache() {
                    redisTemplate.opsForValue().set("user:456", "data");
                }
                
                public void deleteFromCache() {
                    redisTemplate.delete("user:789");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("CacheService.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 3);
        
        // Check GET
        let get_method = &result.classes[0].methods[0];
        assert_eq!(get_method.redis_operations.len(), 1);
        assert_eq!(get_method.redis_operations[0].operation_type, RedisOpType::Get);
        assert_eq!(get_method.redis_operations[0].key_pattern, "user:123");
        
        // Check SET
        let set_method = &result.classes[0].methods[1];
        assert_eq!(set_method.redis_operations.len(), 1);
        assert_eq!(set_method.redis_operations[0].operation_type, RedisOpType::Set);
        assert_eq!(set_method.redis_operations[0].key_pattern, "user:456");
        
        // Check DELETE
        let delete_method = &result.classes[0].methods[2];
        assert_eq!(delete_method.redis_operations.len(), 1);
        assert_eq!(delete_method.redis_operations[0].operation_type, RedisOpType::Delete);
        assert_eq!(delete_method.redis_operations[0].key_pattern, "user:789");
    }
    
    #[test]
    fn test_extract_method_calls() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            public class Calculator {
                public int add(int a, int b) {
                    return a + b;
                }
                
                public int calculate() {
                    int result = add(5, 3);
                    System.out.println(result);
                    return result;
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("Calculator.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 2);
        
        // Check method calls in calculate method
        let calculate_method = &result.classes[0].methods[1];
        assert!(calculate_method.calls.len() >= 2); // add and println
        
        let call_names: Vec<&str> = calculate_method.calls.iter()
            .map(|c| c.target.as_str())
            .collect();
        assert!(call_names.contains(&"add"));
        assert!(call_names.contains(&"println"));
    }
}
