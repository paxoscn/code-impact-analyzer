# HTTP 接口全限定名格式

## 概述

对于 Java 提供的 HTTP 接口，系统会自动记录接口的全限定名，格式为：

```
{HTTP方法} {application.name}/{context-path}/{类路径}/{方法路径}
```

## 配置来源

系统会自动从项目的 `start/src/main/resources/application.yml` 文件中读取以下配置：

1. **spring.application.name**: 应用名称
   - 如果不存在，则使用项目目录名
   
2. **server.servlet.context-path**: 上下文路径
   - 如果不存在，则使用空字符串

## 示例

### 配置文件示例

`start/src/main/resources/application.yml`:
```yaml
server:
  servlet:
    context-path: /hll-basic-info-api
spring:
  application:
    name: hll-basic-info-api
```

### Java 代码示例

```java
@RestController
@RequestMapping("feign/shop/copy")
public class FeignShopCopyController {
    
    @PostMapping("/query")
    public Response query(@RequestBody GetShopCopyCmd cmd) {
        // ...
    }
}
```

### 解析结果

对于上述代码，系统会解析出以下 HTTP 接口：

```
POST hll-basic-info-api/hll-basic-info-api/feign/shop/copy/query
```

路径组成：
- `hll-basic-info-api` - application.name
- `hll-basic-info-api` - context-path
- `feign/shop/copy` - 类级别的 @RequestMapping 路径
- `query` - 方法级别的 @PostMapping 路径

## 路径组合规则

1. **基础路径**: `{application.name}/{context-path}`
2. **类路径**: 如果类上有 `@RequestMapping` 注解，添加其路径
3. **方法路径**: 添加方法上的 HTTP 注解路径（如 `@PostMapping`, `@GetMapping` 等）
4. 路径拼接时会自动处理斜杠，确保格式正确

## 特殊情况

### 没有 context-path

如果配置文件中没有 `server.servlet.context-path`，则格式为：

```
{HTTP方法} {application.name}/{类路径}/{方法路径}
```

示例：
```
GET test-service/api/users
```

### 没有类级别 RequestMapping

如果类上没有 `@RequestMapping` 注解，则格式为：

```
{HTTP方法} {application.name}/{context-path}/{方法路径}
```

示例：
```
POST my-service/api/users
```

### 没有 application.name

如果配置文件中没有 `spring.application.name`，系统会使用项目目录名作为应用名称。

## 支持的注解

系统支持以下 Spring Framework HTTP 注解：

- `@GetMapping`
- `@PostMapping`
- `@PutMapping`
- `@DeleteMapping`
- `@PatchMapping`
- `@RequestMapping`

## FeignClient 接口

对于 `@FeignClient` 注解的接口，系统会使用不同的路径格式：

```
{HTTP方法} {service-name}/{base-path}/{方法路径}
```

这是因为 FeignClient 是用于调用其他服务的，不需要包含当前应用的配置信息。

示例：
```java
@FeignClient(value = "hll-basic-info-api", path = "/hll-basic-info-api")
public interface BasicInfoFeign {
    @PostMapping("/feign/shop/copy/info")
    Response getInfo(@RequestBody Request request);
}
```

解析结果：
```
POST hll-basic-info-api/hll-basic-info-api/feign/shop/copy/info
```

## 测试

可以运行以下命令测试功能：

```bash
cd code-impact-analyzer
cargo test --test application_config_test
cargo run --example test_application_config
```
