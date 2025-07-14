use crate::functions::{FunctionMetadata, InvokeRequest, InvokeResponse, Result};

/// 执行器特征，为将来的扩展做准备
#[async_trait::async_trait]
pub trait Executor {
    /// 执行函数
    async fn execute(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse>;
}

/// 内嵌执行器 - 直接在当前进程中执行函数
pub struct InlineExecutor;

#[async_trait::async_trait]
impl Executor for InlineExecutor {
    async fn execute(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        // 这里可以实现更复杂的执行逻辑
        // 目前暂时使用 SimpleRuntime 的实现
        let runtime = crate::runtime::SimpleRuntime::new();
        runtime.execute(function, request).await
    }
}

/// 进程执行器 - 在独立进程中执行函数
pub struct ProcessExecutor {
    /// 沙箱执行器
    sandbox: crate::runtime::sandbox::SandboxExecutor,
    /// 资源管理器
    resource_manager: crate::runtime::resource::ResourceManager,
}

impl ProcessExecutor {
    /// 创建新的进程执行器
    pub fn new() -> anyhow::Result<Self> {
        use crate::runtime::sandbox::SandboxConfig;

        let config = SandboxConfig::default();
        let sandbox = crate::runtime::sandbox::SandboxExecutor::new(config)?;
        let resource_manager = crate::runtime::resource::ResourceManager::new();

        Ok(Self {
            sandbox,
            resource_manager,
        })
    }

    /// 使用自定义配置创建进程执行器
    pub fn with_config(config: crate::runtime::sandbox::SandboxConfig) -> anyhow::Result<Self> {
        let sandbox = crate::runtime::sandbox::SandboxExecutor::new(config)?;
        let resource_manager = crate::runtime::resource::ResourceManager::new();

        Ok(Self {
            sandbox,
            resource_manager,
        })
    }
}

#[async_trait::async_trait]
impl Executor for ProcessExecutor {
    async fn execute(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        use crate::functions::FluxError;
        use crate::runtime::compiler::{CompilerConfig, RustCompiler};
        use std::time::Instant;

        let start_time = Instant::now();

        // 检查函数是否需要编译
        let compiler_config = CompilerConfig::default();
        let compiler = RustCompiler::new(compiler_config)
            .map_err(|e| FluxError::Runtime(format!("Failed to create compiler: {e}")))?;

        // 编译函数
        let compiled = compiler
            .compile_function(function)
            .await
            .map_err(|e| FluxError::Runtime(format!("Compilation failed: {e}")))?;

        // 在沙箱中执行
        let sandbox_result = self
            .sandbox
            .execute_in_sandbox(&compiled, request)
            .await
            .map_err(|e| FluxError::Runtime(format!("Sandbox execution failed: {e}")))?;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        let status_str = format!("{:?}", sandbox_result.status);

        // 创建响应
        let response = InvokeResponse {
            output: sandbox_result.output,
            execution_time_ms,
            status: sandbox_result.status,
        };
        tracing::info!(
            "Process execution completed for function '{}': status={}, time={}ms, memory={}MB",
            function.name,
            status_str,
            execution_time_ms,
            sandbox_result.peak_memory_bytes / 1024 / 1024
        );

        Ok(response)
    }
}

/// 容器执行器 - 在容器中执行函数（为将来扩展预留）
pub struct ContainerExecutor;

#[async_trait::async_trait]
impl Executor for ContainerExecutor {
    async fn execute(
        &self,
        _function: &FunctionMetadata,
        _request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        // TODO: 实现容器化执行
        todo!("Container execution not implemented yet")
    }
}
