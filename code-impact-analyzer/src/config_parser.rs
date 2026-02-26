use crate::errors::ParseError;
use crate::types::{HttpMethod, HttpEndpoint};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_yaml::Value as YamlValue;
use std::collections::HashSet;

/// 配置数据结构
#[derive(Debug, Clone, Default)]
pub struct ConfigData {
    pub http_endpoints: Vec<HttpEndpoint>,
    pub kafka_topics: Vec<String>,
    pub db_tables: Vec<String>,
    pub redis_prefixes: Vec<String>,
}

/// 配置解析器 trait
pub trait ConfigParser: Send + Sync {
    fn parse(&self, content: &str) -> Result<ConfigData, ParseError>;
    
    /// 检查是否支持指定的配置格式
    fn supports_format(&self, format: &str) -> bool;
}

/// XML 配置解析器
pub struct XmlConfigParser;

impl ConfigParser for XmlConfigParser {
    fn parse(&self, content: &str) -> Result<ConfigData, ParseError> {
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);
        
        let mut config_data = ConfigData::default();
        let mut buf = Vec::new();
        let mut current_text = String::new();
        let mut in_url = false;
        let mut in_topic = false;
        let mut in_table = false;
        let mut in_redis = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = e.name();
                    let tag_name = String::from_utf8_lossy(name.as_ref()).to_lowercase();
                    
