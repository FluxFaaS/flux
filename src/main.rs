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

    // 构建路由
    let routes = build_routes(scheduler);

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
    info!("  POST /functions/load-file       - Load function from file");
    info!("  POST /functions/load-directory  - Load functions from directory");
    info!("  GET  /cache/stats               - Cache statistics");
    info!("  GET  /monitor/performance       - Performance monitoring");
    info!("  POST /monitor/reset             - Reset monitoring data");
    info!("");
    info!("💡 Use 'flux-cli' command to interact with the server");
    info!("🚀 Server is ready to accept requests!");

    // 启动 HTTP 服务器
    Server::new().bind(addr).serve(routes).await;

    Ok(())
}

/// 预注册示例函数
async fn register_sample_functions(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    use functions::{FunctionMetadata, RegisterFunctionRequest};

    let registry = scheduler.registry();

    let sample_functions = vec![
        RegisterFunctionRequest {
            name: "hello".to_string(),
            description: Some("Hello World 函数".to_string()),
            code: "return \"Hello, World!\"".to_string(),
            timeout_ms: Some(5000),
        },
        RegisterFunctionRequest {
            name: "echo".to_string(),
            description: Some("回声函数".to_string()),
            code: "return input".to_string(),
            timeout_ms: Some(3000),
        },
        RegisterFunctionRequest {
            name: "add".to_string(),
            description: Some("加法函数".to_string()),
            code: "const {a, b} = JSON.parse(input); return (a + b).toString();".to_string(),
            timeout_ms: Some(2000),
        },
    ];

    for func_req in sample_functions {
        let metadata = FunctionMetadata::from_request(func_req);
        registry.register(metadata).await?;
    }

    info!("📚 Sample functions registered successfully");
    Ok(())
}
