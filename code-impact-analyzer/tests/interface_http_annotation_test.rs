use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::java_parser::JavaParser;
use code_impact_analyzer::language_parser::LanguageParser;
use code_impact_analyzer::types::HttpMethod;
use std::path::Path;

#[test]
fn test_interface_http_annotation_inheritance() {
    let parser = JavaParser::new().unwrap();
    let mut index = CodeIndex::new();
    
    // 定义接口文件
    let interface_code = r#"
package com.example.controller;

import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestMapping;

@RequestMapping("bar")
public interface Bar {
    @PostMapping("/tac")
    String tac();
}
"#;
    
    // 定义实现类文件
    let impl_code = r#"
package com.example.controller;

public class FooController implements Bar {
    public String tac() {
        return "ok";
    }
}
"#;
    
    // 解析接口
    let interface_path = Path::new("src/main/java/com/example/controller/Bar.java");
    let interface_result = parser.parse_file(interface_code, interface_path).unwrap();
    
    // 解析实现类
    let impl_path = Path::new("src/main/java/com/example/controller/FooController.java");
    let impl_result = parser.parse_file(impl_code, impl_path).unwrap();
    
    // 验证接口方法有HTTP注解
    assert_eq!(interface_result.classes.len(), 1);
    let interface_class = &interface_result.classes[0];
    assert_eq!(interface_class.name, "com.example.controller.Bar");
    assert_eq!(interface_class.methods.len(), 1);
    
    let interface_method = &interface_class.methods[0];
    assert_eq!(interface_method.name, "tac");
    assert!(interface_method.http_annotations.is_some());
    
    let interface_http = interface_method.http_annotations.as_ref().unwrap();
    assert_eq!(interface_http.path, "bar/tac");
    
    // 验证实现类方法初始没有HTTP注解
    assert_eq!(impl_result.classes.len(), 1);
    let impl_class = &impl_result.classes[0];
    assert_eq!(impl_class.name, "com.example.controller.FooController");
    assert_eq!(impl_class.implements.len(), 1);
    assert_eq!(impl_class.implements[0], "com.example.controller.Bar");
    assert_eq!(impl_class.methods.len(), 1);
    
    let impl_method = &impl_result.classes[0].methods[0];
    assert_eq!(impl_method.name, "tac");
    assert!(impl_method.http_annotations.is_none(), "实现类方法初始应该没有HTTP注解");
    
    // 索引两个文件
    index.test_index_parsed_file(interface_result).unwrap();
    index.test_index_parsed_file(impl_result).unwrap();
    
    // 传播接口的HTTP注解
    index.propagate_interface_http_annotations();
    
    // 验证实现类方法现在有HTTP注解了
    let impl_method_name = "com.example.controller.FooController::tac()";
    let impl_method_indexed = index.find_method(impl_method_name).unwrap();
    
    assert!(impl_method_indexed.http_annotations.is_some(), "传播后实现类方法应该有HTTP注解");
    let impl_http = impl_method_indexed.http_annotations.as_ref().unwrap();
    assert_eq!(impl_http.path, "bar/tac");
    assert_eq!(impl_http.method, HttpMethod::POST);
}

