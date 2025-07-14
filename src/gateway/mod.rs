#![allow(dead_code)]
use crate::functions::{FunctionMetadata, RegisterFunctionRequest};
use crate::scheduler::SimpleScheduler;
use silent::prelude::*;
use std::sync::Arc;

pub mod handlers;
pub mod routes;

/// FluxFaaS 网关，负责处理 HTTP 请求
#[derive(Debug, Clone)]
pub struct FluxGateway {
    scheduler: Arc<SimpleScheduler>,
}

impl FluxGateway {
    /// 创建新的网关实例
    pub fn new() -> Self {
        Self {
            scheduler: Arc::new(SimpleScheduler::new()),
        }
    }

    /// 使用指定的调度器创建网关
    pub fn with_scheduler(scheduler: SimpleScheduler) -> Self {
        Self {
            scheduler: Arc::new(scheduler),
        }
    }

    /// 获取调度器引用
    pub fn scheduler(&self) -> Arc<SimpleScheduler> {
        self.scheduler.clone()
    }

    /// 构建路由
    pub fn routes(&self) -> RootRoute {
        routes::build_routes()
    }

    /// 注册示例函数
    pub async fn register_sample_functions(&self) -> anyhow::Result<()> {
        let registry = self.scheduler.registry();

        let hello_fn = FunctionMetadata::from_request(RegisterFunctionRequest {
            name: "hello".to_string(),
            description: Some("Hello World 函数".to_string()),
            code: "return \"Hello, World!\"".to_string(),
            timeout_ms: Some(1000),
            version: None,
            dependencies: None,
            parameters: None,
            return_type: None,
        });
        registry
            .register(hello_fn)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // 注册 echo 函数
        let echo_fn = FunctionMetadata::from_request(RegisterFunctionRequest {
            name: "echo".to_string(),
            description: Some("回声函数".to_string()),
            code: "return input".to_string(),
            timeout_ms: Some(1000),
            version: None,
            dependencies: None,
            parameters: None,
            return_type: None,
        });
        registry
            .register(echo_fn)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // 注册 add 函数
        let add_fn = FunctionMetadata::from_request(RegisterFunctionRequest {
            name: "add".to_string(),
            description: Some("加法函数".to_string()),
            code: "return a + b".to_string(),
            timeout_ms: Some(1000),
            version: None,
            dependencies: None,
            parameters: None,
            return_type: None,
        });
        registry
            .register(add_fn)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        tracing::info!("Sample functions registered successfully");
        Ok(())
    }
}

impl Default for FluxGateway {
    fn default() -> Self {
        Self::new()
    }
}
