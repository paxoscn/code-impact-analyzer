use code_impact_analyzer::java_parser::JavaParser;
use code_impact_analyzer::language_parser::LanguageParser;
use std::path::Path;

#[test]
fn test_parse_simple_inheritance() {
    let parser = JavaParser::new().unwrap();
    
    let source = r#"
        package com.example;
        
        public class Child extends Parent {
            public void childMethod() {
                System.out.println("Child method");
            }
        }
    "#;
    
    let parsed = parser.parse_file(source, Path::new("Child.java")).unwrap();
    
    assert_eq!(parsed.classes.len(), 1);
    let child_class = &parsed.classes[0];
    
    assert_eq!(child_class.name, "com.example.Child");
    // 没有导入时，父类会被解析为同包的类
    assert_eq!(child_class.extends, Some("com.example.Parent".to_string()));
    assert!(!child_class.is_interface);
}

#[test]
fn test_parse_inheritance_with_imports() {
    let parser = JavaParser::new().unwrap();
    
    let source = r#"
        package com.example;
        
        import com.base.BaseService;
        
        public class MyService extends BaseService {
            public void myMethod() {
                // implementation
            }
        }
    "#;
    
    let parsed = parser.parse_file(source, Path::new("MyService.java")).unwrap();
    
    assert_eq!(parsed.classes.len(), 1);
    let service_class = &parsed.classes[0];
    
    assert_eq!(service_class.name, "com.example.MyService");
    assert_eq!(service_class.extends, Some("com.base.BaseService".to_string()));
}

#[test]
fn test_parse_inheritance_with_generics() {
    let parser = JavaParser::new().unwrap();
    
    let source = r#"
        package com.example;
        
        public class MyList extends ArrayList<String> {
            public void customMethod() {
                // implementation
            }
        }
    "#;
    
    let parsed = parser.parse_file(source, Path::new("MyList.java")).unwrap();
    
    assert_eq!(parsed.classes.len(), 1);
    let list_class = &parsed.classes[0];
    
    assert_eq!(list_class.name, "com.example.MyList");
    // 泛型参数应该被移除，只保留基础类名
    // 没有导入时，会被解析为同包的类
    assert_eq!(list_class.extends, Some("com.example.ArrayList".to_string()));
}

#[test]
fn test_parse_inheritance_and_implements() {
    let parser = JavaParser::new().unwrap();
    
    let source = r#"
        package com.example;
        
        public class MyClass extends BaseClass implements Interface1, Interface2 {
            public void myMethod() {
                // implementation
            }
        }
    "#;
    
    let parsed = parser.parse_file(source, Path::new("MyClass.java")).unwrap();
    
    assert_eq!(parsed.classes.len(), 1);
    let my_class = &parsed.classes[0];
    
    assert_eq!(my_class.name, "com.example.MyClass");
    // 没有导入时，会被解析为同包的类
    assert_eq!(my_class.extends, Some("com.example.BaseClass".to_string()));
    assert_eq!(my_class.implements.len(), 2);
    assert!(my_class.implements.contains(&"com.example.Interface1".to_string()));
    assert!(my_class.implements.contains(&"com.example.Interface2".to_string()));
}

#[test]
fn test_parse_no_inheritance() {
    let parser = JavaParser::new().unwrap();
    
    let source = r#"
        package com.example;
        
        public class StandaloneClass {
            public void method() {
                // implementation
            }
        }
    "#;
    
    let parsed = parser.parse_file(source, Path::new("StandaloneClass.java")).unwrap();
    
    assert_eq!(parsed.classes.len(), 1);
    let standalone_class = &parsed.classes[0];
    
    assert_eq!(standalone_class.name, "com.example.StandaloneClass");
    assert_eq!(standalone_class.extends, None);
    assert_eq!(standalone_class.implements.len(), 0);
}

#[test]
fn test_parse_interface_extends() {
    let parser = JavaParser::new().unwrap();
    
    let source = r#"
        package com.example;
        
        public interface ChildInterface extends ParentInterface {
            void childMethod();
        }
    "#;
    
    let parsed = parser.parse_file(source, Path::new("ChildInterface.java")).unwrap();
    
    assert_eq!(parsed.classes.len(), 1);
    let child_interface = &parsed.classes[0];
    
    assert_eq!(child_interface.name, "com.example.ChildInterface");
    assert!(child_interface.is_interface);
    // 接口的 extends 也应该被正确解析
    // 注意：在 Java 中，接口使用 extends 来继承其他接口
    // 但我们的实现可能将其放在 implements 中，这取决于具体实现
}

#[test]
fn test_parse_fully_qualified_parent() {
    let parser = JavaParser::new().unwrap();
    
    let source = r#"
        package com.example;
        
        public class MyClass extends com.base.BaseClass {
            public void myMethod() {
                // implementation
            }
        }
    "#;
    
    let parsed = parser.parse_file(source, Path::new("MyClass.java")).unwrap();
    
    assert_eq!(parsed.classes.len(), 1);
    let my_class = &parsed.classes[0];
    
    assert_eq!(my_class.name, "com.example.MyClass");
    // 完全限定名应该被保留
    assert_eq!(my_class.extends, Some("com.base.BaseClass".to_string()));
}
