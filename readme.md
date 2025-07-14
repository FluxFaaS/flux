# FluxFaaS - 动态函数加载已完成

🚀 **FluxFaaS** 是一个用 Rust 构建的轻量级私有 Serverless 平台，基于 Silent 框架。

**第一阶段MVP** ✅ 已完成
**第二阶段动态函数加载** ✅ 已完成
**第三阶段高级调度优化** 🚧 进行中

## ✨ 功能特色

- 🔧 **模块化架构** - 函数注册、调度、执行完全分离
- ⚡ **高性能** - 基于 Tokio 异步运行时
- 🛡️ **类型安全** - 完整的 Rust 类型系统保护
- 🎯 **轻量级** - 最小化依赖，专注核心功能
- 🔄 **可扩展** - 预留多种扩展接口
- 🔥 **动态加载** - 支持从文件和字符串动态加载函数
- 🗄️ **智能缓存** - LRU缓存策略，性能监控
- 🔒 **安全验证** - 严格的代码安全检查和复杂度分析

## 🏗️ 核心架构

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Gateway       │    │   Scheduler     │    │    Runtime      │
│                 │    │                 │    │                 │
│ - HTTP API      │◄──►│ - 函数调度      │◄──►│ - 函数执行      │
│ - 路由管理      │    │ - 负载均衡      │    │ - 超时控制      │
│ - 请求处理      │    │ - 状态管理      │    │ - 智能缓存      │
│ - 文件加载API   │    │ - 性能监控      │    │ - 性能监控      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   Functions     │
                    │                 │
                    │ - 函数注册表    │
                    │ - 元数据管理    │
                    │ - 存储后端      │
                    │ - 文件监控      │
                    │ - 代码验证      │
                    └─────────────────┘
```

## 🚀 快速开始

### 环境要求

- Rust 1.70+
- Cargo

### 安装运行

```bash
# 克隆项目
git clone https://github.com/FluxFaaS/flux.git
cd flux

# 构建项目
cargo build

# 运行 MVP
cargo run
```

### 使用 CLI

程序启动后会进入交互式 CLI 模式：

```
🎯 FluxFaaS Interactive CLI
===========================
Available commands:
  1. list     - List all functions
  2. invoke   - Invoke a function
  3. register - Register a new function
  4. status   - Show system status
  5. help     - Show this help
  6. quit     - Exit the CLI

flux>
```

## 📝 示例使用

### 1. 查看函数列表
```
flux> list
📋 Registered Functions:
------------------------
1. hello - A simple hello world function
2. echo - Echo input back to output
3. add - Add two numbers
Total: 3 functions
```

### 2. 调用函数
```
flux> invoke
Function name: hello
Input (JSON format, or press Enter for empty):
🚀 Invoking function 'hello'...
✅ Function executed successfully!
📊 Execution time: 0ms
📤 Output: {
  "input": {},
  "message": "Hello, World!"
}
🔍 Status: Success
```

### 3. 加法函数示例
```
flux> invoke
Function name: add
Input (JSON format, or press Enter for empty): {"a": 5, "b": 3}
🚀 Invoking function 'add'...
✅ Function executed successfully!
📊 Execution time: 0ms
📤 Output: {
  "result": 8
}
🔍 Status: Success
```

### 4. 注册新函数
```
flux> register
Function name: greet
Description (optional): Greeting function
Code: return "Hello, " + name + "!"
✅ Function 'greet' registered successfully!
```

## 🧪 内置示例函数

### Hello World
- **名称**: `hello`
- **功能**: 返回 "Hello, World!" 消息
- **输入**: 任意 JSON
- **输出**: `{"message": "Hello, World!", "input": <输入>}`

### Echo
- **名称**: `echo`
- **功能**: 回声函数，原样返回输入
- **输入**: 任意 JSON
- **输出**: 输入的原始 JSON

### 加法计算
- **名称**: `add`
- **功能**: 两数相加
- **输入**: `{"a": 数字, "b": 数字}`
- **输出**: `{"result": 和}`

## 🔧 技术栈

- **语言**: Rust 2024 Edition
- **异步运行时**: Tokio
- **Web 框架**: Silent (计划中)
- **序列化**: Serde
- **错误处理**: thiserror + anyhow
- **日志**: tracing
- **时间处理**: chrono
- **ID生成**: scru128

## 📦 项目结构

```
src/
├── main.rs              # 程序入口和 CLI 界面
├── functions/           # 函数管理模块
│   ├── mod.rs          # 函数数据结构和错误定义
│   └── registry.rs     # 函数注册表实现
├── runtime/            # 运行时模块
│   ├── mod.rs         # 简单运行时实现
│   └── executor.rs    # 执行器接口（预留）
├── scheduler/         # 调度器模块
│   ├── mod.rs        # 简单调度器实现
│   └── simple.rs     # 高级调度器（预留）
└── gateway/          # 网关模块（开发中）
    ├── mod.rs       # 网关核心
    ├── routes.rs    # 路由定义
    └── handlers.rs  # 请求处理器
```

## 🛣️ 发展路线

### MVP 阶段 ✅ (已完成)
- [x] 核心架构设计
- [x] 函数注册表
- [x] 函数执行器
- [x] 基础调度器
- [x] CLI 交互界面
- [x] 示例函数

### 第二阶段 ✅ (已完成) - 动态函数加载
- [x] HTTP 网关（基于 Silent）
- [x] RESTful API 接口
- [x] 动态函数加载（文件/字符串）
- [x] 函数热重载和文件监控
- [x] 智能LRU缓存系统
- [x] 代码安全验证器
- [x] 性能监控和统计
- [x] 函数存储后端
- [x] 增强的元数据管理

### 第三阶段 🚧 (进行中) - 高级调度优化
- [ ] 真实Rust代码编译和执行
- [ ] 沙箱隔离执行环境
- [ ] 函数实例生命周期管理
- [ ] 智能负载均衡和调度
- [ ] 资源限制和配额管理
- [ ] 高级错误处理和重试
- [ ] 函数依赖解析和管理

### 第四阶段 🔮 (未来规划) - 生产就绪
- [ ] 容器化执行支持
- [ ] 多语言运行时支持
- [ ] 集群部署和分布式调度
- [ ] Web 管理界面
- [ ] 监控告警系统
- [ ] 日志聚合和分析

## 🤝 贡献指南

1. Fork 项目
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 📄 许可证

本项目采用 Apache-2.0 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

- [Silent](https://github.com/hubertshelley/silent) - Web 框架支持
- [Tokio](https://tokio.rs/) - 异步运行时
- [Serde](https://serde.rs/) - 序列化框架

---

⭐ 如果这个项目对你有帮助，请给我们一个 Star！
