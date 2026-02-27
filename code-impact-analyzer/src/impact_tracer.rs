use std::collections::{HashSet, HashMap};
use crate::code_index::CodeIndex;
use crate::errors::TraceError;
use crate::types::HttpMethod;
use serde::{Deserialize, Serialize};
use petgraph::graph::{DiGraph, NodeIndex};

/// 追溯配置
#[derive(Debug, Clone)]
pub struct TraceConfig {
    /// 最大追溯深度
    pub max_depth: usize,
    /// 是否追溯上游
    pub trace_upstream: bool,
    /// 是否追溯下游
    pub trace_downstream: bool,
    /// 是否追溯跨服务边界
    pub trace_cross_service: bool,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            max_depth: 10,
            trace_upstream: true,
            trace_downstream: true,
            trace_cross_service: true,
        }
    }
}

/// 节点类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    /// 方法节点
    Method { qualified_name: String },
    /// HTTP 端点节点
    HttpEndpoint { path: String, method: String },
    /// Kafka Topic 节点
    KafkaTopic { name: String },
    /// 数据库表节点
    DatabaseTable { name: String },
    /// Redis 键前缀节点
    RedisPrefix { prefix: String },
}

/// 节点元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// 节点标签
    pub label: String,
    /// 附加属性
    pub properties: HashMap<String, String>,
}

/// 影响节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactNode {
    /// 节点唯一标识
    pub id: String,
    /// 节点类型
    pub node_type: NodeType,
    /// 节点元数据
    pub metadata: NodeMetadata,
}

impl ImpactNode {
    /// 创建方法节点
    pub fn method(qualified_name: String) -> Self {
        let id = format!("method:{}", qualified_name);
        Self {
            id: id.clone(),
            node_type: NodeType::Method { qualified_name: qualified_name.clone() },
            metadata: NodeMetadata {
                label: qualified_name,
                properties: HashMap::new(),
            },
        }
    }
    
    /// 创建 HTTP 端点节点
    pub fn http_endpoint(method: HttpMethod, path: String) -> Self {
        let method_str = format!("{:?}", method);
        let id = format!("http:{}:{}", method_str, path);
        Self {
            id: id.clone(),
            node_type: NodeType::HttpEndpoint { 
                path: path.clone(), 
                method: method_str.clone() 
            },
            metadata: NodeMetadata {
                label: format!("{} {}", method_str, path),
                properties: HashMap::new(),
            },
        }
    }
    
    /// 创建 Kafka Topic 节点
    pub fn kafka_topic(name: String) -> Self {
        let id = format!("kafka:{}", name);
        Self {
            id: id.clone(),
            node_type: NodeType::KafkaTopic { name: name.clone() },
            metadata: NodeMetadata {
                label: format!("Kafka: {}", name),
                properties: HashMap::new(),
            },
        }
    }
    
    /// 创建数据库表节点
    pub fn database_table(name: String) -> Self {
        let id = format!("db:{}", name);
        Self {
            id: id.clone(),
            node_type: NodeType::DatabaseTable { name: name.clone() },
            metadata: NodeMetadata {
                label: format!("Table: {}", name),
                properties: HashMap::new(),
            },
        }
    }
    
    /// 创建 Redis 键前缀节点
    pub fn redis_prefix(prefix: String) -> Self {
        let id = format!("redis:{}", prefix);
        Self {
            id: id.clone(),
            node_type: NodeType::RedisPrefix { prefix: prefix.clone() },
            metadata: NodeMetadata {
                label: format!("Redis: {}", prefix),
                properties: HashMap::new(),
            },
        }
    }
}

/// 边类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    /// 方法调用
    MethodCall,
    /// HTTP 调用
    HttpCall,
    /// Kafka 生产/消费
    KafkaProduceConsume,
    /// 数据库读写
    DatabaseReadWrite,
    /// Redis 读写
    RedisReadWrite,
}

/// 边方向
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// 上游（调用者）
    Upstream,
    /// 下游（被调用者）
    Downstream,
}

/// 影响边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactEdge {
    /// 起始节点 ID
    pub from: String,
    /// 目标节点 ID
    pub to: String,
    /// 边类型
    pub edge_type: EdgeType,
    /// 边方向
    pub direction: Direction,
}

/// 影响图（使用 petgraph 的 DiGraph 实现）
#[derive(Debug)]
pub struct ImpactGraph {
    /// petgraph 有向图
    graph: DiGraph<ImpactNode, ImpactEdge>,
    /// 节点 ID 到 NodeIndex 的映射
    node_map: HashMap<String, NodeIndex>,
}

