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

        // 使用 gitpatch crate 解析多个 patch
        let patches = gitpatch::Patch::from_multiple(&content).map_err(|e| ParseError::InvalidFormat {
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
}
