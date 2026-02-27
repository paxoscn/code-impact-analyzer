# 索引文件格式设计

## 概述

为了提高代码影响分析的性能，我们设计了一个持久化的索引文件格式。该索引文件存储了工作空间的完整代码结构信息，避免每次启动时重新解析整个代码库。

## 文件位置

索引文件存储在工作空间根目录下的 `.code-impact-analyzer/` 目录中：

```
<workspace_root>/.code-impact-analyzer/
├── index.json          # 主索引文件
├── index.meta.json     # 元数据文件
└── cache/              # 解析缓存目录（可选）
```

## 文件格式

### 1. 元数据文件 (index.meta.json)

存储索引的元信息，用于判断索引是否需要重建：

```json
{
  "version": "1.0.0",
  "workspace_path": "/path/to/workspace",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z",
  "file_count": 1234,
  "method_count": 5678,
  "checksum": "sha256_hash_of_workspace"
}
```

字段说明：
- `version`: 索引格式版本号，用于兼容性检查
- `workspace_path`: 工作空间绝对路径
- `created_at`: 索引创建时间
- `updated_at`: 索引最后更新时间
- `file_count`: 索引的文件总数
- `method_count`: 索引的方法总数
- `checksum`: 工作空间的校验和（基于文件修改时间）

### 2. 主索引文件 (index.json)

存储完整的代码索引数据：

```json
{
  "methods": {
    "com.example.Service::method": {
      "name": "method",
      "full_qualified_name": "com.example.Service::method",
      "file_path": "src/Service.java",
      "line_range": [10, 20],
      "calls": [
        {
          "target": "com.example.Dao::query",
          "line": 15
        }
      ],
      "http_annotations": {
        "method": "GET",
        "path": "/api/users",
        "path_params": ["id"],
        "is_feign_client": false
      },
      "kafka_operations": [
        {
          "operation_type": "Produce",
          "topic": "user-events",
          "line": 18
        }
      ],
      "db_operations": [
        {
          "operation_type": "Select",
          "table": "users",
          "line": 16
        }
      ],
      "redis_operations": [
        {
          "operation_type": "Get",
          "key_pattern": "user:*",
          "line": 17
        }
      ]
    }
  },
  "method_calls": {
    "com.example.Service::method": [
      "com.example.Dao::query"
    ]
  },
  "reverse_calls": {
    "com.example.Dao::query": [
      "com.example.Service::method"
    ]
  },
  "http_providers": {
    "GET:/api/users": "com.example.Controller::getUsers"
  },
  "http_consumers": {
    "GET:/api/users": [
      "com.example.Client::fetchUsers"
    ]
  },
  "kafka_producers": {
    "user-events": [
      "com.example.Producer::sendEvent"
    ]
  },
  "kafka_consumers": {
    "user-events": [
      "com.example.Consumer::handleEvent"
    ]
  },
  "db_writers": {
    "users": [
      "com.example.Dao::saveUser"
    ]
  },
  "db_readers": {
    "users": [
      "com.example.Dao::getUser"
    ]
  },
  "redis_writers": {
    "user:*": [
      "com.example.Cache::setUser"
    ]
  },
  "redis_readers": {
    "user:*": [
      "com.example.Cache::getUser"
    ]
  },
  "config_associations": {
    "http:GET:/api/users": [
      "com.example.Client::fetchUsers"
    ]
  }
}
```

## 索引生命周期

### 1. 启动时加载

程序启动时的处理流程：

```
1. 检查 .code-impact-analyzer/index.meta.json 是否存在
   ├─ 不存在 → 执行完整索引构建
   └─ 存在 → 继续验证

2. 验证索引有效性
   ├─ 检查版本号是否兼容
   ├─ 检查工作空间路径是否匹配
   └─ 检查校验和是否一致
   
3. 根据验证结果
   ├─ 有效 → 加载 index.json
   └─ 无效 → 重新构建索引
```

### 2. 索引更新策略

支持以下更新策略：

- **完全重建**: 删除现有索引，重新扫描整个工作空间
- **增量更新**: 仅更新变更的文件（未来功能）
- **自动检测**: 基于文件修改时间自动判断是否需要更新

### 3. 索引失效条件

以下情况会导致索引失效，需要重建：

- 索引文件不存在或损坏
- 索引格式版本不兼容
- 工作空间路径变更
- 校验和不匹配（文件有修改）
- 用户手动触发重建

## 性能优化

### 1. 延迟加载

对于大型项目，可以采用延迟加载策略：
- 首先加载元数据和索引结构
- 按需加载具体的方法详情

### 2. 压缩存储

对于大型索引文件，可以使用压缩格式：
- 使用 gzip 压缩 JSON 文件
- 文件名改为 `index.json.gz`

### 3. 分片存储

对于超大型项目，可以将索引分片存储：
```
.code-impact-analyzer/
├── index.meta.json
├── shards/
│   ├── methods_0.json
│   ├── methods_1.json
│   └── ...
```

## 命令行接口

提供以下命令行选项：

```bash
# 强制重建索引
code-impact-analyzer --rebuild-index

# 显示索引信息
code-impact-analyzer --index-info

# 清除索引
code-impact-analyzer --clear-index

# 验证索引
code-impact-analyzer --verify-index
```

## 兼容性

### 版本兼容性

索引格式版本号采用语义化版本：
- 主版本号：不兼容的格式变更
- 次版本号：向后兼容的功能添加
- 修订号：向后兼容的问题修复

### 迁移策略

当索引格式升级时：
1. 尝试自动迁移旧格式到新格式
2. 如果迁移失败，提示用户重建索引
3. 保留旧索引文件作为备份

## 安全性

### 1. 路径安全

- 所有文件路径使用相对路径存储
- 加载时验证路径不越界工作空间

### 2. 数据完整性

- 使用校验和验证索引文件完整性
- 检测并处理损坏的索引文件

### 3. 并发安全

- 使用文件锁防止并发写入
- 支持多进程安全读取
