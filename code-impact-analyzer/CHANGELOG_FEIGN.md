# FeignClient 支持更新日志

## 版本：当前开发版本

### 新增功能

#### Spring Cloud OpenFeign 支持

为 Java 解析器添加了完整的 `@FeignClient` 注解支持，能够准确识别和追踪微服务间的 HTTP 调用。

**主要特性：**

1. **类级别注解解析**
   - 支持 `value` 和 `name` 属性（服务名称）
   - 支持 `path` 属性（基础路径）

2. **方法级别注解解析**
   - 支持所有 Spring HTTP 映射注解
   - 自动组合完整的下游接口路径

3. **路径组合规则**
   - 有 base_path：`service_name/base_path/method_path`
   - 无 base_path：`service_name/method_path`

**示例：**

输入代码：
```java
@FeignClient(value = "hll-basic-info-api", path = "/hll-basic-info-api")
public interface BasicInfoFeign {
    @PostMapping("/feign/shop/copy/info")
    GoodsResponse getGoodsInfo(@RequestBody GoodsInfoRequest request);
}
```

解析结果：
```
POST hll-basic-info-api/hll-basic-info-api/feign/shop/copy/info
```

### 技术实现

**新增结构：**
- `FeignClientInfo` - 存储 FeignClient 注解信息

**新增方法：**
- `extract_feign_client_annotation()` - 提取类级别注解
- `parse_feign_client_annotation()` - 解析注解参数
- `extract_feign_attribute()` - 提取特定属性值
- `extract_feign_http_annotation()` - 组合完整路径

**修改方法：**
- `extract_class_info()` - 添加 FeignClient 信息提取
- `extract_methods_from_class()` - 传递 FeignClient 信息
- `extract_method_info()` - 根据 FeignClient 信息处理 HTTP 注解

### 测试

**新增测试：**
- `test_extract_feign_client_annotation` - 基本功能测试
- `test_extract_feign_client_without_base_path` - 无基础路径测试
- `test_extract_feign_client_with_name_attribute` - name 属性测试

**测试覆盖：**
- 所有现有测试（108 个）全部通过
- 新增 3 个 FeignClient 专项测试
- 实际文件解析验证通过

### 文档

**新增文档：**
- `FEIGN_CLIENT_SUPPORT.md` - 详细功能说明
- `CHANGELOG_FEIGN.md` - 更新日志（本文件）

**更新文档：**
- `README.md` - 添加 FeignClient 支持说明和示例

### 兼容性

- ✅ 向后兼容：不影响现有功能
- ✅ 所有现有测试通过
- ✅ 无破坏性变更

### 使用方法

无需额外配置，工具会自动识别和解析 `@FeignClient` 注解。

运行示例：
```bash
# 测试 FeignClient 解析
cargo run --example test_feign_parsing

# 测试实际文件
cargo run --example test_real_feign

# 运行所有测试
cargo test --lib
```

### 影响范围

这个功能增强使得工具能够：
- ✅ 更准确地追踪微服务间的调用关系
- ✅ 识别 Feign 客户端调用的下游服务和接口
- ✅ 在影响分析中包含跨服务的依赖关系
- ✅ 支持复杂的微服务架构分析

### 后续计划

可能的增强方向：
- 支持 Feign 的 fallback 和 fallbackFactory
- 支持 Feign 的配置属性（如 url、configuration 等）
- 支持 Feign 的继承和组合模式
