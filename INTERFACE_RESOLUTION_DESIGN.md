# 接口解析设计说明

## 问题

在 `test_interface_resolution_in_call_chain` 测试中：

```java
public class UserController {
    private UserService userService;  // 接口类型
    
    public void createUser(String name) {
        userService.saveUser(name);  // 调用接口方法
    }
}
```

当前解析为 `com.example.UserService::saveUser`（接口方法），而不是 `com.example.UserServiceImpl::saveUser`（实现类方法）。

## 这是正确的设计！

### 原因

#### 1. 静态类型 vs 运行时类型

在 Java 中：
```java
UserService userService = new UserServiceImpl();  // 静态类型是 UserService
userService.saveUser("test");  // 静态分析只能知道调用 UserService::saveUser
```

- **静态类型**：编译时的类型，这里是 `UserService`（接口）
- **运行时类型**：实际对象的类型，这里是 `UserServiceImpl`（实现类）

静态代码分析只能获取静态类型信息。

#### 2. 分层设计

系统采用分层设计：

```
┌─────────────────────────────────────┐
│  Java Parser (java_parser.rs)      │  ← 解析单个文件，提取静态类型
│  - 提取类、方法、字段               │
│  - 提取方法调用（使用静态类型）     │
│  - 提取接口实现关系                 │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  Code Index (code_index.rs)        │  ← 索引所有文件，建立全局视图
│  - 索引所有方法                     │
│  - 建立接口→实现类映射              │
│  - 提供 resolve_interface_call()   │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  Impact Tracer (impact_tracer.rs)  │  ← 影响追踪，解析接口调用
│  - 追踪方法调用链                   │
│  - 使用 resolve_interface_call()   │
│  - 如果接口只有1个实现，解析为实现类│
└─────────────────────────────────────┘
```

#### 3. 为什么不在 Java Parser 中解析

Java Parser 不应该解析接口调用，因为：

1. **单一职责**：Parser 只负责解析单个文件
2. **缺少全局信息**：Parser 不知道整个项目的类结构
3. **无法确定实现类**：Parser 不知道接口有几个实现类
4. **依赖注入**：实际运行时可能通过 Spring 等框架注入不同的实现

### 接口解析的时机

接口解析在 **影响追踪阶段** 进行：

```rust
// 在 impact_tracer.rs 中
let resolved_caller = self.index.resolve_interface_call(caller);
let resolved_callee = self.index.resolve_interface_call(callee);
```

`resolve_interface_call` 的逻辑：
1. 检查调用目标是否是接口方法
2. 查找该接口的所有实现类
3. 如果只有 1 个实现类，返回实现类方法
4. 如果有多个实现类，返回原始接口方法（无法确定具体实现）

## 示例

### 场景1：接口只有一个实现类

```java
// UserService.java
public interface UserService {
    void saveUser(String name);
}

// UserServiceImpl.java (唯一实现)
public class UserServiceImpl implements UserService {
    public void saveUser(String name) { ... }
}

// UserController.java
public class UserController {
    private UserService userService;
    
    public void createUser(String name) {
        userService.saveUser(name);  // 静态类型: UserService::saveUser
    }
}
```

**Java Parser 阶段**:
- 解析为 `UserService::saveUser`（静态类型）

**Impact Tracer 阶段**:
- `resolve_interface_call("UserService::saveUser")`
- 发现只有 1 个实现类 `UserServiceImpl`
- 解析为 `UserServiceImpl::saveUser` ✅

### 场景2：接口有多个实现类

```java
// PaymentService.java
public interface PaymentService {
    void processPayment(double amount);
}

// CreditCardPayment.java
public class CreditCardPayment implements PaymentService {
    public void processPayment(double amount) { ... }
}

// PayPalPayment.java
public class PayPalPayment implements PaymentService {
    public void processPayment(double amount) { ... }
}

// PaymentController.java
public class PaymentController {
    private PaymentService paymentService;
    
    public void pay(double amount) {
        paymentService.processPayment(amount);  // 静态类型: PaymentService::processPayment
    }
}
```

**Java Parser 阶段**:
- 解析为 `PaymentService::processPayment`（静态类型）

**Impact Tracer 阶段**:
- `resolve_interface_call("PaymentService::processPayment")`
- 发现有 2 个实现类
- 保持原样 `PaymentService::processPayment` ⚠️
- 无法确定具体使用哪个实现

## 测试验证

```bash
$ cargo test --test interface_resolution_test test_interface_resolution_in_call_chain

Resolved call: com.example.UserService::saveUser
test test_interface_resolution_in_call_chain ... ok
```

输出 `com.example.UserService::saveUser` 是正确的，因为：
1. 这是 Java Parser 的输出（静态类型）
2. 接口解析在 Impact Tracer 中进行
3. 测试没有调用 Impact Tracer，所以没有解析

## 完整流程示例

要看到接口解析的效果，需要运行完整的影响分析：

```bash
$ cargo run --release -- --workspace examples/added-one-line \
                         --diff examples/added-one-line/patches
```

在影响分析图中，如果接口只有一个实现类，会显示实现类的方法。

## 结论

✅ **当前设计是正确的**

- Java Parser 解析静态类型（接口）
- Impact Tracer 解析运行时类型（实现类，如果唯一）
- 这是标准的静态代码分析设计模式

不需要修改 Java Parser 来解析接口调用。

---

**相关代码**:
- `src/java_parser.rs` - 提取静态类型
- `src/code_index.rs` - `resolve_interface_call()` 方法
- `src/impact_tracer.rs` - 使用接口解析
- `tests/interface_resolution_test.rs` - 接口解析测试
