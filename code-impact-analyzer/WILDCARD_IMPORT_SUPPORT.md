# 通配符导入支持

## 功能说明

对于没有明确import的非基础Java类（如`Foo foo;`），当代码中有`import bar.*;`这样的通配符导入时，系统会按照以下优先级解析类的完整限定名：

1. **明确的import语句**：如果有`import com.example.Foo;`，直接使用`com.example.Foo`
2. **通配符导入包**：如果有`import bar.*;`，优先使用`bar.Foo`
3. **当前包**：如果以上都找不到，使用当前代码的包名

## 实现方式

### 启发式方法（当前实现）

由于在解析单个文件时可能还没有完整的全局索引，当前实现使用了启发式方法：

- 如果有通配符导入，优先使用第一个通配符导入的包
- 这在大多数情况下是正确的，因为开发者通常会按照使用频率排列导入语句

### 完整方法（可选）

如果需要更精确的解析，可以使用`parse_file_with_global_types_and_classes`方法，传入全局类索引：

```rust
let mut global_class_index = rustc_hash::FxHashMap::default();
global_class_index.insert("bar.Foo".to_string(), "bar.Foo".to_string());

let result = parser.parse_file_with_global_types_and_classes(
    content, 
    file_path, 
    &global_return_types,
    &global_class_index
);
```

这样系统会在通配符导入的包中查找类是否存在，只有找到了才使用该包名。

## 示例

### 示例 1：通配符导入解析

```java
package com.example.service;

import com.example.model.*;

public class UserService {
    public void processUser() {
        User user = new User();  // User 被解析为 com.example.model.User
        user.setName("test");
    }
}
```

### 示例 2：回退到当前包

```java
package com.example.service;

import com.example.model.*;

public class UserService {
    public void processHelper() {
        Helper helper = new Helper();  // Helper 不在 model 包中
        // 使用启发式方法：先尝试 com.example.model.Helper
        // 如果有全局索引且找不到，则回退到 com.example.service.Helper
    }
}
```

### 示例 3：多个通配符导入

```java
package com.example.service;

import com.example.model.*;
import com.example.dto.*;

public class UserService {
    public void process() {
        User user = new User();      // 使用第一个通配符导入：com.example.model.User
        UserDTO dto = new UserDTO(); // 使用第一个通配符导入：com.example.model.UserDTO
    }
}
```

注意：在启发式方法下，所有未明确导入的类都会使用第一个通配符导入的包。如果需要更精确的解析，应该使用全局类索引。

## 相关代码

- `walk_node_for_import_map_with_wildcards`: 提取通配符导入
- `build_import_map_with_wildcards`: 构建导入映射和通配符列表
- `resolve_full_class_name_with_wildcard_fallback`: 使用启发式方法解析类名
- `resolve_full_class_name_with_wildcards`: 使用全局索引解析类名（更精确）

## 测试

参见 `tests/wildcard_import_resolution_test.rs` 中的测试用例。
