# 图转换功能

## 概述

代码影响分析器现在支持两种图转换功能，用于简化和优化影响图的可视化：

1. **合并相同边** (Merge Duplicate Edges)
2. **隐藏方法节点** (Hide Method Nodes)

## 功能说明

### 1. 合并相同边 (--merge-edges)

**默认值**: 启用 (true)

当两个节点之间存在多条相同类型和方向的边时，将它们合并为一条边。这有助于简化图的显示，避免重复的边造成视觉混乱。

**示例**:
```
转换前:
A -> B (method_call, upstream)
A -> B (method_call, upstream)
A -> B (method_call, upstream)

转换后:
A -> B (method_call, upstream)
```

**使用方法**:
```bash
# 启用合并（默认）
cargo run -- -w /path/to/workspace -d /path/to/diff --merge-edges=true

# 禁用合并
cargo run -- -w /path/to/workspace -d /path/to/diff --merge-edges=false
```

### 2. 隐藏方法节点 (--hide-methods)

**默认值**: 启用 (true)

移除图中的方法调用节点，只保留跨服务边界的节点（HTTP端点、Kafka主题、数据库表、Redis键等）。这个功能通过以下算法实现：

1. 对于每个方法节点，检查其入度 M 和出度 N
2. 如果 M > 0 且 N > 0，则：
   - 移除该方法节点
   - 在所有上游节点和下游节点之间添加 M×N 条边
3. 循环执行此过程，直到所有符合条件的方法节点都被移除
4. 最后应用边合并（如果启用）

**示例**:
```
转换前:
HTTP_Endpoint_A -> Method_1 -> Method_2 -> Kafka_Topic_B
                   (入度=1)    (入度=1)
                   (出度=1)    (出度=1)

转换后:
HTTP_Endpoint_A -> Kafka_Topic_B
```

**使用方法**:
```bash
# 启用隐藏（默认）
cargo run -- -w /path/to/workspace -d /path/to/diff --hide-methods=true

# 禁用隐藏（显示所有方法节点）
cargo run -- -w /path/to/workspace -d /path/to/diff --hide-methods=false
```

## 组合使用

两个功能可以组合使用，以获得最佳的可视化效果：

```bash
# 同时启用两个功能（默认）
cargo run -- -w /path/to/workspace -d /path/to/diff

# 只启用合并边
cargo run -- -w /path/to/workspace -d /path/to/diff --hide-methods=false

# 只启用隐藏方法
cargo run -- -w /path/to/workspace -d /path/to/diff --merge-edges=false

# 禁用所有转换
cargo run -- -w /path/to/workspace -d /path/to/diff --merge-edges=false --hide-methods=false
```

## 应用场景

### 合并相同边
- 当代码中存在多次相同的方法调用时
- 简化复杂的调用关系图
- 减少图的视觉复杂度

### 隐藏方法节点
- 关注跨服务边界的影响
- 分析微服务架构中的服务间依赖
- 简化大型项目的影响图
- 突出显示关键的外部依赖（HTTP、Kafka、数据库等）

## 技术实现

### TraceConfig 配置

```rust
pub struct TraceConfig {
    pub max_depth: usize,
    pub trace_upstream: bool,
    pub trace_downstream: bool,
    pub trace_cross_service: bool,
    pub merge_duplicate_edges: bool,  // 新增
    pub hide_method_nodes: bool,      // 新增
}
```

### ImpactGraph 转换方法

```rust
impl ImpactGraph {
    /// 应用图转换
    pub fn transform(&self, merge_edges: bool, hide_methods: bool) -> Self;
    
    /// 内部方法：隐藏方法节点
    fn hide_method_nodes_internal(&self) -> Self;
    
    /// 内部方法：合并相同边
    fn merge_duplicate_edges_internal(&self) -> Self;
}
```

## 性能考虑

- **合并边**: O(E)，其中 E 是边的数量
- **隐藏方法节点**: O(N × E)，其中 N 是方法节点数量，E 是边的数量
- 转换在追溯完成后执行，不影响追溯性能
- 转换创建新图，不修改原始图

## 注意事项

1. 隐藏方法节点会改变图的拓扑结构，但保留了影响关系
2. 合并边会丢失重复调用的信息
3. 两个功能都是可选的，可以根据需要启用或禁用
4. 默认情况下两个功能都启用，以提供最简洁的可视化效果
