# 接口方法上游追溯功能

## 功能概述

在 `trace_method_upstream` 方法中增强了接口方法的上游追溯能力。现在不仅会查找当前方法的直接调用者，还会查找该方法所在类实现的所有接口中同名方法的调用者。

## 使用场景

在 Java 等面向对象语言中，经常会出现以下模式：

```java
// 接口定义
public interface PaymentService {
    void processPayment(Order order);
}

// 实现类
public class PaymentServiceImpl implements PaymentService {
    @Override
    public void processPayment(Order order) {
        // 实现逻辑
    }
}

// 调用方
public class PaymentController {
    @Autowired
    private PaymentService paymentService;  // 注入的是接口类型
    
    public void handlePayment(Order order) {
        paymentService.processPayment(order);  // 调用接口方法
    }
}
```

在这种情况下：
- `PaymentController` 调用的是 `PaymentService::processPayment`（接口方法）
- 实际执行的是 `PaymentServiceImpl::processPayment`（实现类方法）

## 问题

在之前的实现中，当追溯 `PaymentServiceImpl::processPayment` 的上游调用者时，只会查找直接调用 `PaymentServiceImpl::processPayment` 的方法，而不会找到调用接口方法 `PaymentService::processPayment` 的方法。

这导致无法完整追溯到 `PaymentController`，因为 `PaymentController` 调用的是接口方法，而不是实现类方法。

## 解决方案

### 1. 数据结构增强

在 `CodeIndex` 中添加了反向映射：

```rust
/// 实现类到接口的映射: implementation_class_name -> [interface_names]
class_interfaces: HashMap<String, Vec<String>>,
```

这个映射在索引阶段自动构建，记录每个类实现的所有接口。

### 2. 新增查询方法

```rust
/// 查找类实现的所有接口
pub fn find_class_interfaces(&self, class_name: &str) -> Vec<&str>
```

### 3. 追溯逻辑增强

在 `trace_method_upstream` 方法中：

1. 首先查找直接调用当前方法的调用者
2. 解析方法名，提取类名和方法名
3. 查找该类实现的所有接口
4. 对每个接口，构建接口方法的完整限定名
5. 查找调用接口方法的调用者
6. 合并所有调用者列表

```rust
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
```

## 效果

现在当追溯 `PaymentServiceImpl::processPayment` 的上游时：

1. 查找直接调用 `PaymentServiceImpl::processPayment` 的方法（如果有）
2. 发现 `PaymentServiceImpl` 实现了 `PaymentService` 接口
3. 查找调用 `PaymentService::processPayment` 的方法
4. 找到 `PaymentController::handlePayment`
5. 将 `PaymentController::handlePayment` 添加到影响图中

## 测试

运行测试示例：

```bash
cargo run --example test_interface_upstream_trace
```

测试场景：
- 接口 `PaymentService` 有方法 `processPayment`
- 实现类 `PaymentServiceImpl` 实现了该接口
- `PaymentController` 调用接口方法 `PaymentService::processPayment`
- 追溯 `PaymentServiceImpl::processPayment` 的上游
- 验证能够找到 `PaymentController::handlePayment`

## 兼容性

此功能完全向后兼容：
- 对于没有实现接口的类，行为与之前完全相同
- 对于实现了接口的类，会额外查找接口方法的调用者
- 所有现有测试都通过

## 性能考虑

- 接口实现关系在索引阶段一次性构建，查询时无额外开销
- 对于每个方法，最多额外查询 N 次（N 为该类实现的接口数量）
- 在实际项目中，一个类通常只实现 1-3 个接口，性能影响可忽略不计

## 未来改进

可以考虑的优化方向：
1. 支持接口继承链的追溯（接口继承接口）
2. 支持抽象类的追溯
3. 缓存接口方法的调用者列表，避免重复查询
