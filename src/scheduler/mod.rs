#![allow(dead_code)]
use crate::functions::registry::FunctionRegistry;
use crate::functions::{InvokeRequest, InvokeResponse, Result};
use crate::runtime::SimpleRuntime;
use crate::runtime::loader::FunctionLoader;
use std::sync::Arc;

pub mod simple;

/// 调度器特征
#[async_trait::async_trait]
pub trait Scheduler {
    /// 调度函数执行
    async fn schedule(&self, function_name: &str, request: InvokeRequest)
    -> Result<InvokeResponse>;
}

/// 简单调度器实现
#[derive(Debug, Clone)]
pub struct SimpleScheduler {
    registry: FunctionRegistry,
    runtime: Arc<SimpleRuntime>,
    loader: Arc<FunctionLoader>,
}

impl SimpleScheduler {
    pub fn new() -> Self {
        Self {
            registry: FunctionRegistry::new(),
            runtime: Arc::new(SimpleRuntime::new()),
            loader: Arc::new(FunctionLoader::new()),
        }
    }

    pub fn new_with_compilation() -> anyhow::Result<Self> {
        Ok(Self {
            registry: FunctionRegistry::new(),
            runtime: Arc::new(SimpleRuntime::new_with_compilation()?),
            loader: Arc::new(FunctionLoader::new()),
        })
    }

    pub fn with_registry(registry: FunctionRegistry) -> Self {
        Self {
            registry,
            runtime: Arc::new(SimpleRuntime::new()),
            loader: Arc::new(FunctionLoader::new()),
        }
    }

    pub fn with_loader(loader: Arc<FunctionLoader>) -> Self {
        Self {
            registry: FunctionRegistry::new(),
            runtime: Arc::new(SimpleRuntime::new()),
            loader,
        }
    }

    pub fn with_runtime(runtime: Arc<SimpleRuntime>) -> Self {
        Self {
            registry: FunctionRegistry::new(),
            runtime,
            loader: Arc::new(FunctionLoader::new()),
        }
    }

    /// 获取函数注册表的引用
    pub fn registry(&self) -> &FunctionRegistry {
        &self.registry
    }

    /// 获取运行时的引用
    pub fn runtime(&self) -> &Arc<SimpleRuntime> {
        &self.runtime
    }

    /// 获取函数加载器的引用
    pub fn loader(&self) -> &Arc<FunctionLoader> {
        &self.loader
    }
}

#[async_trait::async_trait]
impl Scheduler for SimpleScheduler {
    async fn schedule(
        &self,
        function_name: &str,
        request: InvokeRequest,
    ) -> Result<InvokeResponse> {
        tracing::info!("Scheduling function: {}", function_name);

        // 从注册表获取函数
        let function = self.registry.get(function_name).await?;

        // 执行函数
        let response = self.runtime.execute(&function, &request).await?;

        tracing::info!("Function {} scheduled and executed", function_name);
        Ok(response)
    }
}

impl Default for SimpleScheduler {
    fn default() -> Self {
        Self::new()
    }
}
