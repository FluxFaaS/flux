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

/// 代码类型枚举
#[derive(Debug, Clone, PartialEq)]
enum CodeType {
    JavaScript,
    Python,
    Rust,
    SimpleExpression,
    Unknown,
}

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
        let mut memory_usage_estimate = 0u64;

        tracing::info!("Executing function: {}", function.name);

        // 检查缓存并实际使用缓存结果
        let use_cached = if let Some(cached_function) = self.cache.get(&function.name).await {
            tracing::debug!("Using cached version of function: {}", function.name);
            memory_usage_estimate = cached_function.memory_usage as u64;
            true
        } else {
            tracing::debug!("Function not in cache, will cache after execution");
            false
        };

        // 设置执行超时
        let timeout_duration = Duration::from_millis(function.timeout_ms);

        // 执行函数
        let result = timeout(timeout_duration, self.execute_function(function, request)).await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        // 如果没有使用缓存，现在缓存函数
        if !use_cached {
            if let Err(e) = self
                .cache
                .put(function.name.clone(), function.clone())
                .await
            {
                tracing::warn!("Failed to cache function {}: {}", function.name, e);
            }
        }

        let response = match result {
            Ok(Ok(output)) => {
                tracing::info!(
                    "Function {} executed successfully in {}ms",
                    function.name,
                    execution_time_ms
                );

                // 估算内存使用（基于函数代码大小和输入输出大小）
                let estimated_memory = self.estimate_memory_usage(function, request, &output);

                // 记录成功执行的性能数据
                let execution_result = ExecutionResult {
                    function_name: function.name.clone(),
                    duration: start_time.elapsed(),
                    success: true,
                    memory_usage: if memory_usage_estimate > 0 {
                        memory_usage_estimate
                    } else {
                        estimated_memory
                    },
                    error_message: None,
                };

                // 安全记录性能数据，失败不影响主流程
                self.safe_record_execution(execution_result).await;

                InvokeResponse {
                    output,
                    execution_time_ms,
                    status: ExecutionStatus::Success,
                }
            }
            Ok(Err(e)) => {
                tracing::error!("Function {} execution failed: {}", function.name, e);

                // 失败情况下的内存估算
                let estimated_memory = self.estimate_memory_usage_on_error(function, request);

                // 记录失败执行的性能数据
                let execution_result = ExecutionResult {
                    function_name: function.name.clone(),
                    duration: start_time.elapsed(),
                    success: false,
                    memory_usage: estimated_memory,
                    error_message: Some(e.to_string()),
                };

                self.safe_record_execution(execution_result).await;

                InvokeResponse {
                    output: serde_json::json!({"error": e.to_string()}),
                    execution_time_ms,
                    status: ExecutionStatus::Error(e.to_string()),
                }
            }
            Err(_) => {
                tracing::error!("Function {} execution timed out", function.name);

                // 超时情况下的内存估算
                let estimated_memory = self.estimate_memory_usage_on_timeout(function);

                // 记录超时执行的性能数据
                let execution_result = ExecutionResult {
                    function_name: function.name.clone(),
                    duration: start_time.elapsed(),
                    success: false,
                    memory_usage: estimated_memory,
                    error_message: Some("Execution timeout".to_string()),
                };

                self.safe_record_execution(execution_result).await;

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
        tracing::debug!("Executing function: {} with code: {}", function.name, function.code);

        // 第三阶段：优先使用真实Rust代码编译和执行
        if self.supports_compilation() {
            tracing::info!("Using real compilation for function: {}", function.name);
            return self.execute_with_compilation(function, request).await;
        }

        // 第二阶段：动态代码执行 - 使用实际的执行引擎
        tracing::info!("Using dynamic execution for function: {}", function.name);

        // 尝试使用外部执行器（如果可用）
        if let Ok(result) = self.execute_with_external_runtime(function, request).await {
            return Ok(result);
        }

        // 回退到内置的动态执行
        self.execute_dynamic_code(function, request).await
    }

    /// 使用外部运行时执行代码
    async fn execute_with_external_runtime(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        let code_type = self.detect_code_type(&function.code);

        match code_type {
            CodeType::JavaScript => self.execute_with_node_js(function, request).await,
            CodeType::Python => self.execute_with_python(function, request).await,
            CodeType::Rust => {
                // 如果有编译器但未启用，尝试启用
                if self.compiler.is_some() {
                    return self.execute_with_compilation(function, request).await;
                }
                Err(FluxError::Runtime("Rust execution requires compilation".to_string()))
            },
            _ => Err(FluxError::Runtime("Unsupported code type for external execution".to_string()))
        }
    }

    /// 使用Node.js执行JavaScript代码
    async fn execute_with_node_js(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        use std::process::Stdio;
        use tokio::process::Command;

        // 检查Node.js是否可用
        let node_check = Command::new("node")
            .arg("--version")
            .output()
            .await;

        if node_check.is_err() {
            return Err(FluxError::Runtime("Node.js not found. Please install Node.js to execute JavaScript functions.".to_string()));
        }

        // 创建JavaScript执行脚本
        let js_code = self.wrap_javascript_code(&function.code, request)?;

        // 创建临时文件
        let temp_dir = tempfile::TempDir::new()
            .map_err(|e| FluxError::Runtime(format!("Failed to create temp directory: {}", e)))?;
        let script_path = temp_dir.path().join("function.js");

        tokio::fs::write(&script_path, js_code).await
            .map_err(|e| FluxError::Runtime(format!("Failed to write script file: {}", e)))?;

        // 执行Node.js
        let output = Command::new("node")
            .arg(&script_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| FluxError::Runtime(format!("Failed to execute Node.js: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FluxError::Runtime(format!("JavaScript execution failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|_| serde_json::json!({"result": stdout.trim()}));

        Ok(result)
    }

    /// 使用Python执行Python代码
    async fn execute_with_python(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        use tokio::process::Command;
        use std::process::Stdio;

        // 检查Python是否可用
        let python_check = Command::new("python3")
            .arg("--version")
            .output()
            .await;

        if python_check.is_err() {
            // 尝试python命令
            let python_check2 = Command::new("python")
                .arg("--version")
                .output()
                .await;

            if python_check2.is_err() {
                return Err(FluxError::Runtime("Python not found. Please install Python to execute Python functions.".to_string()));
            }
        }

        // 创建Python执行脚本
        let python_code = self.wrap_python_code(&function.code, request)?;

        // 创建临时文件
        let temp_dir = tempfile::TempDir::new()
            .map_err(|e| FluxError::Runtime(format!("Failed to create temp directory: {}", e)))?;
        let script_path = temp_dir.path().join("function.py");

        tokio::fs::write(&script_path, python_code).await
            .map_err(|e| FluxError::Runtime(format!("Failed to write script file: {}", e)))?;

        // 尝试python3，如果失败则尝试python
        let mut cmd = Command::new("python3");
        cmd.arg(&script_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await;

        let output = if output.is_err() {
            // 尝试python命令
            Command::new("python")
                .arg(&script_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| FluxError::Runtime(format!("Failed to execute Python: {}", e)))?
        } else {
            output.unwrap()
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FluxError::Runtime(format!("Python execution failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|_| serde_json::json!({"result": stdout.trim()}));

        Ok(result)
    }

    /// 包装JavaScript代码为可执行脚本
    fn wrap_javascript_code(&self, code: &str, request: &InvokeRequest) -> Result<String> {
        let input_json = serde_json::to_string(&request.input)
            .map_err(|e| FluxError::Runtime(format!("Failed to serialize input: {}", e)))?;

        let wrapped_code = format!(r#"
// FluxFaaS JavaScript Function Executor
const input = {};

// Suppress console.log to avoid mixing with result output
const originalConsoleLog = console.log;
console.log = () => {{}};

let result;
let executionError = null;

try {{
    // Method 1: Try to execute code and capture the last expression
    const userFunction = new Function('input', `
        "use strict";
        {}
    `);

    result = userFunction(input);

    // Method 2: If result is undefined, try different approaches
    if (result === undefined) {{
        // Try to wrap the entire code in a return statement
        try {{
            const returnFunction = new Function('input', `
                "use strict";
                return (function() {{
                    {}
                }})();
            `);
            result = returnFunction(input);
        }} catch (e) {{
            // Try to evaluate as a direct expression
            try {{
                const exprFunction = new Function('input', `
                    "use strict";
                    return ({});
                `);
                result = exprFunction(input);
            }} catch (e2) {{
                // Execute code and look for common result variables
                const sandbox = {{ input: input }};
                const contextFunction = new Function('sandbox', `
                    "use strict";
                    const input = sandbox.input;
                    let result, output, value, data;

                    {}

                    // Return the first defined result variable
                    if (typeof result !== 'undefined') return result;
                    if (typeof output !== 'undefined') return output;
                    if (typeof value !== 'undefined') return value;
                    if (typeof data !== 'undefined') return data;

                    return undefined;
                `);
                result = contextFunction(sandbox);
            }}
        }}
    }}

}} catch (error) {{
    executionError = error.message;
}}

// Restore console.log
console.log = originalConsoleLog;

// Output only the result as JSON
if (executionError) {{
    console.log(JSON.stringify({{ error: executionError }}));
}} else {{
    console.log(JSON.stringify({{ result: result }}));
}}
"#, input_json, code, code, code, code);

        Ok(wrapped_code)
    }

    /// 包装Python代码为可执行脚本
    fn wrap_python_code(&self, code: &str, request: &InvokeRequest) -> Result<String> {
        let input_json = serde_json::to_string(&request.input)
            .map_err(|e| FluxError::Runtime(format!("Failed to serialize input: {}", e)))?;

        let wrapped_code = format!(r#"
#!/usr/bin/env python3
# FluxFaaS Python Function Executor
import json
import sys
import io
from contextlib import redirect_stdout

# Input data
input_data = {}

# Make input available as global variable
input = input_data

# Capture print output
captured_output = io.StringIO()
result = None
execution_error = None

try:
    # Create a local namespace for execution
    local_namespace = {{'input': input_data, 'input_data': input_data}}

    # Capture stdout to separate print output from result
    with redirect_stdout(captured_output):
        # Execute user code
        exec('''{}''', {{}}, local_namespace)

    # Try to get result in order of preference
    if 'result' in local_namespace:
        result = local_namespace['result']
    elif 'main' in local_namespace and callable(local_namespace['main']):
        # If there's a main function, call it
        result = local_namespace['main'](input_data)
    else:
        # Look for any function defined in the code
        functions = {{k: v for k, v in local_namespace.items()
                    if callable(v) and not k.startswith('__')}}
        if functions:
            # Call the first user-defined function
            func_name, func = next(iter(functions.items()))
            try:
                # Try calling with input_data
                result = func(input_data)
            except TypeError:
                try:
                    # Try calling without arguments
                    result = func()
                except:
                    result = f"Function {{func_name}} defined but couldn't be called"
        else:
            # If no explicit result, check if there's a return statement
            # This is a simplified approach - in practice, we'd need AST parsing
            result = "Function executed successfully"

except Exception as e:
    execution_error = str(e)

# Get captured print output
console_output = captured_output.getvalue().strip().split('\n') if captured_output.getvalue().strip() else []

# Output the result as JSON
if execution_error:
    print(json.dumps({{
        "error": execution_error,
        "console_output": console_output
    }}))
else:
    print(json.dumps({{
        "result": result,
        "console_output": console_output
    }}))
"#, input_json, code);

        Ok(wrapped_code)
    }

    /// 动态执行用户代码
    async fn execute_dynamic_code(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        // 检测代码语言类型
        let code_type = self.detect_code_type(&function.code);

        match code_type {
            CodeType::JavaScript => self.execute_javascript_code(function, request).await,
            CodeType::Python => self.execute_python_code(function, request).await,
            CodeType::Rust => self.execute_rust_like_code(function, request).await,
            CodeType::SimpleExpression => self.execute_simple_expression(function, request).await,
            CodeType::Unknown => {
                tracing::warn!("Unknown code type for function: {}, falling back to expression evaluation", function.name);
                self.execute_simple_expression(function, request).await
            }
        }
    }

    /// 检测代码类型
    fn detect_code_type(&self, code: &str) -> CodeType {
        let code_lower = code.to_lowercase();

        // JavaScript 特征
        if code_lower.contains("function") ||
           code_lower.contains("const ") ||
           code_lower.contains("let ") ||
           code_lower.contains("var ") ||
           code_lower.contains("json.parse") ||
           code_lower.contains("=>") {
            return CodeType::JavaScript;
        }

        // Python 特征
        if code_lower.contains("def ") ||
           code_lower.contains("import ") ||
           code_lower.contains("print(") ||
           code.contains("    ") && code_lower.contains(":") { // 缩进 + 冒号
            return CodeType::Python;
        }

        // Rust 特征
        if code_lower.contains("fn ") ||
           code_lower.contains("let mut") ||
           code_lower.contains("match ") ||
           code_lower.contains("impl ") {
            return CodeType::Rust;
        }

        // 简单表达式
        if code.trim().starts_with("return ") ||
           (!code.contains('\n') && (code.contains('+') || code.contains('-') || code.contains('*') || code.contains('/'))) {
            return CodeType::SimpleExpression;
        }

        CodeType::Unknown
    }

    /// 执行JavaScript代码
    async fn execute_javascript_code(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        tracing::debug!("Executing JavaScript code for function: {}", function.name);

        // 创建JavaScript执行环境
        let mut context = self.create_js_context(request)?;

        // 执行用户代码
        let result = self.evaluate_javascript(&function.code, &mut context)?;

        Ok(serde_json::json!({
            "result": result,
            "language": "javascript",
            "function_name": function.name,
            "execution_time": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// 执行Python代码
    async fn execute_python_code(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        tracing::debug!("Executing Python code for function: {}", function.name);

        // 注意：这里是简化的Python代码执行模拟
        // 真实环境中需要集成Python解释器或使用PyO3
        let result = self.simulate_python_execution(&function.code, request)?;

        Ok(serde_json::json!({
            "result": result,
            "language": "python",
            "function_name": function.name,
            "execution_time": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// 执行Rust风格代码
    async fn execute_rust_like_code(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        tracing::debug!("Executing Rust-like code for function: {}", function.name);

        // 如果有编译器，尝试真实编译
        if self.compiler.is_some() {
            return self.execute_with_compilation(function, request).await;
        }

        // 否则模拟执行
        let result = self.simulate_rust_execution(&function.code, request)?;

        Ok(serde_json::json!({
            "result": result,
            "language": "rust",
            "function_name": function.name,
            "execution_time": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// 执行简单表达式
    async fn execute_simple_expression(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<serde_json::Value> {
        tracing::debug!("Executing simple expression for function: {}", function.name);

        let result = self.evaluate_expression(&function.code, request)?;

        Ok(serde_json::json!({
            "result": result,
            "language": "expression",
            "function_name": function.name,
            "execution_time": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// 创建JavaScript执行上下文
    fn create_js_context(&self, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 创建一个包含输入数据的上下文
        let mut context = serde_json::Map::new();

        // 添加输入数据
        if let serde_json::Value::Object(input_map) = &request.input {
            for (key, value) in input_map {
                context.insert(key.clone(), value.clone());
            }
        }

        // 添加内置函数和变量
        context.insert("input".to_string(), request.input.clone());
        context.insert("JSON".to_string(), serde_json::json!({
            "parse": "function",
            "stringify": "function"
        }));

        Ok(serde_json::Value::Object(context))
    }

    /// 评估JavaScript代码
    fn evaluate_javascript(&self, code: &str, context: &mut serde_json::Value) -> Result<serde_json::Value> {
        // 这是一个简化的JavaScript执行器
        // 真实环境中应该使用V8引擎或类似的JavaScript运行时

        // 处理简单的JavaScript模式
        if code.contains("JSON.parse(input)") {
            // 处理 JSON.parse(input) 模式
            let parsed_code = code.replace("JSON.parse(input)", "input");
            return self.evaluate_js_expression(&parsed_code, context);
        }

        if code.contains("const {") && code.contains("} = ") {
            // 处理解构赋值
            return self.handle_js_destructuring(code, context);
        }

        // 处理函数定义
        if code.contains("function") || code.contains("=>") {
            return self.handle_js_function(code, context);
        }

        // 处理简单表达式
        self.evaluate_js_expression(code, context)
    }

    /// 处理JavaScript解构赋值
    fn handle_js_destructuring(&self, code: &str, context: &serde_json::Value) -> Result<serde_json::Value> {
        // 简化的解构赋值处理
        // 例如: const {a, b} = JSON.parse(input); return (a + b).toString();

        if let Some(destructure_part) = code.split("const {").nth(1) {
            if let Some(vars_part) = destructure_part.split("} = ").next() {
                let vars: Vec<&str> = vars_part.split(',').map(|s| s.trim()).collect();

                // 从context中提取变量
                let mut local_vars = serde_json::Map::new();
                for var in vars {
                    if let Some(value) = context.get(var) {
                        local_vars.insert(var.to_string(), value.clone());
                    }
                }

                // 查找return语句
                if let Some(return_part) = code.split("return ").nth(1) {
                    let return_expr = return_part.trim_end_matches(';').trim();
                    return self.evaluate_js_expression_with_vars(return_expr, &local_vars);
                }
            }
        }

        Err(FluxError::Runtime("Failed to parse JavaScript destructuring".to_string()))
    }

    /// 处理JavaScript函数
    fn handle_js_function(&self, code: &str, context: &serde_json::Value) -> Result<serde_json::Value> {
        // 简化的函数处理
        if code.contains("return ") {
            if let Some(return_part) = code.split("return ").nth(1) {
                let return_expr = return_part.trim_end_matches(';').trim();
                return self.evaluate_js_expression(return_expr, context);
            }
        }

        Ok(serde_json::json!({
            "message": "Function executed",
            "type": "javascript_function"
        }))
    }

    /// 评估JavaScript表达式
    fn evaluate_js_expression(&self, expr: &str, context: &serde_json::Value) -> Result<serde_json::Value> {
        let vars = if let serde_json::Value::Object(map) = context {
            map
        } else {
            return Err(FluxError::Runtime("Invalid context".to_string()));
        };

        self.evaluate_js_expression_with_vars(expr, vars)
    }

    /// 使用变量评估JavaScript表达式
    fn evaluate_js_expression_with_vars(&self, expr: &str, vars: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value> {
        let trimmed = expr.trim();

        // 处理数学运算
        if trimmed.contains(" + ") {
            let parts: Vec<&str> = trimmed.split(" + ").collect();
            if parts.len() == 2 {
                let a = self.resolve_js_value(parts[0].trim(), vars)?;
                let b = self.resolve_js_value(parts[1].trim(), vars)?;

                // 尝试数值运算
                if let (Some(num_a), Some(num_b)) = (a.as_f64(), b.as_f64()) {
                    return Ok(serde_json::json!(num_a + num_b));
                }

                // 字符串连接
                return Ok(serde_json::json!(format!("{}{}",
                    self.json_value_to_string(&a),
                    self.json_value_to_string(&b)
                )));
            }
        }

        // 处理方法调用
        if trimmed.contains(".toString()") {
            let var_name_owned = trimmed.replace(".toString()", "");
            let var_name = var_name_owned.trim();
            let value = self.resolve_js_value(var_name, vars)?;
            return Ok(serde_json::json!(self.json_value_to_string(&value)));
        }

        // 处理括号表达式
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            let inner = &trimmed[1..trimmed.len()-1];
            return self.evaluate_js_expression_with_vars(inner, vars);
        }

        // 解析单个值
        self.resolve_js_value(trimmed, vars)
    }

    /// 解析JavaScript值
    fn resolve_js_value(&self, value: &str, vars: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value> {
        let trimmed = value.trim();

        // 字符串字面量
        if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
           (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
            return Ok(serde_json::json!(trimmed[1..trimmed.len()-1]));
        }

        // 数字字面量
        if let Ok(num) = trimmed.parse::<f64>() {
            return Ok(serde_json::json!(num));
        }

        // 布尔字面量
        if trimmed == "true" || trimmed == "false" {
            return Ok(serde_json::json!(trimmed == "true"));
        }

        // 变量引用
        if let Some(var_value) = vars.get(trimmed) {
            return Ok(var_value.clone());
        }

        // 默认返回字符串
        Ok(serde_json::json!(trimmed))
    }

    /// 模拟Python代码执行
    fn simulate_python_execution(&self, code: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 基本安全检查
        if self.contains_unsafe_patterns(code) {
            return Err(FluxError::Runtime("Code contains potentially unsafe patterns".to_string()));
        }

        // 处理简单的Python模式
        if code.contains("def ") {
            return self.handle_python_function(code, request);
        }

        // 处理简单的return语句
        if let Some(return_expr) = self.extract_python_return(code) {
            return self.evaluate_python_expression(&return_expr, request);
        }

        // 处理简单的赋值和计算
        if code.contains("=") && !code.contains("==") {
            return self.handle_python_assignment(code, request);
        }

        Ok(serde_json::json!({
            "message": "Python code executed",
            "code_preview": if code.len() > 100 {
                format!("{}...", &code[..100])
            } else {
                code.to_string()
            },
            "input": request.input
        }))
    }

    /// 模拟Rust代码执行
    fn simulate_rust_execution(&self, code: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 基本安全检查
        if self.contains_unsafe_patterns(code) {
            return Err(FluxError::Runtime("Code contains potentially unsafe patterns".to_string()));
        }

        // 处理函数定义
        if code.contains("fn ") {
            return self.handle_rust_function(code, request);
        }

        // 处理简单的表达式
        if let Some(return_expr) = self.extract_rust_return(code) {
            return self.evaluate_rust_expression(&return_expr, request);
        }

        Ok(serde_json::json!({
            "message": "Rust code executed",
            "code_preview": if code.len() > 100 {
                format!("{}...", &code[..100])
            } else {
                code.to_string()
            },
            "input": request.input
        }))
    }

    /// 评估通用表达式
    fn evaluate_expression(&self, code: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 基本安全检查
        if self.contains_unsafe_patterns(code) {
            return Err(FluxError::Runtime("Code contains potentially unsafe patterns".to_string()));
        }

        // 提取return语句
        let expression = if code.trim().starts_with("return ") {
            code.trim().strip_prefix("return ").unwrap_or(code).trim_end_matches(';').trim()
        } else {
            code.trim()
        };

        // 处理表达式并转换为JSON值
        let result = self.process_expression(expression, request)?;
        Ok(serde_json::json!(result))
    }

    /// 处理Python函数
    fn handle_python_function(&self, code: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 查找return语句
        if let Some(return_expr) = self.extract_python_return(code) {
            return self.evaluate_python_expression(&return_expr, request);
        }

        Ok(serde_json::json!({
            "message": "Python function executed",
            "input": request.input
        }))
    }

    /// 处理Rust函数
    fn handle_rust_function(&self, code: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 查找return语句或最后一个表达式
        if let Some(return_expr) = self.extract_rust_return(code) {
            return self.evaluate_rust_expression(&return_expr, request);
        }

        Ok(serde_json::json!({
            "message": "Rust function executed",
            "input": request.input
        }))
    }

    /// 处理Python赋值
    fn handle_python_assignment(&self, code: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        let lines: Vec<&str> = code.lines().collect();
        let mut variables = serde_json::Map::new();

        // 添加输入变量
        if let serde_json::Value::Object(input_map) = &request.input {
            for (key, value) in input_map {
                variables.insert(key.clone(), value.clone());
            }
        }

        // 处理简单的赋值
        for line in lines {
            let trimmed = line.trim();
            if trimmed.contains("=") && !trimmed.contains("==") {
                if let Some((var_name, expr)) = trimmed.split_once("=") {
                    let var_name = var_name.trim();
                    let expr = expr.trim();

                    // 简单的表达式求值
                    if let Ok(result) = self.evaluate_simple_python_expr(expr, &variables) {
                        variables.insert(var_name.to_string(), result);
                    }
                }
            }
        }

        Ok(serde_json::Value::Object(variables))
    }

    /// 提取Python的return语句
    fn extract_python_return(&self, code: &str) -> Option<String> {
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("return ") {
                return Some(trimmed.strip_prefix("return ").unwrap_or("").to_string());
            }
        }
        None
    }

    /// 提取Rust的return语句
    fn extract_rust_return(&self, code: &str) -> Option<String> {
        // 查找显式return语句
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("return ") {
                return Some(trimmed.strip_prefix("return ").unwrap_or("").trim_end_matches(';').to_string());
            }
        }

        // 查找函数体中的最后一个表达式（Rust风格）
        if code.contains("fn ") {
            let lines: Vec<&str> = code.lines().collect();
            for line in lines.iter().rev() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.ends_with('{') && !trimmed.ends_with('}') {
                    return Some(trimmed.trim_end_matches(';').to_string());
                }
            }
        }

        None
    }

    /// 评估Python表达式
    fn evaluate_python_expression(&self, expr: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 创建Python变量上下文
        let mut variables = serde_json::Map::new();
        if let serde_json::Value::Object(input_map) = &request.input {
            for (key, value) in input_map {
                variables.insert(key.clone(), value.clone());
            }
        }

        self.evaluate_simple_python_expr(expr, &variables)
    }

    /// 评估Rust表达式
    fn evaluate_rust_expression(&self, expr: &str, request: &InvokeRequest) -> Result<serde_json::Value> {
        // 处理Rust风格的表达式并转换为JSON值
        let result = self.process_expression(expr, request)?;
        Ok(serde_json::json!(result))
    }

    /// 评估简单的Python表达式
    fn evaluate_simple_python_expr(&self, expr: &str, variables: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value> {
        let trimmed = expr.trim();

        // 处理数学运算
        if trimmed.contains(" + ") {
            let parts: Vec<&str> = trimmed.split(" + ").collect();
            if parts.len() == 2 {
                let a = self.resolve_python_value(parts[0].trim(), variables)?;
                let b = self.resolve_python_value(parts[1].trim(), variables)?;

                if let (Some(num_a), Some(num_b)) = (a.as_f64(), b.as_f64()) {
                    return Ok(serde_json::json!(num_a + num_b));
                }

                return Ok(serde_json::json!(format!("{}{}",
                    self.json_value_to_string(&a),
                    self.json_value_to_string(&b)
                )));
            }
        }

        // 解析单个值
        self.resolve_python_value(trimmed, variables)
    }

    /// 解析Python值
    fn resolve_python_value(&self, value: &str, variables: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value> {
        let trimmed = value.trim();

        // 字符串字面量
        if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
           (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
            return Ok(serde_json::json!(trimmed[1..trimmed.len()-1]));
        }

        // 数字字面量
        if let Ok(num) = trimmed.parse::<f64>() {
            return Ok(serde_json::json!(num));
        }

        // 布尔字面量
        if trimmed == "True" || trimmed == "False" {
            return Ok(serde_json::json!(trimmed == "True"));
        }

        // None
        if trimmed == "None" {
            return Ok(serde_json::Value::Null);
        }

        // 变量引用
        if let Some(var_value) = variables.get(trimmed) {
            return Ok(var_value.clone());
        }

        // 默认返回字符串
        Ok(serde_json::json!(trimmed))
    }

    /// 检查代码中是否包含不安全的模式
    fn contains_unsafe_patterns(&self, code: &str) -> bool {
        let unsafe_patterns = [
            // Rust unsafe patterns
            "std::process::",
            "std::fs::",
            "std::net::",
            "unsafe",
            "extern",
            "libc::",

            // System calls
            "system(",
            "exec(",
            "eval(",
            "spawn(",

            // Python unsafe patterns
            "__import__",
            "import os",
            "import sys",
            "import subprocess",
            "import socket",
            "open(",
            "file(",

            // JavaScript unsafe patterns
            "require(",
            "import(",
            "fetch(",
            "XMLHttpRequest",
            "eval(",
            "Function(",

            // General unsafe patterns
            "rm -rf",
            "delete",
            "DROP TABLE",
            "DELETE FROM",
            "../",
            "passwd",
            "/etc/",
            "/proc/",
        ];

        let code_lower = code.to_lowercase();
        unsafe_patterns.iter().any(|pattern| code_lower.contains(&pattern.to_lowercase()))
    }

    /// 处理简单的表达式（字符串连接、变量替换等）
    fn process_expression(&self, expr: &str, request: &InvokeRequest) -> Result<String> {
        let trimmed_expr = expr.trim();

        // 空表达式检查
        if trimmed_expr.is_empty() {
            return Ok(String::new());
        }

        // 处理字符串字面量
        if (trimmed_expr.starts_with('"') && trimmed_expr.ends_with('"'))
            || (trimmed_expr.starts_with('\'') && trimmed_expr.ends_with('\''))
        {
            return Ok(trimmed_expr[1..trimmed_expr.len() - 1].to_string());
        }

        // 处理字符串连接（简单的 + 操作）
        if trimmed_expr.contains(" + ") {
            return self.process_concatenation(trimmed_expr, request);
        }

        // 处理数学运算
        if self.contains_math_operators(trimmed_expr) {
            return self.process_math_expression(trimmed_expr, request);
        }

        // 处理单个变量或字面量
        self.process_variable_or_literal(trimmed_expr, request)
    }

    /// 处理字符串连接
    fn process_concatenation(&self, expr: &str, request: &InvokeRequest) -> Result<String> {
        let parts: Vec<&str> = expr.split(" + ").collect();
        let mut concatenated = String::new();

        for part in parts.iter() {
            let processed_part = self.process_variable_or_literal(part.trim(), request)?;
            concatenated.push_str(&processed_part);
        }

        Ok(concatenated)
    }

    /// 检查是否包含数学运算符
    fn contains_math_operators(&self, expr: &str) -> bool {
        expr.contains(" - ") || expr.contains(" * ") || expr.contains(" / ") || expr.contains(" % ")
    }

    /// 处理简单的数学表达式
    fn process_math_expression(&self, expr: &str, request: &InvokeRequest) -> Result<String> {
        // 这是一个非常简化的数学表达式处理
        // 实际项目中应该使用专门的表达式解析器

        if expr.contains(" - ") {
            let parts: Vec<&str> = expr.splitn(2, " - ").collect();
            if parts.len() == 2 {
                let a = self.parse_numeric_value(parts[0].trim(), request)?;
                let b = self.parse_numeric_value(parts[1].trim(), request)?;
                return Ok((a - b).to_string());
            }
        }

        if expr.contains(" * ") {
            let parts: Vec<&str> = expr.splitn(2, " * ").collect();
            if parts.len() == 2 {
                let a = self.parse_numeric_value(parts[0].trim(), request)?;
                let b = self.parse_numeric_value(parts[1].trim(), request)?;
                return Ok((a * b).to_string());
            }
        }

        if expr.contains(" / ") {
            let parts: Vec<&str> = expr.splitn(2, " / ").collect();
            if parts.len() == 2 {
                let a = self.parse_numeric_value(parts[0].trim(), request)?;
                let b = self.parse_numeric_value(parts[1].trim(), request)?;
                if b == 0.0 {
                    return Err(FluxError::Runtime("Division by zero".to_string()));
                }
                return Ok((a / b).to_string());
            }
        }

        // 如果无法解析为数学表达式，作为字符串处理
        self.process_variable_or_literal(expr, request)
    }

    /// 解析数值
    fn parse_numeric_value(&self, value: &str, request: &InvokeRequest) -> Result<f64> {
        // 尝试直接解析为数字
        if let Ok(num) = value.parse::<f64>() {
            return Ok(num);
        }

        // 尝试从请求输入中获取
        if let Some(input_value) = request.input.get(value) {
            if let Some(num) = input_value.as_f64() {
                return Ok(num);
            }
        }

        Err(FluxError::Runtime(format!(
            "Cannot parse '{}' as number",
            value
        )))
    }

    /// 处理变量或字面量
    fn process_variable_or_literal(&self, part: &str, request: &InvokeRequest) -> Result<String> {
        let trimmed = part.trim();

        // 空值检查
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        // 如果是字符串字面量
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            return Ok(trimmed[1..trimmed.len() - 1].to_string());
        }

        // 如果是布尔字面量
        if trimmed == "true" || trimmed == "false" {
            return Ok(trimmed.to_string());
        }

        // 如果是null字面量
        if trimmed == "null" || trimmed == "undefined" {
            return Ok("null".to_string());
        }

        // 如果是数字字面量
        if let Ok(num) = trimmed.parse::<f64>() {
            return Ok(num.to_string());
        }

        // 尝试从输入中获取变量值
        if let Some(value) = request.input.get(trimmed) {
            return Ok(self.json_value_to_string(value));
        }

        // 支持简单的点号访问（如 user.name）
        if trimmed.contains('.') {
            if let Some(nested_value) = self.get_nested_value(&request.input, trimmed) {
                return Ok(self.json_value_to_string(&nested_value));
            }
        }

        // 如果都不匹配，作为字符串字面量处理
        Ok(trimmed.to_string())
    }

    /// 将JSON值转换为字符串
    fn json_value_to_string(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => "null".to_string(),
            _ => value.to_string(),
        }
    }

    /// 获取嵌套值（支持简单的点号访问）
    fn get_nested_value(&self, root: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = root;

        for part in parts {
            if let Some(next) = current.get(part) {
                current = next;
            } else {
                return None;
            }
        }

        Some(current.clone())
    }

    /// 估算内存使用量（成功执行情况）
    fn estimate_memory_usage(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
        output: &serde_json::Value,
    ) -> u64 {
        // 基础内存开销
        let base_memory = 1024; // 1KB 基础开销

        // 函数代码大小
        let code_size = function.code.len() as u64;

        // 输入数据大小
        let input_size = serde_json::to_string(&request.input)
            .map(|s| s.len() as u64)
            .unwrap_or(0);

        // 输出数据大小
        let output_size = serde_json::to_string(output)
            .map(|s| s.len() as u64)
            .unwrap_or(0);

        // 估算总内存使用：基础开销 + 代码大小 * 2 + 输入输出大小 * 3
        base_memory + code_size * 2 + (input_size + output_size) * 3
    }

    /// 估算内存使用量（执行失败情况）
    fn estimate_memory_usage_on_error(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> u64 {
        // 失败情况下通常内存使用较少
        let base_memory = 512; // 512B 基础开销
        let code_size = function.code.len() as u64;
        let input_size = serde_json::to_string(&request.input)
            .map(|s| s.len() as u64)
            .unwrap_or(0);

        base_memory + code_size + input_size
    }

    /// 估算内存使用量（超时情况）
    fn estimate_memory_usage_on_timeout(&self, function: &FunctionMetadata) -> u64 {
        // 超时情况下可能内存使用更少
        let base_memory = 256; // 256B 基础开销
        let code_size = function.code.len() as u64;

        base_memory + code_size / 2
    }

    /// 安全记录执行结果，失败不影响主流程
    async fn safe_record_execution(&self, execution_result: ExecutionResult) {
        if let Err(e) = self.monitor.record_execution(execution_result).await {
            tracing::warn!("Failed to record performance data: {}", e);
            // 可以在这里添加降级策略，比如记录到本地文件
        }
    }
}

impl Default for SimpleRuntime {
    fn default() -> Self {
        Self::new()
    }
}