#[test]
fn test_interface_http_annotation_with_parameters() {
    let parser = JavaParser::new().unwrap();
    let mut index = CodeIndex::new();
    
    // 定义接口文件
    let interface_code = r#"
package com.example.api;

import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;

@RequestMapping("/api/users")
public interface UserApi {
    @GetMapping("/{id}")
    User getUser(@PathVariable Long id);
    
    @GetMapping("/list")
    List<User> listUsers();
}
"#;
    
    // 定义实现类文件
    let impl_code = r#"
package com.example.controller;

import com.example.api.UserApi;

public class UserController implements UserApi {
    public User getUser(Long id) {
        return new User(id);
    }
    
    public List<User> listUsers() {
        return new ArrayList<>();
    }
}
"#;
    
    // 解析接口
    let interface_path = Path::new("src/main/java/com/example/api/UserApi.java");
    let interface_result = parser.parse_file(interface_code, interface_path).unwrap();
    
    // 解析实现类
    let impl_path = Path::new("src/main/java/com/example/controller/UserController.java");
    let impl_result = parser.parse_file(impl_code, impl_path).unwrap();
    
    // 验证接口方法有HTTP注解
    assert_eq!(interface_result.classes.len(), 1);
    let interface_class = &interface_result.classes[0];
    assert_eq!(interface_class.name, "com.example.api.UserApi");
    assert_eq!(interface_class.methods.len(), 2);
    
    // 索引两个文件
    index.test_index_parsed_file(interface_result).unwrap();
    index.test_index_parsed_file(impl_result).unwrap();
    
    // 传播接口的HTTP注解
    index.propagate_interface_http_annotations();
    
    // 验证带参数的方法
    let get_user_method = "com.example.controller.UserController::getUser(Long)";
    let get_user_indexed = index.find_method(get_user_method).unwrap();
    
    assert!(get_user_indexed.http_annotations.is_some(), "getUser方法应该有HTTP注解");
    let get_user_http = get_user_indexed.http_annotations.as_ref().unwrap();
    assert_eq!(get_user_http.path, "api/users/{id}");
    assert_eq!(get_user_http.method, HttpMethod::GET);
    
    // 验证无参数的方法
    let list_users_method = "com.example.controller.UserController::listUsers()";
    let list_users_indexed = index.find_method(list_users_method).unwrap();
    
    assert!(list_users_indexed.http_annotations.is_some(), "listUsers方法应该有HTTP注解");
    let list_users_http = list_users_indexed.http_annotations.as_ref().unwrap();
    assert_eq!(list_users_http.path, "api/users/list");
    assert_eq!(list_users_http.method, HttpMethod::GET);
}

#[test]
fn test_interface_http_annotation_not_override_existing() {
    let parser = JavaParser::new().unwrap();
    let mut index = CodeIndex::new();
    
    // 定义接口文件
    let interface_code = r#"
package com.example.api;

import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;

@RequestMapping("/api")
public interface BaseApi {
    @GetMapping("/base")
    String baseMethod();
}
"#;
    
    // 定义实现类文件（方法有自己的HTTP注解）
    let impl_code = r#"
package com.example.controller;

import com.example.api.BaseApi;
import org.springframework.web.bind.annotation.PostMapping;

public class CustomController implements BaseApi {
    @PostMapping("/custom")
    public String baseMethod() {
        return "custom";
    }
}
"#;
    
    // 解析接口
    let interface_path = Path::new("src/main/java/com/example/api/BaseApi.java");
    let interface_result = parser.parse_file(interface_code, interface_path).unwrap();
    
    // 解析实现类
    let impl_path = Path::new("src/main/java/com/example/controller/CustomController.java");
    let impl_result = parser.parse_file(impl_code, impl_path).unwrap();
    
    // 索引两个文件
    index.test_index_parsed_file(interface_result).unwrap();
    index.test_index_parsed_file(impl_result).unwrap();
    
    // 验证实现类方法已经有HTTP注解
    let impl_method = "com.example.controller.CustomController::baseMethod()";
    let impl_method_before = index.find_method(impl_method).unwrap();
    assert!(impl_method_before.http_annotations.is_some());
    let http_before = impl_method_before.http_annotations.as_ref().unwrap();
    assert_eq!(http_before.path, "custom");  // 注意：没有前导斜杠
    assert_eq!(http_before.method, HttpMethod::POST);
    
    // 传播接口的HTTP注解
    index.propagate_interface_http_annotations();
    
    // 验证实现类方法的HTTP注解没有被覆盖
    let impl_method_after = index.find_method(impl_method).unwrap();
    assert!(impl_method_after.http_annotations.is_some());
    let http_after = impl_method_after.http_annotations.as_ref().unwrap();
    assert_eq!(http_after.path, "custom", "实现类的HTTP注解不应该被接口的注解覆盖");
    assert_eq!(http_after.method, HttpMethod::POST);
}