impl Default for ImpactGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ImpactGraph {
    /// 创建新的影响图
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }
    
    /// 添加节点
    /// 
    /// # Arguments
    /// * `node` - 影响节点
    /// 
    /// # Returns
    /// * `NodeIndex` - 节点在图中的索引
    pub fn add_node(&mut self, node: ImpactNode) -> NodeIndex {
        let node_id = node.id.clone();
        
        // 如果节点已存在，返回现有索引
        if let Some(&index) = self.node_map.get(&node_id) {
            return index;
        }
        
        // 添加新节点
        let index = self.graph.add_node(node);
        self.node_map.insert(node_id, index);
        index
    }
    
    /// 添加边
    /// 
    /// # Arguments
    /// * `from` - 起始节点 ID
    /// * `to` - 目标节点 ID
    /// * `edge_type` - 边类型
    /// * `direction` - 边方向
    pub fn add_edge(&mut self, from: &str, to: &str, edge_type: EdgeType, direction: Direction) {
        // 获取节点索引
        let from_index = match self.node_map.get(from) {
            Some(&index) => index,
            None => return, // 节点不存在，跳过
        };
        
        let to_index = match self.node_map.get(to) {
            Some(&index) => index,
            None => return, // 节点不存在，跳过
        };
        
        // 创建边
        let edge = ImpactEdge {
            from: from.to_string(),
            to: to.to_string(),
            edge_type,
            direction,
        };
        
        // 添加边到图中
        self.graph.add_edge(from_index, to_index, edge);
    }
    
    /// 获取所有节点
    pub fn nodes(&self) -> impl Iterator<Item = &ImpactNode> {
        self.graph.node_weights()
    }
    
    /// 获取所有边
    pub fn edges(&self) -> impl Iterator<Item = &ImpactEdge> {
        self.graph.edge_weights()
    }
    
    /// 获取节点数量
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
    
    /// 获取边数量
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
    
    /// 获取节点索引
    pub fn get_node_index(&self, node_id: &str) -> Option<NodeIndex> {
        self.node_map.get(node_id).copied()
    }
    
    /// 获取节点
    pub fn get_node(&self, node_id: &str) -> Option<&ImpactNode> {
        self.node_map.get(node_id)
            .and_then(|&index| self.graph.node_weight(index))
    }
    
    /// 获取底层 petgraph DiGraph 的引用
    pub fn graph(&self) -> &DiGraph<ImpactNode, ImpactEdge> {
        &self.graph
    }
    
    /// 输出为 DOT 格式（用于 Graphviz 可视化）
    /// 
    /// # Returns
    /// * `String` - DOT 格式的图描述
    pub fn to_dot(&self) -> String {
        use petgraph::dot::{Dot, Config};
        
        // 使用 petgraph 的 Dot 格式化器
        let dot = Dot::with_attr_getters(
            &self.graph,
            &[Config::EdgeNoLabel, Config::NodeNoLabel],
            &|_, edge| {
                let edge_data = edge.weight();
                let edge_type_str = match edge_data.edge_type {
                    EdgeType::MethodCall => "method_call",
                    EdgeType::HttpCall => "http_call",
                    EdgeType::KafkaProduceConsume => "kafka",
                    EdgeType::DatabaseReadWrite => "database",
                    EdgeType::RedisReadWrite => "redis",
                };
                let direction_str = match edge_data.direction {
                    Direction::Upstream => "upstream",
                    Direction::Downstream => "downstream",
                };
                format!("label=\"{}\" dir=\"{}\"", edge_type_str, direction_str)
            },
            &|_, (_, node)| {
                let node_type_str = match &node.node_type {
                    NodeType::Method { .. } => "method",
                    NodeType::HttpEndpoint { .. } => "http",
                    NodeType::KafkaTopic { .. } => "kafka",
                    NodeType::DatabaseTable { .. } => "database",
                    NodeType::RedisPrefix { .. } => "redis",
                };
                let shape = match &node.node_type {
                    NodeType::Method { .. } => "box",
                    NodeType::HttpEndpoint { .. } => "ellipse",
                    NodeType::KafkaTopic { .. } => "diamond",
                    NodeType::DatabaseTable { .. } => "cylinder",
                    NodeType::RedisPrefix { .. } => "hexagon",
                };
                format!("label=\"{}\" shape=\"{}\" type=\"{}\"", 
                    node.metadata.label, shape, node_type_str)
            },
        );
        
        format!("{:?}", dot)
    }
    
    /// 输出为 JSON 格式
    /// 
    /// # Returns
    /// * `Ok(String)` - JSON 格式的图描述
    /// * `Err(serde_json::Error)` - 序列化错误
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        use serde_json::json;
        
        // 收集所有节点
        let nodes: Vec<_> = self.graph.node_weights()
            .map(|node| {
                json!({
                    "id": node.id,
                    "type": match &node.node_type {
                        NodeType::Method { qualified_name } => json!({
                            "kind": "method",
                            "qualified_name": qualified_name
                        }),
                        NodeType::HttpEndpoint { path, method } => json!({
                            "kind": "http_endpoint",
                            "path": path,
                            "method": method
                        }),
                        NodeType::KafkaTopic { name } => json!({
                            "kind": "kafka_topic",
                            "name": name
                        }),
                        NodeType::DatabaseTable { name } => json!({
                            "kind": "database_table",
                            "name": name
                        }),
                        NodeType::RedisPrefix { prefix } => json!({
                            "kind": "redis_prefix",
                            "prefix": prefix
                        }),
                    },
                    "label": node.metadata.label,
                    "properties": node.metadata.properties
                })
            })
            .collect();
        
        // 收集所有边
        let edges: Vec<_> = self.graph.edge_weights()
            .map(|edge| {
                json!({
                    "from": edge.from,
                    "to": edge.to,
                    "type": match edge.edge_type {
                        EdgeType::MethodCall => "method_call",
                        EdgeType::HttpCall => "http_call",
                        EdgeType::KafkaProduceConsume => "kafka_produce_consume",
                        EdgeType::DatabaseReadWrite => "database_read_write",
                        EdgeType::RedisReadWrite => "redis_read_write",
                    },
                    "direction": match edge.direction {
                        Direction::Upstream => "upstream",
                        Direction::Downstream => "downstream",
                    }
                })
            })
            .collect();
        
        // 构建完整的 JSON 对象
        let graph_json = json!({
            "nodes": nodes,
            "edges": edges,
            "node_count": self.node_count(),
            "edge_count": self.edge_count()
        });
        
        serde_json::to_string_pretty(&graph_json)
    }
    
    /// 检测图中的循环依赖
    /// 
    /// # Returns
    /// * `Vec<Vec<String>>` - 循环路径列表，每个循环是一个节点 ID 列表
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        use petgraph::algo::tarjan_scc;
        
        // 使用 Tarjan 算法查找强连通分量
        let sccs = tarjan_scc(&self.graph);
        
        // 过滤出包含多个节点的强连通分量（即循环）
        let mut cycles = Vec::new();
        for scc in sccs {
            if scc.len() > 1 {
                // 这是一个循环
                let cycle: Vec<String> = scc.iter()
                    .filter_map(|&node_idx| {
                        self.graph.node_weight(node_idx)
                            .map(|node| node.id.clone())
                    })
                    .collect();
                cycles.push(cycle);
            }
        }
        
        cycles
    }
}

