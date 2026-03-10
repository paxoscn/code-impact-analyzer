# 通配符导入解析修复

## 问题描述

Java接口通过通配符导入的类的包名没有被正确解析。

### 示例代码
```java
// foo/Bar.java
package foo;
public class Bar {}

// tac/tic.java
package tac;
import foo.*;
interface tic {
    void toe(Bar bar);
}
```

### 问题表现

**修复前：**
- 方法被索引为：`tic::toe(Bar)`
- 类名缺少包名前缀
- 参数类型未通过通配符导入解析

**期望行为：**
- 方法应被索引为：`tac.tic::toe(foo.Bar)`
- 类名包含完整包名
- 参数类型通过通配符导入正确解析

## 根本原因

1. **包名提取问题**：`extract_package_name` 方法只处理 `scoped_identifier` 类型的包声明，没有处理单级包名的 `identifier` 类型
2. **参数类型解析问题**：`extract_parameter_types` 方法使用 `build_import_map` 而不是 `build_import_map_with_wildcards`，导致通配符导入信息丢失
3. **通配符导入提取问题**：`walk_node_for_import_map_with_wildcards` 方法只处理 `scoped_identifier`，没有处理单级包名的 `identifier` 类型

## 修复方案

### 1. 修复包名提取（`extract_package_name`）

**文件：** `src/java_parser.rs`

**修改：** 添加对 `identifier` 类型的支持

```rust
fn extract_package_name(&self, source: &str, tree: &tree_sitter::Tree) -> Option<String> {
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();
    
    for child in root_node.children(&mut cursor) {
        if child.kind() == "package_declaration" {
            // 在 package_declaration 中查找 scoped_identifier 或 identifier
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
    
    None
}
```

### 2. 修复参数类型解析（`extract_parameter_types`）

**文件：** `src/java_parser.rs`

**修改：** 使用 `build_import_map_with_wildcards` 和 `resolve_full_class_name_with_wildcard_fallback`

```rust
fn extract_parameter_types(&self, source: &str, method_node: &tree_sitter::Node, tree: &tree_sitter::Tree) -> Vec<String> {
    let mut param_types = Vec::new();
    
    // 获取导入映射（包括通配符导入）和包名
    let (import_map, wildcard_imports) = self.build_import_map_with_wildcards(source, tree);
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
                        // 解析为完整类名（支持通配符导入）
                        let full_type = if is_primitive_or_common_type(&param_type) {
                            // 对基本类型进行自动装箱
                            autobox_type(&param_type)
                        } else {
                            self.resolve_full_class_name_with_wildcard_fallback(&param_type, &import_map, &wildcard_imports, &package_name)
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
```

### 3. 修复通配符导入提取（`walk_node_for_import_map_with_wildcards`）

**文件：** `src/java_parser.rs`

**修改：** 添加对 `identifier` 类型的支持

```rust
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
                } else if child.kind() == "identifier" {
                    // 处理单级包名的通配符导入，如 import foo.*;
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
```

## 测试验证

创建了新的测试 `test_interface_wildcard_import_resolution` 来验证修复：

**文件：** `tests/wildcard_import_test.rs`

测试验证：
1. 接口名称包含完整包名（`tac.tic`）
2. 参数类型通过通配符导入正确解析（`foo.Bar`）
3. 方法签名完整正确（`tac.tic::toe(foo.Bar)`）

## 影响范围

- 所有使用通配符导入的Java接口和类
- 方法参数类型解析
- 方法签名生成
- 跨文件类型推断

## 测试结果

所有测试通过，包括：
- 新增的通配符导入测试
- 现有的所有集成测试
- 现有的通配符导入解析测试

## 总结

此修复确保了Java接口通过通配符导入的类能够被正确解析为完全限定名，提高了代码索引的准确性和跨文件类型推断的可靠性。
