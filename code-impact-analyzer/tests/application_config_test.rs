use code_impact_analyzer::language_parser::LanguageParser;
use code_impact_analyzer::java_parser::JavaParser;
use code_impact_analyzer::types::HttpMethod;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_http_endpoint_with_application_config() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    
    // 创建 start/src/main/resources 目录
    let resources_dir = project_root
        .join("start")
        .join("src")
        .join("main")
        .join("resources");
    fs::create_dir_all(&resources_dir).unwrap();
    
    // 创建 application.yml 文件
    let app_yml_path = resources_dir.join("application.yml");
    let mut app_yml_file = fs::File::create(&app_yml_path).unwrap();
    writeln!(app_yml_file, "server:").unwrap();
    writeln!(app_yml_file, "  servlet:").unwrap();
    writeln!(app_yml_file, "    context-path: /hll-basic-info-api").unwrap();
    writeln!(app_yml_file, "spring:").unwrap();
    writeln!(app_yml_file, "  application:").unwrap();
    writeln!(app_yml_file, "    name: hll-basic-info-api").unwrap();
    
    // 创建 adapter 模块目录
    let adapter_dir = project_root
        .join("basic-info-adapter")
        .join("src")
        .join("main")
        .join("java")
        .join("com")
        .join("hll")
        .join("basic")
        .join("api")
        .join("adapter")
        .join("feign");
    fs::create_dir_all(&adapter_dir).unwrap();
    
    // 创建 Java 控制器文件
    let controller_path = adapter_dir.join("FeignShopCopyController.java");
    let mut controller_file = fs::File::create(&controller_path).unwrap();
    writeln!(controller_file, "package com.hll.basic.api.adapter.feign;").unwrap();
    writeln!(controller_file, "").unwrap();
    writeln!(controller_file, "import org.springframework.web.bind.annotation.*;").unwrap();
    writeln!(controller_file, "").unwrap();
    writeln!(controller_file, "@RestController").unwrap();
    writeln!(controller_file, "@RequestMapping(\"feign/shop/copy\")").unwrap();
    writeln!(controller_file, "public class FeignShopCopyController {{").unwrap();
    writeln!(controller_file, "    @PostMapping(\"/query\")").unwrap();
    writeln!(controller_file, "    public Response query() {{").unwrap();
    writeln!(controller_file, "        return null;").unwrap();
    writeln!(controller_file, "    }}").unwrap();
    writeln!(controller_file, "}}").unwrap();
    
    // 解析文件
    let parser = JavaParser::new().unwrap();
    let content = fs::read_to_string(&controller_path).unwrap();
    let result = parser.parse_file(&content, &controller_path).unwrap();
    
    // 验证结果
    assert_eq!(result.classes.len(), 1);
    let class = &result.classes[0];
    assert_eq!(class.name, "com.hll.basic.api.adapter.feign.FeignShopCopyController");
    
    assert_eq!(class.methods.len(), 1);
    let method = &class.methods[0];
    assert_eq!(method.name, "query");
    
    // 验证 HTTP 注解
    assert!(method.http_annotations.is_some(), "HTTP annotation should be present");
    let http = method.http_annotations.as_ref().unwrap();
    assert_eq!(http.method, HttpMethod::POST);
    
    // 验证完整路径格式：application.name/context-path/class-path/method-path
    // 期望：POST hll-basic-info-api/hll-basic-info-api/feign/shop/copy/query
    assert_eq!(
        http.path,
        "hll-basic-info-api/hll-basic-info-api/feign/shop/copy/query",
        "HTTP path should be: application.name/context-path/class-path/method-path"
    );
}