/// 影响追溯器
pub struct ImpactTracer<'a> {
    /// 代码索引引用
    index: &'a CodeIndex,
    /// 追溯配置
    config: TraceConfig,
}

impl<'a> ImpactTracer<'a> {
    /// 创建新的影响追溯器
    pub fn new(index: &'a CodeIndex, config: TraceConfig) -> Self {
        Self { index, config }
    }
    
    /// 追溯影响
    /// 
    /// # Arguments
    /// * `changed_methods` - 变更的方法列表
    /// 
    /// # Returns
    /// * `Ok(ImpactGraph)` - 影响图
    /// * `Err(TraceError)` - 追溯错误
    pub fn trace_impact(&self, changed_methods: &[String]) -> Result<ImpactGraph, TraceError> {
        let mut graph = ImpactGraph::new();
        let mut visited = HashSet::new();
        
        for method in changed_methods {
            // 添加变更方法节点
            let node = ImpactNode::method(method.clone());
            graph.add_node(node);
            
            // 追溯上游
            if self.config.trace_upstream {
                self.trace_method_upstream(method, 0, &mut visited, &mut graph);
            }
            
            // 追溯下游
            if self.config.trace_downstream {
                // 为下游追溯创建新的 visited 集合
                let mut downstream_visited = HashSet::new();
                self.trace_method_downstream(method, 0, &mut downstream_visited, &mut graph);
            }
        }
        
        Ok(graph)
    }
    
    /// 追溯方法的上游调用链（DFS）
    /// 
    /// # Arguments
    /// * `method` - 方法名
    /// * `depth` - 当前深度
    /// * `visited` - 已访问节点集合（用于循环检测）
    /// * `graph` - 影响图
    fn trace_method_upstream(
        &self,
        method: &str,
        depth: usize,
        visited: &mut HashSet<String>,
        graph: &mut ImpactGraph,
    ) {
        // 深度限制检查
        if depth >= self.config.max_depth {
            return;
        }
        
        // 循环检测
        if visited.contains(method) {
            return;
        }
        
        visited.insert(method.to_string());
        
        // 查找所有调用当前方法的方法（上游）
        let mut all_callers = self.index.find_callers(method);
        
        // 查找该方法所在类实现的所有接口中的同名方法的调用者
        if let Some(pos) = method.rfind("::") {
            let class_name = &method[..pos];
            let method_name = &method[pos + 2..];
            
            // 获取该类实现的所有接口
            let interfaces = self.index.find_class_interfaces(class_name);
            
            for interface_name in interfaces {
                // 构建接口方法的完整限定名
                let interface_method = format!("{}::{}", interface_name, method_name);
                
                // 查找调用接口方法的调用者
                let interface_callers = self.index.find_callers(&interface_method);
                
                // 合并到总的调用者列表中
                all_callers.extend(interface_callers);
            }
        }
        
        if method.contains("sendCoupon") {
            println!("method = {}, all_callers = {:?}", method, all_callers);
        }
        
        for caller in all_callers {
            // 解析接口调用：如果调用者调用的是接口方法，且接口只有一个实现类，
            // 则将调用目标替换为实现类的方法
            let resolved_caller = self.index.resolve_interface_call(caller);
            
            // 检查调用者是否在索引中（忽略外部库）
            if self.index.find_method(&resolved_caller).is_none() {
                continue;
            }
            
            // 添加调用者节点
            let caller_node = ImpactNode::method(resolved_caller.clone());
            graph.add_node(caller_node);
            
            // 构建节点 ID
            let caller_id = format!("method:{}", resolved_caller);
            let method_id = format!("method:{}", method);
            
            // 添加边：caller -> method
            graph.add_edge(
                &caller_id,
                &method_id,
                EdgeType::MethodCall,
                Direction::Upstream,
            );
            
            // 递归追溯上游
            self.trace_method_upstream(&resolved_caller, depth + 1, visited, graph);
        }
        
        // 跨服务追溯（上游方向）
        if self.config.trace_cross_service {
            self.trace_cross_service(method, visited, graph);
        }
    }
    
