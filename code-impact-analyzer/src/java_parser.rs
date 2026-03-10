use std::path::Path;
use std::sync::Mutex;
use std::fs;
use tree_sitter::Parser;
use regex::Regex;
use serde_yaml::Value as YamlValue;
use crate::errors::ParseError;
use crate::language_parser::{LanguageParser, ParsedFile, ClassInfo, MethodInfo, MethodCall};
use crate::types::*;

/// 判断是否是基本类型或常用类型
fn is_primitive_or_common_type(type_name: &str) -> bool {
    matches!(
        type_name,
        // 基本类型
        "int" | "long" | "short" | "byte" |
        "float" | "double" |
        "boolean" |
        "char" |
        "void" |
        // java.lang 包中的常用类
        "String" | "Object" | "Integer" | "Long" | "Short" | "Byte" |
        "Float" | "Double" | "Boolean" | "Character" |
        "StringBuilder" | "StringBuffer" |
        "Math" | "System" | "Class" | "Thread" |
        // 常用集合类（简单名称）
        "List" | "ArrayList" | "LinkedList" |
        "Set" | "HashSet" | "TreeSet" |
        "Map" | "HashMap" | "TreeMap" | "LinkedHashMap" |
        "Collection" | "Queue" | "Deque"
    ) || type_name.contains("<") || type_name.contains("[")  // 泛型和数组保持原样
}

/// Java 自动装箱：将基本类型转换为对应的包装类型
/// 
/// # Arguments
/// * `type_name` - 类型名称（可能是基本类型或包装类型）
/// 
/// # Returns
/// 返回对应的包装类型，如果不是基本类型则返回原类型
/// 
/// # Examples
/// ```ignore
/// assert_eq!(autobox_type("int"), "Integer");
/// assert_eq!(autobox_type("Integer"), "Integer");
/// assert_eq!(autobox_type("String"), "String");
/// ```
fn autobox_type(type_name: &str) -> String {
    match type_name {
        "int" => "Integer".to_string(),
        "long" => "Long".to_string(),
        "short" => "Short".to_string(),
        "byte" => "Byte".to_string(),
        "float" => "Float".to_string(),
        "double" => "Double".to_string(),
        "boolean" => "Boolean".to_string(),
        "char" => "Character".to_string(),
        "void" => "Void".to_string(),
        _ => type_name.to_string(),
    }
}

/// 移除类型中的泛型信息
/// 例如：List<String> -> List, Map<K,V> -> Map
fn remove_generics(type_name: &str) -> String {
    if let Some(pos) = type_name.find('<') {
        type_name[..pos].to_string()
    } else {
        type_name.to_string()
    }
}


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