#[test]
fn test_http_endpoint_without_context_path() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    
    // 创建 start/src/main/resources 目录
    let resources_dir = project_root
        .join("start")
        .join("src")
        .join("main")
        .join("resources");
    fs::create_dir_all(&resources_dir).unwrap();
    
    // 创建 application.yml 文件（没有 context-path）
    let app_yml_path = resources_dir.join("application.yml");
    let mut app_yml_file = fs::File::create(&app_yml_path).unwrap();
    writeln!(app_yml_file, "spring:").unwrap();
    writeln!(app_yml_file, "  application:").unwrap();
    writeln!(app_yml_file, "    name: test-service").unwrap();
    
    // 创建 adapter 模块目录
    let adapter_dir = project_root
        .join("test-adapter")
        .join("src")
        .join("main")
        .join("java")
        .join("com")
        .join("example");
    fs::create_dir_all(&adapter_dir).unwrap();
    
    // 创建 Java 控制器文件
    let controller_path = adapter_dir.join("TestController.java");
    let mut controller_file = fs::File::create(&controller_path).unwrap();
    writeln!(controller_file, "package com.example;").unwrap();
    writeln!(controller_file, "").unwrap();
    writeln!(controller_file, "import org.springframework.web.bind.annotation.*;").unwrap();
    writeln!(controller_file, "").unwrap();
    writeln!(controller_file, "@RestController").unwrap();
    writeln!(controller_file, "@RequestMapping(\"/api\")").unwrap();
    writeln!(controller_file, "public class TestController {{").unwrap();
    writeln!(controller_file, "    @GetMapping(\"/test\")").unwrap();
    writeln!(controller_file, "    public String test() {{").unwrap();
    writeln!(controller_file, "        return \"test\";").unwrap();
    writeln!(controller_file, "    }}").unwrap();
    writeln!(controller_file, "}}").unwrap();
    
    // 解析文件
    let parser = JavaParser::new().unwrap();
    let content = fs::read_to_string(&controller_path).unwrap();
    let result = parser.parse_file(&content, &controller_path).unwrap();
    
    // 验证结果
    assert_eq!(result.classes.len(), 1);
    let class = &result.classes[0];
    
    assert_eq!(class.methods.len(), 1);
    let method = &class.methods[0];
    
    // 验证 HTTP 注解
    assert!(method.http_annotations.is_some());
    let http = method.http_annotations.as_ref().unwrap();
    assert_eq!(http.method, HttpMethod::GET);
    
    // 没有 context-path 时，格式应为：application.name/class-path/method-path
    assert_eq!(
        http.path,
        "test-service/api/test",
        "HTTP path should be: application.name/class-path/method-path"
    );
}

#[test]
fn test_http_endpoint_without_class_mapping() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    
    // 创建 start/src/main/resources 目录
    let resources_dir = project_root
        .join("start")
        .join("src")
        .join("main")
        .join("resources");
    fs::create_dir_all(&resources_dir).unwrap();
    
    // 创建 application.yml 文件
    let app_yml_path = resources_dir.join("application.yml");
    let mut app_yml_file = fs::File::create(&app_yml_path).unwrap();
    writeln!(app_yml_file, "server:").unwrap();
    writeln!(app_yml_file, "  servlet:").unwrap();
    writeln!(app_yml_file, "    context-path: /api").unwrap();
    writeln!(app_yml_file, "spring:").unwrap();
    writeln!(app_yml_file, "  application:").unwrap();
    writeln!(app_yml_file, "    name: my-service").unwrap();
    
    // 创建 adapter 模块目录
    let adapter_dir = project_root
        .join("my-adapter")
        .join("src")
        .join("main")
        .join("java")
        .join("com")
        .join("example");
    fs::create_dir_all(&adapter_dir).unwrap();
    
    // 创建 Java 控制器文件（没有类级别的 @RequestMapping）
    let controller_path = adapter_dir.join("SimpleController.java");
    let mut controller_file = fs::File::create(&controller_path).unwrap();
    writeln!(controller_file, "package com.example;").unwrap();
    writeln!(controller_file, "").unwrap();
    writeln!(controller_file, "import org.springframework.web.bind.annotation.*;").unwrap();
    writeln!(controller_file, "").unwrap();
    writeln!(controller_file, "@RestController").unwrap();
    writeln!(controller_file, "public class SimpleController {{").unwrap();
    writeln!(controller_file, "    @PostMapping(\"/users\")").unwrap();
    writeln!(controller_file, "    public String createUser() {{").unwrap();
    writeln!(controller_file, "        return \"created\";").unwrap();
    writeln!(controller_file, "    }}").unwrap();
    writeln!(controller_file, "}}").unwrap();
    
    // 解析文件
    let parser = JavaParser::new().unwrap();
    let content = fs::read_to_string(&controller_path).unwrap();
    let result = parser.parse_file(&content, &controller_path).unwrap();
    
    // 验证结果
    assert_eq!(result.classes.len(), 1);
    let class = &result.classes[0];
    
    assert_eq!(class.methods.len(), 1);
    let method = &class.methods[0];
    
    // 验证 HTTP 注解
    assert!(method.http_annotations.is_some());
    let http = method.http_annotations.as_ref().unwrap();
    assert_eq!(http.method, HttpMethod::POST);
    
    // 没有类级别 RequestMapping 时，格式应为：application.name/context-path/method-path
    assert_eq!(
        http.path,
        "my-service/api/users",
        "HTTP path should be: application.name/context-path/method-path"
    );
}