    /// 追溯方法的下游调用链（DFS）
    /// 
    /// # Arguments
    /// * `method` - 方法名
    /// * `depth` - 当前深度
    /// * `visited` - 已访问节点集合（用于循环检测）
    /// * `graph` - 影响图
    fn trace_method_downstream(
        &self,
        method: &str,
        depth: usize,
        visited: &mut HashSet<String>,
        graph: &mut ImpactGraph,
    ) {
        // 深度限制检查
        if depth >= self.config.max_depth {
            return;
        }
        
        // 循环检测
        if visited.contains(method) {
            return;
        }
        
        visited.insert(method.to_string());
        
        // 查找当前方法调用的所有方法（下游）
        let callees = self.index.find_callees(method);
        
        for callee in callees {
            // 解析接口调用：如果被调用的是接口方法，且接口只有一个实现类，
            // 则将调用目标替换为实现类的方法
            let resolved_callee = self.index.resolve_interface_call(callee);
            
            // 检查被调用者是否在索引中（忽略外部库）
            if self.index.find_method(&resolved_callee).is_none() {
                continue;
            }
            
            // 添加被调用者节点
            let callee_node = ImpactNode::method(resolved_callee.clone());
            graph.add_node(callee_node);
            
            // 构建节点 ID
            let method_id = format!("method:{}", method);
            let callee_id = format!("method:{}", resolved_callee);
            
            // 添加边：method -> callee
            graph.add_edge(
                &method_id,
                &callee_id,
                EdgeType::MethodCall,
                Direction::Downstream,
            );
            
            // 递归追溯下游
            self.trace_method_downstream(&resolved_callee, depth + 1, visited, graph);
        }
        
        // 跨服务追溯
        if self.config.trace_cross_service {
            self.trace_cross_service(method, visited, graph);
        }
    }
    
    /// 追溯跨服务边界的调用关系
    /// 
    /// # Arguments
    /// * `method` - 方法名
    /// * `visited` - 已访问节点集合（用于循环检测）
    /// * `graph` - 影响图
    fn trace_cross_service(
        &self,
        method: &str,
        visited: &mut HashSet<String>,
        graph: &mut ImpactGraph,
    ) {
        // 获取方法信息
        let method_info = match self.index.find_method(method) {
            Some(info) => info,
            None => return,
        };
        
        // 1. HTTP 接口追溯
        self.trace_http_interface(method, method_info, visited, graph);
        
        // 2. Kafka Topic 追溯
        self.trace_kafka_topic(method, method_info, visited, graph);
        
        // 3. 数据库表追溯
        self.trace_database_table(method, method_info, visited, graph);
        
        // 4. Redis 键追溯
        self.trace_redis_key(method, method_info, visited, graph);
    }
    
    /// 追溯 HTTP 接口的双向关系
    fn trace_http_interface(
        &self,
        method: &str,
        method_info: &crate::language_parser::MethodInfo,
        visited: &mut HashSet<String>,
        graph: &mut ImpactGraph,
    ) {
        use crate::types::HttpEndpoint;
        
        // 如果当前方法有 HTTP 注解
        if let Some(http_annotation) = &method_info.http_annotations {
            let endpoint = HttpEndpoint {
                method: http_annotation.method.clone(),
                path_pattern: http_annotation.path.clone(),
            };
            
            // 创建 HTTP 端点节点
            let endpoint_node = ImpactNode::http_endpoint(
                http_annotation.method.clone(),
                http_annotation.path.clone(),
            );
            let endpoint_id = endpoint_node.id.clone();
            graph.add_node(endpoint_node);
            
            // 方法节点 ID
            let method_id = format!("method:{}", method);
            
            // 根据 is_feign_client 标志判断是提供者还是消费者
            if http_annotation.is_feign_client {
                // Feign 调用：HTTP 节点是方法的下游
                // 添加边：method -> endpoint (调用者 -> 被调用的HTTP接口)
                graph.add_edge(
                    &method_id,
                    &endpoint_id,
                    EdgeType::HttpCall,
                    Direction::Downstream,
                );
                
                // 查找提供该接口的方法（其他服务）
                let providers = self.index.find_http_providers(&endpoint);
                for provider in providers {
                    if !visited.contains(provider) {
                        // 添加提供者节点
                        let provider_node = ImpactNode::method(provider.to_string());
                        let provider_id = provider_node.id.clone();
                        graph.add_node(provider_node);
                        
                        // 添加边：endpoint -> provider (HTTP接口 -> 提供者方法)
                        graph.add_edge(
                            &endpoint_id,
                            &provider_id,
                            EdgeType::HttpCall,
                            Direction::Downstream,
                        );
                        
                        // 继续追溯提供者的下游
                        let mut provider_visited = visited.clone();
                        self.trace_method_downstream(provider, 0, &mut provider_visited, graph);
                    }
                }
            } else {
                // HTTP 接口声明：HTTP 节点是方法的上游
                // 添加边：endpoint -> method (HTTP接口 -> 提供者方法)
                graph.add_edge(
                    &endpoint_id,
                    &method_id,
                    EdgeType::HttpCall,
                    Direction::Upstream,
                );
                
                // 查找所有调用该接口的消费者（Feign 客户端）
                let consumers = self.index.find_http_consumers(&endpoint);
                for consumer in consumers {
                    if !visited.contains(consumer) {
                        // 添加消费者节点
                        let consumer_node = ImpactNode::method(consumer.to_string());
                        let consumer_id = consumer_node.id.clone();
                        graph.add_node(consumer_node);
                        
                        // 添加边：consumer -> endpoint (消费者方法 -> HTTP接口)
                        graph.add_edge(
                            &consumer_id,
                            &endpoint_id,
                            EdgeType::HttpCall,
                            Direction::Upstream,
                        );
                        
                        // 继续追溯消费者的上游
                        let mut consumer_visited = visited.clone();
                        self.trace_method_upstream(consumer, 0, &mut consumer_visited, graph);
                    }
                }
            }
        }
    }
    
