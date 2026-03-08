use std::path::Path;
use std::fs;
use crate::errors::ParseError;
use crate::types::MethodLocation;

/// Git patch 文件变更类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

/// Hunk 中的单行信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkLine {
    pub line_type: LineType,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineType {
    Context,
    Added,
    Removed,
}

/// Patch 文件中的 hunk（变更块）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<HunkLine>,
}

/// 文件变更信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub file_path: String,
    pub change_type: ChangeType,
    pub hunks: Vec<Hunk>,
}

/// Patch 解析器
pub struct PatchParser;

impl PatchParser {
    /// 从 Java 字段名生成 getter 方法名
    /// 例如：userName -> getUserName, isActive -> isActive
    fn generate_getter_name(field_name: &str) -> String {
        if field_name.starts_with("is") && field_name.len() > 2 {
            // 对于 boolean 类型的 isXxx 字段，getter 通常保持原样
            field_name.to_string()
        } else {
            // 首字母大写
            let mut chars = field_name.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => format!("get{}{}", first.to_uppercase(), chars.as_str()),
            }
        }
    }

    /// 从 Java 字段名生成 setter 方法名
    /// 例如：userName -> setUserName, isActive -> setActive
    fn generate_setter_name(field_name: &str) -> String {
        let field_name = if field_name.starts_with("is") && field_name.len() > 2 {
            // 对于 isXxx 字段，移除 is 前缀
            &field_name[2..]
        } else {
            field_name
        };
        
        // 首字母大写
        let mut chars = field_name.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => format!("set{}{}", first.to_uppercase(), chars.as_str()),
        }
    }

    /// 检测 Java 字段声明
    /// 返回 (字段类型, 字段名) 如果检测到字段声明
    fn detect_java_field(line: &str) -> Option<(String, String)> {
        let line = line.trim();
        
        // 跳过注释和空行
        if line.is_empty() || line.starts_with("//") || line.starts_with("/*") || line.starts_with("*") {
            return None;
        }
        
        // 跳过方法声明（包含括号）
        if line.contains('(') {
            return None;
        }
        
        // 跳过类声明、接口声明、注解等
        if line.contains("class ") || line.contains("interface ") || line.contains("enum ") 
            || line.starts_with('@') || line.starts_with("import ") || line.starts_with("package ") {
            return None;
        }
        
        // 简单的字段声明模式：[修饰符] 类型 字段名 [= 初始值];
        // 例如：private String userName;
        //      public static final int MAX_SIZE = 100;
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() < 2 {
            return None;
        }
        
        // 查找类型和字段名
        let mut type_idx = None;
        let mut field_idx = None;
        
        for (i, part) in parts.iter().enumerate() {
            // 跳过修饰符
            if matches!(*part, "private" | "protected" | "public" | "static" | "final" | "transient" | "volatile") {
                continue;
            }
            
            // 第一个非修饰符的词是类型
            if type_idx.is_none() {
                type_idx = Some(i);
                continue;
            }
            
            // 第二个非修饰符的词是字段名
            if field_idx.is_none() {
                field_idx = Some(i);
                break;
            }
        }
        
        if let (Some(type_idx), Some(field_idx)) = (type_idx, field_idx) {
            let field_type = parts[type_idx].to_string();
            let mut field_name = parts[field_idx].to_string();
            
            // 移除字段名后面的分号、等号等
            if let Some(pos) = field_name.find(|c| c == ';' || c == '=' || c == ',') {
                field_name = field_name[..pos].to_string();
            }
            
            // 验证字段名是有效的标识符
            if !field_name.is_empty() && field_name.chars().next().unwrap().is_alphabetic() {
                return Some((field_type, field_name));
            }
        }
        
        None
    }

    /// 从文件变更中提取被修改的 Java 字段，并生成对应的 getter/setter 方法
    /// 
    /// # 参数
    /// * `file_change` - 文件变更信息
    /// 
    /// # 返回
    /// * 被修改字段对应的方法名列表（包括 getter 和 setter）
    pub fn extract_modified_field_methods(
        file_change: &FileChange,
    ) -> Vec<String> {
        // 只处理 Java 文件
        if !file_change.file_path.ends_with(".java") {
            return Vec::new();
        }
        
        let mut methods = Vec::new();
        
        for hunk in &file_change.hunks {
            for line in &hunk.lines {
                // 只关注被添加或删除的行（修改的字段）
                if !matches!(line.line_type, LineType::Added | LineType::Removed) {
                    continue;
                }
                
                // 检测是否是字段声明
                if let Some((field_type, field_name)) = Self::detect_java_field(&line.content) {
                    log::debug!(
                        "Detected field modification in {}: {} {}",
                        file_change.file_path,
                        field_type,
                        field_name
                    );
                    
                    // 生成 getter 方法名
                    let getter = Self::generate_getter_name(&field_name);
                    methods.push(getter);
                    
                    // 生成 setter 方法名（带参数类型）
                    let setter = format!("{}({})", Self::generate_setter_name(&field_name), field_type);
                    methods.push(setter);
                }
            }
        }
        
        // 去重
        methods.sort();
        methods.dedup();
        
        methods
    }

    /// 解析 Git patch 文件
    /// 
    /// # 参数
    /// * `path` - patch 文件路径
    /// 
    /// # 返回
    /// * `Ok(Vec<FileChange>)` - 成功解析的文件变更列表
    /// * `Err(ParseError)` - 解析失败错误
    pub fn parse_patch_file(path: &Path) -> Result<Vec<FileChange>, ParseError> {
        // 读取文件内容
        let content = fs::read_to_string(path).map_err(|e| ParseError::InvalidFormat {
            message: format!("Failed to read patch file: {}", e),
        })?;

        // 预处理 patch 内容：移除 "-- " 分隔符后面的所有内容
        // git format-patch 生成的文件会在最后添加 "-- " 和版本号等信息
        // 这些内容会导致 gitpatch crate panic
        let cleaned_content = Self::remove_trailing_content(&content);

        // 使用 gitpatch crate 解析多个 patch
        let patches = gitpatch::Patch::from_multiple(&cleaned_content).map_err(|e| ParseError::InvalidFormat {
            message: format!("Failed to parse patch: {}", e),
        })?;

        let mut file_changes = Vec::new();

        for patch in patches {
            // 简化的二进制文件检测：检查是否有 hunks
            // 二进制文件通常没有 hunks，只有 meta 信息
            if patch.hunks.is_empty() {
                log::warn!("Skipping file without hunks (possibly binary): {:?}", patch.new.path);
                continue;
            }

            // 确定变更类型
            // 在 gitpatch 中，path 是 Cow<str>，空路径用 "/dev/null" 表示
            let change_type = if patch.old.path == "/dev/null" {
                ChangeType::Added
            } else if patch.new.path == "/dev/null" {
                ChangeType::Deleted
            } else {
                ChangeType::Modified
            };

            // 获取文件路径（优先使用新路径，除非是删除）
            let mut file_path = if patch.new.path != "/dev/null" {
                patch.new.path.to_string()
            } else {
                patch.old.path.to_string()
            };

            // 移除 Git diff 的 a/ 或 b/ 前缀
            if file_path.starts_with("a/") || file_path.starts_with("b/") {
                file_path = file_path[2..].to_string();
            }

            // 转换 hunks
            let hunks = patch.hunks.iter().map(|h| {
                let lines = h.lines.iter().map(|line| {
                    let line_type = match line {
                        gitpatch::Line::Context(_) => LineType::Context,
                        gitpatch::Line::Add(_) => LineType::Added,
                        gitpatch::Line::Remove(_) => LineType::Removed,
                    };
                    let content = match line {
                        gitpatch::Line::Context(s) |
                        gitpatch::Line::Add(s) |
                        gitpatch::Line::Remove(s) => s.to_string(),
                    };
                    HunkLine { line_type, content }
                }).collect();

                Hunk {
                    old_start: h.old_range.start as usize,
                    old_lines: h.old_range.count as usize,
                    new_start: h.new_range.start as usize,
                    new_lines: h.new_range.count as usize,
                    lines,
                }
            }).collect();

            file_changes.push(FileChange {
                file_path,
                change_type,
                hunks,
            });
        }

        Ok(file_changes)
    }

    /// 移除 patch 内容中 "-- " 分隔符后面的所有内容
    /// 
    /// git format-patch 生成的文件会在最后添加 "-- " 分隔符和版本号等信息，
    /// 这些内容会导致 gitpatch crate panic。
    /// 
    /// # 参数
    /// * `content` - 原始 patch 内容
    /// 
    /// # 返回
    /// * 清理后的 patch 内容
    fn remove_trailing_content(content: &str) -> String {
        let mut result = String::new();
        let mut lines = content.lines().peekable();
        
        while let Some(line) = lines.next() {
            // 检查是否是 "-- " 分隔符（注意后面有空格或者是行尾）
            if line == "--" || line.starts_with("-- ") {
                // 找到分隔符，停止处理
                // 但保留这一行，因为它可能是 patch 格式的一部分
                result.push_str(line);
                result.push('\n');
                
                // 检查下一行是否为空或者是版本号等信息
                if let Some(next_line) = lines.peek() {
                    // 如果下一行不是空行，说明后面有额外内容，需要截断
                    if !next_line.trim().is_empty() {
                        log::debug!("Removing trailing content after '-- ' separator");
                        break;
                    }
                }
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
        
        result
    }

    /// 从文件变更中提取被修改的方法
    /// 
    /// # 参数
    /// * `file_change` - 文件变更信息
    /// * `source_content` - 源文件内容
    /// * `language` - 编程语言类型
    /// 
    /// # 返回
    /// * `Ok(Vec<MethodLocation>)` - 被修改的方法列表
    /// * `Err(ParseError)` - 解析失败错误
    /// 
    /// # 注意
    /// 此方法需要语言解析器支持，当前版本返回空列表作为占位
    pub fn extract_modified_methods(
        &self,
        file_change: &FileChange,
        _source_content: &str,
        _language: &str,
    ) -> Result<Vec<MethodLocation>, ParseError> {
        // 收集所有被修改的行号范围
        let mut modified_ranges = Vec::new();
        
        for hunk in &file_change.hunks {
            // 对于新文件或修改的文件，我们关注新文件中的行号
            let start = hunk.new_start;
            let end = hunk.new_start + hunk.new_lines;
            modified_ranges.push((start, end));
        }

        // TODO: 实际的方法提取需要语言解析器
        // 当前返回空列表，将在后续任务中实现完整的语言解析
        log::debug!(
            "Modified line ranges in {}: {:?}",
            file_change.file_path,
            modified_ranges
        );

        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_patch() {
        let patch_content = r#"diff --git a/test.txt b/test.txt
index 1234567..abcdefg 100644
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,3 @@
 line 1
-line 2
+line 2 modified
 line 3
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "test.txt");
        assert_eq!(changes[0].change_type, ChangeType::Modified);
        assert_eq!(changes[0].hunks.len(), 1);
    }

    #[test]
    fn test_parse_multiple_files() {
        let patch_content = r#"diff --git a/file1.txt b/file1.txt
index 1234567..abcdefg 100644
--- a/file1.txt
+++ b/file1.txt
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
diff --git a/file2.txt b/file2.txt
index 2345678..bcdefgh 100644
--- a/file2.txt
+++ b/file2.txt
@@ -1,2 +1,3 @@
 line 1
 line 2
+line 3
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "file1.txt");
        assert_eq!(changes[1].file_path, "file2.txt");
    }

    #[test]
    fn test_parse_added_file() {
        let patch_content = r#"diff --git a/new_file.txt b/new_file.txt
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/new_file.txt
@@ -0,0 +1,3 @@
+line 1
+line 2
+line 3
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "new_file.txt");
        assert_eq!(changes[0].change_type, ChangeType::Added);
    }

    #[test]
    fn test_parse_deleted_file() {
        let patch_content = r#"diff --git a/old_file.txt b/old_file.txt
deleted file mode 100644
index 1234567..0000000
--- a/old_file.txt
+++ /dev/null
@@ -1,3 +0,0 @@
-line 1
-line 2
-line 3
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "old_file.txt");
        assert_eq!(changes[0].change_type, ChangeType::Deleted);
    }

    #[test]
    fn test_parse_invalid_patch() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"not a valid patch").unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_modified_methods_placeholder() {
        let parser = PatchParser;
        let file_change = FileChange {
            file_path: "test.rs".to_string(),
            change_type: ChangeType::Modified,
            hunks: vec![
                Hunk {
                    old_start: 10,
                    old_lines: 5,
                    new_start: 10,
                    new_lines: 6,
                    lines: vec![],
                }
            ],
        };

        let result = parser.extract_modified_methods(&file_change, "", "rust");
        assert!(result.is_ok());
        // 当前实现返回空列表
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_hunk_line_types() {
        let patch_content = r#"diff --git a/test.txt b/test.txt
index 1234567..abcdefg 100644
--- a/test.txt
+++ b/test.txt
@@ -1,4 +1,4 @@
 context line
-removed line
+added line
 another context
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        let hunk = &changes[0].hunks[0];
        
        // 验证行类型
        assert_eq!(hunk.lines[0].line_type, LineType::Context);
        assert_eq!(hunk.lines[1].line_type, LineType::Removed);
        assert_eq!(hunk.lines[2].line_type, LineType::Added);
        assert_eq!(hunk.lines[3].line_type, LineType::Context);
    }

    #[test]
    fn test_parse_patch_with_trailing_content() {
        // 测试 patch 文件在 -- 后面还有内容的情况（如 git format-patch 生成的文件）
        let patch_content = r#"From 7de8e2bd4da14f2744ee7baa83d5fa86dc065700 Mon Sep 17 00:00:00 2001
From: Test User <test@example.com>
Date: Mon, 3 Jun 2024 15:46:03 +0800
Subject: [PATCH] Test patch

---
 test.txt | 1 +
 1 file changed, 1 insertion(+)

diff --git a/test.txt b/test.txt
index 1234567..abcdefg 100644
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+line 2
 line 3
-- 


"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok(), "Should parse patch with trailing content after --");
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "test.txt");
        assert_eq!(changes[0].change_type, ChangeType::Modified);
    }

    #[test]
    fn test_parse_patch_with_version_line_after_separator() {
        // 测试 patch 文件在 -- 后面有版本号的情况
        let patch_content = r#"diff --git a/test.txt b/test.txt
index 1234567..abcdefg 100644
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+line 2
 line 3
-- 
2.39.0

"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok(), "Should parse patch with version line after --");
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "test.txt");
    }

    #[test]
    fn test_parse_patch_with_multiple_trailing_lines() {
        // 测试 patch 文件在 -- 后面有多行内容的情况
        let patch_content = r#"diff --git a/test.txt b/test.txt
index 1234567..abcdefg 100644
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+line 2
 line 3
-- 
Some random text
Another line
2.39.0
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok(), "Should parse patch with multiple trailing lines after --");
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "test.txt");
    }

    #[test]
    fn test_remove_trailing_content() {
        // 测试移除尾部内容的功能
        let content = "diff --git a/test.txt b/test.txt\n\
                       --- a/test.txt\n\
                       +++ b/test.txt\n\
                       @@ -1,1 +1,2 @@\n\
                       +line 1\n\
                       -- \n\
                       2.39.0\n";
        
        let cleaned = PatchParser::remove_trailing_content(content);
        assert!(!cleaned.contains("2.39.0"), "Should remove version number after --");
        assert!(cleaned.contains("-- "), "Should keep the -- separator line");
    }

    #[test]
    fn test_remove_trailing_content_with_empty_line() {
        // 测试 -- 后面只有空行的情况（应该保留）
        let content = "diff --git a/test.txt b/test.txt\n\
                       --- a/test.txt\n\
                       +++ b/test.txt\n\
                       @@ -1,1 +1,2 @@\n\
                       +line 1\n\
                       -- \n\
                       \n\
                       \n";
        
        let cleaned = PatchParser::remove_trailing_content(content);
        assert!(cleaned.contains("-- "), "Should keep the -- separator line");
        // 空行应该被保留
    }

    #[test]
    fn test_remove_trailing_content_no_separator() {
        // 测试没有 -- 分隔符的情况
        let content = "diff --git a/test.txt b/test.txt\n\
                       --- a/test.txt\n\
                       +++ b/test.txt\n\
                       @@ -1,1 +1,2 @@\n\
                       +line 1\n";
        
        let cleaned = PatchParser::remove_trailing_content(content);
        assert_eq!(cleaned, format!("{}\n", content.trim_end()), "Should keep content unchanged");
    }

    #[test]
    fn test_generate_getter_name() {
        assert_eq!(PatchParser::generate_getter_name("userName"), "getUserName");
        assert_eq!(PatchParser::generate_getter_name("age"), "getAge");
        assert_eq!(PatchParser::generate_getter_name("isActive"), "isActive");
        assert_eq!(PatchParser::generate_getter_name("isValid"), "isValid");
    }

    #[test]
    fn test_generate_setter_name() {
        assert_eq!(PatchParser::generate_setter_name("userName"), "setUserName");
        assert_eq!(PatchParser::generate_setter_name("age"), "setAge");
        assert_eq!(PatchParser::generate_setter_name("isActive"), "setActive");
        assert_eq!(PatchParser::generate_setter_name("isValid"), "setValid");
    }

    #[test]
    fn test_detect_java_field() {
        // 正常的字段声明
        assert_eq!(
            PatchParser::detect_java_field("    private String userName;"),
            Some(("String".to_string(), "userName".to_string()))
        );
        
        assert_eq!(
            PatchParser::detect_java_field("public int age;"),
            Some(("int".to_string(), "age".to_string()))
        );
        
        assert_eq!(
            PatchParser::detect_java_field("protected boolean isActive;"),
            Some(("boolean".to_string(), "isActive".to_string()))
        );
        
        // 带初始值的字段
        assert_eq!(
            PatchParser::detect_java_field("private String name = \"test\";"),
            Some(("String".to_string(), "name".to_string()))
        );
        
        // 静态常量
        assert_eq!(
            PatchParser::detect_java_field("public static final int MAX_SIZE = 100;"),
            Some(("int".to_string(), "MAX_SIZE".to_string()))
        );
        
        // 泛型类型
        assert_eq!(
            PatchParser::detect_java_field("private List<String> items;"),
            Some(("List<String>".to_string(), "items".to_string()))
        );
        
        // 不应该匹配的情况
        assert_eq!(PatchParser::detect_java_field("public void doSomething() {"), None);
        assert_eq!(PatchParser::detect_java_field("public class User {"), None);
        assert_eq!(PatchParser::detect_java_field("@Override"), None);
        assert_eq!(PatchParser::detect_java_field("// comment"), None);
        assert_eq!(PatchParser::detect_java_field("import java.util.List;"), None);
    }

    #[test]
    fn test_extract_modified_field_methods() {
        let patch_content = r#"diff --git a/User.java b/User.java
index 1234567..abcdefg 100644
--- a/User.java
+++ b/User.java
@@ -5,7 +5,7 @@ public class User {
     private Long id;
     
-    private String userName;
+    private String username;
     
     private int age;
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        
        let methods = PatchParser::extract_modified_field_methods(&changes[0]);
        
        // 应该生成 getter 和 setter 方法
        assert!(methods.contains(&"getUserName".to_string()));
        assert!(methods.contains(&"setUserName(String)".to_string()));
        assert!(methods.contains(&"getUsername".to_string()));
        assert!(methods.contains(&"setUsername(String)".to_string()));
    }

    #[test]
    fn test_extract_modified_field_methods_boolean() {
        let patch_content = r#"diff --git a/User.java b/User.java
index 1234567..abcdefg 100644
--- a/User.java
+++ b/User.java
@@ -5,7 +5,7 @@ public class User {
     private String name;
     
-    private boolean isActive;
+    private boolean isEnabled;
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        let methods = PatchParser::extract_modified_field_methods(&changes[0]);
        
        // boolean 字段的 getter 保持 isXxx 格式，setter 是 setXxx
        assert!(methods.contains(&"isActive".to_string()));
        assert!(methods.contains(&"setActive(boolean)".to_string()));
        assert!(methods.contains(&"isEnabled".to_string()));
        assert!(methods.contains(&"setEnabled(boolean)".to_string()));
    }

    #[test]
    fn test_extract_modified_field_methods_non_java() {
        let patch_content = r#"diff --git a/test.txt b/test.txt
index 1234567..abcdefg 100644
--- a/test.txt
+++ b/test.txt
@@ -1,2 +1,2 @@
-private String userName;
+private String username;
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(patch_content.as_bytes()).unwrap();
        
        let result = PatchParser::parse_patch_file(temp_file.path());
        assert!(result.is_ok());
        
        let changes = result.unwrap();
        let methods = PatchParser::extract_modified_field_methods(&changes[0]);
        
        // 非 Java 文件不应该生成方法
        assert_eq!(methods.len(), 0);
    }

}
