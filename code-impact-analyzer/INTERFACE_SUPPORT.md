# Java 接口和抽象方法支持

## 概述

Java 解析器现在完全支持接口（interface）和抽象方法的解析，使得代码影响分析能够正确追溯接口定义和实现之间的关系。

## 支持的特性

### 1. 接口声明解析

解析器能够识别和提取接口声明：

```java
public interface UserService {
    void saveUser(String name);
    User findUser(Long id);
}
```

解析结果：
- 接口名：`com.example.UserService`（包含完整包名）
- 方法列表：`saveUser`, `findUser`
- 每个方法的完整限定名：`com.example.UserService::saveUser`

### 2. 抽象方法提取

接口中的所有方法（抽象方法）都会被正确提取和索引：

```java
public interface ShopCopyService {
    Response query(GetShopCopyCmd cmd);
    Response clone(ShopCloneCmd cmd);
    Response restore(ShopRestoreCmd cmd);
}
```

所有三个方法都会被索引，可以用于影响分析。

### 3. 接口和实现类同时解析

当一个文件包含接口和实现类时，两者都会被正确解析：

```java
public interface UserService {
    void saveUser(String name);
}

public class UserServiceImpl implements UserService {
    @Override
    public void saveUser(String name) {
        // implementation
    }
}
```

解析结果：
- 2 个类/接口
- 每个都有自己的方法列表

## 技术实现

### 修改的文件

- `code-impact-analyzer/src/java_parser.rs`

### 关键修改

1. **扩展节点类型识别**：
   - 从只识别 `class_declaration` 扩展到同时识别 `interface_declaration`

2. **扩展方法体识别**：
   - 从只查找 `class_body` 扩展到同时查找 `interface_body`

### Tree-sitter 节点结构

接口的 AST 结构：
```
interface_declaration
├── modifiers (public)
├── identifier (接口名)
└── interface_body
    ├── method_declaration (抽象方法 1)
    ├── method_declaration (抽象方法 2)
    └── ...
```

## 使用场景

### 1. 微服务 Feign 客户端

```java
// 客户端接口定义
@FeignClient("user-service")
public interface UserServiceClient {
    @GetMapping("/users/{id}")
    User getUser(@PathVariable Long id);
}

// 服务端实现
@RestController
public class UserController implements UserServiceClient {
    @Override
    public User getUser(Long id) {
        return userRepository.findById(id);
    }
}
```

现在可以正确追溯从客户端接口到服务端实现的调用关系。

### 2. Service 层接口

```java
// Service 接口
public interface OrderService {
    Order createOrder(OrderRequest request);
}

// Service 实现
@Service
public class OrderServiceImpl implements OrderService {
    @Override
    public Order createOrder(OrderRequest request) {
        // business logic
    }
}
```

修改接口方法签名时，能够找到所有实现类。

### 3. API 契约定义

```java
// API 接口定义
public interface PaymentApi {
    Response processPayment(PaymentRequest request);
    Response refund(RefundRequest request);
}
```

接口方法会被索引，可以追溯所有调用方。

## 测试

运行接口解析测试：

```bash
cd code-impact-analyzer
cargo run --example test_interface
```

测试真实接口文件：

```bash
cargo run --example test_real_interface
```

## 兼容性

- ✅ 完全向后兼容
- ✅ 不影响现有类解析功能
- ✅ 不需要修改配置
- ✅ 所有现有测试通过

## 限制

目前的实现：
- ✅ 支持接口声明
- ✅ 支持接口中的抽象方法
- ✅ 支持完整包名提取
- ⚠️ 不追踪接口继承关系（interface extends）
- ⚠️ 不追踪默认方法实现（Java 8+）

这些限制不影响大多数使用场景，因为影响分析主要关注方法调用关系。

## 示例输出

解析 `ShopCopyService.java` 接口：

```
类/接口数量: 1

类/接口名: com.hll.basic.api.app.client.api.ShopCopyService
行范围: 14-27
方法数量: 5
  - 方法: query (行 16-16)
    完整限定名: com.hll.basic.api.app.client.api.ShopCopyService::query
  - 方法: clone (行 19-19)
    完整限定名: com.hll.basic.api.app.client.api.ShopCopyService::clone
  - 方法: restore (行 21-21)
    完整限定名: com.hll.basic.api.app.client.api.ShopCopyService::restore
  - 方法: menu (行 23-23)
    完整限定名: com.hll.basic.api.app.client.api.ShopCopyService::menu
  - 方法: info (行 25-25)
    完整限定名: com.hll.basic.api.app.client.api.ShopCopyService::info
```

## 总结

Java 接口和抽象方法现在已经完全支持，使得代码影响分析工具能够更准确地追溯 Java 项目中的方法调用关系，特别是在使用面向接口编程和微服务架构的场景中。