                    // 检测 HTTP 相关标签
                    if tag_name.contains("url") || tag_name.contains("endpoint") 
                        || tag_name.contains("api") || tag_name.contains("http") {
                        in_url = true;
                    }
                    // 检测 Kafka 相关标签
                    else if tag_name.contains("topic") || tag_name.contains("kafka") {
                        in_topic = true;
                    }
                    // 检测数据库相关标签
                    else if tag_name.contains("table") || tag_name.contains("entity") 
                        || tag_name.contains("database") {
                        in_table = true;
                    }
                    // 检测 Redis 相关标签
                    else if tag_name.contains("redis") || tag_name.contains("cache") 
                        || tag_name.contains("key") {
                        in_redis = true;
                    }
                    current_text.clear();
                }
                Ok(Event::Text(e)) => {
                    current_text = e.unescape()
                        .map_err(|e| ParseError::InvalidFormat { 
                            message: format!("XML text unescape error: {}", e) 
                        })?
                        .to_string();
                }
                Ok(Event::End(_)) => {
                    let text = current_text.trim();
                    if !text.is_empty() {
                        if in_url {
                            extract_http_endpoint(text, &mut config_data);
                            in_url = false;
                        } else if in_topic {
                            config_data.kafka_topics.push(text.to_string());
                            in_topic = false;
                        } else if in_table {
                            config_data.db_tables.push(text.to_string());
                            in_table = false;
                        } else if in_redis {
                            config_data.redis_prefixes.push(text.to_string());
                            in_redis = false;
                        }
                    }
                    current_text.clear();
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(ParseError::InvalidFormat {
                        message: format!("XML parse error: {}", e),
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        // 去重
        deduplicate_config_data(&mut config_data);
        Ok(config_data)
    }
    
    fn supports_format(&self, format: &str) -> bool {
        format == "xml"
    }
}

/// YAML 配置解析器
pub struct YamlConfigParser;

impl ConfigParser for YamlConfigParser {
    fn parse(&self, content: &str) -> Result<ConfigData, ParseError> {
        let yaml: YamlValue = serde_yaml::from_str(content)
            .map_err(|e| ParseError::InvalidFormat {
                message: format!("YAML parse error: {}", e),
            })?;

        let mut config_data = ConfigData::default();
        extract_from_yaml(&yaml, &mut config_data);
        
        // 去重
        deduplicate_config_data(&mut config_data);
        Ok(config_data)
    }
    
    fn supports_format(&self, format: &str) -> bool {
        format == "yaml"
    }
}

/// 从 YAML 值中递归提取配置信息
fn extract_from_yaml(value: &YamlValue, config_data: &mut ConfigData) {
    match value {
        YamlValue::Mapping(map) => {
            for (key, val) in map {
                if let Some(key_str) = key.as_str() {
                    let key_lower = key_str.to_lowercase();
                    
                    // 检测 HTTP 相关键
                    if key_lower.contains("url") || key_lower.contains("endpoint") 
                        || key_lower.contains("api") || key_lower.contains("http") {
                        if let Some(text) = val.as_str() {
                            extract_http_endpoint(text, config_data);
                        }
                    }
                    // 检测 Kafka 相关键
                    else if key_lower.contains("topic") {
                        // 处理字符串值
                        if let Some(text) = val.as_str() {
                            config_data.kafka_topics.push(text.to_string());
                        }
                        // 处理数组值
                        else if let Some(seq) = val.as_sequence() {
                            for item in seq {
                                if let Some(text) = item.as_str() {
                                    config_data.kafka_topics.push(text.to_string());
                                }
                            }
                        }
                    }
                    // 检测数据库相关键
                    else if key_lower.contains("table") || key_lower.contains("entity") 
                        || key_lower.contains("database") {
                        // 处理字符串值
                        if let Some(text) = val.as_str() {
                            config_data.db_tables.push(text.to_string());
                        }
                        // 处理数组值
                        else if let Some(seq) = val.as_sequence() {
                            for item in seq {
                                if let Some(text) = item.as_str() {
                                    config_data.db_tables.push(text.to_string());
                                }
                            }
                        }
                    }
                    // 检测 Redis 相关键
                    else if key_lower.contains("redis") || key_lower.contains("cache") 
                        || key_lower.contains("key") {
                        // 处理字符串值
                        if let Some(text) = val.as_str() {
                            config_data.redis_prefixes.push(text.to_string());
                        }
                        // 处理数组值
                        else if let Some(seq) = val.as_sequence() {
                            for item in seq {
                                if let Some(text) = item.as_str() {
                                    config_data.redis_prefixes.push(text.to_string());
                                }
                            }
                        }
                    }
                }
                // 递归处理嵌套结构
                extract_from_yaml(val, config_data);
            }
        }
        YamlValue::Sequence(seq) => {
            for item in seq {
                extract_from_yaml(item, config_data);
            }
        }
        _ => {}
    }
}

/// 从文本中提取 HTTP 端点信息
fn extract_http_endpoint(text: &str, config_data: &mut ConfigData) {
    // 尝试从 URL 中提取路径
    if text.starts_with("http://") || text.starts_with("https://") {
        if let Some(path_start) = text.find("://").and_then(|i| text[i+3..].find('/')) {
            let path = &text[path_start + text.find("://").unwrap() + 3..];
            // 移除查询参数
            let path = path.split('?').next().unwrap_or(path);
            config_data.http_endpoints.push(HttpEndpoint {
                method: HttpMethod::GET, // 默认 GET
                path_pattern: path.to_string(),
            });
        }
    } else if text.starts_with('/') {
        // 直接是路径
        let path = text.split('?').next().unwrap_or(text);
        config_data.http_endpoints.push(HttpEndpoint {
            method: HttpMethod::GET, // 默认 GET
            path_pattern: path.to_string(),
        });
    }
}

/// 去重配置数据
fn deduplicate_config_data(config_data: &mut ConfigData) {
    // 去重 HTTP 端点
    let mut seen_endpoints = HashSet::new();
    config_data.http_endpoints.retain(|ep| seen_endpoints.insert(ep.clone()));
    
    // 去重 Kafka topics
    let mut seen_topics = HashSet::new();
    config_data.kafka_topics.retain(|t| seen_topics.insert(t.clone()));
    
    // 去重数据库表
    let mut seen_tables = HashSet::new();
    config_data.db_tables.retain(|t| seen_tables.insert(t.clone()));
    
    // 去重 Redis 前缀
    let mut seen_prefixes = HashSet::new();
    config_data.redis_prefixes.retain(|p| seen_prefixes.insert(p.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_parser_basic() {
        let xml = r#"
            <config>
                <http>
                    <url>http://example.com/api/users</url>
                </http>
                <kafka>
                    <topic>user-events</topic>
                </kafka>
                <database>
                    <table>users</table>
                </database>
                <redis>
                    <key>user:*</key>
                </redis>
            </config>
        "#;

        let parser = XmlConfigParser;
        let result = parser.parse(xml).unwrap();

        assert_eq!(result.http_endpoints.len(), 1);
        assert_eq!(result.http_endpoints[0].path_pattern, "/api/users");
        assert_eq!(result.kafka_topics, vec!["user-events"]);
        assert_eq!(result.db_tables, vec!["users"]);
        assert_eq!(result.redis_prefixes, vec!["user:*"]);
    }

    #[test]
    fn test_yaml_parser_basic() {
        let yaml = r#"
            http:
              url: http://example.com/api/users
            kafka:
              topic: user-events
            database:
              table: users
            redis:
              key: "user:*"
        "#;

        let parser = YamlConfigParser;
        let result = parser.parse(yaml).unwrap();

        assert_eq!(result.http_endpoints.len(), 1);
        assert_eq!(result.http_endpoints[0].path_pattern, "/api/users");
        assert_eq!(result.kafka_topics, vec!["user-events"]);
        assert_eq!(result.db_tables, vec!["users"]);
        assert_eq!(result.redis_prefixes, vec!["user:*"]);
    }

    #[test]
    fn test_xml_parser_nested() {
        let xml = r#"
            <config>
                <services>
                    <service name="user-service">
                        <endpoint>http://localhost:8080/api/users/{id}</endpoint>
                    </service>
                </services>
            </config>
        "#;

        let parser = XmlConfigParser;
        let result = parser.parse(xml).unwrap();

        assert_eq!(result.http_endpoints.len(), 1);
        assert_eq!(result.http_endpoints[0].path_pattern, "/api/users/{id}");
    }

    #[test]
    fn test_yaml_parser_nested() {
        let yaml = r#"
            services:
              user-service:
                api:
                  url: http://localhost:8080/api/users/{id}
                kafka:
                  topics:
                    - user-created
                    - user-updated
        "#;

        let parser = YamlConfigParser;
        let result = parser.parse(yaml).unwrap();

        assert_eq!(result.http_endpoints.len(), 1);
        assert!(result.kafka_topics.contains(&"user-created".to_string()));
        assert!(result.kafka_topics.contains(&"user-updated".to_string()));
    }

    #[test]
    fn test_xml_parser_invalid_format() {
        // Test with mismatched tags which will cause an error
        let xml = "<config><tag>value</invalid>";

        let parser = XmlConfigParser;
        let result = parser.parse(xml);

        assert!(result.is_err());
        match result {
            Err(ParseError::InvalidFormat { .. }) => {}
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_yaml_parser_invalid_format() {
        let yaml = "invalid: yaml: content: [unclosed";

        let parser = YamlConfigParser;
        let result = parser.parse(yaml);

        assert!(result.is_err());
        match result {
            Err(ParseError::InvalidFormat { .. }) => {}
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_deduplication() {
        let xml = r#"
            <config>
                <topic>user-events</topic>
                <topic>user-events</topic>
                <table>users</table>
                <table>users</table>
            </config>
        "#;

        let parser = XmlConfigParser;
        let result = parser.parse(xml).unwrap();

        assert_eq!(result.kafka_topics.len(), 1);
        assert_eq!(result.db_tables.len(), 1);
    }
}
