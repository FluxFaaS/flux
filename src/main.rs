#![allow(clippy::uninlined_format_args)]

use std::net::SocketAddr;
use tracing::info;

mod functions;
mod gateway;
mod runtime;
mod scheduler;

use gateway::routes::build_routes;
use scheduler::SimpleScheduler;
use silent::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    info!("🚀 Starting FluxFaaS HTTP Server...");

    // 初始化调度器
    let scheduler = Arc::new(SimpleScheduler::new());

    // 预注册示例函数
    register_sample_functions(&scheduler).await?;

    // 创建配置并注入 scheduler
    let mut configs = Configs::default();
    configs.insert(scheduler);

    // 构建路由（不再需要传递 scheduler）
    let routes = build_routes();

    // 配置服务器地址
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;

    info!("🌐 FluxFaaS HTTP Server starting on http://{}", addr);
    info!("📋 Available endpoints:");
    info!("  GET  /health                    - Health check");
    info!("  GET  /functions                 - List all functions");
    info!("  POST /functions                 - Register new function");
    info!("  GET  /functions/:name           - Get function details");
    info!("  DELETE /functions/:name         - Delete function");
    info!("  POST /invoke/:name              - Invoke function");
    info!("  GET  /status                    - System status");
    info!("  POST /load/file                 - Load function from file");
    info!("  POST /load/directory            - Load functions from directory");
    info!("  GET  /cache/stats               - Cache statistics");
    info!("  GET  /performance/stats         - Performance statistics");
    info!("  POST /reset                     - Reset scheduler");
    info!("");
    info!("💡 Use 'flux-cli' command to interact with the server");
    info!("🚀 Server is ready to accept requests!");

    // 启动 HTTP 服务器，使用 with_configs 注入配置
    Server::new()
        .with_configs(configs)
        .bind(addr)
        .serve(routes)
        .await;

    Ok(())
}

/// 预注册示例函数
async fn register_sample_functions(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    use functions::{FunctionMetadata, RegisterFunctionRequest, ScriptType};

    let registry = scheduler.registry();

    let sample_functions = vec![
        RegisterFunctionRequest {
            name: "hello".to_string(),
            description: Some("Hello World 函数".to_string()),
            code: "return \"Hello, World!\"".to_string(),
            timeout_ms: Some(5000),
            version: None,
            dependencies: None,
            parameters: None,
            return_type: None,
            script_type: ScriptType::JavaScript,
        },
        RegisterFunctionRequest {
            name: "echo".to_string(),
            description: Some("回声函数".to_string()),
            code: "return input".to_string(),
            timeout_ms: Some(3000),
            version: None,
            dependencies: None,
            parameters: None,
            return_type: None,
            script_type: ScriptType::JavaScript,
        },
        RegisterFunctionRequest {
            name: "add".to_string(),
            description: Some("加法函数".to_string()),
            code: "const {a, b} = JSON.parse(input); return (a + b).toString();".to_string(),
            timeout_ms: Some(2000),
            version: None,
            dependencies: None,
            parameters: None,
            return_type: None,
            script_type: ScriptType::JavaScript,
        },
    ];

    for func_req in sample_functions {
        let metadata = FunctionMetadata::from_request(func_req);
        registry.register(metadata).await?;
    }

    info!("📚 Sample functions registered successfully");
    Ok(())
}
