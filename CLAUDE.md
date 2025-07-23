# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

FluxFaaS 是一个用 Rust 构建的高性能 Serverless 执行引擎，基于 Silent 框架。项目目前处于第四阶段开发中，已经实现了真实的 Rust 代码编译和执行能力。

## 常用命令

### 构建和运行
```bash
# 构建项目（开发模式）
cargo build

# 构建项目（发布模式）
cargo build --release

# 运行主服务器
cargo run --release

# 运行 CLI 客户端
cargo run --bin flux-cli
```

### 开发工具
```bash
# 代码格式化（使用 rustfmt.toml 配置）
cargo fmt

# 静态分析
cargo clippy

# 运行测试
cargo test

# 运行特定测试示例
cargo run --bin test-compiler
cargo run --bin test-sandbox
cargo run --bin test-isolated-executor
```

## 核心架构

FluxFaaS 采用模块化架构，包含四个主要模块：

### 1. Functions 模块 (`src/functions/`)
- **registry.rs**: 函数注册表，管理函数元数据和生命周期
- **storage.rs**: 函数存储后端，支持持久化
- **watcher.rs**: 文件监控系统，支持热重载

### 2. Runtime 模块 (`src/runtime/`)
- **compiler.rs**: 真实 Rust 代码编译器，集成 rustc
- **sandbox.rs**: 沙箱隔离执行环境，进程级隔离
- **instance.rs**: 函数实例管理，支持实例池
- **executor.rs**: 执行引擎核心
- **resource.rs**: 系统资源监控和限制
- **cache.rs**: 智能 LRU 缓存系统
- **validator.rs**: 代码安全验证器

### 3. Scheduler 模块 (`src/scheduler/`)
- **pool.rs**: 函数实例池管理
- **balancer.rs**: 智能负载均衡器
- **lifecycle.rs**: 生命周期管理
- **simple.rs**: 简单调度器实现

### 4. Gateway 模块 (`src/gateway/`)
- **routes.rs**: HTTP 路由定义
- **handlers.rs**: 请求处理器
- 基于 Silent Web 框架

## 技术栈和依赖

### 核心依赖
- **silent**: Web 框架 (v2)
- **tokio**: 异步运行时 (full features)
- **serde/serde_json**: 序列化
- **libloading**: 动态库加载 (用于真实编译执行)
- **sysinfo**: 系统资源监控
- **scru128**: ID 生成 (替代 UUID)

### 开发工具
- **tracing**: 日志记录
- **anyhow/thiserror**: 错误处理
- **tempfile**: 临时文件管理
- **nix/libc**: 系统调用和进程管理

## 关键特性

### 真实代码执行
- 使用 rustc 编译器编译真实 Rust 代码
- 通过 libloading 动态加载编译后的库
- 支持复杂的 Rust 语法和标准库

### 沙箱隔离
- 进程级隔离执行环境
- 系统资源配额管理
- 网络和文件系统访问控制

### 智能调度
- 函数实例池管理
- 负载均衡算法
- 生命周期优化
- 断路器模式

### 性能监控
- 实时资源使用监控
- 编译时间和执行时间统计
- 缓存命中率追踪

## 服务器端点

默认服务器地址: `http://127.0.0.1:3000`

主要 API 端点：
- `GET /health` - 健康检查
- `GET /functions` - 列出所有函数
- `POST /functions` - 注册新函数
- `POST /invoke/:name` - 调用函数
- `POST /load/file` - 从文件加载函数
- `GET /status` - 系统状态
- `GET /performance/stats` - 性能统计

## 示例函数

项目包含多个示例函数在 `examples/functions/` 目录：
- `greet.rs`: 简单问候函数
- `multiply.rs`: 数学计算函数
- `uppercase.rs`: 字符串处理函数

## 缓存机制

编译后的动态库缓存在 `flux_cache/` 目录，使用 MD5 哈希命名。

## 开发注意事项

- 项目使用 Rust 2024 Edition
- 代码格式化配置在 `rustfmt.toml` (max_width = 100)
- 使用 scru128 生成 ID 而不是 UUID
- 所有异步操作基于 Tokio
- 错误处理遵循 Rust 最佳实践（anyhow + thiserror）
