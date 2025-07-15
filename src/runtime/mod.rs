#![allow(dead_code)]
use crate::functions::{
    ExecutionStatus, FluxError, FunctionMetadata, InvokeRequest, InvokeResponse, Result,
};
use crate::runtime::cache::FunctionCache;
use crate::runtime::compiler::{CompilerConfig, RustCompiler};
use crate::runtime::monitor::{ExecutionResult, PerformanceMonitor};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;

pub mod cache;
pub mod compiler;
pub mod executor;
pub mod instance;
pub mod loader;
pub mod monitor;
pub mod resource;
pub mod sandbox;
pub mod validator;

/// 简单的函数执行器
#[derive(Debug)]
pub struct SimpleRuntime {
    /// 函数缓存
    cache: Arc<FunctionCache>,
    /// 性能监控器
    monitor: Arc<PerformanceMonitor>,
    /// Rust代码编译器（可选）
    compiler: Option<Arc<RustCompiler>>,
    /// 是否启用真实编译
    enable_compilation: bool,
}

impl SimpleRuntime {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(FunctionCache::default()),
            monitor: Arc::new(PerformanceMonitor::new()),
            compiler: None,
            enable_compilation: false,
        }
    }

    pub fn with_cache(cache: Arc<FunctionCache>) -> Self {
        Self {
            cache,
            monitor: Arc::new(PerformanceMonitor::new()),
            compiler: None,
            enable_compilation: false,
        }
    }

    pub fn with_monitor(monitor: Arc<PerformanceMonitor>) -> Self {
        Self {
            cache: Arc::new(FunctionCache::default()),
            monitor,
            compiler: None,
            enable_compilation: false,
        }
    }

    /// 创建支持真实编译的运行时
    pub fn new_with_compilation() -> anyhow::Result<Self> {
        let config = CompilerConfig::default();
        let compiler = RustCompiler::new(config)?;

        Ok(Self {
            cache: Arc::new(FunctionCache::default()),
            monitor: Arc::new(PerformanceMonitor::new()),
            compiler: Some(Arc::new(compiler)),
            enable_compilation: true,
        })
    }

    /// 使用自定义配置创建支持编译的运行时
    pub fn new_with_compiler_config(config: CompilerConfig) -> anyhow::Result<Self> {
        let compiler = RustCompiler::new(config)?;

        Ok(Self {
            cache: Arc::new(FunctionCache::default()),
            monitor: Arc::new(PerformanceMonitor::new()),
            compiler: Some(Arc::new(compiler)),
            enable_compilation: true,
        })
    }

    /// 启用或禁用编译
    pub fn set_compilation_enabled(&mut self, enabled: bool) {
        self.enable_compilation = enabled;
    }

    /// 检查是否支持编译
    pub fn supports_compilation(&self) -> bool {
        self.compiler.is_some() && self.enable_compilation
    }

    /// 使用真实编译执行函数
    async fn execute_with_compilation(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        let compiler = self
            .compiler
            .as_ref()
            .ok_or_else(|| FluxError::Runtime("Compiler not available".to_string()))?;

        // 编译函数
        let compiled = compiler
            .compile_function(function)
            .await
            .map_err(|e| FluxError::Runtime(format!("Compilation failed: {e}")))?;

        // 执行编译后的函数
        let response = compiler
            .execute_compiled_function(&compiled, request)
            .await
            .map_err(|e| FluxError::Runtime(format!("Execution failed: {e}")))?;

        Ok(response.output)
    }

    /// 获取性能监控器引用
    pub fn monitor(&self) -> &Arc<PerformanceMonitor> {
        &self.monitor
    }

    /// 获取缓存引用
    pub fn cache(&self) -> &Arc<FunctionCache> {
        &self.cache
    }

    /// 执行函数
    pub async fn execute(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        let start_time = Instant::now();

        tracing::info!("Executing function: {}", function.name);

        // 尝试从缓存获取编译后的函数
        if let Some(_cached_function) = self.cache.get(&function.name).await {
            tracing::debug!("Using cached version of function: {}", function.name);
            // 可以在这里使用预编译的结果来优化执行
        } else {
            // 缓存函数以备下次使用
            if let Err(e) = self
                .cache
                .put(function.name.clone(), function.clone())
                .await
            {
                tracing::warn!("Failed to cache function {}: {}", function.name, e);
            }
        }

        // 设置执行超时
        let timeout_duration = Duration::from_millis(function.timeout_ms);

        let result = timeout(timeout_duration, self.execute_function(function, request)).await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        let response = match result {
            Ok(Ok(output)) => {
                tracing::info!(
                    "Function {} executed successfully in {}ms",
                    function.name,
                    execution_time_ms
                );

                // 记录成功执行的性能数据
                let execution_result = ExecutionResult {
                    function_name: function.name.clone(),
                    duration: start_time.elapsed(),
                    success: true,
                    memory_usage: 1024, // 估算值，实际项目中应该测量真实内存使用
                    error_message: None,
                };

                if let Err(e) = self.monitor.record_execution(execution_result).await {
                    tracing::warn!("Failed to record performance data: {}", e);
                }

                InvokeResponse {
                    output,
                    execution_time_ms,
                    status: ExecutionStatus::Success,
                }
            }
            Ok(Err(e)) => {
                tracing::error!("Function {} execution failed: {}", function.name, e);

                // 记录失败执行的性能数据
                let execution_result = ExecutionResult {
                    function_name: function.name.clone(),
                    duration: start_time.elapsed(),
                    success: false,
                    memory_usage: 512, // 失败情况下的估算内存使用
                    error_message: Some(e.to_string()),
                };

                if let Err(monitor_err) = self.monitor.record_execution(execution_result).await {
                    tracing::warn!("Failed to record performance data: {}", monitor_err);
                }

                InvokeResponse {
                    output: serde_json::json!({"error": e.to_string()}),
                    execution_time_ms,
                    status: ExecutionStatus::Error(e.to_string()),
                }
            }
            Err(_) => {
                tracing::error!("Function {} execution timed out", function.name);

                // 记录超时执行的性能数据
                let execution_result = ExecutionResult {
                    function_name: function.name.clone(),
                    duration: start_time.elapsed(),
                    success: false,
                    memory_usage: 256, // 超时情况下的估算内存使用
                    error_message: Some("Execution timeout".to_string()),
                };

                if let Err(e) = self.monitor.record_execution(execution_result).await {
                    tracing::warn!("Failed to record performance data: {}", e);
                }

                InvokeResponse {
                    output: serde_json::json!({"error": "Execution timeout"}),
                    execution_time_ms,
                    status: ExecutionStatus::Timeout,
                }
            }
        };

        Ok(response)
    }

    /// 实际执行函数代码
    async fn execute_function(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        // 第三阶段：支持真实Rust代码编译和执行
        if self.supports_compilation() {
            return self.execute_with_compilation(function, request).await;
        }

        // 保留向后兼容：简单的字符串处理示例
        match function.name.as_str() {
            "hello" => {
                // 示例：Hello World 函数
                Ok(serde_json::json!({
                    "message": "Hello, World!",
                    "input": request.input
                }))
            }
            "echo" => {
                // 示例：Echo 函数
                Ok(request.input.clone())
            }
            "add" => {
                // 示例：加法函数
                if let (Some(a), Some(b)) = (
                    request.input.get("a").and_then(|v| v.as_f64()),
                    request.input.get("b").and_then(|v| v.as_f64()),
                ) {
                    Ok(serde_json::json!({"result": a + b}))
                } else {
                    Err(FluxError::Runtime(
                        "Invalid input for add function".to_string(),
                    ))
                }
            }
            _ => {
                // 对于其他函数，尝试简单的代码执行模拟
                // 这是一个非常简化的实现，仅用于 MVP 演示
                self.simulate_code_execution(&function.code, request).await
            }
        }
    }

    /// 模拟代码执行（MVP 阶段的简化实现）
    async fn simulate_code_execution(
        &self,
        code: &str,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        // 这是一个非常简化的代码执行模拟
        // 真实的实现应该使用沙盒环境或容器

        if code.contains("return") {
            // 提取 return 语句后的内容
            let return_part = code
                .split("return")
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_end_matches(';')
                .trim();

            // 处理简单的字符串操作和变量替换
            let result = self.process_expression(return_part, request)?;

            Ok(serde_json::json!({
                "result": result,
                "input": request.input
            }))
        } else {
            Ok(serde_json::json!({
                "message": "Function executed",
                "code": code,
                "input": request.input
            }))
        }
    }

    /// 处理简单的表达式（字符串连接、变量替换等）
    fn process_expression(&self, expr: &str, request: &InvokeRequest) -> Result<String> {
        let mut result = expr.to_string();

        // 移除多余的引号
        if result.starts_with('"') && result.ends_with('"') {
            result = result[1..result.len() - 1].to_string();
        }

        // 处理字符串连接（简单的 + 操作）
        if result.contains(" + ") {
            let parts: Vec<&str> = result.split(" + ").collect();
            let mut concatenated = String::new();

            for part in parts.iter() {
                let processed_part = self.process_variable_or_literal(part.trim(), request)?;
                concatenated.push_str(&processed_part);
            }

            return Ok(concatenated);
        }

        // 处理单个变量或字面量
        self.process_variable_or_literal(&result, request)
    }

    /// 处理变量或字面量
    fn process_variable_or_literal(&self, part: &str, request: &InvokeRequest) -> Result<String> {
        let trimmed = part.trim();

        // 如果是字符串字面量
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            return Ok(trimmed[1..trimmed.len() - 1].to_string());
        }

        // 如果是单引号字符串
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
            return Ok(trimmed[1..trimmed.len() - 1].to_string());
        }

        // 如果是变量引用，尝试从输入中获取
        if let Some(value) = request.input.get(trimmed) {
            if let Some(str_val) = value.as_str() {
                return Ok(str_val.to_string());
            } else if let Some(num_val) = value.as_f64() {
                return Ok(num_val.to_string());
            } else if let Some(bool_val) = value.as_bool() {
                return Ok(bool_val.to_string());
            } else {
                return Ok(value.to_string());
            }
        }

        // 如果是数字字面量
        if trimmed.parse::<f64>().is_ok() {
            return Ok(trimmed.to_string());
        }

        // 默认作为字符串字面量处理
        Ok(trimmed.to_string())
    }
}

impl Default for SimpleRuntime {
    fn default() -> Self {
        Self::new()
    }
}
