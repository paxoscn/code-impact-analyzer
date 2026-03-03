# 接口映射持久化实现

## 概述

本次修改将 `interface_implementations` 和 `class_interfaces` 两个映射添加到索引文件的序列化和反序列化过程中，确保这些关键的接口实现关系数据能够被持久化存储。

## 背景

在之前的实现中，`CodeIndex` 包含两个重要的映射：
- `interface_implementations`: 接口 -> 实现类列表
- `class_interfaces`: 实现类 -> 接口列表

这些映射在运行时被构建和使用，但在索引序列化时被忽略了。这导致每次加载索引后，这些映射都是空的，需要重新解析所有文件才能重建。

## 修改内容

### 1. SerializableIndex 结构体 (index_storage.rs)

添加了两个新字段：

```rust
pub struct SerializableIndex {
    // ... 其他字段 ...
    
    /// 接口到实现类的映射
    pub interface_implementations: HashMap<String, Vec<String>>,
    
    /// 实现类到接口的映射
    pub class_interfaces: HashMap<String, Vec<String>>,
}
```

### 2. CodeIndex 访问方法 (code_index.rs)

添加了两个公共迭代器方法，用于访问接口映射：

```rust
/// 获取所有接口实现映射
pub fn interface_implementations(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
    self.interface_implementations.iter()
}

/// 获取所有类接口映射
pub fn class_interfaces(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
    self.class_interfaces.iter()
}
```

添加了两个内部方法，用于反序列化时设置映射：

```rust
/// 内部方法：直接设置接口实现映射
#[doc(hidden)]
pub fn set_interface_implementations(&mut self, interface_implementations: FxHashMap<String, Vec<String>>) {
    self.interface_implementations = interface_implementations;
}

/// 内部方法：直接设置类接口映射
#[doc(hidden)]
pub fn set_class_interfaces(&mut self, class_interfaces: FxHashMap<String, Vec<String>>) {
    self.class_interfaces = class_interfaces;
}
```

### 3. 序列化实现 (index_storage.rs)

在 `serialize_index` 方法中添加了接口映射的收集：

```rust
// 收集接口实现映射
let mut interface_implementations = HashMap::new();
for (interface, implementations) in code_index.interface_implementations() {
    interface_implementations.insert(interface.clone(), implementations.clone());
}

// 收集类接口映射
let mut class_interfaces = HashMap::new();
for (class, interfaces) in code_index.class_interfaces() {
    class_interfaces.insert(class.clone(), interfaces.clone());
}
```

### 4. 反序列化实现 (index_storage.rs)

在 `deserialize_index` 方法中添加了接口映射的恢复：

```rust
// 恢复接口实现映射
let interface_implementations: FxHashMap<String, Vec<String>> = data.interface_implementations
    .into_iter()
    .collect();
code_index.set_interface_implementations(interface_implementations);

// 恢复类接口映射
let class_interfaces: FxHashMap<String, Vec<String>> = data.class_interfaces
    .into_iter()
    .collect();
code_index.set_class_interfaces(class_interfaces);
```

### 5. 导入更新 (index_storage.rs)

添加了 `FxHashMap` 的导入：

```rust
use rustc_hash::FxHashMap;
```

## 测试

添加了新的测试 `test_interface_mappings_persistence` 来验证接口映射的序列化和反序列化：

- 创建包含接口实现关系的索引
- 保存索引到文件
- 从文件加载索引
- 验证接口映射被正确恢复

所有相关测试均通过：
- `test_interface_mappings_persistence` ✓
- `test_interface_upstream_tracing` ✓
- `test_multiple_interfaces_upstream_tracing` ✓
- `test_interface_resolution_*` ✓

## 影响

### 性能提升
- 加载索引后不再需要重新解析文件来重建接口映射
- 减少了索引加载后的初始化时间

### 功能完整性
- 接口相关的影响分析功能在索引加载后立即可用
- `resolve_interface_call` 方法可以正确工作
- 上游追踪可以正确处理接口调用

### 数据一致性
- 索引文件现在包含完整的代码关系信息
- 多次保存和加载不会丢失接口映射数据

## 注意事项

1. 索引文件格式已更新，旧的索引文件在加载时接口映射将为空（但不会报错）
2. 建议在升级后重新构建索引以获得完整的接口映射数据
3. `merge` 方法已经包含了接口映射的合并逻辑，无需额外修改

## 相关文件

- `code-impact-analyzer/src/code_index.rs`
- `code-impact-analyzer/src/index_storage.rs`
- `code-impact-analyzer/tests/index_storage_test.rs`