    /// 追溯 Kafka Topic 的双向关系
    fn trace_kafka_topic(
        &self,
        method: &str,
        method_info: &crate::language_parser::MethodInfo,
        visited: &mut HashSet<String>,
        graph: &mut ImpactGraph,
    ) {
        use crate::types::KafkaOpType;
        
        let method_id = format!("method:{}", method);
        
        for kafka_op in &method_info.kafka_operations {
            let topic_node = ImpactNode::kafka_topic(kafka_op.topic.clone());
            let topic_id = topic_node.id.clone();
            graph.add_node(topic_node);
            
            match kafka_op.operation_type {
                KafkaOpType::Produce => {
                    // 当前方法是生产者
                    // 添加边：method -> topic
                    graph.add_edge(
                        &method_id,
                        &topic_id,
                        EdgeType::KafkaProduceConsume,
                        Direction::Downstream,
                    );
                    
                    // 查找所有消费该 Topic 的消费者
                    let consumers = self.index.find_kafka_consumers(&kafka_op.topic);
                    for consumer in consumers {
                        if !visited.contains(consumer) {
                            // 添加消费者节点
                            let consumer_node = ImpactNode::method(consumer.to_string());
                            let consumer_id = consumer_node.id.clone();
                            graph.add_node(consumer_node);
                            
                            // 添加边：topic -> consumer
                            graph.add_edge(
                                &topic_id,
                                &consumer_id,
                                EdgeType::KafkaProduceConsume,
                                Direction::Downstream,
                            );
                            
                            // 继续追溯消费者的下游
                            let mut consumer_visited = visited.clone();
                            self.trace_method_downstream(consumer, 0, &mut consumer_visited, graph);
                        }
                    }
                }
                KafkaOpType::Consume => {
                    // 当前方法是消费者
                    // 添加边：topic -> method
                    graph.add_edge(
                        &topic_id,
                        &method_id,
                        EdgeType::KafkaProduceConsume,
                        Direction::Upstream,
                    );
                    
                    // 查找所有生产该 Topic 的生产者
                    let producers = self.index.find_kafka_producers(&kafka_op.topic);
                    for producer in producers {
                        if !visited.contains(producer) {
                            // 添加生产者节点
                            let producer_node = ImpactNode::method(producer.to_string());
                            let producer_id = producer_node.id.clone();
                            graph.add_node(producer_node);
                            
                            // 添加边：producer -> topic
                            graph.add_edge(
                                &producer_id,
                                &topic_id,
                                EdgeType::KafkaProduceConsume,
                                Direction::Upstream,
                            );
                            
                            // 继续追溯生产者的上游
                            let mut producer_visited = visited.clone();
                            self.trace_method_upstream(producer, 0, &mut producer_visited, graph);
                        }
                    }
                }
            }
        }
    }
    
    /// 追溯数据库表的双向关系
    fn trace_database_table(
        &self,
        method: &str,
        method_info: &crate::language_parser::MethodInfo,
        visited: &mut HashSet<String>,
        graph: &mut ImpactGraph,
    ) {
        use crate::types::DbOpType;
        
        let method_id = format!("method:{}", method);
        
        for db_op in &method_info.db_operations {
            let table_node = ImpactNode::database_table(db_op.table.clone());
            let table_id = table_node.id.clone();
            graph.add_node(table_node);
            
            match db_op.operation_type {
                DbOpType::Select => {
                    // 当前方法读取表
                    // 添加边：table -> method
                    graph.add_edge(
                        &table_id,
                        &method_id,
                        EdgeType::DatabaseReadWrite,
                        Direction::Upstream,
                    );
                    
                    // 查找所有写入该表的方法
                    let writers = self.index.find_db_writers(&db_op.table);
                    for writer in writers {
                        if !visited.contains(writer) {
                            // 添加写入者节点
                            let writer_node = ImpactNode::method(writer.to_string());
                            let writer_id = writer_node.id.clone();
                            graph.add_node(writer_node);
                            
                            // 添加边：writer -> table
                            graph.add_edge(
                                &writer_id,
                                &table_id,
                                EdgeType::DatabaseReadWrite,
                                Direction::Upstream,
                            );
                            
                            // 继续追溯写入者的上游
                            let mut writer_visited = visited.clone();
                            self.trace_method_upstream(writer, 0, &mut writer_visited, graph);
                        }
                    }
                }
                DbOpType::Insert | DbOpType::Update | DbOpType::Delete => {
                    // 当前方法写入表
                    // 添加边：method -> table
                    graph.add_edge(
                        &method_id,
                        &table_id,
                        EdgeType::DatabaseReadWrite,
                        Direction::Downstream,
                    );
                    
                    // 查找所有读取该表的方法
                    let readers = self.index.find_db_readers(&db_op.table);
                    for reader in readers {
                        if !visited.contains(reader) {
                            // 添加读取者节点
                            let reader_node = ImpactNode::method(reader.to_string());
                            let reader_id = reader_node.id.clone();
                            graph.add_node(reader_node);
                            
                            // 添加边：table -> reader
                            graph.add_edge(
                                &table_id,
                                &reader_id,
                                EdgeType::DatabaseReadWrite,
                                Direction::Downstream,
                            );
                            
                            // 继续追溯读取者的下游
                            let mut reader_visited = visited.clone();
                            self.trace_method_downstream(reader, 0, &mut reader_visited, graph);
                        }
                    }
                }
            }
        }
    }
    
