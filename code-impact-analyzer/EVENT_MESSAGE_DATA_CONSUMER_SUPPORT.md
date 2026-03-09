# EventMessageDataConsumer 接口支持

## 功能概述

为实现了 `EventMessageDataConsumer` 接口的 Java 类的 `doConsumer` 方法自动添加 Kafka 消费上游追踪。

## 支持的场景

### 1. 类级别 @KafkaListener 注解

```java
@KafkaListener(topics = "user-events")
public class UserEventConsumer implements EventMessageDataConsumer<UserEvent> {
    @Override
    public void doConsumer(UserEvent event) {
        // 处理用户事件
    }
}
```

**识别结果**: 自动提取 topic 为 `user-events`

### 2. 方法级别 @KafkaListener 注解

```java
public class OrderEventConsumer implements EventMessageDataConsumer<OrderEvent> {
    @Override
    @KafkaListener(topics = "order-events")
    public void doConsumer(OrderEvent event) {
        // 处理订单事件
    }
}
```

**识别结果**: 自动提取 topic 为 `order-events`

### 3. 无注解（从类名推断）

```java
public class PaymentEventConsumer implements EventMessageDataConsumer<PaymentEvent> {
    @Override
    public void doConsumer(PaymentEvent event) {
        // 处理支付事件
    }
}
```

**识别结果**: 从类名 `PaymentEventConsumer` 自动推断 topic 为 `payment-events`

## Topic 推断规则

当没有显式的 `@KafkaListener` 注解时，系统会从类名自动推断 Kafka topic：

1. 移除常见后缀：`Consumer`、`Listener`、`Handler`
2. 将驼峰命名转换为短横线分隔：`UserEvent` → `user-event`
3. 添加复数后缀：`user-event` → `user-events`

### 示例

| 类名 | 推断的 Topic |
|------|-------------|
| `UserEventConsumer` | `user-events` |
| `OrderEventConsumer` | `order-events` |
| `PaymentEventConsumer` | `payment-events` |
| `NotificationListener` | `notifications` |
| `MessageHandler` | `messages` |

## 上游追踪

系统会自动建立 Kafka 生产者和消费者之间的关系：

```
OrderService::createOrder()  →  kafka:order-events  →  OrderEventConsumer::doConsumer()
    (生产者)                      (Kafka Topic)              (消费者)
```

### 追溯示例

```rust
// 从消费者追溯上游
let result = tracer.trace_impact(&["com.example.OrderEventConsumer::doConsumer(OrderEvent)"]);

// 结果包含：
// 1. 消费者方法节点
// 2. Kafka Topic 节点
// 3. 所有生产该 Topic 的生产者方法节点
```

## 实现细节

### 1. 泛型接口识别

修改了 `extract_implements_interfaces` 方法，支持识别泛型接口：

```rust
// 处理泛型类型（如 EventMessageDataConsumer<UserEvent>）
else if type_child.kind() == "generic_type" {
    // 提取基础接口名，忽略泛型参数
}
```

### 2. 类级别注解提取

添加了 `extract_class_kafka_listener` 方法：

```rust
fn extract_class_kafka_listener(&self, source: &str, class_node: &tree_sitter::Node) -> Option<String>
```

### 3. Topic 推断

添加了 `infer_kafka_topic_from_class_name` 方法：

```rust
fn infer_kafka_topic_from_class_name(&self, class_name: &str) -> Option<String>
```

### 4. 方法后处理

在 `extract_methods_with_return_types` 中添加后处理逻辑：

```rust
// 1. 如果方法没有 Kafka 操作，检查类级别注解
if method_info.kafka_operations.is_empty() {
    if let Some(topic) = &class_kafka_topic {
        // 添加类级别的 topic
    }
}

// 2. 如果是 EventMessageDataConsumer 的 doConsumer 方法
if method_info.name == "doConsumer" {
    for interface in &class_implements {
        if interface.contains("EventMessageDataConsumer") {
            // 从类名推断 topic
        }
    }
}
```

## 测试

运行测试：

```bash
# 测试基本功能
cargo run --example test_event_consumer

# 测试上游追踪
cargo run --example test_event_consumer_upstream
```

## 与其他功能的集成

此功能与现有的 Kafka 追踪功能完全集成：

- ✅ 支持 Kafka 生产者到消费者的双向追踪
- ✅ 支持跨服务边界追踪
- ✅ 与 HTTP、数据库、Redis 等其他追踪功能协同工作
- ✅ 支持影响图可视化

## 注意事项

1. **接口名称匹配**: 系统通过检查接口名称是否包含 `EventMessageDataConsumer` 来识别，因此完整类名应包含此字符串
2. **方法名称**: 只有名为 `doConsumer` 的方法会被自动处理
3. **优先级**: 显式注解 > 类级别注解 > 类名推断
4. **类名推断**: 仅在没有任何 `@KafkaListener` 注解时才会从类名推断 topic

## 未来改进

- [ ] 支持从配置文件读取 topic 映射
- [ ] 支持更复杂的 topic 命名规则
- [ ] 支持多个 topic 的消费者
- [ ] 支持条件消费（基于消息内容的路由）
