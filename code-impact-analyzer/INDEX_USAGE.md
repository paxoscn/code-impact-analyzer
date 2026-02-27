# 索引功能使用指南

## 概述

代码影响分析工具现在支持索引缓存功能，可以显著提高大型项目的分析速度。索引会自动保存到工作空间的 `.code-impact-analyzer/` 目录中。

## 基本使用

### 1. 首次运行（自动构建索引）

第一次运行时，工具会自动扫描整个工作空间并构建索引：

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches
```

输出示例：
```
[INFO] No valid index found, building new index
[INFO] Workspace indexed successfully
[INFO] Index saved successfully: 1234 methods in 567 files
[INFO] Starting analysis...
```

### 2. 后续运行（使用缓存）

后续运行时，工具会自动加载缓存的索引，大幅提升速度：

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches
```

输出示例：
```
[INFO] Loading index from ".code-impact-analyzer"
[INFO] Index loaded successfully: 1234 methods
[INFO] Starting analysis...
```

## 索引管理命令

### 查看索引信息

查看当前索引的详细信息：

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --index-info
```

输出示例：
```
Index Information:
  Version: 1.0.0
  Workspace: /path/to/workspace
  Created: 2024-01-01 10:00:00
  Updated: 2024-01-01 10:00:00
  Files: 567
  Methods: 1234
  Checksum: a1b2c3d4e5f6
```

### 验证索引有效性

检查索引是否仍然有效（文件是否有修改）：

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --verify-index
```

输出示例：
```
Index is valid
```

或者：
```
Index is invalid or outdated
```

### 强制重建索引

当代码有大量修改时，可以强制重建索引：

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --rebuild-index
```

输出示例：
```
[INFO] Force rebuild enabled, clearing existing index
[INFO] No valid index found, building new index
[INFO] Workspace indexed successfully
[INFO] Index saved successfully: 1234 methods in 567 files
```

### 清除索引

删除缓存的索引文件：

```bash
code-impact-analyzer \
  --workspace /path/to/workspace \
  --diff /path/to/patches \
  --clear-index
```

输出示例：
```
[INFO] Clearing index...
Index cleared successfully
```

## 索引自动更新

工具会自动检测工作空间的变化：

1. **文件修改检测**: 基于文件修改时间计算校验和
2. **自动失效**: 当检测到文件修改时，自动重建索引
3. **版本兼容**: 索引格式升级时自动重建

### 触发自动重建的情况

- 工作空间中的源文件被修改
- 工作空间路径变更
- 索引格式版本不兼容
- 索引文件损坏

## 性能对比

### 大型项目示例

假设一个包含 1000 个 Java 文件的项目：

**首次运行（构建索引）**:
```
索引构建时间: ~30 秒
分析时间: ~5 秒
总时间: ~35 秒
```

**后续运行（使用缓存）**:
```
索引加载时间: ~1 秒
分析时间: ~5 秒
总时间: ~6 秒
```

**性能提升**: 约 6 倍

## 索引文件结构

索引文件存储在工作空间的隐藏目录中：

```
<workspace>/.code-impact-analyzer/
├── index.meta.json      # 元数据（版本、时间戳、校验和）
└── index.json           # 索引数据（方法、调用关系等）
```

### 元数据文件示例

```json
{
  "version": "1.0.0",
  "workspace_path": "/path/to/workspace",
  "created_at": 1704096000,
  "updated_at": 1704096000,
  "file_count": 567,
  "method_count": 1234,
  "checksum": "a1b2c3d4e5f6"
}
```

### 索引数据文件示例

```json
{
  "methods": {
    "com.example.Service::method": {
      "name": "method",
      "full_qualified_name": "com.example.Service::method",
      "file_path": "src/Service.java",
      "line_range": [10, 20],
      "calls": [...],
      "http_annotations": {...},
      "kafka_operations": [...],
      "db_operations": [...],
      "redis_operations": [...]
    }
  },
  "method_calls": {...},
  "reverse_calls": {...},
  "http_providers": {...},
  "http_consumers": {...},
  ...
}
```

## 最佳实践

### 1. CI/CD 集成

在 CI/CD 流程中使用索引缓存：

```yaml
# .gitlab-ci.yml 示例
analyze:
  script:
    # 首次运行会构建索引
    - code-impact-analyzer --workspace . --diff patches/
  cache:
    paths:
      - .code-impact-analyzer/
  artifacts:
    paths:
      - impact-graph.dot
```

### 2. 定期清理

对于长期运行的项目，建议定期清理索引：

```bash
# 每周清理一次索引
code-impact-analyzer --workspace . --diff patches/ --clear-index
code-impact-analyzer --workspace . --diff patches/
```

### 3. 版本控制

建议将索引目录添加到 `.gitignore`：

```gitignore
# .gitignore
.code-impact-analyzer/
```

### 4. 大型项目优化

对于超大型项目（10000+ 文件），可以考虑：

- 使用 `--rebuild-index` 在夜间批处理中重建索引
- 定期验证索引有效性
- 监控索引文件大小

## 故障排查

### 问题：索引加载失败

**症状**:
```
[WARN] Failed to load index: ..., will rebuild
```

**解决方案**:
1. 检查索引文件是否存在
2. 验证文件权限
3. 使用 `--clear-index` 清除损坏的索引
4. 使用 `--rebuild-index` 强制重建

### 问题：索引总是被重建

**症状**:
```
[INFO] Index is invalid or outdated, will rebuild
```

**可能原因**:
1. 文件频繁修改
2. 工作空间路径变化
3. 时钟不同步

**解决方案**:
1. 使用 `--verify-index` 检查索引状态
2. 确保工作空间路径稳定
3. 检查系统时间设置

### 问题：索引文件过大

**症状**:
索引文件占用大量磁盘空间

**解决方案**:
1. 清理不需要的源文件
2. 使用 `--clear-index` 清除旧索引
3. 考虑排除某些目录（未来功能）

## 高级配置

### 环境变量（未来功能）

```bash
# 自定义索引目录
export CODE_IMPACT_INDEX_DIR=/custom/path

# 禁用索引缓存
export CODE_IMPACT_NO_CACHE=1

# 索引压缩
export CODE_IMPACT_COMPRESS_INDEX=1
```

### 配置文件（未来功能）

```toml
# .code-impact-analyzer.toml
[index]
enabled = true
directory = ".code-impact-analyzer"
compress = false
max_age_days = 7

[index.exclude]
patterns = ["**/test/**", "**/target/**"]
```

## 常见问题

**Q: 索引文件可以提交到版本控制吗？**

A: 不建议。索引文件包含绝对路径和机器特定的信息，应该添加到 `.gitignore`。

**Q: 多个开发者可以共享索引吗？**

A: 不可以。索引包含工作空间的绝对路径，每个开发者应该维护自己的索引。

**Q: 索引会自动更新吗？**

A: 会。工具会检测文件修改并自动重建索引。

**Q: 如何在 Docker 容器中使用索引？**

A: 可以将索引目录挂载为 volume：
```bash
docker run -v /path/to/workspace:/workspace \
           -v /path/to/index:/workspace/.code-impact-analyzer \
           code-impact-analyzer --workspace /workspace --diff /patches
```

**Q: 索引格式会变化吗？**

A: 可能会。工具会自动检测版本不兼容并重建索引。

## 参考资料

- [索引格式设计文档](INDEX_FORMAT.md)
- [项目 README](README.md)
- [使用指南](USAGE.md)