/// 方法返回类型映射
/// 用于在推断参数类型时查找方法的返回类型
type MethodReturnTypeMap = std::collections::HashMap<String, String>;

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
    
    /// 使用全局方法返回类型映射解析文件
    /// 
    /// 此方法用于两遍索引策略的第二遍，使用全局返回类型映射来推断跨文件的方法调用参数类型
    /// 
    /// # Arguments
    /// * `content` - 文件内容
    /// * `file_path` - 文件路径
    /// * `global_return_types` - 全局方法返回类型映射（来自所有文件）
    /// * `global_class_index` - 全局类索引（用于解析通配符导入）
    /// 
    /// # Returns
    /// * `Ok(ParsedFile)` - 解析成功
    /// * `Err(ParseError)` - 解析失败
    pub fn parse_file_with_global_types(
        &self,
        content: &str,
        file_path: &Path,
        global_return_types: &rustc_hash::FxHashMap<String, String>,
    ) -> Result<ParsedFile, ParseError> {
        // 创建空的全局类索引（向后兼容）
        let empty_class_index = rustc_hash::FxHashMap::default();
        self.parse_file_with_global_types_and_classes(content, file_path, global_return_types, &empty_class_index)
    }
    
    /// 使用全局方法返回类型映射和类索引解析文件
    /// 
    /// # Arguments
    /// * `content` - 文件内容
    /// * `file_path` - 文件路径
    /// * `global_return_types` - 全局方法返回类型映射（来自所有文件）
    /// * `global_class_index` - 全局类索引（用于解析通配符导入）
    /// 
    /// # Returns
    /// * `Ok(ParsedFile)` - 解析成功
    /// * `Err(ParseError)` - 解析失败
    pub fn parse_file_with_global_types_and_classes(
        &self,
        content: &str,
        file_path: &Path,
        global_return_types: &rustc_hash::FxHashMap<String, String>,
        global_class_index: &rustc_hash::FxHashMap<String, String>,
    ) -> Result<ParsedFile, ParseError> {
        // 检查第一行是否包含 "Generated" 字样，如果是则跳过解析
        if let Some(first_line) = content.lines().next() {
            if first_line.contains("Generated") {
                // 返回空的解析结果
                return Ok(ParsedFile {
                    file_path: file_path.to_path_buf(),
                    language: "java".to_string(),
                    classes: vec![],
                    functions: vec![],
                    imports: vec![],
                });
            }
        }
        
        let tree = self.parser.lock().unwrap().parse(content, None)
            .ok_or_else(|| ParseError::InvalidFormat {
                message: "Failed to parse Java file".to_string(),
            })?;
        
        // 第一遍：提取类和方法，建立文件内方法返回类型映射
        let (mut classes, file_return_types) = self.extract_classes_with_return_types(content, file_path, &tree);
        
        // 合并文件内和全局返回类型映射
        let mut combined_return_types = MethodReturnTypeMap::default();
        for (k, v) in global_return_types.iter() {
            combined_return_types.insert(k.clone(), v.clone());
        }
        combined_return_types.extend(file_return_types);
        
        // 第二遍：使用合并后的返回类型映射重新提取方法调用
        for class in &mut classes {
            for method in &mut class.methods {
                // 找到对应的方法节点并重新提取调用
                let root_node = tree.root_node();
                if let Some(method_node) = self.find_method_node(content, &root_node, &class.name, &method.name, &method.full_qualified_name, &tree) {
                    method.calls = self.extract_method_calls_with_return_types_and_index(content, &method_node, &tree, &combined_return_types, &class.name, global_class_index);
                }
            }
        }
        
        let imports = self.extract_imports(content, &tree);
        
        Ok(ParsedFile {
            file_path: file_path.to_path_buf(),
            language: "java".to_string(),
            classes,
            functions: vec![], // Java 使用类和方法，不使用顶层函数
            imports,
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
        
        // 提取继承的父类
        let extends = self.extract_extends_class(source, &class_node, tree);
        
        // 提取类级别的 FeignClient 注解
        let feign_client_info = self.extract_feign_client_annotation(source, &class_node);
        
        // 提取类级别的 RequestMapping 注解
        let class_request_mapping = self.extract_class_level_request_mapping(source, &class_node);
        
        // 提取类级别的 KafkaListener 注解
        let class_kafka_topic = self.extract_class_kafka_listener(source, &class_node);
        
        // 提取类中的方法
        let methods = self.extract_methods_from_class(source, file_path, &class_node, &full_class_name, tree, &feign_client_info, &class_request_mapping, &class_kafka_topic, &implements, app_config);
        
        Some(ClassInfo {
            name: full_class_name,
            methods,
            line_range: (line_start, line_end),
            is_interface,
            implements,
            extends,
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
                            // 处理简单类型标识符
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
                            // 处理泛型类型（如 EventMessageDataConsumer<UserEvent>）
                            else if type_child.kind() == "generic_type" {
                                // 在 generic_type 中查找 type_identifier（基础接口名）
                                let mut generic_cursor = type_child.walk();
                                for generic_child in type_child.children(&mut generic_cursor) {
                                    if generic_child.kind() == "type_identifier" {
                                        if let Some(interface_name) = source.get(generic_child.byte_range()) {
                                            let full_interface_name = self.resolve_full_class_name(
                                                interface_name,
                                                &import_map,
                                                &package_name,
                                            );
                                            interfaces.push(full_interface_name);
                                            break; // 只需要基础接口名，不需要泛型参数
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        interfaces
    }
    /// 提取类继承的父类
    /// 提取类继承的父类
    fn extract_extends_class(&self, source: &str, class_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Option<String> {
        // 构建导入映射，用于将简单类名转换为完整类名
        let import_map = self.build_import_map(source, tree);
        let package_name = self.extract_package_name(source, tree);

        // 查找 superclass 节点（包含 extends 子句）
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "superclass" {
                // 在 superclass 中查找 type_identifier、generic_type 或 scoped_type_identifier
                let mut super_cursor = child.walk();
                for super_child in child.children(&mut super_cursor) {
                    // 处理简单类型标识符
                    if super_child.kind() == "type_identifier" {
                        if let Some(parent_name) = source.get(super_child.byte_range()) {
                            // 尝试将简单类名转换为完整类名
                            let full_parent_name = self.resolve_full_class_name(
                                parent_name,
                                &import_map,
                                &package_name,
                            );
                            return Some(full_parent_name);
                        }
                    }
                    // 处理完全限定名（如 com.base.BaseClass）
                    else if super_child.kind() == "scoped_type_identifier" {
                        if let Some(parent_name) = source.get(super_child.byte_range()) {
                            // 完全限定名直接返回
                            return Some(parent_name.to_string());
                        }
                    }
                    // 处理泛型类型（如 BaseService<T>）
                    else if super_child.kind() == "generic_type" {
                        // 在 generic_type 中查找 type_identifier 或 scoped_type_identifier（基础类名）
                        let mut generic_cursor = super_child.walk();
                        for generic_child in super_child.children(&mut generic_cursor) {
                            if generic_child.kind() == "type_identifier" {
                                if let Some(parent_name) = source.get(generic_child.byte_range()) {
                                    let full_parent_name = self.resolve_full_class_name(
                                        parent_name,
                                        &import_map,
                                        &package_name,
                                    );
                                    return Some(full_parent_name);
                                }
                            } else if generic_child.kind() == "scoped_type_identifier" {
                                if let Some(parent_name) = source.get(generic_child.byte_range()) {
                                    // 完全限定名直接返回
                                    return Some(parent_name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    
    /// 将简单类名解析为完整类名
    fn resolve_full_class_name(
        &self,
        simple_name: &str,
        import_map: &std::collections::HashMap<String, String>,
        package_name: &Option<String>,
    ) -> String {
        // 如果已经包含点号，说明已经是完整类名
        if simple_name.contains('.') {
            return simple_name.to_string();
        }
        
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
    
    /// 将简单类名解析为完整类名（支持通配符导入和全局索引查找）
    fn resolve_full_class_name_with_wildcards(
        &self,
        simple_name: &str,
        import_map: &std::collections::HashMap<String, String>,
        wildcard_imports: &[String],
        package_name: &Option<String>,
        global_types: &rustc_hash::FxHashMap<String, String>,
    ) -> String {
        // 如果已经包含点号，说明已经是完整类名
        if simple_name.contains('.') {
            return simple_name.to_string();
        }
        
        // 如果是基础类型或常用类型，直接返回
        if is_primitive_or_common_type(simple_name) {
            return simple_name.to_string();
        }
        
        // 首先尝试从导入映射中查找（明确的import语句）
        if let Some(full_name) = import_map.get(simple_name) {
            return full_name.clone();
        }
        
        // 尝试在通配符导入的包中查找
        for wildcard_package in wildcard_imports {
            let candidate = format!("{}.{}", wildcard_package, simple_name);
            // 在全局索引中查找这个候选类名
            if global_types.contains_key(&candidate) {
                return candidate;
            }
        }
        
        // 如果在通配符导入中没找到，假设在同一个包中
        if let Some(pkg) = package_name {
            return format!("{}.{}", pkg, simple_name);
        }
        
        // 否则返回简单类名
        simple_name.to_string()
    }
    
    /// 将简单类名解析为完整类名（支持通配符导入，使用启发式回退）
    /// 
    /// 这个方法不需要全局索引，而是使用启发式方法：
    /// 1. 首先尝试明确的import语句
    /// 2. 如果有通配符导入，优先使用第一个通配符导入的包
    /// 3. 最后回退到当前包
    fn resolve_full_class_name_with_wildcard_fallback(
        &self,
        simple_name: &str,
        import_map: &std::collections::HashMap<String, String>,
        wildcard_imports: &[String],
        package_name: &Option<String>,
    ) -> String {
        // 如果已经包含点号，说明已经是完整类名
        if simple_name.contains('.') {
            return simple_name.to_string();
        }
        
        // 如果是基础类型或常用类型，直接返回
        if is_primitive_or_common_type(simple_name) {
            return simple_name.to_string();
        }
        
        // 首先尝试从导入映射中查找（明确的import语句）
        if let Some(full_name) = import_map.get(simple_name) {
            return full_name.clone();
        }
        
        // 如果有通配符导入，优先使用第一个通配符导入的包
        // 注意：这是一个启发式方法，可能不总是正确的
        // 但在没有全局索引的情况下，这是一个合理的假设
        if let Some(wildcard_package) = wildcard_imports.first() {
            return format!("{}.{}", wildcard_package, simple_name);
        }
        
        // 如果没有通配符导入，假设在同一个包中
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
        class_kafka_topic: &Option<String>,
        class_implements: &[String],
        app_config: &ApplicationConfig,
    ) -> Vec<MethodInfo> {
        let mut methods = Vec::new();
        
        // 提取简单类名（不含包名）
        let simple_class_name = class_name.split('.').last().unwrap_or(class_name);
        
        // 查找类体或接口体
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "class_body" || child.kind() == "interface_body" {
                let mut body_cursor = child.walk();
                for body_child in child.children(&mut body_cursor) {
                    // 处理普通方法声明和接口方法声明
                    if body_child.kind() == "method_declaration" {
                        if let Some(method_info) = self.extract_method_info(source, file_path, body_child, class_name, simple_class_name, tree, feign_client_info, class_request_mapping, class_kafka_topic, class_implements, app_config) {
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
        simple_class_name: &str,
        tree: &tree_sitter::Tree,
        feign_client_info: &Option<FeignClientInfo>,
        class_request_mapping: &Option<String>,
        class_kafka_topic: &Option<String>,
        class_implements: &[String],
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
        
        // 提取参数类型列表
        let param_types = self.extract_parameter_types(source, &method_node, tree);
        
        // 构建完整的方法签名：ClassName::methodName(Type1,Type2,...)
        let full_qualified_name = if param_types.is_empty() {
            format!("{}::{}()", class_name, name)
        } else {
            format!("{}::{}({})", class_name, name, param_types.join(","))
        };
        
        // 提取方法调用
        let calls = self.extract_method_calls(source, &method_node, tree);
        
        // 提取 HTTP 注解（如果是 FeignClient，需要组合类级别和方法级别的注解）
        let http_annotations = if let Some(feign_info) = feign_client_info {
            self.extract_feign_http_annotation(source, &method_node, feign_info)
        } else {
            self.extract_http_annotations(source, &method_node, class_request_mapping, app_config)
        };
        
        // 提取 Kafka 操作
        let mut kafka_operations = self.extract_kafka_operations(source, &method_node);
        
        // 如果方法级别没有 Kafka 注解，但类级别有，则使用类级别的
        if kafka_operations.is_empty() {
            if let Some(topic) = class_kafka_topic {
                kafka_operations.push(KafkaOperation {
                    operation_type: KafkaOpType::Consume,
                    topic: topic.clone(),
                    line: line_start,
                });
            }
        }
        
        // 如果类实现了 EventMessageDataConsumer 接口，且方法名是 doConsumer，自动添加 Kafka 消费关系
        // 注意：这里假设 topic 信息需要从其他地方获取（如配置文件或类名推断）
        if kafka_operations.is_empty() && name == "doConsumer" {
            for interface in class_implements {
                if interface.contains("EventMessageDataConsumer") {
                    // 从类名推断 topic（例如：UserEventConsumer -> user-events）
                    // 这是一个简化的实现，实际项目中可能需要更复杂的逻辑
                    if let Some(topic) = self.infer_kafka_topic_from_class_name(simple_class_name) {
                        kafka_operations.push(KafkaOperation {
                            operation_type: KafkaOpType::Consume,
                            topic,
                            line: line_start,
                        });
                    }
                    break;
                }
            }
        }
        
        // 提取返回类型（用于Mapper类的数据库操作判断）
        let return_type = self.extract_return_type(source, &method_node, tree);
        
        // 提取数据库操作
        let db_operations = self.extract_db_operations(source, &method_node, simple_class_name, &return_type);
        
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
            return_type,
        })
    }
    
    /// 提取方法信息（带返回类型映射）
    fn extract_method_info_with_return_types(
        &self,
        source: &str,
        file_path: &Path,
        method_node: tree_sitter::Node,
        class_name: &str,
        simple_class_name: &str,
        tree: &tree_sitter::Tree,
        feign_client_info: &Option<FeignClientInfo>,
        class_request_mapping: &Option<String>,
        app_config: &ApplicationConfig,
        method_return_types: &MethodReturnTypeMap,
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
        
        // 提取参数类型列表
        let param_types = self.extract_parameter_types(source, &method_node, tree);
        
        // 构建完整的方法签名：ClassName::methodName(Type1,Type2,...)
        let full_qualified_name = if param_types.is_empty() {
            format!("{}::{}()", class_name, name)
        } else {
            format!("{}::{}({})", class_name, name, param_types.join(","))
        };
        
        // 提取方法调用（传入返回类型映射和类名）
        let calls = self.extract_method_calls_with_return_types(source, &method_node, tree, method_return_types, class_name);
        
        // 提取 HTTP 注解（如果是 FeignClient，需要组合类级别和方法级别的注解）
        let http_annotations = if let Some(feign_info) = feign_client_info {
            self.extract_feign_http_annotation(source, &method_node, feign_info)
        } else {
            self.extract_http_annotations(source, &method_node, class_request_mapping, app_config)
        };
        
        // 提取 Kafka 操作
        let kafka_operations = self.extract_kafka_operations(source, &method_node);
        
        // 提取返回类型（用于Mapper类的数据库操作判断）
        let return_type = self.extract_return_type(source, &method_node, tree);
        
        // 提取数据库操作
        let db_operations = self.extract_db_operations(source, &method_node, simple_class_name, &return_type);
        
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
            return_type,
        })
    }
    
    /// 提取方法参数类型列表（包含完整包名）
    fn extract_parameter_types(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Vec<String> {
        let mut param_types = Vec::new();
        
        // 获取导入映射和包名
        let import_map = self.build_import_map(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        let mut cursor = method_node.walk();
        
        // 查找 formal_parameters 节点
        for child in method_node.children(&mut cursor) {
            if child.kind() == "formal_parameters" {
                let mut param_cursor = child.walk();
                
                // 遍历每个 formal_parameter
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "formal_parameter" {
                        // 提取参数类型
                        if let Some(param_type) = self.extract_parameter_type(source, &param_child) {
                            // 解析为完整类名
                            let full_type = if is_primitive_or_common_type(&param_type) {
                                // 对基本类型进行自动装箱
                                autobox_type(&param_type)
                            } else {
                                self.resolve_full_class_name(&param_type, &import_map, &package_name)
                            };
                            param_types.push(full_type);
                        }
                    }
                }
                break;
            }
        }
        
        param_types
    }
    
    /// 提取单个参数的类型
    fn extract_parameter_type(&self, source: &str, param_node: &tree_sitter::Node) -> Option<String> {
        let mut cursor = param_node.walk();
        
        for child in param_node.children(&mut cursor) {
            let kind = child.kind();
            
            // 处理各种类型节点
            match kind {
                "type_identifier" | "integral_type" | "floating_point_type" | "boolean_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        return Some(text.to_string());
                    }
                }
                "generic_type" => {
                    // 处理泛型类型，如 List<String> -> List
                    if let Some(text) = source.get(child.byte_range()) {
                        return Some(remove_generics(text));
                    }
                }
                "array_type" => {
                    // 处理数组类型，如 String[] -> String[]
                    // 但如果是泛型数组，如 List<String>[] -> List[]
                    if let Some(text) = source.get(child.byte_range()) {
                        return Some(remove_generics(text));
                    }
                }
                "scoped_type_identifier" => {
                    // 处理带包名的类型，如 java.util.List
                    if let Some(text) = source.get(child.byte_range()) {
                        return Some(remove_generics(text));
                    }
                }
                _ => {}
            }
        }
        
        None
    }
    
    /// 提取方法调用

    fn extract_method_calls(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Vec<MethodCall> {
        // 使用空的返回类型映射（向后兼容）
        let empty_map = MethodReturnTypeMap::new();
        // 使用空字符串作为类名（向后兼容，不会添加类名前缀）
        self.extract_method_calls_with_return_types(source, method_node, tree, &empty_map, "")
    }
    
    /// 提取方法调用（带返回类型映射）
    fn extract_method_calls_with_return_types(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        tree: &tree_sitter::Tree,
        method_return_types: &MethodReturnTypeMap,
        class_name: &str,
    ) -> Vec<MethodCall> {
        // 使用空的全局类索引（向后兼容）
        let empty_index = rustc_hash::FxHashMap::default();
        self.extract_method_calls_with_return_types_and_index(source, method_node, tree, method_return_types, class_name, &empty_index)
    }
    
    /// 提取方法调用（带返回类型映射和全局类索引）
    fn extract_method_calls_with_return_types_and_index(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        tree: &tree_sitter::Tree,
        method_return_types: &MethodReturnTypeMap,
        class_name: &str,
        global_class_index: &rustc_hash::FxHashMap<String, String>,
    ) -> Vec<MethodCall> {
        let mut calls = Vec::new();
        
        // 提取导入语句，建立简单类名到完整类名的映射
        let import_map = self.build_import_map(source, tree);
        
        // 提取包名
        let package_name = self.extract_package_name(source, tree);
        
        // 提取类中的字段声明和方法内的本地变量，建立变量名到类型的映射（使用全局类索引）
        let field_types = self.extract_field_types_with_global_index(source, method_node, tree, global_class_index);
        
        self.walk_node_for_calls_with_return_types(source, *method_node, &mut calls, &field_types, &import_map, method_return_types, class_name, &package_name);
        calls
    }
    
    /// 构建导入映射：简单类名 -> 完整类名
    fn build_import_map(&self, source: &str, tree: &tree_sitter::Tree) -> std::collections::HashMap<String, String> {
        let mut import_map = std::collections::HashMap::new();
        let root_node = tree.root_node();
        
        self.walk_node_for_import_map(source, root_node, &mut import_map);
        
        import_map
    }
    
    /// 构建导入映射，同时返回通配符导入的包列表
    fn build_import_map_with_wildcards(&self, source: &str, tree: &tree_sitter::Tree) -> (std::collections::HashMap<String, String>, Vec<String>) {
        let mut import_map = std::collections::HashMap::new();
        let mut wildcard_imports = Vec::new();
        let root_node = tree.root_node();
        
        self.walk_node_for_import_map_with_wildcards(source, root_node, &mut import_map, &mut wildcard_imports);
        
        (import_map, wildcard_imports)
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
    
    /// 递归遍历节点构建导入映射，同时收集通配符导入
    fn walk_node_for_import_map_with_wildcards(
        &self,
        source: &str,
        node: tree_sitter::Node,
        import_map: &mut std::collections::HashMap<String, String>,
        wildcard_imports: &mut Vec<String>,
    ) {
        if node.kind() == "import_declaration" {
            let import_text = source.get(node.byte_range()).unwrap_or("");
            
            // 检查是否是通配符导入 (import bar.*;)
            if import_text.contains("*") {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "scoped_identifier" {
                        if let Some(package_path) = source.get(child.byte_range()) {
                            wildcard_imports.push(package_path.to_string());
                        }
                    }
                }
            } else {
                // 普通导入
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
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_import_map_with_wildcards(source, child, import_map, wildcard_imports);
        }
    }
    
    /// 提取类中的字段类型映射（包括类字段、方法参数和方法内的本地变量）
    fn extract_field_types(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> std::collections::HashMap<String, String> {
        // 使用空的全局类索引（向后兼容）
        let empty_index = rustc_hash::FxHashMap::default();
        self.extract_field_types_with_global_index(source, method_node, tree, &empty_index)
    }
    
    /// 提取类中的字段类型映射（使用全局类索引）
    fn extract_field_types_with_global_index(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        tree: &tree_sitter::Tree,
        global_class_index: &rustc_hash::FxHashMap<String, String>,
    ) -> std::collections::HashMap<String, String> {
        let mut field_types = std::collections::HashMap::new();
        
        // 获取导入映射（包括通配符导入）和包名，用于解析完整类名
        let (import_map, wildcard_imports) = self.build_import_map_with_wildcards(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
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
        
        // 解析类字段的完整类名（使用全局类索引）
        let mut resolved_field_types = std::collections::HashMap::new();
        for (var_name, simple_type) in field_types.iter() {
            let full_type = if is_primitive_or_common_type(simple_type) {
                simple_type.clone()
            } else {
                self.resolve_full_class_name_with_wildcards(simple_type, &import_map, &wildcard_imports, &package_name, global_class_index)
            };
            resolved_field_types.insert(var_name.clone(), full_type);
        }
        field_types = resolved_field_types;
        
        // 2. 提取方法参数
        self.extract_method_parameter_types_with_wildcards_and_index(source, method_node, &import_map, &wildcard_imports, &package_name, &mut field_types, global_class_index);
        
        // 3. 提取方法内的本地变量
        self.extract_local_variable_types_with_wildcards_and_index(source, method_node, tree, &mut field_types, global_class_index);
        
        field_types
    }
    
    /// 提取方法参数类型
    fn extract_method_parameter_types(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        import_map: &std::collections::HashMap<String, String>,
        package_name: &Option<String>,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        let mut param_types = std::collections::HashMap::new();
        
        // 查找 formal_parameters 节点
        let mut cursor = method_node.walk();
        for child in method_node.children(&mut cursor) {
            if child.kind() == "formal_parameters" {
                let mut param_cursor = child.walk();
                
                // 遍历每个 formal_parameter
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "formal_parameter" {
                        self.extract_parameter_name_and_type(source, &param_child, &mut param_types);
                    }
                }
                break;
            }
        }
        
        // 将简单类名解析为完整类名
        for (var_name, simple_type) in param_types.iter() {
            // 对于基本类型和常用类型，保持原样
            let full_type = if is_primitive_or_common_type(simple_type) {
                simple_type.clone()
            } else {
                self.resolve_full_class_name(simple_type, import_map, package_name)
            };
            field_types.insert(var_name.clone(), full_type);
        }
    }
    
    /// 提取方法参数类型（支持通配符导入）
    fn extract_method_parameter_types_with_wildcards(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        import_map: &std::collections::HashMap<String, String>,
        wildcard_imports: &[String],
        package_name: &Option<String>,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        let mut param_types = std::collections::HashMap::new();
        
        // 查找 formal_parameters 节点
        let mut cursor = method_node.walk();
        for child in method_node.children(&mut cursor) {
            if child.kind() == "formal_parameters" {
                let mut param_cursor = child.walk();
                
                // 遍历每个 formal_parameter
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "formal_parameter" {
                        self.extract_parameter_name_and_type(source, &param_child, &mut param_types);
                    }
                }
                break;
            }
        }
        
        // 将简单类名解析为完整类名（支持通配符导入）
        for (var_name, simple_type) in param_types.iter() {
            // 对于基本类型和常用类型，保持原样
            let full_type = if is_primitive_or_common_type(simple_type) {
                simple_type.clone()
            } else {
                self.resolve_full_class_name_with_wildcard_fallback(simple_type, import_map, wildcard_imports, package_name)
            };
            field_types.insert(var_name.clone(), full_type);
        }
    }
    
    /// 提取方法参数类型（使用全局类索引）
    fn extract_method_parameter_types_with_wildcards_and_index(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        import_map: &std::collections::HashMap<String, String>,
        wildcard_imports: &[String],
        package_name: &Option<String>,
        field_types: &mut std::collections::HashMap<String, String>,
        global_class_index: &rustc_hash::FxHashMap<String, String>,
    ) {
        let mut param_types = std::collections::HashMap::new();
        
        // 查找 formal_parameters 节点
        let mut cursor = method_node.walk();
        for child in method_node.children(&mut cursor) {
            if child.kind() == "formal_parameters" {
                let mut param_cursor = child.walk();
                
                // 遍历每个 formal_parameter
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "formal_parameter" {
                        self.extract_parameter_name_and_type(source, &param_child, &mut param_types);
                    }
                }
                break;
            }
        }
        
        // 将简单类名解析为完整类名（使用全局类索引）
        for (var_name, simple_type) in param_types.iter() {
            // 对于基本类型和常用类型，保持原样
            let full_type = if is_primitive_or_common_type(simple_type) {
                simple_type.clone()
            } else {
                self.resolve_full_class_name_with_wildcards(simple_type, import_map, wildcard_imports, package_name, global_class_index)
            };
            field_types.insert(var_name.clone(), full_type);
        }
    }    
    /// 从参数声明中提取参数名和类型
    fn extract_parameter_name_and_type(
        &self,
        source: &str,
        param_node: &tree_sitter::Node,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        let mut param_type = None;
        let mut param_name = None;
        
        let mut cursor = param_node.walk();
        for child in param_node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        param_type = Some(remove_generics(text));
                    }
                }
                "generic_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        param_type = Some(remove_generics(text));
                    }
                }
                "integral_type" | "floating_point_type" | "boolean_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        param_type = Some(text.to_string());
                    }
                }
                "array_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        param_type = Some(remove_generics(text));
                    }
                }
                "identifier" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        param_name = Some(text.to_string());
                    }
                }
                _ => {}
            }
        }
        
        if let (Some(name), Some(type_name)) = (param_name, param_type) {
            field_types.insert(name, type_name);
        }
    }
    
    /// 提取方法内的本地变量类型，并解析为完整类名
    fn extract_local_variable_types(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        tree: &tree_sitter::Tree,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        // 先保存已有的变量（类字段和方法参数），它们已经被解析过了
        let existing_vars: std::collections::HashSet<String> = field_types.keys().cloned().collect();
        
        // 提取本地变量的简单类型
        self.walk_node_for_local_vars(source, *method_node, field_types);
        
        // 获取导入映射和包名，用于解析完整类名
        let import_map = self.build_import_map(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        // 只解析新添加的本地变量
        let mut resolved_types = std::collections::HashMap::new();
        for (var_name, simple_type) in field_types.iter() {
            let full_type = if existing_vars.contains(var_name) {
                // 已经解析过的变量，保持原样
                simple_type.clone()
            } else if is_primitive_or_common_type(simple_type) {
                // 基本类型和常用类型，保持原样
                simple_type.clone()
            } else {
                // 新的本地变量，解析为完整类名
                self.resolve_full_class_name(simple_type, &import_map, &package_name)
            };
            resolved_types.insert(var_name.clone(), full_type);
        }
        
        // 更新 field_types
        *field_types = resolved_types;
    }
    
    /// 提取方法内的本地变量类型，并解析为完整类名（支持通配符导入）
    fn extract_local_variable_types_with_wildcards(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        tree: &tree_sitter::Tree,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        // 先保存已有的变量（类字段和方法参数），它们已经被解析过了
        let existing_vars: std::collections::HashSet<String> = field_types.keys().cloned().collect();
        
        // 提取本地变量的简单类型
        self.walk_node_for_local_vars(source, *method_node, field_types);
        
        // 获取导入映射（包括通配符导入）和包名，用于解析完整类名
        let (import_map, wildcard_imports) = self.build_import_map_with_wildcards(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        // 只解析新添加的本地变量
        let mut resolved_types = std::collections::HashMap::new();
        for (var_name, simple_type) in field_types.iter() {
            let full_type = if existing_vars.contains(var_name) {
                // 已经解析过的变量，保持原样
                simple_type.clone()
            } else if is_primitive_or_common_type(simple_type) {
                // 基本类型和常用类型，保持原样
                simple_type.clone()
            } else {
                // 新的本地变量，解析为完整类名（支持通配符导入）
                self.resolve_full_class_name_with_wildcard_fallback(simple_type, &import_map, &wildcard_imports, &package_name)
            };
            resolved_types.insert(var_name.clone(), full_type);
        }
        
        // 更新 field_types
        *field_types = resolved_types;
    }
    
    /// 提取方法内的本地变量类型，并解析为完整类名（使用全局类索引）
    fn extract_local_variable_types_with_wildcards_and_index(
        &self,
        source: &str,
        method_node: &tree_sitter::Node,
        tree: &tree_sitter::Tree,
        field_types: &mut std::collections::HashMap<String, String>,
        global_class_index: &rustc_hash::FxHashMap<String, String>,
    ) {
        // 先保存已有的变量（类字段和方法参数），它们已经被解析过了
        let existing_vars: std::collections::HashSet<String> = field_types.keys().cloned().collect();
        
        // 提取本地变量的简单类型
        self.walk_node_for_local_vars(source, *method_node, field_types);
        
        // 获取导入映射（包括通配符导入）和包名，用于解析完整类名
        let (import_map, wildcard_imports) = self.build_import_map_with_wildcards(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        // 只解析新添加的本地变量
        let mut resolved_types = std::collections::HashMap::new();
        for (var_name, simple_type) in field_types.iter() {
            let full_type = if existing_vars.contains(var_name) {
                // 已经解析过的变量，保持原样
                simple_type.clone()
            } else if is_primitive_or_common_type(simple_type) {
                // 基本类型和常用类型，保持原样
                simple_type.clone()
            } else {
                // 新的本地变量，解析为完整类名（使用全局类索引）
                self.resolve_full_class_name_with_wildcards(simple_type, &import_map, &wildcard_imports, &package_name, global_class_index)
            };
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
        } else if node.kind() == "lambda_expression" {
            // 提取 lambda 表达式的参数类型
            self.extract_lambda_parameter_types(source, node, field_types);
        } else if node.kind() == "enhanced_for_statement" {
            // 提取增强for循环的变量类型
            // for (Type var : collection) { ... }
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            
            // 查找类型和变量名
            let mut var_type: Option<String> = None;
            let mut var_name: Option<String> = None;
            
            for child in children {
                if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                    if let Ok(type_text) = child.utf8_text(source.as_bytes()) {
                        var_type = Some(type_text.to_string());
                    }
                } else if child.kind() == "identifier" && var_type.is_some() && var_name.is_none() {
                    // 第一个identifier是变量名（在类型之后）
                    if let Ok(name_text) = child.utf8_text(source.as_bytes()) {
                        var_name = Some(name_text.to_string());
                    }
                }
            }
            
            if let (Some(var_type), Some(var_name)) = (var_type, var_name) {
                field_types.insert(var_name, var_type);
            }
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
                "integral_type" | "floating_point_type" | "boolean_type" => {
                    // 基本类型
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
    
    /// 提取 lambda 表达式的参数类型
    /// 例如：list.stream().map(item -> ...) 中的 item 参数
    fn extract_lambda_parameter_types(
        &self,
        source: &str,
        lambda_node: tree_sitter::Node,
        field_types: &mut std::collections::HashMap<String, String>,
    ) {
        // 查找 lambda 参数
        let mut cursor = lambda_node.walk();
        let mut param_names = Vec::new();
        
        for child in lambda_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                // 单个参数：item -> ...
                if let Some(param_name) = source.get(child.byte_range()) {
                    param_names.push(param_name.to_string());
                }
            } else if child.kind() == "inferred_parameters" {
                // 多个参数：(a, b) -> ...
                let mut param_cursor = child.walk();
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "identifier" {
                        if let Some(param_name) = source.get(param_child.byte_range()) {
                            param_names.push(param_name.to_string());
                        }
                    }
                }
            } else if child.kind() == "formal_parameters" {
                // 显式类型参数：(String a, int b) -> ...
                let mut param_cursor = child.walk();
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "formal_parameter" {
                        // 提取参数名和类型
                        self.extract_parameter_name_and_type(source, &param_child, field_types);
                    }
                }
                return; // 显式类型参数已处理完毕
            }
        }
        
        // 如果有参数名但没有显式类型，尝试从上下文推断类型
        if !param_names.is_empty() {
            // 查找 lambda 的父节点，看是否是 stream().map() 等操作
            if let Some(parent) = lambda_node.parent() {
                if parent.kind() == "argument_list" {
                    // lambda 作为参数传递
                    if let Some(method_invocation) = parent.parent() {
                        if method_invocation.kind() == "method_invocation" {
                            // 尝试推断 lambda 参数类型
                            if let Some(param_type) = self.infer_lambda_parameter_type_from_stream(
                                source,
                                &method_invocation,
                                field_types,
                            ) {
                                // 将推断的类型赋给第一个参数（通常 stream 操作只有一个参数）
                                if let Some(first_param) = param_names.first() {
                                    field_types.insert(first_param.clone(), param_type);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    /// 从 stream 操作推断 lambda 参数类型
    /// 例如：list.stream().map(item -> ...) 中，如果 list 是 List<Tac>，则 item 是 Tac
    fn infer_lambda_parameter_type_from_stream(
        &self,
        source: &str,
        method_invocation: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        // 查找方法名
        let mut cursor = method_invocation.walk();
        let mut method_name = None;
        let mut object_node = None;
        
        for child in method_invocation.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(name) = source.get(child.byte_range()) {
                    method_name = Some(name.to_string());
                }
            } else if child.kind() == "method_invocation" {
                // 链式调用的对象部分
                object_node = Some(child);
            } else if child.kind() == "field_access" {
                // 字段访问作为对象
                object_node = Some(child);
            }
        }
        
        // 检查是否是 stream 相关的方法（map, filter, forEach 等）
        if let Some(method) = &method_name {
            if method == "map" || method == "filter" || method == "forEach" 
                || method == "flatMap" || method == "peek" {
                // 尝试从对象推断类型
                if let Some(obj_node) = object_node {
                    return self.infer_stream_element_type(source, &obj_node, field_types);
                }
            }
        }
        
        None
    }
    
    /// 推断 stream 的元素类型
    /// 例如：list.stream() 中，如果 list 是 List<Tac>，则 stream 元素是 Tac
    fn infer_stream_element_type(
        &self,
        source: &str,
        stream_node: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        // 如果是 method_invocation，检查是否是 stream() 调用
        if stream_node.kind() == "method_invocation" {
            let mut cursor = stream_node.walk();
            let mut method_name = None;
            let mut object_node = None;
            
            for child in stream_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Some(name) = source.get(child.byte_range()) {
                        method_name = Some(name.to_string());
                    }
                } else if child.kind() == "method_invocation" {
                    object_node = Some(child);
                } else if child.kind() == "field_access" {
                    object_node = Some(child);
                }
            }
            
            // 如果是 stream() 方法，从对象推断类型
            if method_name.as_deref() == Some("stream") {
                if let Some(obj_node) = object_node {
                    return self.infer_collection_element_type(source, &obj_node, field_types);
                }
            } else if method_name.as_deref() == Some("map") 
                || method_name.as_deref() == Some("filter")
                || method_name.as_deref() == Some("flatMap") {
                // 链式调用，继续向上推断
                if let Some(obj_node) = object_node {
                    return self.infer_stream_element_type(source, &obj_node, field_types);
                }
            }
        } else if stream_node.kind() == "field_access" {
            // 处理 obj.field.stream() 的情况
            let mut cursor = stream_node.walk();
            for child in stream_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    // 这是字段名或变量名
                    if let Some(var_name) = source.get(child.byte_range()) {
                        if let Some(var_type) = field_types.get(var_name) {
                            return self.extract_generic_type(var_type);
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// 推断集合的元素类型
    /// 例如：list 是 List<Tac>，则元素类型是 Tac
    fn infer_collection_element_type(
        &self,
        source: &str,
        collection_node: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        // 如果是 method_invocation，可能是 new ArrayList<>(...) 或其他方法调用
        if collection_node.kind() == "method_invocation" {
            // 检查是否是方法调用（如 tic(...)）
            // 这种情况需要查找方法的返回类型，暂时跳过
            // 递归处理链式调用
            let mut cursor = collection_node.walk();
            for child in collection_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Some(var_name) = source.get(child.byte_range()) {
                        if let Some(var_type) = field_types.get(var_name) {
                            return self.extract_generic_type(var_type);
                        }
                    }
                } else if child.kind() == "argument_list" {
                    // 检查参数列表中是否有变量，可能是 new ArrayList<>(list) 的情况
                    let mut arg_cursor = child.walk();
                    for arg_child in child.children(&mut arg_cursor) {
                        if arg_child.kind() == "identifier" {
                            if let Some(arg_name) = source.get(arg_child.byte_range()) {
                                if let Some(arg_type) = field_types.get(arg_name) {
                                    return self.extract_generic_type(arg_type);
                                }
                            }
                        } else if arg_child.kind() == "object_creation_expression" {
                            // 处理 new ArrayList<>(list) 的情况
                            return self.infer_collection_element_type(source, &arg_child, field_types);
                        }
                    }
                }
            }
        } else if collection_node.kind() == "object_creation_expression" {
            // 处理 new ArrayList<>(list) 或 new ArrayList<Tac>()
            let mut cursor = collection_node.walk();
            let mut found_generic_type = None;
            
            for child in collection_node.children(&mut cursor) {
                if child.kind() == "generic_type" || child.kind() == "type_identifier" {
                    // 检查是否有显式的泛型类型
                    if let Some(type_str) = source.get(child.byte_range()) {
                        if let Some(generic_type) = self.extract_generic_type(type_str) {
                            if !generic_type.is_empty() {
                                found_generic_type = Some(generic_type);
                            }
                        }
                    }
                }
            }
            
            // 如果找到了显式的泛型类型，直接返回
            if let Some(generic_type) = found_generic_type {
                return Some(generic_type);
            }
            
            // 否则，检查构造函数参数来推断类型
            let mut cursor2 = collection_node.walk();
            for child in collection_node.children(&mut cursor2) {
                if child.kind() == "argument_list" {
                    // 检查构造函数参数
                    let mut arg_cursor = child.walk();
                    for arg_child in child.children(&mut arg_cursor) {
                        if arg_child.kind() == "identifier" {
                            if let Some(arg_name) = source.get(arg_child.byte_range()) {
                                if let Some(arg_type) = field_types.get(arg_name) {
                                    return self.extract_generic_type(arg_type);
                                }
                            }
                        }
                    }
                }
            }
        } else if collection_node.kind() == "field_access" {
            // 处理 obj.field 的情况
            let mut cursor = collection_node.walk();
            for child in collection_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Some(var_name) = source.get(child.byte_range()) {
                        if let Some(var_type) = field_types.get(var_name) {
                            return self.extract_generic_type(var_type);
                        }
                    }
                }
            }
        } else if collection_node.kind() == "identifier" {
            // 直接是变量名
            if let Some(var_name) = source.get(collection_node.byte_range()) {
                if let Some(var_type) = field_types.get(var_name) {
                    return self.extract_generic_type(var_type);
                }
            }
        }
        
        None
    }
    
    /// 从泛型类型中提取元素类型
    /// 例如：List<Tac> -> Tac, ArrayList<String> -> String
    fn extract_generic_type(&self, type_str: &str) -> Option<String> {
        // 查找 < 和 > 之间的内容
        if let Some(start) = type_str.find('<') {
            if let Some(end) = type_str.rfind('>') {
                if start < end {
                    let generic_type = &type_str[start + 1..end];
                    // 处理嵌套泛型，如 List<List<String>>，这里简化处理，只取第一层
                    return Some(generic_type.trim().to_string());
                }
            }
        }
        None
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
        // 使用空的返回类型映射（向后兼容）
        let empty_map = MethodReturnTypeMap::new();
        let empty_package = None;
        // 使用空字符串作为类名（向后兼容，不会添加类名前缀）
        self.walk_node_for_calls_with_return_types(source, node, calls, field_types, import_map, &empty_map, "", &empty_package);
    }
    
    /// 递归遍历节点查找方法调用（带返回类型映射）
    fn walk_node_for_calls_with_return_types(
        &self,
        source: &str,
        node: tree_sitter::Node,
        calls: &mut Vec<MethodCall>,
        field_types: &std::collections::HashMap<String, String>,
        import_map: &std::collections::HashMap<String, String>,
        method_return_types: &MethodReturnTypeMap,
        class_name: &str,
        package_name: &Option<String>,
    ) {
        if node.kind() == "method_invocation" {
            // 查找方法调用的对象和方法名
            let mut cursor = node.walk();
            let mut identifiers = Vec::new();
            let mut scoped_identifiers = Vec::new();
            let mut argument_list_node = None;
            let mut has_method_invocation_object = false;
            let mut field_access_object = None;
            
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
                } else if child.kind() == "method_invocation" {
                    // 链式调用：obj.method1().method2()
                    has_method_invocation_object = true;
                } else if child.kind() == "field_access" {
                    // 字段访问作为对象：Foo.BAR.method()
                    field_access_object = Some(child);
                } else if child.kind() == "argument_list" {
                    argument_list_node = Some(child);
                }
            }
            
            let line = node.start_position().row + 1;
            
            // 提取参数类型（传入返回类型映射和包名）
            let arg_types = if let Some(arg_node) = argument_list_node {
                self.extract_argument_types_with_return_types(source, &arg_node, field_types, import_map, method_return_types, package_name)
            } else {
                Vec::new()
            };
            
            // 处理静态方法调用：ClassName.staticMethod() 或 package.ClassName.staticMethod()
            if !scoped_identifiers.is_empty() && !identifiers.is_empty() {
                // scoped_identifier 包含类名（可能带包名），identifier 是方法名
                let class_name = &scoped_identifiers[0];
                let method_name = &identifiers[identifiers.len() - 1];
                
                // 尝试将简单类名转换为完整类名
                let full_class_name = import_map.get(class_name)
                    .unwrap_or(class_name);
                
                let target = if arg_types.is_empty() {
                    format!("{}::{}()", full_class_name, method_name)
                } else {
                    format!("{}::{}({})", full_class_name, method_name, arg_types.join(","))
                };
                
                calls.push(MethodCall {
                    target,
                    line,
                });
                return;
            }
            
            // 处理 field_access 作为对象的情况：Foo.BAR.method()
            if let Some(field_access_node) = field_access_object {
                if !identifiers.is_empty() {
                    let method_name = &identifiers[identifiers.len() - 1];
                    let field_access_text = &source[field_access_node.byte_range()];
                    
                    // 解析 field_access，如 "Foo.BAR"
                    // 需要推断 Foo.BAR 的类型
                    let object_type = self.infer_field_access_type(
                        field_access_text,
                        import_map,
                        package_name,
                    );
                    
                    if let Some(obj_type) = object_type {
                        let target = if arg_types.is_empty() {
                            format!("{}::{}()", obj_type, method_name)
                        } else {
                            format!("{}::{}({})", obj_type, method_name, arg_types.join(","))
                        };
                        
                        calls.push(MethodCall {
                            target,
                            line,
                        });
                        return;
                    }
                }
            }
            
            // 对于 obj.method() 形式，有两个 identifier：对象名和方法名
            // 对于 method() 形式，只有一个 identifier：方法名
            // 对于 this.method() 形式，会有 "this" 关键字
            let (object_name, method_name) = if identifiers.len() >= 2 {
                (Some(identifiers[0].clone()), identifiers[identifiers.len() - 1].clone())
            } else if identifiers.len() == 1 {
                (None, identifiers[0].clone())
            } else {
                return;
            };
            
            // 检查是否有 "this" 关键字
            let mut has_this = false;
            let mut cursor_check = node.walk();
            for child in node.children(&mut cursor_check) {
                if child.kind() == "this" {
                    has_this = true;
                    break;
                }
            }
            
            // 如果有对象名，尝试解析为完整的类名::方法名
            let target = if let Some(obj) = object_name {
                // 检查是否是 "this"
                if obj == "this" || has_this {
                    // this.method() 调用，使用当前类名
                    if !class_name.is_empty() {
                        if arg_types.is_empty() {
                            format!("{}::{}()", class_name, method_name)
                        } else {
                            format!("{}::{}({})", class_name, method_name, arg_types.join(","))
                        }
                    } else {
                        if arg_types.is_empty() {
                            format!("{}()", method_name)
                        } else {
                            format!("{}({})", method_name, arg_types.join(","))
                        }
                    }
                } else if let Some(class_type) = field_types.get(&obj) {
                    // 尝试将简单类名转换为完整类名
                    let full_class_name = import_map.get(class_type)
                        .unwrap_or(class_type);
                    
                    if arg_types.is_empty() {
                        format!("{}::{}()", full_class_name, method_name)
                    } else {
                        format!("{}::{}({})", full_class_name, method_name, arg_types.join(","))
                    }
                } else {
                    // 可能是静态方法调用，尝试从 import_map 中查找
                    if let Some(full_class_name) = import_map.get(&obj) {
                        if arg_types.is_empty() {
                            format!("{}::{}()", full_class_name, method_name)
                        } else {
                            format!("{}::{}({})", full_class_name, method_name, arg_types.join(","))
                        }
                    } else {
                        if arg_types.is_empty() {
                            format!("{}()", method_name)
                        } else {
                            format!("{}({})", method_name, arg_types.join(","))
                        }
                    }
                }
            } else {
                // 无对象名的调用，如 method()
                // 如果是链式调用（对象是另一个方法调用），不添加类名
                // 否则，应该是调用当前类的方法
                if has_method_invocation_object {
                    // 链式调用，不添加类名前缀
                    if arg_types.is_empty() {
                        format!("{}()", method_name)
                    } else {
                        format!("{}({})", method_name, arg_types.join(","))
                    }
                } else if !class_name.is_empty() {
                    // 当前类的方法调用
                    if arg_types.is_empty() {
                        format!("{}::{}()", class_name, method_name)
                    } else {
                        format!("{}::{}({})", class_name, method_name, arg_types.join(","))
                    }
                } else {
                    if arg_types.is_empty() {
                        format!("{}()", method_name)
                    } else {
                        format!("{}({})", method_name, arg_types.join(","))
                    }
                }
            };
            
            calls.push(MethodCall {
                target,
                line,
            });
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_calls_with_return_types(source, child, calls, field_types, import_map, method_return_types, class_name, package_name);
        }
    }
    
    /// 提取方法调用的参数类型
    fn extract_argument_types(
        &self,
        source: &str,
        argument_list_node: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
        import_map: &std::collections::HashMap<String, String>,
    ) -> Vec<String> {
        // 使用空的返回类型映射和空的包名（向后兼容）
        let empty_map = MethodReturnTypeMap::new();
        let empty_package = None;
        self.extract_argument_types_with_return_types(source, argument_list_node, field_types, import_map, &empty_map, &empty_package)
    }
    
    /// 提取方法调用的参数类型（带返回类型映射）
    fn extract_argument_types_with_return_types(
        &self,
        source: &str,
        argument_list_node: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
        import_map: &std::collections::HashMap<String, String>,
        method_return_types: &MethodReturnTypeMap,
        package_name: &Option<String>,
    ) -> Vec<String> {
        let mut arg_types = Vec::new();
        let mut cursor = argument_list_node.walk();
        
        for child in argument_list_node.children(&mut cursor) {
            // 跳过括号和逗号
            if child.kind() == "(" || child.kind() == ")" || child.kind() == "," {
                continue;
            }
            
            // 推断参数类型（传入返回类型映射和包名）
            if let Some(arg_type) = self.infer_argument_type_with_return_types(source, &child, field_types, import_map, method_return_types, package_name) {
                arg_types.push(arg_type);
            }
        }
        
        arg_types
    }
    
    /// 推断参数的类型
    fn infer_argument_type(
        &self,
        source: &str,
        arg_node: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
        import_map: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        // 使用空的返回类型映射和空的包名（向后兼容）
        let empty_map = MethodReturnTypeMap::new();
        let empty_package = None;
        self.infer_argument_type_with_return_types(source, arg_node, field_types, import_map, &empty_map, &empty_package)
    }
    
    /// 推断参数的类型（带返回类型映射）
    fn infer_argument_type_with_return_types(
        &self,
        source: &str,
        arg_node: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
        import_map: &std::collections::HashMap<String, String>,
        method_return_types: &MethodReturnTypeMap,
        package_name: &Option<String>,
    ) -> Option<String> {
        match arg_node.kind() {
            // 字符串字面量
            "string_literal" => Some("String".to_string()),
            
            // 整数字面量 - 自动装箱为 Integer/Long
            "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal" | "binary_integer_literal" => {
                // 检查是否有 L 后缀
                if let Some(text) = source.get(arg_node.byte_range()) {
                    if text.ends_with('L') || text.ends_with('l') {
                        Some(autobox_type("long"))
                    } else {
                        Some(autobox_type("int"))
                    }
                } else {
                    Some(autobox_type("int"))
                }
            }
            
            // 浮点数字面量 - 自动装箱为 Float/Double
            "decimal_floating_point_literal" | "hex_floating_point_literal" => {
                if let Some(text) = source.get(arg_node.byte_range()) {
                    if text.ends_with('f') || text.ends_with('F') {
                        Some(autobox_type("float"))
                    } else {
                        Some(autobox_type("double"))
                    }
                } else {
                    Some(autobox_type("double"))
                }
            }
            
            // 布尔字面量 - 自动装箱为 Boolean
            "true" | "false" => Some(autobox_type("boolean")),
            
            // null 字面量
            "null_literal" => Some("Object".to_string()),
            
            // 字符字面量 - 自动装箱为 Character
            "character_literal" => Some(autobox_type("char")),
            
            // 标识符（变量名）- 对变量类型也进行自动装箱
            "identifier" => {
                if let Some(var_name) = source.get(arg_node.byte_range()) {
                    // 从 field_types 中查找变量类型，去除泛型，并进行自动装箱
                    field_types.get(var_name).map(|t| {
                        let type_without_generics = remove_generics(t);
                        autobox_type(&type_without_generics)
                    })
                } else {
                    None
                }
            }
            
            // 字段访问：obj.field
            "field_access" => {
                // 尝试推断字段访问的类型
                if let Some(field_access_text) = source.get(arg_node.byte_range()) {
                    // 使用 infer_field_access_type 推断类型
                    self.infer_field_access_type(field_access_text, import_map, package_name)
                        .or(Some("Object".to_string()))
                } else {
                    Some("Object".to_string())
                }
            }
            
            // 方法调用：obj.method() 或 method()
            "method_invocation" => {
                // 尝试推断方法返回类型（使用返回类型映射），并进行自动装箱
                self.infer_method_return_type_with_map(source, arg_node, field_types, import_map, method_return_types, package_name)
                    .map(|t| autobox_type(&t))
                    .or(Some("Object".to_string()))
            }
            
            // 对象创建：new ClassName() 或 new ClassName<T>()
            "object_creation_expression" => {
                let mut cursor = arg_node.walk();
                for child in arg_node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        if let Some(type_name) = source.get(child.byte_range()) {
                            // 使用 resolve_full_class_name 解析完整类名
                            let simple_type = remove_generics(type_name);
                            let full_type = if is_primitive_or_common_type(&simple_type) {
                                simple_type
                            } else {
                                self.resolve_full_class_name(&simple_type, import_map, package_name)
                            };
                            return Some(full_type);
                        }
                    } else if child.kind() == "scoped_type_identifier" {
                        // 处理完整包名类型，如 com.example.model.User
                        if let Some(type_name) = source.get(child.byte_range()) {
                            let simple_type = remove_generics(type_name);
                            return Some(simple_type);
                        }
                    } else if child.kind() == "generic_type" {
                        // 处理泛型类型，如 ArrayList<String> -> ArrayList
                        if let Some(type_name) = source.get(child.byte_range()) {
                            let simple_type = remove_generics(type_name);
                            let full_type = if is_primitive_or_common_type(&simple_type) {
                                simple_type
                            } else {
                                self.resolve_full_class_name(&simple_type, import_map, package_name)
                            };
                            return Some(full_type);
                        }
                    }
                }
                Some("Object".to_string())
            }
            
            // 数组创建：new int[10] 或 new List<String>[10]
            "array_creation_expression" => {
                let mut cursor = arg_node.walk();
                for child in arg_node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        if let Some(type_name) = source.get(child.byte_range()) {
                            return Some(format!("{}[]", type_name));
                        }
                    } else if child.kind() == "integral_type" || child.kind() == "floating_point_type" {
                        if let Some(type_name) = source.get(child.byte_range()) {
                            return Some(format!("{}[]", type_name));
                        }
                    } else if child.kind() == "generic_type" {
                        // 处理泛型数组，如 List<String>[] -> List[]
                        if let Some(type_name) = source.get(child.byte_range()) {
                            return Some(format!("{}[]", remove_generics(type_name)));
                        }
                    }
                }
                Some("Object[]".to_string())
            }
            
            // 类型转换：(Type) value 或 (Type<T>) value
            "cast_expression" => {
                let mut cursor = arg_node.walk();
                for child in arg_node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        if let Some(type_name) = source.get(child.byte_range()) {
                            // 使用 resolve_full_class_name 解析完整类名
                            let simple_type = remove_generics(type_name);
                            let full_type = if is_primitive_or_common_type(&simple_type) {
                                simple_type
                            } else {
                                self.resolve_full_class_name(&simple_type, import_map, package_name)
                            };
                            return Some(full_type);
                        }
                    } else if child.kind() == "generic_type" {
                        // 处理泛型类型转换，如 (List<String>) -> List
                        if let Some(type_name) = source.get(child.byte_range()) {
                            let simple_type = remove_generics(type_name);
                            let full_type = if is_primitive_or_common_type(&simple_type) {
                                simple_type
                            } else {
                                self.resolve_full_class_name(&simple_type, import_map, package_name)
                            };
                            return Some(full_type);
                        }
                    }
                }
                None
            }
            
            // 二元表达式：a + b
            "binary_expression" => {
                // 简化处理，返回 Object
                Some("Object".to_string())
            }
            
            // 其他表达式
            _ => {
                // 默认返回 Object
                Some("Object".to_string())
            }
        }
    }
    
    /// 推断字段访问的类型，如 Foo.BAR 的类型是 Foo
    fn infer_field_access_type(
        &self,
        field_access_text: &str,
        import_map: &std::collections::HashMap<String, String>,
        package_name: &Option<String>,
    ) -> Option<String> {
        // 解析 field_access，如 "Foo.BAR"
        // 对于枚举常量，Foo.BAR 的类型就是 Foo
        if let Some(dot_pos) = field_access_text.rfind('.') {
            let type_name = &field_access_text[..dot_pos];
            let field_name = &field_access_text[dot_pos + 1..];
            
            // 检查字段名是否是全大写（枚举常量的常见命名）
            // 或者首字母大写（也可能是枚举常量）
            if field_name.chars().next().map_or(false, |c| c.is_uppercase()) {
                // 尝试解析类型名为完整类名
                let full_type = if is_primitive_or_common_type(type_name) {
                    type_name.to_string()
                } else {
                    self.resolve_full_class_name(type_name, import_map, package_name)
                };
                return Some(full_type);
            }
        }
        
        None
    }
    
    /// 尝试推断方法调用的返回类型（使用返回类型映射）
    fn infer_method_return_type_with_map(
        &self,
        source: &str,
        method_invocation_node: &tree_sitter::Node,
        field_types: &std::collections::HashMap<String, String>,
        import_map: &std::collections::HashMap<String, String>,
        method_return_types: &MethodReturnTypeMap,
        package_name: &Option<String>,
    ) -> Option<String> {
        // 提取方法名和对象类型
        let mut cursor = method_invocation_node.walk();
        let mut identifiers = Vec::new();
        let mut argument_list_node = None;
        let mut object_method_invocation = None;
        let mut field_access_object = None;
        
        for child in method_invocation_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    identifiers.push(text.to_string());
                }
            } else if child.kind() == "method_invocation" {
                // 链式调用：obj.method1().method2()
                object_method_invocation = Some(child);
            } else if child.kind() == "field_access" {
                // 字段访问作为对象：Foo.BAR.method()
                field_access_object = Some(child);
            } else if child.kind() == "argument_list" {
                argument_list_node = Some(child);
            }
        }
        
        if identifiers.is_empty() {
            return None;
        }
        
        // 提取参数类型
        let arg_types = if let Some(arg_node) = argument_list_node {
            self.extract_argument_types_with_return_types(source, &arg_node, field_types, import_map, method_return_types, package_name)
        } else {
            Vec::new()
        };
        
        // 处理 field_access 作为对象的情况：Foo.BAR.method()
        if let Some(field_access_node) = field_access_object {
            if !identifiers.is_empty() {
                let method_name = &identifiers[identifiers.len() - 1];
                let field_access_text = &source[field_access_node.byte_range()];
                
                // 推断 field_access 的类型
                if let Some(object_type) = self.infer_field_access_type(
                    field_access_text,
                    import_map,
                    package_name,
                ) {
                    // 构建方法签名
                    let method_signature = if arg_types.is_empty() {
                        format!("{}::{}()", object_type, method_name)
                    } else {
                        format!("{}::{}({})", object_type, method_name, arg_types.join(","))
                    };
                    
                    // 从映射中查找返回类型
                    if let Some(return_type) = method_return_types.get(&method_signature) {
                        return Some(return_type.clone());
                    }
                }
            }
        }
        
        // 处理链式调用：obj.method1().method2()
        if let Some(obj_method_node) = object_method_invocation {
            // 先推断前一个方法调用的返回类型
            if let Some(object_type) = self.infer_method_return_type_with_map(source, &obj_method_node, field_types, import_map, method_return_types, package_name) {
                // 使用返回类型作为当前方法的对象类型
                let method_name = identifiers.last()?;
                
                // 尝试将简单类名转换为完整类名
                let full_class_name = import_map.get(&object_type)
                    .unwrap_or(&object_type);
                
                // 构建方法签名
                let method_signature = if arg_types.is_empty() {
                    format!("{}::{}()", full_class_name, method_name)
                } else {
                    format!("{}::{}({})", full_class_name, method_name, arg_types.join(","))
                };
                
                // 从映射中查找返回类型
                if let Some(return_type) = method_return_types.get(&method_signature) {
                    return Some(return_type.clone());
                }
                
                // 如果没找到，尝试在所有方法签名中查找匹配的（可能是同一文件中的类）
                // 查找所有以 ::methodName() 结尾的签名
                let method_suffix = if arg_types.is_empty() {
                    format!("::{}()", method_name)
                } else {
                    format!("::{}({})", method_name, arg_types.join(","))
                };
                
                for (sig, ret_type) in method_return_types.iter() {
                    if sig.ends_with(&method_suffix) {
                        // 检查类名是否匹配（简单类名）
                        if let Some(class_part) = sig.split("::").next() {
                            if class_part.ends_with(&object_type) {
                                return Some(ret_type.clone());
                            }
                        }
                    }
                }
            }
            return None;
        }
        
        // 对于 obj.method() 形式
        if identifiers.len() >= 2 {
            let object_name = &identifiers[0];
            let method_name = &identifiers[identifiers.len() - 1];
            
            // 获取对象的类型
            if let Some(class_type) = field_types.get(object_name) {
                // 尝试将简单类名转换为完整类名
                let full_class_name = import_map.get(class_type)
                    .unwrap_or(class_type);
                
                // 构建方法签名
                let method_signature = if arg_types.is_empty() {
                    format!("{}::{}()", full_class_name, method_name)
                } else {
                    format!("{}::{}({})", full_class_name, method_name, arg_types.join(","))
                };
                
                // 从映射中查找返回类型
                if let Some(return_type) = method_return_types.get(&method_signature) {
                    return Some(return_type.clone());
                }
            }
        }
        
        None
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
    
    /// 提取类级别的 Kafka 注解
    fn extract_class_kafka_listener(&self, source: &str, class_node: &tree_sitter::Node) -> Option<String> {
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                if let Some(text) = source.get(child.byte_range()) {
                    if text.contains("@KafkaListener") {
                        let topic_pattern = Regex::new(r#"topics\s*=\s*"([^"]+)""#).unwrap();
                        if let Some(cap) = topic_pattern.captures(text) {
                            if let Some(topic) = cap.get(1) {
                                return Some(topic.as_str().to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }
    
    /// 从类名推断 Kafka topic
    /// 例如：UserEventConsumer -> user-events
    ///      OrderEventConsumer -> order-events
    fn infer_kafka_topic_from_class_name(&self, class_name: &str) -> Option<String> {
        // 移除常见的后缀
        let name = class_name
            .trim_end_matches("Consumer")
            .trim_end_matches("Listener")
            .trim_end_matches("Handler");
        
        if name.is_empty() || name == class_name {
            return None;
        }
        
        // 将驼峰命名转换为短横线分隔
        // 例如：UserEvent -> user-event
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            if i > 0 && ch.is_uppercase() {
                result.push('-');
            }
            result.push(ch.to_lowercase().next().unwrap());
        }
        
        // 添加 -s 后缀（复数形式）
        if !result.ends_with('s') {
            result.push('s');
        }
        
        Some(result)
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
    fn extract_return_type(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Option<String> {
        let mut cursor = method_node.walk();
        let mut simple_type = None;
        
        for child in method_node.children(&mut cursor) {
            // 查找返回类型节点
            if child.kind() == "void_type" {
                return Some("void".to_string());
            } else if child.kind() == "type_identifier" || child.kind() == "integral_type" {
                if let Some(text) = source.get(child.byte_range()) {
                    simple_type = Some(text.to_string());
                    break;
                }
            } else if child.kind() == "generic_type" {
                // 处理泛型类型，如 List<User>
                if let Some(text) = source.get(child.byte_range()) {
                    simple_type = Some(remove_generics(text));
                    break;
                }
            } else if child.kind() == "array_type" {
                // 处理数组类型，如 User[]
                if let Some(text) = source.get(child.byte_range()) {
                    simple_type = Some(remove_generics(text));
                    break;
                }
            } else if child.kind() == "scoped_type_identifier" {
                // 处理完整包名类型，如 com.example.User
                if let Some(text) = source.get(child.byte_range()) {
                    return Some(remove_generics(text));
                }
            }
        }
        
        // 如果找到了简单类型，解析为完整类名
        if let Some(type_name) = simple_type {
            // 对于基本类型和常用类型，保持原样
            if is_primitive_or_common_type(&type_name) {
                return Some(type_name);
            }
            
            // 解析为完整类名
            let import_map = self.build_import_map(source, tree);
            let package_name = self.extract_package_name(source, tree);
            return Some(self.resolve_full_class_name(&type_name, &import_map, &package_name));
        }
        
        None
    }
    
    fn extract_db_operations(&self, source: &str, method_node: &tree_sitter::Node, class_name: &str, return_type: &Option<String>) -> Vec<DbOperation> {
        let mut operations = Vec::new();
        
        // 检查类名是否以Mapper结尾
        if class_name.ends_with("Mapper") {
            // 提取表名：去掉Mapper后缀
            let table_name = class_name.strip_suffix("Mapper").unwrap_or(class_name);
            
            // 根据返回类型判断操作类型
            // void或int为写操作，其他为读操作
            let op_type = match return_type.as_deref() {
                Some("void") | Some("int") => DbOpType::Update, // 写操作统一用Update表示
                _ => DbOpType::Select, // 读操作用Select表示
            };
            
            operations.push(DbOperation {
                operation_type: op_type,
                table: table_name.to_string(),
                line: method_node.start_position().row + 1,
            });
        } else {
            // 保留原有的SQL匹配逻辑（用于非Mapper类）
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
        // 检查第一行是否包含 "Generated" 字样，如果是则跳过解析
        if let Some(first_line) = content.lines().next() {
            if first_line.contains("Generated") {
                // 返回空的解析结果
                return Ok(ParsedFile {
                    file_path: file_path.to_path_buf(),
                    language: "java".to_string(),
                    classes: vec![],
                    functions: vec![],
                    imports: vec![],
                });
            }
        }
        
        let tree = self.parser.lock().unwrap().parse(content, None)
            .ok_or_else(|| ParseError::InvalidFormat {
                message: "Failed to parse Java file".to_string(),
            })?;
        
        // 第一遍：提取类和方法，建立方法返回类型映射
        let (mut classes, method_return_types) = self.extract_classes_with_return_types(content, file_path, &tree);
        
        // 第二遍：使用返回类型映射重新提取方法调用
        for class in &mut classes {
            for method in &mut class.methods {
                // 找到对应的方法节点并重新提取调用
                let root_node = tree.root_node();
                if let Some(method_node) = self.find_method_node(content, &root_node, &class.name, &method.name, &method.full_qualified_name, &tree) {
                    method.calls = self.extract_method_calls_with_return_types(content, &method_node, &tree, &method_return_types, &class.name);
                }
            }
        }
        
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

// Helper methods for JavaParser (not part of LanguageParser trait)
impl JavaParser {
    /// 查找指定方法的节点
    fn find_method_node<'a>(
        &self,
        source: &str,
        node: &tree_sitter::Node<'a>,
        class_name: &str,
        method_name: &str,
        full_qualified_name: &str,
        tree: &tree_sitter::Tree,
    ) -> Option<tree_sitter::Node<'a>> {
        // 递归查找类节点
        if node.kind() == "class_declaration" || node.kind() == "interface_declaration" {
            // 检查是否是目标类
            if let Some(found_class_name) = self.extract_class_name(source, node) {
                let package_name = self.extract_package_name_from_node(source, node);
                let full_class_name = if let Some(pkg) = package_name {
                    format!("{}.{}", pkg, found_class_name)
                } else {
                    found_class_name.clone()
                };
                
                if full_class_name == class_name {
                    // 在类体中查找方法
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "class_body" || child.kind() == "interface_body" {
                            let mut body_cursor = child.walk();
                            for body_child in child.children(&mut body_cursor) {
                                if body_child.kind() == "method_declaration" {
                                    // 检查方法名和签名
                                    if let Some(found_method_name) = self.extract_method_name(source, &body_child) {
                                        if found_method_name == method_name {
                                            // 验证完整签名
                                            let param_types = self.extract_parameter_types(source, &body_child, tree);
                                            let found_signature = if param_types.is_empty() {
                                                format!("{}::{}()", class_name, found_method_name)
                                            } else {
                                                format!("{}::{}({})", class_name, found_method_name, param_types.join(","))
                                            };
                                            
                                            if found_signature == full_qualified_name {
                                                return Some(body_child);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // 递归查找子节点
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = self.find_method_node(source, &child, class_name, method_name, full_qualified_name, tree) {
                return Some(found);
            }
        }
        
        None
    }
    
    /// 从节点向上查找包名
    fn extract_package_name_from_node(&self, source: &str, node: &tree_sitter::Node) -> Option<String> {
        // 向上查找到根节点
        let mut current = Some(*node);
        while let Some(n) = current {
            if n.parent().is_none() {
                // 找到根节点，在其子节点中查找 package_declaration
                let mut cursor = n.walk();
                for child in n.children(&mut cursor) {
                    if child.kind() == "package_declaration" {
                        let mut pkg_cursor = child.walk();
                        for pkg_child in child.children(&mut pkg_cursor) {
                            if pkg_child.kind() == "scoped_identifier" || pkg_child.kind() == "identifier" {
                                if let Some(text) = source.get(pkg_child.byte_range()) {
                                    return Some(text.to_string());
                                }
                            }
                        }
                    }
                }
                break;
            }
            current = n.parent();
        }
        None
    }
    
    /// 提取类名（不含包名）
    fn extract_class_name(&self, source: &str, class_node: &tree_sitter::Node) -> Option<String> {
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    return Some(text.to_string());
                }
            }
        }
        None
    }
    /// 提取类信息，同时建立方法返回类型映射
    fn extract_classes_with_return_types(&self, source: &str, file_path: &Path, tree: &tree_sitter::Tree) -> (Vec<ClassInfo>, MethodReturnTypeMap) {
        let mut classes = Vec::new();
        let mut method_return_types = MethodReturnTypeMap::new();
        let root_node = tree.root_node();
        
        self.walk_node_for_classes_with_return_types(source, file_path, root_node, &mut classes, &mut method_return_types, tree);
        
        (classes, method_return_types)
    }
    
    /// 递归遍历节点查找类声明，同时收集方法返回类型
    fn walk_node_for_classes_with_return_types(
        &self,
        source: &str,
        file_path: &Path,
        node: tree_sitter::Node,
        classes: &mut Vec<ClassInfo>,
        method_return_types: &mut MethodReturnTypeMap,
        tree: &tree_sitter::Tree,
    ) {
        if node.kind() == "class_declaration" || node.kind() == "interface_declaration" {
            if let Some(class_info) = self.extract_class_info_with_return_types(source, file_path, node, tree, method_return_types) {
                classes.push(class_info);
            }
        } else if node.kind() == "enum_declaration" {
            if let Some(enum_info) = self.extract_enum_info_with_return_types(source, file_path, node, tree, method_return_types) {
                classes.push(enum_info);
            }
        }
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node_for_classes_with_return_types(source, file_path, child, classes, method_return_types, tree);
        }
    }
    
    /// 提取类信息，同时收集方法返回类型
    fn extract_class_info_with_return_types(
        &self,
        source: &str,
        file_path: &Path,
        class_node: tree_sitter::Node,
        tree: &tree_sitter::Tree,
        method_return_types: &mut MethodReturnTypeMap,
    ) -> Option<ClassInfo> {
        // 先提取类的基本信息（不包含方法）
        let class_info = self.extract_class_info(source, file_path, class_node, tree, &ApplicationConfig::default())?;
        
        // 然后提取方法，同时收集返回类型
        let methods = self.extract_methods_with_return_types(
            source,
            file_path,
            &class_node,
            &class_info.name,
            tree,
            method_return_types,
        );
        
        Some(ClassInfo {
            name: class_info.name,
            methods,
            line_range: class_info.line_range,
            is_interface: class_info.is_interface,
            implements: class_info.implements,
            extends: class_info.extends,
        })
    }
    
    /// 提取枚举信息，同时收集方法返回类型
    fn extract_enum_info_with_return_types(
        &self,
        source: &str,
        file_path: &Path,
        enum_node: tree_sitter::Node,
        tree: &tree_sitter::Tree,
        method_return_types: &mut MethodReturnTypeMap,
    ) -> Option<ClassInfo> {
        // 提取枚举名称
        let enum_name = self.extract_class_name(source, &enum_node)?;
        
        // 获取包名
        let package_name = self.extract_package_name(source, tree);
        let full_name = if let Some(pkg) = package_name {
            format!("{}.{}", pkg, enum_name)
        } else {
            enum_name.clone()
        };
        
        // 提取枚举常量
        let _enum_constants = self.extract_enum_constants(source, &enum_node);
        
        // 提取方法（从 enum_body -> enum_body_declarations）
        let mut methods = Vec::new();
        
        // 首先找到 enum_body
        if let Some(body_node) = enum_node.child_by_field_name("body") {
            // 然后在 enum_body 中查找 enum_body_declarations
            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
                if child.kind() == "enum_body_declarations" {
                    // 直接使用 enum_body_declarations 节点提取方法
                    methods = self.extract_methods_with_return_types(
                        source,
                        file_path,
                        &child,
                        &full_name,
                        tree,
                        method_return_types,
                    );
                    
                    // 为字段生成 getter/setter
                    self.generate_getters_for_fields(
                        source,
                        file_path,
                        &child,
                        &full_name,
                        &mut methods,
                        method_return_types,
                        tree,
                    );
                    
                    self.generate_setters_for_fields(
                        source,
                        file_path,
                        &child,
                        &full_name,
                        &mut methods,
                        tree,
                    );
                    break;
                }
            }
        }
        
        Some(ClassInfo {
            name: full_name,
            methods,
            line_range: (enum_node.start_position().row + 1, enum_node.end_position().row + 1),
            is_interface: false,
            implements: vec![],
            extends: None,
        })
    }
    
    /// 提取枚举常量
    fn extract_enum_constants(
        &self,
        source: &str,
        enum_node: &tree_sitter::Node,
    ) -> Vec<String> {
        let mut constants = Vec::new();
        
        if let Some(body_node) = enum_node.child_by_field_name("body") {
            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
                if child.kind() == "enum_constant" {
                    // enum_constant 的第一个子节点通常是 identifier
                    let mut const_cursor = child.walk();
                    for const_child in child.children(&mut const_cursor) {
                        if const_child.kind() == "identifier" {
                            let name = &source[const_child.byte_range()];
                            constants.push(name.to_string());
                            break;
                        }
                    }
                }
            }
        }
        
        constants
    }
    
    /// 提取方法，同时收集返回类型
    fn extract_methods_with_return_types(
        &self,
        source: &str,
        file_path: &Path,
        class_node: &tree_sitter::Node,
        class_name: &str,
        tree: &tree_sitter::Tree,
        method_return_types: &mut MethodReturnTypeMap,
    ) -> Vec<MethodInfo> {
        let mut methods = Vec::new();
        
        // 获取简单类名（用于Mapper判断）
        let simple_class_name = class_name.split('.').last().unwrap_or(class_name);
        
        // 提取 FeignClient 注解
        let feign_client_info = self.extract_feign_client_annotation(source, class_node);
        
        // 提取类级别的 @RequestMapping
        let class_request_mapping = self.extract_class_level_request_mapping(source, class_node);
        
        // 提取类级别的 @KafkaListener
        let class_kafka_topic = self.extract_class_kafka_listener(source, class_node);
        
        // 提取实现的接口列表
        let class_implements = self.extract_implements_interfaces(source, class_node, tree);
        
        // 加载应用配置
        let app_config = self.load_application_config(file_path);
        
        // 确定要处理的节点
        // 如果传入的就是 enum_body_declarations，直接处理它
        // 否则查找 class_body 或 interface_body
        let nodes_to_process: Vec<tree_sitter::Node> = if class_node.kind() == "enum_body_declarations" {
            vec![*class_node]
        } else {
            let mut cursor = class_node.walk();
            class_node.children(&mut cursor)
                .filter(|child| {
                    child.kind() == "class_body" 
                    || child.kind() == "interface_body"
                })
                .collect()
        };
        
        for body_node in nodes_to_process {
            let mut body_cursor = body_node.walk();
            for body_child in body_node.children(&mut body_cursor) {
                // 处理普通方法声明和接口方法声明
                if body_child.kind() == "method_declaration" {
                    // 先收集返回类型
                    if let Some(return_type) = self.extract_return_type(source, &body_child, tree) {
                        // 获取方法名
                        if let Some(method_name) = self.extract_method_name(source, &body_child) {
                                // 获取参数类型
                                let param_types = self.extract_parameter_types(source, &body_child, tree);
                                
                                // 构建方法签名
                                let method_signature = if param_types.is_empty() {
                                    format!("{}::{}()", class_name, method_name)
                                } else {
                                    format!("{}::{}({})", class_name, method_name, param_types.join(","))
                                };
                                
                                // 存储返回类型
                                method_return_types.insert(method_signature, return_type);
                            }
                        }
                        
                        // 然后提取完整的方法信息
                        if let Some(mut method_info) = self.extract_method_info_with_return_types(
                            source,
                            file_path,
                            body_child,
                            class_name,
                            simple_class_name,
                            tree,
                            &feign_client_info,
                            &class_request_mapping,
                            &app_config,
                            method_return_types,
                        ) {
                            // 后处理：添加类级别的 Kafka 注解
                            if method_info.kafka_operations.is_empty() {
                                if let Some(topic) = &class_kafka_topic {
                                    method_info.kafka_operations.push(KafkaOperation {
                                        operation_type: KafkaOpType::Consume,
                                        topic: topic.clone(),
                                        line: method_info.line_range.0,
                                    });
                                }
                            }
                            
                            // 后处理：为 EventMessageDataConsumer 的 doConsumer 方法添加 Kafka 消费关系
                            if method_info.kafka_operations.is_empty() && method_info.name == "doConsumer" {
                                for interface in &class_implements {
                                    if interface.contains("EventMessageDataConsumer") {
                                        if let Some(topic) = self.infer_kafka_topic_from_class_name(simple_class_name) {
                                            method_info.kafka_operations.push(KafkaOperation {
                                                operation_type: KafkaOpType::Consume,
                                                topic,
                                                line: method_info.line_range.0,
                                            });
                                        }
                                        break;
                                    }
                                }
                            }
                            
                            methods.push(method_info);
                        }
                    }
                }
        }
        
        // 自动生成 getter 方法（如果字段存在但 getter 不存在）
        self.generate_getters_for_fields(
            source,
            file_path,
            class_node,
            class_name,
            &mut methods,
            method_return_types,
            tree,
        );
        
        // 自动生成 setter 方法（如果字段存在但 setter 不存在）
        self.generate_setters_for_fields(
            source,
            file_path,
            class_node,
            class_name,
            &mut methods,
            tree,
        );
        
        methods
    }
    
    /// 为类字段自动生成 getter 方法（如果不存在）
    fn generate_getters_for_fields(
        &self,
        source: &str,
        file_path: &Path,
        class_node: &tree_sitter::Node,
        class_name: &str,
        methods: &mut Vec<MethodInfo>,
        method_return_types: &mut MethodReturnTypeMap,
        tree: &tree_sitter::Tree,
    ) {
        // 提取类的所有字段
        let fields = self.extract_class_fields(source, class_node);
        
        // 获取导入映射和包名，用于解析完整类名
        let import_map = self.build_import_map(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        // 为每个字段检查是否存在对应的 getter
        for (field_name, simple_field_type) in fields {
            // 解析字段类型为完整类名
            let field_type = if is_primitive_or_common_type(&simple_field_type) {
                simple_field_type
            } else {
                self.resolve_full_class_name(&simple_field_type, &import_map, &package_name)
            };
            
            // 生成 getter 方法名：foo -> getFoo
            let getter_name = format!("get{}{}", 
                field_name.chars().next().unwrap().to_uppercase(),
                &field_name[1..]
            );
            
            // 检查是否已经存在这个 getter 方法
            let getter_exists = methods.iter().any(|m| m.name == getter_name);
            
            if !getter_exists {
                // 生成 getter 方法
                let method_signature = format!("{}::{}()", class_name, getter_name);
                let line = class_node.start_position().row + 1;
                
                // 添加到方法列表
                methods.push(MethodInfo {
                    name: getter_name.clone(),
                    full_qualified_name: method_signature.clone(),
                    file_path: file_path.to_path_buf(),
                    line_range: (line, line),
                    calls: vec![],
                    http_annotations: None,
                    kafka_operations: vec![],
                    db_operations: vec![],
                    redis_operations: vec![],
                    return_type: Some(field_type.clone()),
                });
                
                // 添加返回类型映射
                method_return_types.insert(method_signature, field_type);
            }
        }
    }
    
    /// 为类字段自动生成 setter 方法（如果不存在）
    fn generate_setters_for_fields(
        &self,
        source: &str,
        file_path: &Path,
        class_node: &tree_sitter::Node,
        class_name: &str,
        methods: &mut Vec<MethodInfo>,
        tree: &tree_sitter::Tree,
    ) {
        // 提取类的所有字段
        let fields = self.extract_class_fields(source, class_node);
        
        // 获取导入映射和包名，用于解析完整类名
        let import_map = self.build_import_map(source, tree);
        let package_name = self.extract_package_name(source, tree);
        
        // 为每个字段检查是否存在对应的 setter
        for (field_name, simple_field_type) in fields {
            // 解析字段类型为完整类名
            let field_type = if is_primitive_or_common_type(&simple_field_type) {
                simple_field_type
            } else {
                self.resolve_full_class_name(&simple_field_type, &import_map, &package_name)
            };
            
            // 生成 setter 方法名：foo -> setFoo
            let setter_name = format!("set{}{}", 
                field_name.chars().next().unwrap().to_uppercase(),
                &field_name[1..]
            );
            
            // 检查是否已经存在这个 setter 方法
            let setter_exists = methods.iter().any(|m| m.name == setter_name);
            
            if !setter_exists {
                // 生成 setter 方法签名：setFoo(FieldType)
                let method_signature = format!("{}::{}({})", class_name, setter_name, field_type);
                let line = class_node.start_position().row + 1;
                
                // 添加到方法列表
                methods.push(MethodInfo {
                    name: setter_name.clone(),
                    full_qualified_name: method_signature.clone(),
                    file_path: file_path.to_path_buf(),
                    line_range: (line, line),
                    calls: vec![],
                    http_annotations: None,
                    kafka_operations: vec![],
                    db_operations: vec![],
                    redis_operations: vec![],
                    return_type: Some("void".to_string()),
                });
            }
        }
    }
    
    /// 提取类的所有字段（字段名 -> 字段类型）
    fn extract_class_fields(
        &self,
        source: &str,
        class_node: &tree_sitter::Node,
    ) -> Vec<(String, String)> {
        let mut fields = Vec::new();
        
        // 确定要处理的节点
        // 如果传入的就是 enum_body_declarations，直接处理它
        // 否则查找 class_body
        let nodes_to_process: Vec<tree_sitter::Node> = if class_node.kind() == "enum_body_declarations" {
            vec![*class_node]
        } else {
            let mut cursor = class_node.walk();
            class_node.children(&mut cursor)
                .filter(|child| child.kind() == "class_body")
                .collect()
        };
        
        for body_node in nodes_to_process {
            let mut body_cursor = body_node.walk();
            for body_child in body_node.children(&mut body_cursor) {
                // 查找字段声明
                if body_child.kind() == "field_declaration" {
                    // 提取字段类型和名称
                    if let Some((field_type, field_names)) = self.extract_field_declaration(source, &body_child) {
                        for field_name in field_names {
                            fields.push((field_name, field_type.clone()));
                        }
                    }
                }
            }
        }
        
        fields
    }
    
    /// 提取字段声明的类型和名称
    fn extract_field_declaration(
        &self,
        source: &str,
        field_node: &tree_sitter::Node,
    ) -> Option<(String, Vec<String>)> {
        let mut field_type = None;
        let mut field_names = Vec::new();
        
        let mut cursor = field_node.walk();
        for child in field_node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" | "integral_type" | "floating_point_type" | "boolean_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        field_type = Some(remove_generics(text));
                    }
                }
                "generic_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        field_type = Some(remove_generics(text));
                    }
                }
                "array_type" => {
                    if let Some(text) = source.get(child.byte_range()) {
                        field_type = Some(remove_generics(text));
                    }
                }
                "variable_declarator" => {
                    // 提取变量名
                    let mut var_cursor = child.walk();
                    for var_child in child.children(&mut var_cursor) {
                        if var_child.kind() == "identifier" {
                            if let Some(text) = source.get(var_child.byte_range()) {
                                field_names.push(text.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        if let Some(ftype) = field_type {
            if !field_names.is_empty() {
                return Some((ftype, field_names));
            }
        }
        
        None
    }
    
    /// 提取方法名
    fn extract_method_name(&self, source: &str, method_node: &tree_sitter::Node) -> Option<String> {
        let mut cursor = method_node.walk();
        for child in method_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Some(text) = source.get(child.byte_range()) {
                    return Some(text.to_string());
                }
            }
        }
        None
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
                public User getUser(String id, int age) {
                    userService.updateUser("123", 25, true);
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
    fn test_extract_method_with_parameters() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class UserService {
    public User getUser(String id) {
        return null;
    }
    
    public void updateUser(String id, int age, boolean active) {
        // update logic
    }
    
    public List<User> findUsers(List<String> ids, Map<String, Object> filters) {
        return null;
    }
    
    public void processArray(String[] names, int[][] matrix) {
        // process
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("UserService.java")).unwrap();
        
        assert_eq!(result.classes.len(), 1);
        let class = &result.classes[0];
        assert_eq!(class.methods.len(), 4);
        
        // 测试单参数方法
        let method1 = &class.methods[0];
        assert_eq!(method1.name, "getUser");
        assert_eq!(method1.full_qualified_name, "com.example.UserService::getUser(String)");
        
        // 测试多参数方法
        let method2 = &class.methods[1];
        assert_eq!(method2.name, "updateUser");
        assert_eq!(method2.full_qualified_name, "com.example.UserService::updateUser(String,Integer,Boolean)");
        
        // 测试泛型参数方法（泛型被移除）
        let method3 = &class.methods[2];
        assert_eq!(method3.name, "findUsers");
        assert_eq!(method3.full_qualified_name, "com.example.UserService::findUsers(List,Map)");
        
        // 测试数组参数方法
        let method4 = &class.methods[3];
        assert_eq!(method4.name, "processArray");
        assert_eq!(method4.full_qualified_name, "com.example.UserService::processArray(String[],int[][])");
    }
    
    #[test]
    fn test_extract_method_calls_with_arguments() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class UserController {
    private UserService userService;
    
    public void testMethod() {
        // 字面量参数
        userService.updateUser("123", 25, true);
        
        // 变量参数
        String userId = "456";
        int age = 30;
        userService.updateUser(userId, age, false);
        
        // 混合参数
        userService.processData("test", 100, userId);
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("UserController.java")).unwrap();
        
        assert_eq!(result.classes.len(), 1);
        let class = &result.classes[0];
        // UserController 有 testMethod 和自动生成的 getUserService getter
        let method = class.methods.iter()
            .find(|m| m.name == "testMethod")
            .expect("Should find testMethod");
        
        assert_eq!(method.calls.len(), 3);
        
        // 第一个调用：字面量参数
        assert_eq!(method.calls[0].target, "com.example.UserService::updateUser(String,Integer,Boolean)");
        
        // 第二个调用：变量参数
        assert_eq!(method.calls[1].target, "com.example.UserService::updateUser(String,Integer,Boolean)");
        
        // 第三个调用：混合参数
        assert_eq!(method.calls[2].target, "com.example.UserService::processData(String,Integer,String)");
    }
    
    #[test]
    fn test_extract_method_calls_with_method_parameters() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class TestService {
    private UserRepository userRepository;
    
    public void processUser(String userId, int age, boolean active) {
        // userId, age, active 是方法参数
        userRepository.updateUser(userId, age, active);
        
        // 使用字段
        userRepository.findUser(userId);
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("TestService.java")).unwrap();
        
        assert_eq!(result.classes.len(), 1);
        let class = &result.classes[0];
        // TestService 有 processUser 和自动生成的 getUserRepository getter
        let method = class.methods.iter()
            .find(|m| m.name == "processUser")
            .expect("Should find processUser method");
        
        assert_eq!(method.calls.len(), 2);
        
        // 第一个调用：使用方法参数
        assert_eq!(method.calls[0].target, "com.example.UserRepository::updateUser(String,Integer,Boolean)");
        
        // 第二个调用：使用方法参数
        assert_eq!(method.calls[1].target, "com.example.UserRepository::findUser(String)");
    }
    
    #[test]
    fn test_extract_method_calls_with_nested_calls() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class UserRepository {
    public User findUser(String id) {
        return null;
    }
}

public class User {
    public String getName() {
        return null;
    }
}

public class DataProcessor {
    public void process(User user) {
        // process
    }
    
    public void process(String name) {
        // process
    }
}

public class TestService {
    private UserRepository userRepository;
    private DataProcessor processor;
    
    public void processData() {
        // 嵌套方法调用：userRepository.findUser() 返回 User，应该推断为 User 类型
        processor.process(userRepository.findUser("123"));
        
        // 链式调用：findUser() 返回 User，getName() 返回 String
        processor.process(userRepository.findUser("456").getName());
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("TestService.java")).unwrap();
        
        assert_eq!(result.classes.len(), 4);
        
        // 找到 TestService 类
        let test_service = result.classes.iter()
            .find(|c| c.name == "com.example.TestService")
            .expect("Should find TestService class");
        
        // TestService 有 processData 方法，还有自动生成的 getter
        let method = test_service.methods.iter()
            .find(|m| m.name == "processData")
            .expect("Should find processData method");
        
        // 打印调用以便调试
        for (i, call) in method.calls.iter().enumerate() {
            eprintln!("Call {}: {}", i, call.target);
        }
        
        // 验证方法调用
        assert!(method.calls.len() >= 2, "Should have at least 2 method calls");
        
        // 第一个调用应该推断出 User 类型（从 findUser 的返回类型）
        let first_process_call = method.calls.iter()
            .find(|c| c.target.starts_with("com.example.DataProcessor::process"))
            .expect("Should find process call");
        
        // 验证：应该是 process(com.example.User) 而不是 process(Object)
        assert_eq!(
            first_process_call.target,
            "com.example.DataProcessor::process(com.example.User)",
            "Should infer User type from findUser() return type"
        );
    }
    
    #[test]
    fn test_extract_method_calls_with_nested_calls_with_getter() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

import com.user.User;

public class UserRepository {
    private User user;
}

public class User {
    public String getName() {
        return null;
    }
}

public class DataProcessor {
    public void process(User user) {
        // process
    }
    
    public void process(String name) {
        // process
    }
}

public class TestService {
    private DataProcessor processor;
    
    public void processData(UserRepository userRepository) {
        // 嵌套方法调用：userRepository.getUser() 返回 User，应该推断为 User 类型
        processor.process(userRepository.getUser());
        
        // 链式调用：findUser() 返回 User，getName() 返回 String
        processor.process(userRepository.getUser().getName());
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("TestService.java")).unwrap();
        
        assert_eq!(result.classes.len(), 4);
        
        // 找到 TestService 类
        let test_service = result.classes.iter()
            .find(|c| c.name == "com.example.TestService")
            .expect("Should find TestService class");
        
        // TestService 有 processData 方法，还有自动生成的 getProcessor getter
        assert!(test_service.methods.len() >= 1, "Should have at least 1 method");
        
        let method = test_service.methods.iter()
            .find(|m| m.name == "processData")
            .expect("Should find processData method");
        
        // 打印调用以便调试
        for (i, call) in method.calls.iter().enumerate() {
            eprintln!("Call {}: {}", i, call.target);
        }
        
        // 验证方法调用
        assert!(method.calls.len() >= 2, "Should have at least 2 method calls");
        
        // 第一个调用应该推断出 User 类型（从 findUser 的返回类型）
        let first_process_call = method.calls.iter()
            .find(|c| c.target.starts_with("com.example.DataProcessor::process"))
            .expect("Should find process call");
        
        // 验证：应该是 process(User) 而不是 process(Object)
        assert_eq!(
            first_process_call.target,
            "com.example.DataProcessor::process(com.user.User)",
            "Should infer User type from getUser() return type"
        );
    }
    
    #[test]
    fn test_extract_method_calls_with_nested_calls_this_with_getter() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class UserRepository {
    private User user;
}

public class User {
    public String getName() {
        return null;
    }
}

public class TestService {
    public void process(User user, String name) {
        // process
    }
    
    public void process(String name) {
        // process
    }
    
    public void processData(UserRepository userRepository) {
        // 嵌套方法调用：userRepository.getUser() 返回 User，应该推断为 User 类型
        process(userRepository.getUser(), userRepository.getUser().getName());
        
        // 链式调用：findUser() 返回 User，getName() 返回 String
        process(userRepository.getUser().getName());
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("TestService.java")).unwrap();
        
        assert_eq!(result.classes.len(), 3);
        
        // 找到 TestService 类
        let test_service = result.classes.iter()
            .find(|c| c.name == "com.example.TestService")
            .expect("Should find TestService class");
        
        // TestService 有 processData 方法，还有自动生成的 getProcessor getter
        assert!(test_service.methods.len() >= 1, "Should have at least 1 method");
        
        let method = test_service.methods.iter()
            .find(|m| m.name == "processData")
            .expect("Should find processData method");
        
        // 打印调用以便调试
        for (i, call) in method.calls.iter().enumerate() {
            eprintln!("Call {}: {}", i, call.target);
        }
        
        // 验证方法调用
        assert!(method.calls.len() >= 2, "Should have at least 2 method calls");
        
        // 第一个调用应该推断出 User 类型（从 findUser 的返回类型）
        let first_process_call = method.calls.iter()
            .find(|c| c.target.starts_with("com.example.TestService::process"))
            .expect("Should find process call");
        
        // 验证：应该是 process(com.example.User,String) 而不是 process(Object,String)
        assert_eq!(
            first_process_call.target,
            "com.example.TestService::process(com.example.User,String)",
            "Should infer User type from getUser() return type"
        );
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
        
        // 检查是否包含 add 方法调用（带参数类型）
        assert!(call_names.iter().any(|name| name.contains("add")));
        // 检查是否包含 println 方法调用
        assert!(call_names.iter().any(|name| name.contains("println")));
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
        // TestController 有 testMethod 和自动生成的 getEquipmentManageExe getter
        let method = result.classes[0].methods.iter()
            .find(|m| m.name == "testMethod")
            .expect("Should find testMethod");
        
        assert_eq!(method.calls.len(), 1);
        // 应该解析为完整的类名::方法名(参数类型)格式
        assert_eq!(method.calls[0].target, "com.hualala.shop.equipment.EquipmentManageExe::listExecuteSchedule(String)");
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
        // TestController 只有 testMethod，没有字段所以没有 getter
        assert_eq!(result.classes[0].methods.len(), 1);
        
        let method = &result.classes[0].methods[0];
        assert_eq!(method.calls.len(), 1);
        
        // 应该解析为完整的导入类名::方法名格式
        assert_eq!(
            method.calls[0].target,
            "com.hualala.shop.equipment.EquipmentManageExe::listExecuteSchedule(String)"
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
        // TestController 有 testMethod 和自动生成的 getFieldExe getter
        let method = result.classes[0].methods.iter()
            .find(|m| m.name == "testMethod")
            .expect("Should find testMethod");
        
        assert_eq!(method.calls.len(), 2, "Should have 2 method calls");
        
        // 两个调用都应该解析为完整的类名::方法名格式
        for call in &method.calls {
            assert_eq!(
                call.target,
                "com.hualala.shop.equipment.EquipmentManageExe::listExecuteSchedule(String)"
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
        // TestService 有 testMethod, localMethod 和自动生成的 getUserService getter
        assert!(result.classes[0].methods.len() >= 2, "Should have at least 2 methods");
        
        let test_method = result.classes[0].methods.iter()
            .find(|m| m.name == "testMethod")
            .expect("Should find testMethod");
        
        let call_names: Vec<&str> = test_method.calls.iter()
            .map(|c| c.target.as_str())
            .collect();
        
        // Verify all method calls are captured correctly (now with parameter types)
        assert!(call_names.iter().any(|name| name.contains("localMethod")), "Should find localMethod");
        assert!(call_names.iter().any(|name| name.contains("findUser")), "Should find findUser");
        assert!(call_names.iter().any(|name| name.contains("getRepository")), "Should find getRepository");
        assert!(call_names.iter().any(|name| name.contains("save")), "Should find save");
        assert!(call_names.iter().any(|name| name.contains("println")), "Should find println");
        assert!(call_names.iter().any(|name| name.contains("updateUser")), "Should find updateUser");
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
        // TestStaticMethod 有 testMethod 和自动生成的 getUserService getter
        let method = result.classes[0].methods.iter()
            .find(|m| m.name == "testMethod")
            .expect("Should find testMethod");
        
        let call_targets: Vec<&str> = method.calls.iter()
            .map(|c| c.target.as_str())
            .collect();
        
        // 验证静态方法调用被正确识别为 ClassName::methodName(参数类型)
        assert!(
            call_targets.iter().any(|t| t.contains("StringUtils::isEmpty")),
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
    
    #[test]
    fn test_extract_mapper_db_operations() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example.mapper;
            
            import java.util.List;
            
            public interface UserMapper {
                // 写操作：返回void
                void insertUser(User user);
                
                // 写操作：返回int
                int updateUser(User user);
                
                // 读操作：返回对象
                User selectUserById(Long id);
                
                // 读操作：返回List
                List<User> selectAllUsers();
                
                // 读操作：返回数组
                User[] selectUserArray();
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("UserMapper.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 5);
        
        // 检查 insertUser - 写操作
        let insert_method = &result.classes[0].methods[0];
        assert_eq!(insert_method.name, "insertUser");
        assert_eq!(insert_method.db_operations.len(), 1);
        assert_eq!(insert_method.db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(insert_method.db_operations[0].table, "User");
        
        // 检查 updateUser - 写操作
        let update_method = &result.classes[0].methods[1];
        assert_eq!(update_method.name, "updateUser");
        assert_eq!(update_method.db_operations.len(), 1);
        assert_eq!(update_method.db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(update_method.db_operations[0].table, "User");
        
        // 检查 selectUserById - 读操作
        let select_method = &result.classes[0].methods[2];
        assert_eq!(select_method.name, "selectUserById");
        assert_eq!(select_method.db_operations.len(), 1);
        assert_eq!(select_method.db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(select_method.db_operations[0].table, "User");
        
        // 检查 selectAllUsers - 读操作
        let select_all_method = &result.classes[0].methods[3];
        assert_eq!(select_all_method.name, "selectAllUsers");
        assert_eq!(select_all_method.db_operations.len(), 1);
        assert_eq!(select_all_method.db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(select_all_method.db_operations[0].table, "User");
        
        // 检查 selectUserArray - 读操作
        let select_array_method = &result.classes[0].methods[4];
        assert_eq!(select_array_method.name, "selectUserArray");
        assert_eq!(select_array_method.db_operations.len(), 1);
        assert_eq!(select_array_method.db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(select_array_method.db_operations[0].table, "User");
    }
}


    #[test]
    fn test_extract_mapper_db_operations() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example.mapper;
            
            public interface UserMapper {
                // 读操作 - 返回User对象
                User selectById(int id);
                
                // 读操作 - 返回List
                List<User> selectAll();
                
                // 写操作 - 返回void
                void insert(User user);
                
                // 写操作 - 返回int
                int update(User user);
                
                // 写操作 - 返回int
                int deleteById(int id);
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("UserMapper.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 5);
        
        // 检查 selectById - 读操作
        let select_by_id = &result.classes[0].methods[0];
        assert_eq!(select_by_id.db_operations.len(), 1);
        assert_eq!(select_by_id.db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(select_by_id.db_operations[0].table, "User");
        
        // 检查 selectAll - 读操作
        let select_all = &result.classes[0].methods[1];
        assert_eq!(select_all.db_operations.len(), 1);
        assert_eq!(select_all.db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(select_all.db_operations[0].table, "User");
        
        // 检查 insert - 写操作（void返回）
        let insert = &result.classes[0].methods[2];
        assert_eq!(insert.db_operations.len(), 1);
        assert_eq!(insert.db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(insert.db_operations[0].table, "User");
        
        // 检查 update - 写操作（int返回）
        let update = &result.classes[0].methods[3];
        assert_eq!(update.db_operations.len(), 1);
        assert_eq!(update.db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(update.db_operations[0].table, "User");
        
        // 检查 deleteById - 写操作（int返回）
        let delete = &result.classes[0].methods[4];
        assert_eq!(delete.db_operations.len(), 1);
        assert_eq!(delete.db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(delete.db_operations[0].table, "User");
    }
    
    #[test]
    fn test_non_mapper_class_no_auto_db_operations() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example.service;
            
            public class UserService {
                // 非Mapper类的方法不应该自动识别为数据库操作
                User findById(int id) {
                    return null;
                }
                
                void save(User user) {
                    // do something
                }
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("UserService.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 2);
        
        // 非Mapper类的方法不应该有数据库操作
        let find_method = &result.classes[0].methods[0];
        assert_eq!(find_method.db_operations.len(), 0);
        
        let save_method = &result.classes[0].methods[1];
        assert_eq!(save_method.db_operations.len(), 0);
    }


    #[test]
    fn test_extract_mapper_with_full_package_name() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
            package com.example.dao;
            
            import java.util.List;
            
            public interface OrderMapper {
                // 读操作
                Order findById(Long id);
                
                // 读操作 - 返回泛型List
                List<Order> findAll();
                
                // 写操作 - 返回void
                void insertOrder(Order order);
                
                // 写操作 - 返回int（影响行数）
                int updateOrder(Order order);
            }
        "#;
        
        let result = parser.parse_file(source, Path::new("OrderMapper.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        
        // 验证类名包含完整包名
        assert_eq!(result.classes[0].name, "com.example.dao.OrderMapper");
        
        // 验证所有方法都有数据库操作
        assert_eq!(result.classes[0].methods.len(), 4);
        
        for method in &result.classes[0].methods {
            assert_eq!(method.db_operations.len(), 1, "Method {} should have 1 db operation", method.name);
            assert_eq!(method.db_operations[0].table, "Order", "Table name should be Order for method {}", method.name);
        }
        
        // 验证读写操作类型
        assert_eq!(result.classes[0].methods[0].db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(result.classes[0].methods[1].db_operations[0].operation_type, DbOpType::Select);
        assert_eq!(result.classes[0].methods[2].db_operations[0].operation_type, DbOpType::Update);
        assert_eq!(result.classes[0].methods[3].db_operations[0].operation_type, DbOpType::Update);
    }

    #[test]
    fn test_extract_self_method_calls() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class UserService {
    private String name;
    
    public void processUser(String userId) {
        // 直接调用自身方法（无对象名）
        validateUser(userId);
        
        // 使用 this 调用自身方法
        this.saveUser(userId);
        
        // 调用另一个自身方法
        notifyUser(userId);
    }
    
    private void validateUser(String userId) {
        // validate
    }
    
    private void saveUser(String userId) {
        // save
    }
    
    private void notifyUser(String userId) {
        // notify
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("UserService.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        
        let class = &result.classes[0];
        assert_eq!(class.name, "com.example.UserService");
        
        // 找到 processUser 方法
        let process_method = class.methods.iter()
            .find(|m| m.name == "processUser")
            .expect("Should find processUser method");
        
        // 打印调用以便调试
        for (i, call) in process_method.calls.iter().enumerate() {
            eprintln!("Call {}: {}", i, call.target);
        }
        
        // 应该有3个方法调用
        assert_eq!(process_method.calls.len(), 3, "Should have 3 method calls");
        
        // 验证所有调用都包含完整的类名
        assert_eq!(
            process_method.calls[0].target,
            "com.example.UserService::validateUser(String)",
            "Direct call should include class name"
        );
        
        assert_eq!(
            process_method.calls[1].target,
            "com.example.UserService::saveUser(String)",
            "this.method() call should include class name"
        );
        
        assert_eq!(
            process_method.calls[2].target,
            "com.example.UserService::notifyUser(String)",
            "Another direct call should include class name"
        );
    }

    #[test]
    fn test_extract_method_calls_with_nested_calls_with_getter() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class User {
    private String name;
    private int age;
    
    // 注意：没有 getName() 和 getAge() 方法
}

public class UserRepository {
    public User findUser(String id) {
        return null;
    }
}

public class DataProcessor {
    public void process(String name) {
        // process
    }
    
    public void processAge(int age) {
        // process
    }
}

public class TestService {
    private UserRepository userRepository;
    private DataProcessor processor;
    
    public void processData() {
        // 嵌套调用：findUser() 返回 User，然后调用 getName()
        // 即使 User 类没有定义 getName()，也应该能推断出它存在
        processor.process(userRepository.findUser("123").getName());
        
        // 同样，getAge() 也应该被推断出来
        processor.processAge(userRepository.findUser("456").getAge());
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("TestService.java")).unwrap();
        
        // 验证 User 类应该有自动生成的 getter 方法
        let user_class = result.classes.iter()
            .find(|c| c.name == "com.example.User")
            .expect("Should find User class");
        
        // 打印方法以便调试
        eprintln!("User class methods:");
        for method in &user_class.methods {
            eprintln!("  - {} -> {}", method.name, method.full_qualified_name);
        }
        
        // 应该有 getName() 和 getAge() 方法（自动生成）
        assert!(
            user_class.methods.iter().any(|m| m.name == "getName"),
            "Should have auto-generated getName() method"
        );
        
        assert!(
            user_class.methods.iter().any(|m| m.name == "getAge"),
            "Should have auto-generated getAge() method"
        );
        
        // 找到 TestService 类
        let test_service = result.classes.iter()
            .find(|c| c.name == "com.example.TestService")
            .expect("Should find TestService class");
        
        let method = &test_service.methods[0];
        
        // 打印调用以便调试
        eprintln!("TestService.processData() calls:");
        for (i, call) in method.calls.iter().enumerate() {
            eprintln!("  Call {}: {}", i, call.target);
        }
        
        // 验证方法调用能够正确推断类型
        // 第一个 process 调用应该推断出 String 类型（从 getName() 返回）
        let first_process = method.calls.iter()
            .find(|c| c.target.starts_with("com.example.DataProcessor::process("))
            .expect("Should find process call");
        
        assert_eq!(
            first_process.target,
            "com.example.DataProcessor::process(String)",
            "Should infer String type from getName() return type"
        );
        
        // 第二个 processAge 调用应该推断出 int 类型（从 getAge() 返回）
        let process_age = method.calls.iter()
            .find(|c| c.target.starts_with("com.example.DataProcessor::processAge("))
            .expect("Should find processAge call");
        
        assert_eq!(
            process_age.target,
            "com.example.DataProcessor::processAge(Integer)",
            "Should infer int type from getAge() return type"
        );
    }


    #[test]
    fn test_auto_generated_getters_and_setters() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class User {
    private String name;
    private int age;
    private boolean active;
}
        "#;
        
        let result = parser.parse_file(source, Path::new("User.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        
        let user_class = &result.classes[0];
        assert_eq!(user_class.name, "com.example.User");
        
        // 应该有 3 个字段，所以应该有 6 个方法（3个getter + 3个setter）
        assert_eq!(user_class.methods.len(), 6, "Should have 3 getters and 3 setters");
        
        // 验证 getter 方法
        let get_name = user_class.methods.iter()
            .find(|m| m.name == "getName")
            .expect("Should have getName() method");
        assert_eq!(get_name.full_qualified_name, "com.example.User::getName()");
        assert_eq!(get_name.return_type, Some("String".to_string()));
        
        let get_age = user_class.methods.iter()
            .find(|m| m.name == "getAge")
            .expect("Should have getAge() method");
        assert_eq!(get_age.full_qualified_name, "com.example.User::getAge()");
        assert_eq!(get_age.return_type, Some("int".to_string()));
        
        let get_active = user_class.methods.iter()
            .find(|m| m.name == "getActive")
            .expect("Should have getActive() method");
        assert_eq!(get_active.full_qualified_name, "com.example.User::getActive()");
        assert_eq!(get_active.return_type, Some("boolean".to_string()));
        
        // 验证 setter 方法
        let set_name = user_class.methods.iter()
            .find(|m| m.name == "setName")
            .expect("Should have setName() method");
        assert_eq!(set_name.full_qualified_name, "com.example.User::setName(String)");
        assert_eq!(set_name.return_type, Some("void".to_string()));
        
        let set_age = user_class.methods.iter()
            .find(|m| m.name == "setAge")
            .expect("Should have setAge() method");
        assert_eq!(set_age.full_qualified_name, "com.example.User::setAge(int)");
        assert_eq!(set_age.return_type, Some("void".to_string()));
        
        let set_active = user_class.methods.iter()
            .find(|m| m.name == "setActive")
            .expect("Should have setActive() method");
        assert_eq!(set_active.full_qualified_name, "com.example.User::setActive(boolean)");
        assert_eq!(set_active.return_type, Some("void".to_string()));
    }

    #[test]
    fn test_no_duplicate_getters_and_setters() {
        let parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class User {
    private String name;
    
    // 已经定义了 getter，不应该重复生成
    public String getName() {
        return name;
    }
    
    // 已经定义了 setter，不应该重复生成
    public void setName(String name) {
        this.name = name;
    }
}
        "#;
        
        let result = parser.parse_file(source, Path::new("User.java")).unwrap();
        assert_eq!(result.classes.len(), 1);
        
        let user_class = &result.classes[0];
        
        // 应该只有 2 个方法（已定义的 getter 和 setter），不应该重复生成
        assert_eq!(user_class.methods.len(), 2, "Should not duplicate existing methods");
        
        // 验证方法名
        let method_names: Vec<&str> = user_class.methods.iter()
            .map(|m| m.name.as_str())
            .collect();
        
        assert!(method_names.contains(&"getName"));
        assert!(method_names.contains(&"setName"));
        
        // 确保没有重复
        assert_eq!(
            user_class.methods.iter().filter(|m| m.name == "getName").count(),
            1,
            "Should have exactly one getName method"
        );
        assert_eq!(
            user_class.methods.iter().filter(|m| m.name == "setName").count(),
            1,
            "Should have exactly one setName method"
        );
    }
