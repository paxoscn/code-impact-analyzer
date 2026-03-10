use code_impact_analyzer::code_index::CodeIndex;
use code_impact_analyzer::java_parser::JavaParser;
use code_impact_analyzer::language_parser::LanguageParser;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_interface_wildcard_import_resolution() {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // Create foo/Bar.java
    let foo_dir = base_path.join("foo");
    fs::create_dir_all(&foo_dir).unwrap();
    fs::write(
        foo_dir.join("Bar.java"),
        "package foo;\npublic class Bar {}\n"
    ).unwrap();

    // Create tac/tic.java with wildcard import
    let tac_dir = base_path.join("tac");
    fs::create_dir_all(&tac_dir).unwrap();
    fs::write(
        tac_dir.join("tic.java"),
        "package tac;\nimport foo.*;\ninterface tic {\n    void toe(Bar bar);\n}\n"
    ).unwrap();

    // Create parsers
    let parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(JavaParser::new().unwrap()),
    ];

    // Index the files using two-pass indexing
    let mut index = CodeIndex::new();
    index.index_workspace_two_pass(base_path, &parsers).unwrap();

    // Test expected behavior: method should be indexed with fully qualified names
    let toe_method_expected = index.find_method("tac.tic::toe(foo.Bar)");
    assert!(toe_method_expected.is_some(), 
        "Parameter type 'Bar' from wildcard import 'foo.*' should be resolved to 'foo.Bar', \
         and interface name should include package prefix 'tac.tic'");
    
    let method = toe_method_expected.unwrap();
    assert_eq!(method.name, "toe");
    assert_eq!(method.full_qualified_name, "tac.tic::toe(foo.Bar)", 
        "Method signature should contain fully qualified parameter type 'foo.Bar'");
}
