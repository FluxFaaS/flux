# FluxFaaS 自定义Rust编译路径配置

## 概述

FluxFaaS 现在支持配置自定义的Rust编译输出路径，允许用户将编译产物输出到指定目录，而不是默认的 `target/` 目录。

## 配置方式

### 沙箱执行器配置

```rust
use flux::runtime::sandbox::SandboxConfig;
use std::path::PathBuf;

let sandbox_config = SandboxConfig {
    // ... 其他配置 ...
    rust_target_dir: Some(PathBuf::from("~/rust_target")),
    // ... 其他配置 ...
};
```

### 编译器配置

```rust
use flux::runtime::compiler::CompilerConfig;
use std::path::PathBuf;

let mut compiler_config = CompilerConfig::default();
compiler_config.rust_target_dir = Some(PathBuf::from("~/rust_target"));
```

## 实现原理

1. **环境变量设置**: 系统通过设置 `CARGO_TARGET_DIR` 环境变量告知cargo使用自定义路径
2. **路径展开**: 支持波浪号 `~` 路径展开，自动转换为实际的用户目录路径
3. **统一配置**: 编译器和沙箱执行器使用相同的路径配置确保一致性

## 技术细节

### 依赖添加

项目添加了 `shellexpand` 依赖用于处理波浪号路径展开：

```toml
shellexpand = "3.1"
```

### 实现修改

1. **SandboxConfig** 增加 `rust_target_dir` 字段
2. **CompilerConfig** 增加 `rust_target_dir` 字段
3. **create_function_executor** 方法支持自定义路径
4. **compile_to_dylib** 方法支持自定义路径

## 使用示例

设置用户编译路径为 `~/rust_target/`:

```rust
// 配置沙箱
let sandbox_config = SandboxConfig {
    enable_process_isolation: true,
    rust_target_dir: Some(PathBuf::from("~/rust_target")),
    ..Default::default()
};

// 配置编译器
let mut compiler_config = CompilerConfig::default();
compiler_config.rust_target_dir = Some(PathBuf::from("~/rust_target"));
```

## 验证方法

运行测试后，检查自定义目录中的编译产物：

```bash
# 查看编译产物
ls -la ~/rust_target/debug/
ls -la ~/rust_target/release/

# 查找动态库
find ~/rust_target -name "*.dylib" -o -name "*.so" -o -name "*.dll"
```

## 注意事项

1. 确保自定义路径目录存在且具有写权限
2. 路径中的波浪号 `~` 会自动展开为用户目录
3. 编译器和沙箱执行器应使用相同的路径配置以保持一致性
4. 自定义路径为可选配置，未设置时使用默认的项目内 `target/` 目录

## 性能影响

- 使用自定义路径不会影响编译性能
- 可以利用SSD等高性能存储设备作为编译输出目录
- 统一的编译路径有助于缓存复用和清理管理
