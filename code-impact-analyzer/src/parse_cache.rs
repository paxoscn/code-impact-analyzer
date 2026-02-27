use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::language_parser::ParsedFile;
use crate::errors::ParseError;

/// 解析缓存
/// 
/// 缓存已解析的文件，避免重复解析相同的文件
/// 
/// # 属性 48: 缓存一致性
/// 对于任意源文件，第一次解析和第二次从缓存读取应该返回完全相同的解析结果
pub struct ParseCache {
    /// 缓存映射: 文件路径 -> 解析结果
    cache: HashMap<PathBuf, ParsedFile>,
}

impl ParseCache {
    /// 创建新的解析缓存
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
    
    /// 获取或解析文件
    /// 
    /// 如果文件已在缓存中，直接返回缓存的结果
    /// 否则，使用提供的解析函数解析文件并缓存结果
    /// 
    /// # Arguments
    /// * `path` - 文件路径
    /// * `parse_fn` - 解析函数，接受文件路径并返回解析结果
    /// 
    /// # Returns
    /// * `Ok(&ParsedFile)` - 解析结果的引用
    /// * `Err(ParseError)` - 解析失败
    pub fn get_or_parse<F>(
        &mut self,
        path: &Path,
        parse_fn: F,
    ) -> Result<&ParsedFile, ParseError>
    where
        F: FnOnce(&Path) -> Result<ParsedFile, ParseError>,
    {
        // 检查缓存中是否已有该文件
        if !self.cache.contains_key(path) {
            // 缓存中没有，执行解析
            let parsed = parse_fn(path)?;
            self.cache.insert(path.to_path_buf(), parsed);
        }
        
        // 返回缓存中的结果
        Ok(self.cache.get(path).unwrap())
    }
    
    /// 清空缓存
    pub fn clear(&mut self) {
        self.cache.clear();
    }
    
    /// 获取缓存大小（缓存的文件数量）
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    
    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
    
    /// 检查文件是否在缓存中
    pub fn contains(&self, path: &Path) -> bool {
        self.cache.contains_key(path)
    }
}

impl Default for ParseCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_parser::{ClassInfo, MethodInfo};
    
    fn create_test_parsed_file(file_path: &Path) -> ParsedFile {
        ParsedFile {
            file_path: file_path.to_path_buf(),
            language: "java".to_string(),
            classes: vec![
                ClassInfo {
                    name: "TestClass".to_string(),
                    methods: vec![
                        MethodInfo {
                            name: "testMethod".to_string(),
                            full_qualified_name: "com.example.TestClass::testMethod".to_string(),
                            file_path: file_path.to_path_buf(),
                            line_range: (10, 20),
                            calls: vec![],
                            http_annotations: None,
                            kafka_operations: vec![],
                            db_operations: vec![],
                            redis_operations: vec![],
                        },
                    ],
                    line_range: (5, 25),
                    is_interface: false,
                    implements: vec![],
                },
            ],
            functions: vec![],
            imports: vec![],
        }
    }
    
    #[test]
    fn test_new_cache() {
        let cache = ParseCache::new();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }
    
    #[test]
    fn test_get_or_parse_first_time() {
        let mut cache = ParseCache::new();
        let path = Path::new("test.java");
        
        let mut parse_count = 0;
        
        let result = cache.get_or_parse(path, |p| {
            parse_count += 1;
            Ok(create_test_parsed_file(p))
        });
        
        assert!(result.is_ok());
        assert_eq!(parse_count, 1);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(path));
    }
    
    #[test]
    fn test_get_or_parse_cached() {
        let mut cache = ParseCache::new();
        let path = Path::new("test.java");
        
        let mut parse_count = 0;
        
        // 第一次解析
        {
            let result1 = cache.get_or_parse(path, |p| {
                parse_count += 1;
                Ok(create_test_parsed_file(p))
            });
            assert!(result1.is_ok());
            assert_eq!(parse_count, 1);
        }
        
        // 第二次应该从缓存读取
        {
            let result2 = cache.get_or_parse(path, |p| {
                parse_count += 1;
                Ok(create_test_parsed_file(p))
            });
            assert!(result2.is_ok());
            assert_eq!(parse_count, 1); // 解析函数不应该被再次调用
        }
        
        // 验证缓存中有该文件
        assert!(cache.contains(path));
    }
    
    #[test]
    fn test_cache_consistency() {
        // 属性 48: 缓存一致性测试
        // 对于任意源文件，第一次解析和第二次从缓存读取应该返回完全相同的解析结果
        
        let mut cache = ParseCache::new();
        let path = Path::new("consistency_test.java");
        
        // 第一次解析并保存结果
        let file_path;
        let language;
        let classes_len;
        let functions_len;
        let imports_len;
        let first_class_name;
        let first_method_name;
        
        {
            let result1 = cache.get_or_parse(path, |p| {
                Ok(create_test_parsed_file(p))
            }).unwrap();
            
            file_path = result1.file_path.clone();
            language = result1.language.clone();
            classes_len = result1.classes.len();
            functions_len = result1.functions.len();
            imports_len = result1.imports.len();
            first_class_name = result1.classes[0].name.clone();
            first_method_name = result1.classes[0].methods[0].name.clone();
        }
        
        // 第二次从缓存读取并验证
        {
            let result2 = cache.get_or_parse(path, |p| {
                Ok(create_test_parsed_file(p))
            }).unwrap();
            
            // 验证结果完全一致
            assert_eq!(result2.file_path, file_path);
            assert_eq!(result2.language, language);
            assert_eq!(result2.classes.len(), classes_len);
            assert_eq!(result2.functions.len(), functions_len);
            assert_eq!(result2.imports.len(), imports_len);
            assert_eq!(result2.classes[0].name, first_class_name);
            assert_eq!(result2.classes[0].methods[0].name, first_method_name);
        }
    }
    
    #[test]
    fn test_multiple_files() {
        let mut cache = ParseCache::new();
        let path1 = Path::new("test1.java");
        let path2 = Path::new("test2.java");
        
        // 解析第一个文件
        let result1 = cache.get_or_parse(path1, |p| {
            Ok(create_test_parsed_file(p))
        });
        assert!(result1.is_ok());
        
        // 解析第二个文件
        let result2 = cache.get_or_parse(path2, |p| {
            Ok(create_test_parsed_file(p))
        });
        assert!(result2.is_ok());
        
        assert_eq!(cache.len(), 2);
        assert!(cache.contains(path1));
        assert!(cache.contains(path2));
    }
    
    #[test]
    fn test_parse_error() {
        let mut cache = ParseCache::new();
        let path = Path::new("error.java");
        
        let result = cache.get_or_parse(path, |_| {
            Err(ParseError::InvalidFormat {
                message: "Test error".to_string(),
            })
        });
        
        assert!(result.is_err());
        assert_eq!(cache.len(), 0); // 错误时不应该缓存
    }
    
    #[test]
    fn test_clear() {
        let mut cache = ParseCache::new();
        let path = Path::new("test.java");
        
        cache.get_or_parse(path, |p| {
            Ok(create_test_parsed_file(p))
        }).unwrap();
        
        assert_eq!(cache.len(), 1);
        
        cache.clear();
        
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert!(!cache.contains(path));
    }
}
