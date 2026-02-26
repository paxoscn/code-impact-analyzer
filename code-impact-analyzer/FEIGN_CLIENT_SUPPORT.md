# FeignClient 支持文档

## 概述

Code Impact Analyzer 现在支持解析 Spring Cloud OpenFeign 的 `@FeignClient` 注解，能够正确识别和记录 Feign 客户端调用的下游 HTTP 接口。

## 功能说明

### FeignClient 注解解析

对于使用 `@FeignClient` 注解的接口，解析器会：

1. 提取类级别的 `@FeignClient` 注解信息：
   - `value` 或 `name` 属性：服务名称
   - `path` 属性：基础路径（可选）

2. 提取方法级别的 HTTP 映射注解：
   - `@GetMapping`
   - `@PostMapping`
   - `@PutMapping`
   - `@DeleteMapping`
   - `@PatchMapping`
   - `@RequestMapping`

3. 组合完整的下游接口路径：
   ```
   {service_name}/{base_path}/{method_path}
   ```

### 示例

#### 输入代码

```java
package com.hualala.shop.domain.feign;

import org.springframework.cloud.openfeign.FeignClient;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;

@FeignClient(value = "hll-basic-info-api", path = "/hll-basic-info-api")
public interface BasicInfoFeign {
    @PostMapping("/feign/shop/copy/info")
    GoodsResponse getGoodsInfo(@RequestBody GoodsInfoRequest request);
}
```

#### 解析结果

- 类名：`com.hualala.shop.domain.feign.BasicInfoFeign`
- 方法名：`getGoodsInfo`
- HTTP 接口：`POST hll-basic-info-api/hll-basic-info-api/feign/shop/copy/info`

### 路径组合规则

1. **有 base_path 的情况**：
   ```
   service_name/base_path/method_path
   ```
   例如：`hll-basic-info-api/hll-basic-info-api/feign/shop/copy/info`

2. **没有 base_path 的情况**：
   ```
   service_name/method_path
   ```
   例如：`user-service/api/users`

3. **使用 name 属性代替 value**：
   ```java
   @FeignClient(name = "order-service", path = "/orders")
   ```
   解析结果：`order-service/orders/...`

## 测试

运行以下命令测试 FeignClient 解析功能：

```bash
# 运行所有 FeignClient 相关测试
cargo test --lib java_parser::tests::test_extract_feign_client -- --nocapture

# 运行示例程序
cargo run --example test_feign_parsing
cargo run --example test_real_feign
```

## 实现细节

### 新增结构

```rust
struct FeignClientInfo {
    service_name: String,
    base_path: Option<String>,
}
```

### 关键方法

1. `extract_feign_client_annotation()` - 提取类级别的 FeignClient 注解
2. `parse_feign_client_annotation()` - 解析 FeignClient 注解参数
3. `extract_feign_attribute()` - 提取注解中的特定属性值
4. `extract_feign_http_annotation()` - 组合类级别和方法级别的路径信息

## 影响范围

这个功能增强了 Java 解析器的能力，使其能够：

- 更准确地追踪微服务之间的调用关系
- 识别 Feign 客户端调用的下游服务和接口
- 在影响分析中包含跨服务的依赖关系

## 兼容性

- 向后兼容：不影响现有的 HTTP 注解解析功能
- 所有现有测试（108 个）均通过
- 新增 3 个 FeignClient 相关测试
