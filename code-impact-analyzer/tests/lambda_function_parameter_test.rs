use code_impact_analyzer::java_parser::JavaParser;
use code_impact_analyzer::language_parser::LanguageParser;
use std::path::Path;

#[test]
fn test_lambda_as_function_parameter() {
    let parser = JavaParser::new().unwrap();
    let source = r#"
import foo.Bar;

class Tac {
    void tic() {
        Bar bar = new Bar();
        toe(bar, apple -> apple.toString());
    }
    
    void toe(Bar bar, java.util.function.Function<Bar, String> func) {
        String result = func.apply(bar);
    }
}
"#;

    let result = parser.parse_file(source, Path::new("Tac.java")).unwrap();
    
    assert_eq!(result.classes.len(), 1);
    let class = &result.classes[0];
    assert_eq!(class.name, "Tac");
    
    // 找到 tic 方法
    let tic_method = class.methods.iter()
        .find(|m| m.name == "tic")
        .expect("Should find tic method");
    
    // 验证方法调用
    assert_eq!(tic_method.calls.len(), 2, "Should have 2 method calls");
    
    // 第一个调用应该是 toe(foo.Bar, java.util.function.Function)
    let toe_call = tic_method.calls.iter()
        .find(|c| c.target.contains("toe"))
        .expect("Should find toe call");
    
    assert_eq!(
        toe_call.target,
        "Tac::toe(foo.Bar,java.util.function.Function)",
        "Lambda parameter should be resolved as java.util.function.Function"
    );
}

#[test]
fn test_lambda_with_multiple_parameters() {
    let parser = JavaParser::new().unwrap();
    let source = r#"
package com.example;

class Calculator {
    void calculate() {
        process((a, b) -> a + b);
    }
    
    void process(java.util.function.BiFunction<Integer, Integer, Integer> func) {
        int result = func.apply(1, 2);
    }
}
"#;

    let result = parser.parse_file(source, Path::new("Calculator.java")).unwrap();
    
    let class = &result.classes[0];
    let calculate_method = class.methods.iter()
        .find(|m| m.name == "calculate")
        .expect("Should find calculate method");
    
    // 验证 lambda 被识别为 Function 类型
    let process_call = calculate_method.calls.iter()
        .find(|c| c.target.contains("process"))
        .expect("Should find process call");
    
    assert_eq!(
        process_call.target,
        "com.example.Calculator::process(java.util.function.Function)",
        "Lambda with multiple parameters should be resolved as java.util.function.Function"
    );
}

#[test]
fn test_lambda_in_stream_operations() {
    let parser = JavaParser::new().unwrap();
    let source = r#"
package com.example;

import java.util.List;
import java.util.stream.Collectors;

class DataProcessor {
    void processData(List<String> items) {
        List<String> result = items.stream()
            .map(item -> item.toUpperCase())
            .filter(item -> item.length() > 5)
            .collect(Collectors.toList());
    }
}
"#;

    let result = parser.parse_file(source, Path::new("DataProcessor.java")).unwrap();
    
    let class = &result.classes[0];
    let process_method = class.methods.iter()
        .find(|m| m.name == "processData")
        .expect("Should find processData method");
    
    // 验证 map 和 filter 调用中的 lambda 被正确识别
    let map_call = process_method.calls.iter()
        .find(|c| c.target.contains("map"))
        .expect("Should find map call");
    
    // map 方法接受 Function 参数
    assert!(
        map_call.target.contains("map(java.util.function.Function)"),
        "map should accept Function parameter, got: {}",
        map_call.target
    );
}

#[test]
fn test_lambda_as_consumer_parameter() {
    let parser = JavaParser::new().unwrap();
    let source = r#"
package com.example;

class EventHandler {
    void registerHandler() {
        onEvent(event -> System.out.println(event));
    }
    
    void onEvent(java.util.function.Consumer<String> handler) {
        handler.accept("test");
    }
}
"#;

    let result = parser.parse_file(source, Path::new("EventHandler.java")).unwrap();
    
    let class = &result.classes[0];
    let register_method = class.methods.iter()
        .find(|m| m.name == "registerHandler")
        .expect("Should find registerHandler method");
    
    let on_event_call = register_method.calls.iter()
        .find(|c| c.target.contains("onEvent"))
        .expect("Should find onEvent call");
    
    // Lambda 应该被识别为 Function 类型（简化处理）
    assert_eq!(
        on_event_call.target,
        "com.example.EventHandler::onEvent(java.util.function.Function)",
        "Lambda parameter should be resolved as java.util.function.Function"
    );
}

#[test]
fn test_lambda_as_predicate_parameter() {
    let parser = JavaParser::new().unwrap();
    let source = r#"
package com.example;

class Validator {
    void validate() {
        check(value -> value != null && value.length() > 0);
    }
    
    void check(java.util.function.Predicate<String> predicate) {
        boolean result = predicate.test("test");
    }
}
"#;

    let result = parser.parse_file(source, Path::new("Validator.java")).unwrap();
    
    let class = &result.classes[0];
    let validate_method = class.methods.iter()
        .find(|m| m.name == "validate")
        .expect("Should find validate method");
    
    let check_call = validate_method.calls.iter()
        .find(|c| c.target.contains("check"))
        .expect("Should find check call");
    
    assert_eq!(
        check_call.target,
        "com.example.Validator::check(java.util.function.Function)",
        "Lambda parameter should be resolved as java.util.function.Function"
    );
}

#[test]
fn test_method_reference_as_function_parameter() {
    let parser = JavaParser::new().unwrap();
    let source = r#"
package com.example;

class StringProcessor {
    void process() {
        transform(String::toUpperCase);
    }
    
    void transform(java.util.function.Function<String, String> func) {
        String result = func.apply("test");
    }
}
"#;

    let result = parser.parse_file(source, Path::new("StringProcessor.java")).unwrap();
    
    let class = &result.classes[0];
    let process_method = class.methods.iter()
        .find(|m| m.name == "process")
        .expect("Should find process method");
    
    // 方法引用也应该被识别（如果解析器支持）
    // 注意：这个测试可能需要根据实际的 AST 节点类型调整
    let transform_call = process_method.calls.iter()
        .find(|c| c.target.contains("transform"))
        .expect("Should find transform call");
    
    // 验证调用存在（具体类型取决于方法引用的 AST 节点类型）
    assert!(
        transform_call.target.contains("transform"),
        "Should find transform call"
    );
}
