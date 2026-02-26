# HTTP 接口方向修复

## 问题描述

在之前的实现中，HTTP 接口声明和 Feign 调用的上下游关系处理不正确：

- **HTTP 接口声明**（如 `@GetMapping`）：HTTP 节点被错误地设置为 Java 方法的下游
- **Feign 调用**：HTTP 节点也被设置为 Java 方法的下游

这导致影响追溯图中的边方向不符合实际的调用关系。

## 正确的关系

根据实际的调用关系，应该是：

1. **HTTP 接口声明**：HTTP 节点应该是 Java 方法的**上游**
   - 外部调用者通过 HTTP 接口调用这个方法
   - 边方向：`HTTP 端点 -> Java 方法`

2. **Feign 调用**：HTTP 节点应该是 Java 方法的**下游**
   - 方法通过 Feign 客户端调用其他服务的 HTTP 接口
   - 边方向：`Java 方法 -> HTTP 端点`

## 解决方案

### 1. 在 `HttpAnnotation` 中添加标志字段

在 `types.rs` 中的 `HttpAnnotation` 结构体中添加 `is_feign_client` 字段：

```rust
pub struct HttpAnnotation {
    pub method: HttpMethod,
    pub path: String,
    pub path_params: Vec<String>,
    pub is_feign_client: bool,  // 新增字段
}
```

### 2. 在 Java 解析器中设置标志

在 `java_parser.rs` 中：

- 对于 Feign 客户端方法，设置 `is_feign_client = true`
- 对于普通 HTTP 接口声明，设置 `is_feign_client = false`

```rust
// Feign 调用
Some(HttpAnnotation {
    method: method_http.method,
    path: full_path,
    path_params: method_http.path_params,
    is_feign_client: true,  // Feign 调用
})

// 普通 HTTP 接口
Some(HttpAnnotation {
    method,
    path: path_str,
    path_params,
    is_feign_client: false,  // 普通 HTTP 接口声明
})
```

### 3. 更新索引逻辑

在 `code_index.rs` 中的 `index_http_annotation` 方法中，根据 `is_feign_client` 标志决定是索引为提供者还是消费者：

```rust
fn index_http_annotation(&mut self, method_name: &str, annotation: &HttpAnnotation) {
    let endpoint = HttpEndpoint {
        method: annotation.method.clone(),
        path_pattern: annotation.path.clone(),
    };
    
    if annotation.is_feign_client {
        // Feign 消费者
        self.http_consumers
            .entry(endpoint)
            .or_insert_with(Vec::new)
            .push(method_name.to_string());
    } else {
        // HTTP 接口提供者
        self.http_providers.insert(endpoint, method_name.to_string());
    }
}
```

### 4. 更新影响追溯逻辑

在 `impact_tracer.rs` 中的 `trace_http_interface` 方法中，根据 `is_feign_client` 标志设置正确的边方向：

```rust
if http_annotation.is_feign_client {
    // Feign 调用：HTTP 节点是方法的下游
    graph.add_edge(
        &method_id,
        &endpoint_id,
        EdgeType::HttpCall,
        Direction::Downstream,
    );
    // ... 查找提供者
} else {
    // HTTP 接口声明：HTTP 节点是方法的上游
    graph.add_edge(
        &endpoint_id,
        &method_id,
        EdgeType::HttpCall,
        Direction::Upstream,
    );
    // ... 查找消费者
}
```

### 5. 添加 `find_http_providers` 方法

在 `code_index.rs` 中添加新方法来查找 HTTP 端点的提供者：

```rust
pub fn find_http_providers(&self, endpoint: &HttpEndpoint) -> Vec<&str> {
    self.http_providers
        .get(endpoint)
        .map(|provider| vec![provider.as_str()])
        .unwrap_or_default()
}
```

## 测试验证

创建了 `http_direction_test.rs` 测试文件，包含以下测试用例：

1. `test_http_interface_provider_direction` - 验证普通 HTTP 接口被正确索引为提供者
2. `test_feign_client_consumer_direction` - 验证 Feign 调用被正确索引为消费者
3. `test_feign_client_with_base_path` - 验证带 base path 的 Feign 调用
4. `test_http_provider_and_consumer_different_endpoints` - 验证提供者和消费者可以共存

所有测试均通过。

## 影响范围

这次修改影响以下文件：

1. `src/types.rs` - 添加 `is_feign_client` 字段
2. `src/java_parser.rs` - 设置 `is_feign_client` 标志
3. `src/rust_parser.rs` - 为 Rust 设置 `is_feign_client = false`
4. `src/code_index.rs` - 更新索引逻辑，添加 `find_http_providers` 方法
5. `src/impact_tracer.rs` - 更新影响追溯逻辑
6. `tests/http_direction_test.rs` - 新增测试文件

## 向后兼容性

这是一个破坏性变更，因为 `HttpAnnotation` 结构体添加了新字段。所有创建 `HttpAnnotation` 的代码都需要更新以包含 `is_feign_client` 字段。

## 示例

### HTTP 接口提供者

```java
@RestController
public class UserController {
    @GetMapping("/api/users/{id}")
    public User getUser(@PathVariable Long id) {
        // ...
    }
}
```

影响图：
```
HTTP:GET:/api/users/{id} -> UserController::getUser
```

### Feign 客户端调用

```java
@FeignClient(value = "user-service", path = "/api")
public interface UserClient {
    @GetMapping("/users/{id}")
    User getUser(@PathVariable Long id);
}
```

影响图：
```
UserClient::getUser -> HTTP:GET:user-service/api/users/{id}
```

## 总结

通过在 `HttpAnnotation` 中添加 `is_feign_client` 标志，我们能够准确区分 HTTP 接口声明和 Feign 调用，从而正确设置影响追溯图中的边方向。这使得影响分析更加准确，能够正确反映实际的调用关系。
