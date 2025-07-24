# FluxFaaS HTTP架构使用指南

## 概述

FluxFaaS 现在支持两种运行模式：
1. **传统CLI模式**：本地交互式命令行界面
2. **HTTP客户端模式**：通过HTTP API调用远程或本地服务

## 程序结构

### 主程序 (`flux`)
- **路径**: `src/main.rs`
- **编译**: `cargo run --bin flux` 或 `cargo build --bin flux`
- **功能**: 传统的交互式CLI界面，包含所有FluxFaaS核心功能

### HTTP客户端 (`flux-cli`)
- **路径**: `src/bin/flux-cli.rs`
- **编译**: `cargo run --bin flux-cli` 或 `cargo build --bin flux-cli`
- **功能**: 通过HTTP API调用FluxFaaS服务器的CLI客户端

## 使用方法

### 模式1：传统CLI模式
```bash
# 启动传统CLI界面
cargo run --bin flux

# 或构建后运行
cargo build --release
./target/release/flux
```

这将启动完整的FluxFaaS系统，包括：
- 函数注册和管理
- 函数执行运行时
- 缓存系统
- 性能监控
- SCRU128 ID管理

### 模式2：HTTP客户端模式

#### 步骤1：准备HTTP服务器
目前，由于Silent框架的API复杂性，HTTP服务器模式暂时延期实现。
客户端已经准备就绪，等待服务器端的适配。

#### 步骤2：启动客户端
```bash
# 连接到默认服务器 (http://127.0.0.1:3000)
cargo run --bin flux-cli

# 或指定服务器地址
FLUX_SERVER_URL=http://localhost:8080 cargo run --bin flux-cli
```

## 功能对比

| 功能 | 传统CLI模式 | HTTP客户端模式 |
|------|------------|-------------|
| 查看所有函数 | ✅ | ✅ (待服务器) |
| 调用函数 | ✅ | ✅ (待服务器) |
| 注册新函数 | ✅ | ✅ (待服务器) |
| 从文件加载函数 | ✅ | ✅ (待服务器) |
| 从目录批量加载 | ✅ | ✅ (待服务器) |
| 查看系统状态 | ✅ | ✅ (待服务器) |
| 查看缓存统计 | ✅ | ✅ (待服务器) |
| 查看性能监控 | ✅ | ✅ (待服务器) |
| 重置监控数据 | ✅ | ✅ (待服务器) |

## 环境变量

### HTTP客户端配置
- `FLUX_SERVER_URL`: FluxFaaS服务器地址 (默认: http://127.0.0.1:3000)

### HTTP服务器配置 (计划中)
- `FLUX_HOST`: 服务器绑定地址 (默认: 127.0.0.1)
- `FLUX_PORT`: 服务器端口 (默认: 3000)

## 开发状态

### ✅ 已完成
- 独立的CLI客户端程序
- 完整的HTTP API客户端功能
- 所有现有CLI功能的HTTP版本
- 连接检查和错误处理
- 环境变量配置支持

### 🚧 进行中
- Silent框架HTTP服务器适配
- API接口标准化
- 错误处理统一

### 📋 计划中
- HTTP服务器完整实现
- 多客户端支持
- 认证和授权机制
- 服务发现和负载均衡

## 架构优势

1. **模块化**: CLI和服务器端完全解耦
2. **兼容性**: 保持原有CLI功能完整性
3. **扩展性**: 支持远程调用和分布式部署
4. **一致性**: HTTP客户端提供与本地CLI相同的用户体验

## 下一步开发

1. 完成Silent框架HTTP服务器实现
2. 测试HTTP API的完整功能
3. 添加认证和安全性功能
4. 支持配置文件管理
5. 优化错误处理和用户体验

这种架构为FluxFaaS向分布式Serverless平台发展奠定了基础。

