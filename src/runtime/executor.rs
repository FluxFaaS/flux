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

/// 进程执行器 - 在独立进程中执行函数（为将来扩展预留）
pub struct ProcessExecutor;

#[async_trait::async_trait]
impl Executor for ProcessExecutor {
    async fn execute(
        &self,
        _function: &FunctionMetadata,
        _request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        // TODO: 实现进程隔离的执行
        todo!("Process isolation not implemented yet")
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
