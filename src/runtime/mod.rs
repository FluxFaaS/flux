use crate::functions::{
    ExecutionStatus, FluxError, FunctionMetadata, InvokeRequest, InvokeResponse, Result,
};
use std::time::{Duration, Instant};
use tokio::time::timeout;

pub mod executor;

/// 简单的函数执行器
#[derive(Debug, Clone)]
pub struct SimpleRuntime;

impl SimpleRuntime {
    pub fn new() -> Self {
        Self
    }

    /// 执行函数
    pub async fn execute(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        let start_time = Instant::now();

        tracing::info!("Executing function: {}", function.name);

        // 设置执行超时
        let timeout_duration = Duration::from_millis(function.timeout_ms);

        let result = timeout(timeout_duration, self.execute_function(function, request)).await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                tracing::info!(
                    "Function {} executed successfully in {}ms",
                    function.name,
                    execution_time_ms
                );
                Ok(InvokeResponse {
                    output,
                    execution_time_ms,
                    status: ExecutionStatus::Success,
                })
            }
            Ok(Err(e)) => {
                tracing::error!("Function {} execution failed: {}", function.name, e);
                Ok(InvokeResponse {
                    output: serde_json::json!({"error": e.to_string()}),
                    execution_time_ms,
                    status: ExecutionStatus::Error(e.to_string()),
                })
            }
            Err(_) => {
                tracing::error!("Function {} execution timed out", function.name);
                Ok(InvokeResponse {
                    output: serde_json::json!({"error": "Execution timeout"}),
                    execution_time_ms,
                    status: ExecutionStatus::Timeout,
                })
            }
        }
    }

    /// 实际执行函数代码
    async fn execute_function(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        // MVP 阶段：简单的字符串处理示例
        // 在真实环境中，这里应该是动态编译和执行用户代码

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
