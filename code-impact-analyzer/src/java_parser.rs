use std::path::Path;
use std::sync::Mutex;
use std::fs;
use tree_sitter::Parser;
use regex::Regex;
use serde_yaml::Value as YamlValue;
use crate::errors::ParseError;
use crate::language_parser::{LanguageParser, ParsedFile, ClassInfo, MethodInfo, MethodCall};
use crate::types::*;

/// FeignClient 注解信息
#[derive(Debug, Clone)]
struct FeignClientInfo {
    /// 服务名称（value 或 name 属性）
    service_name: String,
    /// 基础路径（path 属性）
    base_path: Option<String>,
}

/// 应用配置信息
#[derive(Debug, Clone, Default)]
struct ApplicationConfig {
    /// 应用名称（从 spring.application.name 读取）
    application_name: Option<String>,
    /// 上下文路径（从 server.servlet.context-path 读取）
    context_path: Option<String>,
}

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
    
    /// 从项目根目录查找并解析 application.yml 配置文件
    /// 
    /// 查找路径：start/src/main/resources/application.yml
    fn load_application_config(&self, file_path: &Path) -> ApplicationConfig {
        // 尝试找到项目根目录
        let mut current = file_path;
        let mut project_root = None;
        
        // 向上查找，直到找到包含 start 目录的项目根目录
        while let Some(parent) = current.parent() {
            let start_dir = parent.join("start");
            if start_dir.exists() && start_dir.is_dir() {
                project_root = Some(parent);
                break;
            }
            current = parent;
        }
        
        // 如果没找到 start 目录，尝试从当前文件路径推断
        if project_root.is_none() {
            current = file_path;
            while let Some(parent) = current.parent() {
                // 检查是否在某个模块目录下（如 hll-shop-manager-adapter）
                if let Some(name) = parent.file_name() {
                    let name_str = name.to_string_lossy();
                    // 如果是模块目录，其父目录可能是项目根目录
                    if name_str.contains("-adapter") || name_str.contains("-app") 
                        || name_str.contains("-client") || name_str.contains("-domain")
                        || name_str.contains("-infrastructure") {
                        if let Some(potential_root) = parent.parent() {
                            let start_dir = potential_root.join("start");
                            if start_dir.exists() && start_dir.is_dir() {
                                project_root = Some(potential_root);
                                break;
                            }
                        }
                    }
                }
                current = parent;
            }
        }
        
        let project_root = match project_root {
            Some(root) => root,
            None => return ApplicationConfig::default(),
        };
        
        // 构建 application.yml 的路径
        let config_path = project_root
            .join("start")
            .join("src")
            .join("main")
            .join("resources")
            .join("application.yml");
        
        // 读取并解析配置文件
        if let Ok(content) = fs::read_to_string(&config_path) {
            self.parse_application_yml(&content, project_root)
        } else {
            ApplicationConfig::default()
        }
    }
    
    /// 解析 application.yml 文件内容
    fn parse_application_yml(&self, content: &str, project_root: &Path) -> ApplicationConfig {
        let mut config = ApplicationConfig::default();
        
        // 解析 YAML
        if let Ok(yaml) = serde_yaml::from_str::<YamlValue>(content) {
            // 提取 spring.application.name
            if let Some(spring) = yaml.get("spring") {
                if let Some(application) = spring.get("application") {
                    if let Some(name) = application.get("name") {
                        if let Some(name_str) = name.as_str() {
                            config.application_name = Some(name_str.to_string());
                        }
                    }
                }
            }
            
            // 提取 server.servlet.context-path
            if let Some(server) = yaml.get("server") {
                if let Some(servlet) = server.get("servlet") {
                    if let Some(context_path) = servlet.get("context-path") {
                        if let Some(path_str) = context_path.as_str() {
                            config.context_path = Some(path_str.to_string());
                        }
                    }
                }
            }
        }
        
        // 如果没有找到 application.name，使用项目目录名
        if config.application_name.is_none() {
            if let Some(dir_name) = project_root.file_name() {
                config.application_name = Some(dir_name.to_string_lossy().to_string());
            }
        }
        
        // 如果没有找到 context-path，使用空字符串
        if config.context_path.is_none() {
            config.context_path = Some(String::new());
        }
        
        config
    }
    
    /// 提取类信息
    fn extract_classes(&self, source: &str, file_path: &Path, tree: &tree_sitter::Tree) -> Vec<ClassInfo> {
        let mut classes = Vec::new();
        let root_node = tree.root_node();
        
        // 加载应用配置
        let app_config = self.load_application_config(file_path);
        
        self.walk_node_for_classes(source, file_path, root_node, &mut classes, tree, &app_config);
        
        classes
    }
    
    /// 递归遍历节点查找类声明和接口声明
    fn walk_node_for_classes(&self, source: &str, file_path: &Path, node: tree_sitter::Node, classes: &mut Vec<ClassInfo>, tree: &tree_sitter::Tree, app_config: &ApplicationConfig) {
        // 处理类声明和接口声明
        if node.kind() == "class_declaration" || node.kind() == "interface_declaration" {
            if let Some(class_info) = self.extract_class_info(source, file_path, node, tree, app_config) {
                classes.push(class_info);
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_classes(source, file_path, child, classes, tree, app_config);
        }
    }
    
    /// 从类节点提取类信息
    fn extract_class_info(&self, source: &str, file_path: &Path, class_node: tree_sitter::Node, tree: &tree_sitter::Tree, app_config: &ApplicationConfig) -> Option<ClassInfo> {
        // 判断是否是接口
        let is_interface = class_node.kind() == "interface_declaration";
        
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
        
        let simple_name = class_name?;
        
        // 提取包名，构建完整的类名
        let package_name = self.extract_package_name(source, tree);
        let full_class_name = if let Some(pkg) = package_name {
            format!("{}.{}", pkg, simple_name)
        } else {
            simple_name.clone()
        };
        
        let line_start = class_node.start_position().row + 1;
        let line_end = class_node.end_position().row + 1;
        
        // 提取实现的接口列表
        let implements = self.extract_implements_interfaces(source, &class_node, tree);
        
        // 提取类级别的 FeignClient 注解
        let feign_client_info = self.extract_feign_client_annotation(source, &class_node);
        
        // 提取类级别的 RequestMapping 注解
        let class_request_mapping = self.extract_class_level_request_mapping(source, &class_node);
        
        // 提取类中的方法
        let methods = self.extract_methods_from_class(source, file_path, &class_node, &full_class_name, tree, &feign_client_info, &class_request_mapping, app_config);
        
        Some(ClassInfo {
            name: full_class_name,
            methods,
            line_range: (line_start, line_end),
            is_interface,
            implements,
        })
    }
    
    /// 提取类实现的接口列表
    fn extract_implements_interfaces(&self, source: &str, class_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Vec<String> {
        let mut interfaces = Vec::new();
        
        // 构建导入映射，用于将简单类名转换为完整类名
        let import_map = self.build_import_map(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        // 查找 super_interfaces 节点（包含 implements 子句）
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "super_interfaces" {
                // 在 super_interfaces 中查找 type_list（implements 后的接口列表）
                let mut super_cursor = child.walk();
                for super_child in child.children(&mut super_cursor) {
                    if super_child.kind() == "type_list" {
                        // 提取接口名称
                        let mut type_cursor = super_child.walk();
                        for type_child in super_child.children(&mut type_cursor) {
                            if type_child.kind() == "type_identifier" {
                                if let Some(interface_name) = source.get(type_child.byte_range()) {
                                    // 尝试将简单类名转换为完整类名
                                    let full_interface_name = self.resolve_full_class_name(
                                        interface_name,
                                        &import_map,
                                        &package_name,
                                    );
                                    interfaces.push(full_interface_name);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        interfaces
    }
    
    /// 将简单类名解析为完整类名
    fn resolve_full_class_name(
        &self,
        simple_name: &str,
        import_map: &std::collections::HashMap<String, String>,
        package_name: &Option<String>,
    ) -> String {
        // 首先尝试从导入映射中查找
        if let Some(full_name) = import_map.get(simple_name) {
            return full_name.clone();
        }
        
        // 如果没有找到，假设在同一个包中
        if let Some(pkg) = package_name {
            return format!("{}.{}", pkg, simple_name);
        }
        
        // 否则返回简单类名
        simple_name.to_string()
    }
    
    /// 提取包名
    fn extract_package_name(&self, source: &str, tree: &tree_sitter::Tree) -> Option<String> {
        let root_node = tree.root_node();
        let mut cursor = root_node.walk();
        
        for child in root_node.children(&mut cursor) {
            if child.kind() == "package_declaration" {
                // 在 package_declaration 中查找 scoped_identifier
                let mut pkg_cursor = child.walk();
                for pkg_child in child.children(&mut pkg_cursor) {
                    if pkg_child.kind() == "scoped_identifier" {
                        if let Some(text) = source.get(pkg_child.byte_range()) {
                            return Some(text.to_string());
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// 提取类级别的 FeignClient 注解
    fn extract_feign_client_annotation(&self, source: &str, class_node: &tree_sitter::Node) -> Option<FeignClientInfo> {
        // 查找类节点的 modifiers 子节点
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                // 在 modifiers 中查找注解
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "marker_annotation" || mod_child.kind() == "annotation" {
                        if let Some(feign_info) = self.parse_feign_client_annotation(source, mod_child) {
                            return Some(feign_info);
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// 提取类级别的 RequestMapping 注解
    fn extract_class_level_request_mapping(&self, source: &str, class_node: &tree_sitter::Node) -> Option<String> {
        // 查找类节点的 modifiers 子节点
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                // 在 modifiers 中查找注解
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "marker_annotation" || mod_child.kind() == "annotation" {
                        if let Some(path) = self.parse_request_mapping_annotation(source, mod_child) {
                            return Some(path);
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// 解析 RequestMapping 注解获取路径
    fn parse_request_mapping_annotation(&self, source: &str, annotation_node: tree_sitter::Node) -> Option<String> {
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
        
        // 检查是否是 RequestMapping 注解
        if !name.contains("RequestMapping") {
            return None;
        }
        
        // 提取路径
        self.extract_path_from_args(&annotation_args)
    }
    
    /// 解析 FeignClient 注解
    fn parse_feign_client_annotation(&self, source: &str, annotation_node: tree_sitter::Node) -> Option<FeignClientInfo> {
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
        
        // 检查是否是 FeignClient 注解
        if !name.contains("FeignClient") {
            return None;
        }
        
        let args_text = annotation_args?;
        
        // 提取 value 或 name 属性（服务名称）
        let service_name = self.extract_feign_attribute(&args_text, "value")
            .or_else(|| self.extract_feign_attribute(&args_text, "name"))?;
        
        // 提取 path 属性（基础路径）
        let base_path = self.extract_feign_attribute(&args_text, "path");
        
        Some(FeignClientInfo {
            service_name,
            base_path,
        })
    }
    
    /// 从 FeignClient 注解参数中提取指定属性的值
    fn extract_feign_attribute(&self, args: &str, attr_name: &str) -> Option<String> {
        // 匹配 attr_name = "value" 格式
        let pattern = format!(r#"{}\s*=\s*"([^"]+)""#, attr_name);
        let re = Regex::new(&pattern).ok()?;
        
        if let Some(cap) = re.captures(args) {
            return cap.get(1).map(|m| m.as_str().to_string());
        }
        
        None
    }
    
    /// 提取 Feign 方法的 HTTP 注解（组合类级别和方法级别的路径）
    fn extract_feign_http_annotation(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        feign_info: &FeignClientInfo,
    ) -> Option<HttpAnnotation> {
        // 提取方法级别的 HTTP 注解（不使用应用配置，因为这是调用其他服务）
        let method_http = self.extract_http_annotations_raw(source, method_node)?;
        
        // 组合路径：service_name/base_path/method_path
        let mut full_path = feign_info.service_name.clone();
        
        if let Some(base_path) = &feign_info.base_path {
            // 确保路径正确拼接
            if !full_path.ends_with('/') {
                full_path.push('/');
            }
            full_path.push_str(base_path.trim_start_matches('/'));
        }
        
        // 添加方法路径
        let method_path = method_http.path.trim_start_matches('/');
        if !full_path.ends_with('/') && !method_path.is_empty() {
            full_path.push('/');
        }
        full_path.push_str(method_path);
        
        Some(HttpAnnotation {
            method: method_http.method,
            path: full_path,
            path_params: method_http.path_params,
            is_feign_client: true,  // Feign 调用
        })
    }
    
    /// 从类节点中提取方法（包括接口中的抽象方法）
    fn extract_methods_from_class(
        &self,
        source: &str,
        file_path: &Path,
        class_node: &tree_sitter::Node,
        class_name: &str,
        tree: &tree_sitter::Tree,
        feign_client_info: &Option<FeignClientInfo>,
        class_request_mapping: &Option<String>,
        app_config: &ApplicationConfig,
    ) -> Vec<MethodInfo> {
        let mut methods = Vec::new();
        
        // 查找类体或接口体
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "class_body" || child.kind() == "interface_body" {
                let mut body_cursor = child.walk();
                for body_child in child.children(&mut body_cursor) {
                    // 处理普通方法声明和接口方法声明
                    if body_child.kind() == "method_declaration" {
                        if let Some(method_info) = self.extract_method_info(source, file_path, body_child, class_name, tree, feign_client_info, class_request_mapping, app_config) {
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
        file_path: &Path,
        method_node: tree_sitter::Node,
        class_name: &str,
        tree: &tree_sitter::Tree,
        feign_client_info: &Option<FeignClientInfo>,
        class_request_mapping: &Option<String>,
        app_config: &ApplicationConfig,
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
        let calls = self.extract_method_calls(source, &method_node, tree);
        
        // 提取 HTTP 注解（如果是 FeignClient，需要组合类级别和方法级别的注解）
        let http_annotations = if let Some(feign_info) = feign_client_info {
            self.extract_feign_http_annotation(source, &method_node, feign_info)
        } else {
            self.extract_http_annotations(source, &method_node, class_request_mapping, app_config)
        };
        
        // 提取 Kafka 操作
        let kafka_operations = self.extract_kafka_operations(source, &method_node);
        
        // 提取数据库操作
        let db_operations = self.extract_db_operations(source, &method_node);
        
        // 提取 Redis 操作
        let redis_operations = self.extract_redis_operations(source, &method_node);
        
        Some(MethodInfo {
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
    
    /// 提取方法调用
    fn extract_method_calls(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Vec<MethodCall> {
        let mut calls = Vec::new();
        
        // 提取导入语句，建立简单类名到完整类名的映射
        let import_map = self.build_import_map(source, tree);
        
        // 提取类中的字段声明和方法内的本地变量，建立变量名到类型的映射
        let field_types = self.extract_field_types(source, method_node, tree);
        
        self.walk_node_for_calls(source, *method_node, &mut calls, &field_types, &import_map);
        calls
    }
    
    /// 构建导入映射：简单类名 -> 完整类名
    fn build_import_map(&self, source: &str, tree: &tree_sitter::Tree) -> std::collections::HashMap<String, String> {
        let mut import_map = std::collections::HashMap::new();
        let root_node = tree.root_node();
        
        self.walk_node_for_import_map(source, root_node, &mut import_map);
        
        import_map
    }
    
    /// 递归遍历节点构建导入映射
    fn walk_node_for_import_map(
        &self,
        source: &str,
        node: tree_sitter::Node,
        import_map: &mut std::collections::HashMap<String, String>,
    ) {
        if node.kind() == "import_declaration" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "scoped_identifier" {
                    if let Some(full_name) = source.get(child.byte_range()) {
                        // 从完整类名中提取简单类名
                        if let Some(simple_name) = full_name.split('.').last() {
                            import_map.insert(simple_name.to_string(), full_name.to_string());
                        }
                    }
                }
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_import_map(source, child, import_map);
        }
    }
    
    /// 提取类中的字段类型映射（包括类字段和方法内的本地变量）
    fn extract_field_types(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> std::collections::HashMap<String, String> {
        let mut field_types = std::collections::HashMap::new();
        
        // 1. 向上查找到类节点，提取类字段
        let mut current = method_node.parent();
        while let Some(node) = current {
            if node.kind() == "class_declaration" {
                // 在类体中查找字段声明
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "class_body" {
                        let mut body_cursor = child.walk();
                        for body_child in child.children(&mut body_cursor) {
                            if body_child.kind() == "field_declaration" {
                                self.extract_field_type_from_declaration(source, body_child, &mut field_types);
                            }
                        }
                    }
                }
                break;
            }
            current = node.parent();
        }
        
        // 2. 提取方法内的本地变量
        self.extract_local_variable_types(source, method_node, tree, &mut field_types);
        
        field_types
    }
    
    /// 提取方法内的本地变量类型，并解析为完整类名
    fn extract_local_variable_types(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        tree: &tree_sitter::Tree,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        // 先提取本地变量的简单类型
        self.walk_node_for_local_vars(source, *method_node, field_types);
        
        // 获取导入映射和包名，用于解析完整类名
        let import_map = self.build_import_map(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        // 将简单类名解析为完整类名
        let mut resolved_types = std::collections::HashMap::new();
        for (var_name, simple_type) in field_types.iter() {
            let full_type = self.resolve_full_class_name(simple_type, &import_map, &package_name);
            resolved_types.insert(var_name.clone(), full_type);
        }
        
        // 更新 field_types
        *field_types = resolved_types;
    }
    
    /// 递归遍历节点查找本地变量声明
    fn walk_node_for_local_vars(
        &self,
        source: &str,
        node: tree_sitter::Node,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        if node.kind() == "local_variable_declaration" {
            // 提取本地变量的类型和名称
            self.extract_field_type_from_declaration(source, node, field_types);
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_local_vars(source, child, field_types);
        }
    }
    
    /// 从字段声明中提取字段名和类型
    fn extract_field_type_from_declaration(
        &self,
        source: &str,
        field_node: tree_sitter::Node,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        let mut field_type = None;
        let mut field_name = None;
        
        let mut cursor = field_node.walk();
        for child in field_node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" | "generic_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        field_type = Some(text.to_string());
                    }
                }
                "variable_declarator" => {
                    // 在 variable_declarator 中查找 identifier
                    let mut var_cursor = child.walk();
                    for var_child in child.children(&mut var_cursor) {
                        if var_child.kind() == "identifier" {
                            if let Some(text) = source.get(var_child.byte_range()) {
                                field_name = Some(text.to_string());
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        if let (Some(name), Some(type_name)) = (field_name, field_type) {
            field_types.insert(name, type_name);
        }
    }
    
    /// 递归遍历节点查找方法调用
    fn walk_node_for_calls(
        &self,
        source: &str,
        node: tree_sitter::Node,
        calls: &mut Vec<MethodCall>,
        field_types: &std::collections::HashMap<String, String>,
        import_map: &std::collections::HashMap<String, String>,
    ) {
        if node.kind() == "method_invocation" {
            // 查找方法调用的对象和方法名
            let mut cursor = node.walk();
            let mut identifiers = Vec::new();
            let mut scoped_identifiers = Vec::new();
            
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Some(text) = source.get(child.byte_range()) {
                        identifiers.push(text.to_string());
                    }
                } else if child.kind() == "scoped_identifier" {
                    // 处理静态方法调用，如 ClassName.staticMethod()
                    if let Some(text) = source.get(child.byte_range()) {
                        scoped_identifiers.push(text.to_string());
                    }
                }
            }
            
            let line = node.start_position().row + 1;
            
            // 处理静态方法调用：ClassName.staticMethod() 或 package.ClassName.staticMethod()
            if !scoped_identifiers.is_empty() && !identifiers.is_empty() {
                // scoped_identifier 包含类名（可能带包名），identifier 是方法名
                let class_name = &scoped_identifiers[0];
                let method_name = &identifiers[identifiers.len() - 1];
                
                // 尝试将简单类名转换为完整类名
                let full_class_name = import_map.get(class_name)
                    .unwrap_or(class_name);
                
                let target = format!("{}::{}", full_class_name, method_name);
                calls.push(MethodCall {
                    target,
                    line,
                });
                return;
            }
            
            // 对于 obj.method() 形式，有两个 identifier：对象名和方法名
            // 对于 method() 形式，只有一个 identifier：方法名
            let (object_name, method_name) = if identifiers.len() >= 2 {
                (Some(identifiers[0].clone()), identifiers[identifiers.len() - 1].clone())
            } else if identifiers.len() == 1 {
                (None, identifiers[0].clone())
            } else {
                return;
            };
            
            // 如果有对象名，尝试解析为完整的类名::方法名
            let target = if let Some(obj) = object_name {
                if let Some(class_type) = field_types.get(&obj) {
                    // 尝试将简单类名转换为完整类名
                    let full_class_name = import_map.get(class_type)
                        .unwrap_or(class_type);
                    format!("{}::{}", full_class_name, method_name)
                } else {
                    // 可能是静态方法调用，尝试从 import_map 中查找
                    if let Some(full_class_name) = import_map.get(&obj) {
                        format!("{}::{}", full_class_name, method_name)
                    } else {
                        method_name.clone()
                    }
                }
            } else {
                method_name.clone()
            };
            
            calls.push(MethodCall {
                target,
                line,
            });
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_calls(source, child, calls, field_types, import_map);
        }
    }
    
    /// 提取 HTTP 注解（Spring Framework）
    fn extract_http_annotations(&self, source: &str, method_node: &tree_sitter::Node, class_request_mapping: &Option<String>, app_config: &ApplicationConfig) -> Option<HttpAnnotation> {
        // 查找方法节点的 modifiers 子节点
        let mut cursor = method_node.walk();
        for child in method_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                // 在 modifiers 中查找注解
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "marker_annotation" || mod_child.kind() == "annotation" {
                        if let Some(mut http_ann) = self.parse_http_annotation(source, mod_child) {
                            // 组合完整路径：application.name/context-path/class-path/method-path
                            let mut full_path = String::new();
                            
                            // 添加 application.name
                            if let Some(app_name) = &app_config.application_name {
                                full_path.push_str(app_name);
                            }
                            
                            // 添加 context-path
                            if let Some(context_path) = &app_config.context_path {
                                if !context_path.is_empty() {
                                    if !full_path.is_empty() && !full_path.ends_with('/') {
                                        full_path.push('/');
                                    }
                                    full_path.push_str(context_path.trim_start_matches('/'));
                                }
                            }
                            
                            // 添加类级别的 RequestMapping 路径
                            if let Some(class_path) = class_request_mapping {
                                if !full_path.is_empty() && !full_path.ends_with('/') {
                                    full_path.push('/');
                                }
                                full_path.push_str(class_path.trim_start_matches('/'));
                            }
                            
                            // 添加方法级别的路径
                            let method_path = http_ann.path.trim_start_matches('/');
                            if !full_path.is_empty() && !full_path.ends_with('/') && !method_path.is_empty() {
                                full_path.push('/');
                            }
                            full_path.push_str(method_path);
                            
                            http_ann.path = full_path;
                            return Some(http_ann);
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// 提取 HTTP 注解（原始版本，不包含应用配置）
    /// 用于 FeignClient 等场景
    fn extract_http_annotations_raw(&self, source: &str, method_node: &tree_sitter::Node) -> Option<HttpAnnotation> {
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
            is_feign_client: false,  // 普通 HTTP 接口声明
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
        
        let classes = self.extract_classes(content, file_path, &tree);
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
    fn test_parse_interface() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            public interface ShopCopyService {
                Response query(GetShopCopyCmd cmd);
                Response clone(ShopCloneCmd cmd);
                Response restore(ShopRestoreCmd cmd);
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("ShopCopyService.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].name, "com.example.ShopCopyService");
        assert_eq!(result.classes[0].methods.len(), 3);
        assert_eq!(result.classes[0].methods[0].name, "query");
        assert_eq!(result.classes[0].methods[1].name, "clone");
        assert_eq!(result.classes[0].methods[2].name, "restore");
    }
    
    #[test]
    fn test_parse_interface_with_implementation() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            public interface UserService {
                void saveUser(String name);
            }
            
            public class UserServiceImpl implements UserService {
                @Override
                public void saveUser(String name) {
                    // implementation
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("UserService.java")).unwrap();
        assert_eq!(result.classes.len(), 2);
        
        // Check interface
        assert_eq!(result.classes[0].name, "com.example.UserService");
        assert_eq!(result.classes[0].methods.len(), 1);
        assert_eq!(result.classes[0].methods[0].name, "saveUser");
        
        // Check implementation
        assert_eq!(result.classes[1].name, "com.example.UserServiceImpl");
        assert_eq!(result.classes[1].methods.len(), 1);
        assert_eq!(result.classes[1].methods[0].name, "saveUser");
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
        // 路径可能包含应用名称前缀，所以我们检查它是否以正确的路径结尾
        assert!(http.path.ends_with("users/{id}"), "Path should end with users/{{id}}, got: {}", http.path);
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
    
    #[test]
    fn test_extract_field_access_method_calls() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            import com.hualala.shop.equipment.EquipmentManageExe;
            
            public class TestController {
                private EquipmentManageExe equipmentManageExe;
                
                public void testMethod() {
                    equipmentManageExe.listExecuteSchedule("");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("TestController.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        assert_eq!(method.calls.len(), 1);
        // 应该解析为完整的类名::方法名格式
        assert_eq!(method.calls[0].target, "com.hualala.shop.equipment.EquipmentManageExe::listExecuteSchedule");
    }
    
    #[test]
    fn test_extract_local_variable_method_calls() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            public class TestLocalVariable {
                public void go() {
                    Foo foo = new Foo();
                    foo.bar();
                }
            }
            
            class Foo {
                public void bar() {
                    System.out.println("bar called");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("TestLocalVariable.java")).unwrap();
        assert_eq!(result.classes.len(), 2);
        
        // 查找 TestLocalVariable 类的 go 方法
        let test_class = result.classes.iter()
            .find(|c| c.name.contains("TestLocalVariable"))
            .expect("Should find TestLocalVariable class");
        
        assert_eq!(test_class.methods.len(), 1);
        let go_method = &test_class.methods[0];
        assert_eq!(go_method.name, "go");
        
        // 验证方法调用
        assert_eq!(go_method.calls.len(), 1, "Should have 1 method call");
        
        // 应该解析为 Foo::bar 或 com.example.Foo::bar
        let call_target = &go_method.calls[0].target;
        assert!(
            call_target.contains("Foo::bar"),
            "Should resolve to Foo::bar, got: {}",
            call_target
        );
    }
    
    #[test]
    fn test_extract_local_variable_with_imports() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            import com.hualala.shop.equipment.EquipmentManageExe;
            
            public class TestController {
                public void testMethod() {
                    // 本地变量使用导入的类
                    EquipmentManageExe localExe = new EquipmentManageExe();
                    localExe.listExecuteSchedule("");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("TestController.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        assert_eq!(method.calls.len(), 1);
        
        // 应该解析为完整的导入类名::方法名格式
        assert_eq!(
            method.calls[0].target,
            "com.hualala.shop.equipment.EquipmentManageExe::listExecuteSchedule"
        );
    }
    
    #[test]
    fn test_extract_mixed_field_and_local_variable_calls() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            import com.hualala.shop.equipment.EquipmentManageExe;
            
            public class TestController {
                private EquipmentManageExe fieldExe;
                
                public void testMethod() {
                    // 类字段调用
                    fieldExe.listExecuteSchedule("field");
                    
                    // 本地变量调用
                    EquipmentManageExe localExe = new EquipmentManageExe();
                    localExe.listExecuteSchedule("local");
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("TestController.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        assert_eq!(method.calls.len(), 2, "Should have 2 method calls");
        
        // 两个调用都应该解析为完整的类名::方法名格式
        for call in &method.calls {
            assert_eq!(
                call.target,
                "com.hualala.shop.equipment.EquipmentManageExe::listExecuteSchedule"
            );
        }
    }
    
    #[test]
    fn test_extract_self_type_local_variable() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            public class Builder {
                private String name;
                
                public Builder setName(String name) {
                    this.name = name;
                    return this;
                }
                
                public Builder build() {
                    // 本地变量类型为当前类自身
                    Builder builder = new Builder();
                    builder.setName("test");
                    return builder;
                }
                
                public static Builder createBuilder() {
                    // 静态方法中的本地变量
                    Builder instance = new Builder();
                    instance.setName("static");
                    return instance;
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("Builder.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        
        let builder_class = &result.classes[0];
        assert_eq!(builder_class.name, "com.example.Builder");
        
        // 测试 build() 方法
        let build_method = builder_class.methods.iter()
            .find(|m| m.name == "build")
            .expect("Should find build method");
        
        assert_eq!(build_method.calls.len(), 1, "build() should have 1 method call");
        assert!(
            build_method.calls[0].target.contains("com.example.Builder::setName"),
            "Should resolve to com.example.Builder::setName, got: {}",
            build_method.calls[0].target
        );
        
        // 测试 createBuilder() 静态方法
        let create_method = builder_class.methods.iter()
            .find(|m| m.name == "createBuilder")
            .expect("Should find createBuilder method");
        
        assert_eq!(create_method.calls.len(), 1, "createBuilder() should have 1 method call");
        assert!(
            create_method.calls[0].target.contains("com.example.Builder::setName"),
            "Should resolve to com.example.Builder::setName in static method, got: {}",
            create_method.calls[0].target
        );
    }
    
    #[test]
    fn test_extract_various_method_call_patterns() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            public class TestService {
                private UserService userService;
                
                public void testMethod() {
                    // Direct method call
                    localMethod();
                    
                    // Field access method call
                    userService.findUser();
                    
                    // Chained method call
                    userService.getRepository().save();
                    
                    // Static method call
                    System.out.println("test");
                    
                    // Method call with multiple arguments
                    userService.updateUser(1, "name");
                }
                
                private void localMethod() {
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("TestService.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 2);
        
        let test_method = &result.classes[0].methods[0];
        let call_names: Vec<&str> = test_method.calls.iter()
            .map(|c| c.target.as_str())
            .collect();
        
        // Verify all method calls are captured correctly
        assert!(call_names.contains(&"localMethod"), "Should find localMethod");
        assert!(call_names.contains(&"findUser") || call_names.contains(&"UserService::findUser"), "Should find findUser");
        assert!(call_names.contains(&"getRepository") || call_names.contains(&"UserService::getRepository"), "Should find getRepository");
        assert!(call_names.contains(&"save"), "Should find save");
        assert!(call_names.contains(&"println") || call_names.contains(&"System::println"), "Should find println");
        assert!(call_names.contains(&"updateUser") || call_names.contains(&"UserService::updateUser"), "Should find updateUser");
    }
    
    #[test]
    fn test_extract_feign_client_annotation() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.hualala.shop.domain.feign;
            
            import org.springframework.cloud.openfeign.FeignClient;
            import org.springframework.web.bind.annotation.PostMapping;
            import org.springframework.web.bind.annotation.RequestBody;
            
            @FeignClient(value = "hll-basic-info-api", path = "/hll-basic-info-api")
            public interface BasicInfoFeign {
                @PostMapping("/feign/shop/copy/info")
                GoodsResponse getGoodsInfo(@RequestBody GoodsInfoRequest request);
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("BasicInfoFeign.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].name, "com.hualala.shop.domain.feign.BasicInfoFeign");
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        assert_eq!(method.name, "getGoodsInfo");
        assert!(method.http_annotations.is_some(), "HTTP annotation should be present");
        
        let http = method.http_annotations.as_ref().unwrap();
        assert_eq!(http.method, HttpMethod::POST);
        // 应该组合为：service_name/base_path/method_path
        assert_eq!(http.path, "hll-basic-info-api/hll-basic-info-api/feign/shop/copy/info");
    }
    
    #[test]
    fn test_extract_feign_client_without_base_path() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            import org.springframework.cloud.openfeign.FeignClient;
            import org.springframework.web.bind.annotation.GetMapping;
            
            @FeignClient(value = "user-service")
            public interface UserClient {
                @GetMapping("/api/users")
                User getUser();
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("UserClient.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        assert!(method.http_annotations.is_some());
        
        let http = method.http_annotations.as_ref().unwrap();
        assert_eq!(http.method, HttpMethod::GET);
        // 没有 base_path 时，应该是：service_name/method_path
        assert_eq!(http.path, "user-service/api/users");
    }
    
    #[test]
    fn test_extract_static_method_calls() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            import org.apache.commons.lang3.StringUtils;
            import java.util.Collections;
            
            public class TestStaticMethod {
                private UserService userService;
                
                public void testMethod() {
                    // 静态方法调用 - 使用导入的类
                    String result1 = StringUtils.isEmpty("test");
                    
                    // 静态方法调用 - 使用导入的类
                    List<String> list = Collections.emptyList();
                    
                    // 实例方法调用
                    userService.findUser();
                    
                    // 链式调用
                    Collections.emptyList().stream().filter(x -> x != null);
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("TestStaticMethod.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        let call_targets: Vec<&str> = method.calls.iter()
            .map(|c| c.target.as_str())
            .collect();
        
        // 验证静态方法调用被正确识别为 ClassName::methodName
        assert!(
            call_targets.contains(&"org.apache.commons.lang3.StringUtils::isEmpty"),
            "Should find StringUtils::isEmpty, got: {:?}", call_targets
        );
        assert!(
            call_targets.iter().any(|t| t.contains("Collections::emptyList")),
            "Should find Collections::emptyList, got: {:?}", call_targets
        );
        
        // 验证实例方法调用仍然正常工作
        assert!(
            call_targets.iter().any(|t| t.contains("UserService::findUser")),
            "Should find UserService::findUser (with or without package), got: {:?}", call_targets
        );
    }
    
    #[test]
    fn test_extract_feign_client_with_name_attribute() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example;
            
            import org.springframework.cloud.openfeign.FeignClient;
            import org.springframework.web.bind.annotation.PutMapping;
            
            @FeignClient(name = "order-service", path = "/orders")
            public interface OrderClient {
                @PutMapping("/update")
                void updateOrder();
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("OrderClient.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        assert!(method.http_annotations.is_some());
        
        let http = method.http_annotations.as_ref().unwrap();
        assert_eq!(http.method, HttpMethod::PUT);
        // 使用 name 属性时，应该正常工作
        assert_eq!(http.path, "order-service/orders/update");
    }
}