    /// 追溯 Redis 键的双向关系
    fn trace_redis_key(
        &self,
        method: &str,
        method_info: &crate::language_parser::MethodInfo,
        visited: &mut HashSet<String>,
        graph: &mut ImpactGraph,
    ) {
        use crate::types::RedisOpType;
        
        let method_id = format!("method:{}", method);
        
        for redis_op in &method_info.redis_operations {
            let redis_node = ImpactNode::redis_prefix(redis_op.key_pattern.clone());
            let redis_id = redis_node.id.clone();
            graph.add_node(redis_node);
            
            match redis_op.operation_type {
                RedisOpType::Get => {
                    // 当前方法读取 Redis 键
                    // 添加边：redis -> method
                    graph.add_edge(
                        &redis_id,
                        &method_id,
                        EdgeType::RedisReadWrite,
                        Direction::Upstream,
                    );
                    
                    // 查找所有写入该键的方法
                    let writers = self.index.find_redis_writers(&redis_op.key_pattern);
                    for writer in writers {
                        if !visited.contains(writer) {
                            // 添加写入者节点
                            let writer_node = ImpactNode::method(writer.to_string());
                            let writer_id = writer_node.id.clone();
                            graph.add_node(writer_node);
                            
                            // 添加边：writer -> redis
                            graph.add_edge(
                                &writer_id,
                                &redis_id,
                                EdgeType::RedisReadWrite,
                                Direction::Upstream,
                            );
                            
                            // 继续追溯写入者的上游
                            let mut writer_visited = visited.clone();
                            self.trace_method_upstream(writer, 0, &mut writer_visited, graph);
                        }
                    }
                }
                RedisOpType::Set | RedisOpType::Delete => {
                    // 当前方法写入 Redis 键
                    // 添加边：method -> redis
                    graph.add_edge(
                        &method_id,
                        &redis_id,
                        EdgeType::RedisReadWrite,
                        Direction::Downstream,
                    );
                    
                    // 查找所有读取该键的方法
                    let readers = self.index.find_redis_readers(&redis_op.key_pattern);
                    for reader in readers {
                        if !visited.contains(reader) {
                            // 添加读取者节点
                            let reader_node = ImpactNode::method(reader.to_string());
                            let reader_id = reader_node.id.clone();
                            graph.add_node(reader_node);
                            
                            // 添加边：redis -> reader
                            graph.add_edge(
                                &redis_id,
                                &reader_id,
                                EdgeType::RedisReadWrite,
                                Direction::Downstream,
                            );
                            
                            // 继续追溯读取者的下游
                            let mut reader_visited = visited.clone();
                            self.trace_method_downstream(reader, 0, &mut reader_visited, graph);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_trace_config_default() {
        let config = TraceConfig::default();
        assert_eq!(config.max_depth, 10);
        assert!(config.trace_upstream);
        assert!(config.trace_downstream);
        assert!(config.trace_cross_service);
    }
    
    #[test]
    fn test_impact_node_creation() {
        let method_node = ImpactNode::method("com.example.Test::test".to_string());
        assert_eq!(method_node.id, "method:com.example.Test::test");
        assert!(matches!(method_node.node_type, NodeType::Method { .. }));
        
        let http_node = ImpactNode::http_endpoint(HttpMethod::GET, "/api/test".to_string());
        assert_eq!(http_node.id, "http:GET:/api/test");
        assert!(matches!(http_node.node_type, NodeType::HttpEndpoint { .. }));
        
        let kafka_node = ImpactNode::kafka_topic("test-topic".to_string());
        assert_eq!(kafka_node.id, "kafka:test-topic");
        assert!(matches!(kafka_node.node_type, NodeType::KafkaTopic { .. }));
        
        let db_node = ImpactNode::database_table("users".to_string());
        assert_eq!(db_node.id, "db:users");
        assert!(matches!(db_node.node_type, NodeType::DatabaseTable { .. }));
        
        let redis_node = ImpactNode::redis_prefix("user:*".to_string());
        assert_eq!(redis_node.id, "redis:user:*");
        assert!(matches!(redis_node.node_type, NodeType::RedisPrefix { .. }));
    }
    
    #[test]
    fn test_impact_graph_add_node() {
        let mut graph = ImpactGraph::new();
        let node = ImpactNode::method("com.example.Test::test".to_string());
        let node_id = node.id.clone();
        
        let index = graph.add_node(node);
        
        assert_eq!(graph.node_count(), 1);
        assert!(graph.get_node(&node_id).is_some());
        assert_eq!(graph.get_node_index(&node_id), Some(index));
    }
    
    #[test]
    fn test_impact_graph_add_edge() {
        let mut graph = ImpactGraph::new();
        
        // 先添加节点
        let node_a = ImpactNode::method("A".to_string());
        let node_b = ImpactNode::method("B".to_string());
        graph.add_node(node_a);
        graph.add_node(node_b);
        
        // 添加边
        graph.add_edge(
            "method:A",
            "method:B",
            EdgeType::MethodCall,
            Direction::Downstream,
        );
        
        assert_eq!(graph.edge_count(), 1);
        
        // 验证边的属性
        let edge = graph.edges().next().unwrap();
        assert_eq!(edge.from, "method:A");
        assert_eq!(edge.to, "method:B");
        assert_eq!(edge.edge_type, EdgeType::MethodCall);
        assert_eq!(edge.direction, Direction::Downstream);
    }
    
    #[test]
    fn test_impact_tracer_creation() {
        let index = CodeIndex::new();
        let config = TraceConfig::default();
        let tracer = ImpactTracer::new(&index, config);
        
        assert_eq!(tracer.config.max_depth, 10);
    }
    
    #[test]
    fn test_trace_empty_methods() {
        let index = CodeIndex::new();
        let config = TraceConfig::default();
        let tracer = ImpactTracer::new(&index, config);
        
        let result = tracer.trace_impact(&[]);
        assert!(result.is_ok());
        
        let graph = result.unwrap();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }
    
    #[test]
    fn test_trace_single_method_no_calls() {
        let index = CodeIndex::new();
        let config = TraceConfig::default();
        let tracer = ImpactTracer::new(&index, config);
        
        let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
        assert!(result.is_ok());
        
        let graph = result.unwrap();
        // 应该只有一个节点（变更的方法本身）
        assert_eq!(graph.node_count(), 1);
        assert!(graph.get_node("method:com.example.Test::test").is_some());
        // 没有边
        assert_eq!(graph.edge_count(), 0);
    }
    
    #[test]
    fn test_depth_limit() {
        let index = CodeIndex::new();
        let config = TraceConfig {
            max_depth: 0,
            trace_upstream: true,
            trace_downstream: true,
            trace_cross_service: false,
        };
        let tracer = ImpactTracer::new(&index, config);
        
        let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
        assert!(result.is_ok());
        
        let graph = result.unwrap();
        // 深度为 0，应该只有初始节点
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
    }
    
    #[test]
    fn test_cycle_detection() {
        let index = CodeIndex::new();
        let config = TraceConfig::default();
        let tracer = ImpactTracer::new(&index, config);
        
        // 即使有循环调用，追溯也应该正常完成（通过 visited 集合防止无限递归）
        let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_upstream_only_config() {
        let index = CodeIndex::new();
        let config = TraceConfig {
            max_depth: 10,
            trace_upstream: true,
            trace_downstream: false,
            trace_cross_service: false,
        };
        let tracer = ImpactTracer::new(&index, config);
        
        let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_downstream_only_config() {
        let index = CodeIndex::new();
        let config = TraceConfig {
            max_depth: 10,
            trace_upstream: false,
            trace_downstream: true,
            trace_cross_service: false,
        };
        let tracer = ImpactTracer::new(&index, config);
        
        let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_cross_service_disabled() {
        let index = CodeIndex::new();
        let config = TraceConfig {
            max_depth: 10,
            trace_upstream: true,
            trace_downstream: true,
            trace_cross_service: false,
        };
        let tracer = ImpactTracer::new(&index, config);
        
        let result = tracer.trace_impact(&["com.example.Test::test".to_string()]);
        assert!(result.is_ok());
        
        // 跨服务追溯被禁用，不应该有跨服务节点
        let graph = result.unwrap();
        for node in graph.nodes() {
            assert!(matches!(node.node_type, NodeType::Method { .. }));
        }
    }
    
    #[test]
    fn test_duplicate_node_handling() {
        let mut graph = ImpactGraph::new();
        let node1 = ImpactNode::method("com.example.Test::test".to_string());
        let node2 = ImpactNode::method("com.example.Test::test".to_string());
        
        let index1 = graph.add_node(node1);
        let index2 = graph.add_node(node2);
        
        // 重复添加相同节点应该返回相同的索引
        assert_eq!(index1, index2);
        assert_eq!(graph.node_count(), 1);
    }
    
    #[test]
    fn test_add_edge_with_nonexistent_nodes() {
        let mut graph = ImpactGraph::new();
        
        // 尝试添加边，但节点不存在
        graph.add_edge(
            "method:NonExistent1",
            "method:NonExistent2",
            EdgeType::MethodCall,
            Direction::Downstream,
        );
        
        // 边不应该被添加
        assert_eq!(graph.edge_count(), 0);
    }
    
    #[test]
    fn test_get_node_methods() {
        let mut graph = ImpactGraph::new();
        let node = ImpactNode::method("com.example.Test::test".to_string());
        let node_id = node.id.clone();
        
        graph.add_node(node);
        
        // 测试 get_node
        assert!(graph.get_node(&node_id).is_some());
        assert!(graph.get_node("nonexistent").is_none());
        
        // 测试 get_node_index
        assert!(graph.get_node_index(&node_id).is_some());
        assert!(graph.get_node_index("nonexistent").is_none());
    }
    
    #[test]
    fn test_to_dot_output() {
        let mut graph = ImpactGraph::new();
        
        // 添加一些节点
        let node_a = ImpactNode::method("com.example.A::methodA".to_string());
        let node_b = ImpactNode::method("com.example.B::methodB".to_string());
        graph.add_node(node_a);
        graph.add_node(node_b);
        
        // 添加边
        graph.add_edge(
            "method:com.example.A::methodA",
            "method:com.example.B::methodB",
            EdgeType::MethodCall,
            Direction::Downstream,
        );
        
        // 生成 DOT 格式
        let dot = graph.to_dot();
        
        // 验证 DOT 输出包含关键元素
        assert!(dot.contains("digraph"));
        assert!(dot.contains("com.example.A::methodA"));
        assert!(dot.contains("com.example.B::methodB"));
    }
    
    #[test]
    fn test_to_json_output() {
        let mut graph = ImpactGraph::new();
        
        // 添加不同类型的节点
        let method_node = ImpactNode::method("com.example.Test::test".to_string());
        let http_node = ImpactNode::http_endpoint(HttpMethod::GET, "/api/test".to_string());
        let kafka_node = ImpactNode::kafka_topic("test-topic".to_string());
        
        graph.add_node(method_node);
        graph.add_node(http_node);
        graph.add_node(kafka_node);
        
        // 添加边
        graph.add_edge(
            "method:com.example.Test::test",
            "http:GET:/api/test",
            EdgeType::HttpCall,
            Direction::Downstream,
        );
        
        // 生成 JSON 格式
        let json_result = graph.to_json();
        assert!(json_result.is_ok());
        
        let json = json_result.unwrap();
        
        // 验证 JSON 包含关键元素
        assert!(json.contains("nodes"));
        assert!(json.contains("edges"));
        assert!(json.contains("node_count"));
        assert!(json.contains("edge_count"));
        assert!(json.contains("com.example.Test::test"));
        assert!(json.contains("/api/test"));
        assert!(json.contains("test-topic"));
        
        // 验证可以解析为有效的 JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["node_count"], 3);
        assert_eq!(parsed["edge_count"], 1);
    }
    
    #[test]
    fn test_detect_cycles_no_cycle() {
        let mut graph = ImpactGraph::new();
        
        // 创建一个无循环的图: A -> B -> C
        let node_a = ImpactNode::method("A".to_string());
        let node_b = ImpactNode::method("B".to_string());
        let node_c = ImpactNode::method("C".to_string());
        
        graph.add_node(node_a);
        graph.add_node(node_b);
        graph.add_node(node_c);
        
        graph.add_edge("method:A", "method:B", EdgeType::MethodCall, Direction::Downstream);
        graph.add_edge("method:B", "method:C", EdgeType::MethodCall, Direction::Downstream);
        
        // 检测循环
        let cycles = graph.detect_cycles();
        
        // 应该没有循环
        assert_eq!(cycles.len(), 0);
    }
    
    #[test]
    fn test_detect_cycles_simple_cycle() {
        let mut graph = ImpactGraph::new();
        
        // 创建一个简单循环: A -> B -> A
        let node_a = ImpactNode::method("A".to_string());
        let node_b = ImpactNode::method("B".to_string());
        
        graph.add_node(node_a);
        graph.add_node(node_b);
        
        graph.add_edge("method:A", "method:B", EdgeType::MethodCall, Direction::Downstream);
        graph.add_edge("method:B", "method:A", EdgeType::MethodCall, Direction::Downstream);
        
        // 检测循环
        let cycles = graph.detect_cycles();
        
        // 应该检测到一个循环
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 2);
        assert!(cycles[0].contains(&"method:A".to_string()));
        assert!(cycles[0].contains(&"method:B".to_string()));
    }
    
    #[test]
    fn test_detect_cycles_complex_cycle() {
        let mut graph = ImpactGraph::new();
        
        // 创建一个复杂循环: A -> B -> C -> A
        let node_a = ImpactNode::method("A".to_string());
        let node_b = ImpactNode::method("B".to_string());
        let node_c = ImpactNode::method("C".to_string());
        
        graph.add_node(node_a);
        graph.add_node(node_b);
        graph.add_node(node_c);
        
        graph.add_edge("method:A", "method:B", EdgeType::MethodCall, Direction::Downstream);
        graph.add_edge("method:B", "method:C", EdgeType::MethodCall, Direction::Downstream);
        graph.add_edge("method:C", "method:A", EdgeType::MethodCall, Direction::Downstream);
        
        // 检测循环
        let cycles = graph.detect_cycles();
        
        // 应该检测到一个循环
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 3);
        assert!(cycles[0].contains(&"method:A".to_string()));
        assert!(cycles[0].contains(&"method:B".to_string()));
        assert!(cycles[0].contains(&"method:C".to_string()));
    }
    
    #[test]
    fn test_to_json_empty_graph() {
        let graph = ImpactGraph::new();
        
        let json_result = graph.to_json();
        assert!(json_result.is_ok());
        
        let json = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed["node_count"], 0);
        assert_eq!(parsed["edge_count"], 0);
        assert_eq!(parsed["nodes"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["edges"].as_array().unwrap().len(), 0);
    }
    
    #[test]
    fn test_to_dot_empty_graph() {
        let graph = ImpactGraph::new();
        
        let dot = graph.to_dot();
        
        // 空图也应该生成有效的 DOT 格式
        assert!(dot.contains("digraph"));
    }
}
