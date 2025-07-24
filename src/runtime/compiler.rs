use anyhow::{Context, Result};
use libloading::{Library, Symbol};

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

use crate::functions::{ExecutionStatus, FunctionMetadata, InvokeRequest, InvokeResponse};

/// 编译后的函数信息
#[derive(Debug, Clone)]
pub struct CompiledFunction {
    /// 函数元数据
    pub metadata: FunctionMetadata,
    /// 动态库路径
    pub library_path: PathBuf,
    /// 编译时间戳
    pub compiled_at: chrono::DateTime<chrono::Utc>,
    /// 源代码哈希值（用于缓存失效）
    pub source_hash: String,
    /// 编译所用时间（毫秒）
    pub compile_time_ms: u64,
}

/// 编译器配置
#[derive(Debug, Clone)]
pub struct CompilerConfig {
    /// 编译器路径（默认使用系统rustc）
    pub rustc_path: Option<PathBuf>,
    /// 优化级别（0-3）
    pub opt_level: u8,
    /// 是否启用调试信息
    pub debug: bool,
    /// 编译超时时间（秒）
    pub compile_timeout_secs: u64,
    /// 缓存目录
    pub cache_dir: PathBuf,
    /// 最大缓存数量
    pub max_cache_entries: usize,
    /// 自定义Rust编译目标路径
    pub rust_target_dir: Option<PathBuf>,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            rustc_path: None,
            opt_level: 2,
            debug: false,
            compile_timeout_secs: 30,
            cache_dir: PathBuf::from("./flux_cache"),
            max_cache_entries: 100,
            rust_target_dir: None,
        }
    }
}

/// Rust代码编译器
#[derive(Debug)]
pub struct RustCompiler {
    config: CompilerConfig,
    compiled_functions: Arc<RwLock<HashMap<String, CompiledFunction>>>,
    temp_dirs: Arc<RwLock<Vec<TempDir>>>, // 保持临时目录引用，防止被清理
}

impl RustCompiler {
    /// 创建新的编译器实例
    pub fn new(config: CompilerConfig) -> Result<Self> {
        // 确保缓存目录存在
        fs::create_dir_all(&config.cache_dir)
            .with_context(|| format!("Failed to create cache directory: {:?}", config.cache_dir))?;

        Ok(Self {
            config,
            compiled_functions: Arc::new(RwLock::new(HashMap::new())),
            temp_dirs: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// 检查rustc是否可用
    pub fn check_rustc(&self) -> Result<PathBuf> {
        let rustc_path = if let Some(path) = &self.config.rustc_path {
            path.clone()
        } else {
            // 尝试使用which查找rustc
            which::which("rustc").context(
                "rustc not found in PATH. Please install Rust or specify rustc_path in config",
            )?
        };

        // 验证rustc是否可执行
        let output = Command::new(&rustc_path)
            .arg("--version")
            .output()
            .context("Failed to execute rustc --version")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("rustc is not working properly"));
        }

        tracing::info!(
            "Using rustc: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        );
        Ok(rustc_path)
    }

    /// 编译函数代码
    pub async fn compile_function(&self, function: &FunctionMetadata) -> Result<CompiledFunction> {
        let start_time = std::time::Instant::now();

        // 计算源代码哈希
        let source_hash = format!("{:x}", md5::compute(&function.code));

        // 检查缓存
        if let Some(cached) = self.get_cached_function(&function.name, &source_hash).await {
            tracing::debug!("Using cached compilation for function: {}", function.name);
            return Ok(cached);
        }

        tracing::info!("Compiling function: {}", function.name);

        // 验证rustc可用性
        let rustc_path = self.check_rustc()?;

        // 创建临时工作目录
        let temp_dir = TempDir::new().context("Failed to create temporary directory")?;
        let work_dir = temp_dir.path();

        // 生成Rust源文件
        let source_file = self.generate_source_file(function, work_dir)?;

        // 生成Cargo.toml
        let _cargo_toml = self.generate_cargo_toml(function, work_dir)?;

        // 编译为动态库
        let library_path = self
            .compile_to_dylib(&rustc_path, &source_file, work_dir)
            .await?;

        // 复制到缓存目录
        let cached_library_path =
            self.cache_library(&function.name, &source_hash, &library_path)?;

        let compile_time_ms = start_time.elapsed().as_millis() as u64;

        let compiled_function = CompiledFunction {
            metadata: function.clone(),
            library_path: cached_library_path,
            compiled_at: chrono::Utc::now(),
            source_hash,
            compile_time_ms,
        };

        // 缓存编译结果
        self.cache_compiled_function(&function.name, compiled_function.clone())
            .await;

        // 保持临时目录引用
        {
            let mut temp_dirs = self.temp_dirs.write().await;
            temp_dirs.push(temp_dir);

            // 限制临时目录数量
            if temp_dirs.len() > 10 {
                temp_dirs.remove(0);
            }
        }

        tracing::info!(
            "Successfully compiled function '{}' in {}ms",
            function.name,
            compile_time_ms
        );

        Ok(compiled_function)
    }

    /// 生成Rust源文件
    fn generate_source_file(
        &self,
        function: &FunctionMetadata,
        work_dir: &Path,
    ) -> Result<PathBuf> {
        let source_content = self.wrap_user_code(&function.code)?;

        // 创建src目录
        let src_dir = work_dir.join("src");
        fs::create_dir_all(&src_dir)
            .with_context(|| format!("Failed to create src directory: {src_dir:?}"))?;

        let source_file = src_dir.join("lib.rs");

        fs::write(&source_file, source_content)
            .with_context(|| format!("Failed to write source file: {source_file:?}"))?;

        Ok(source_file)
    }

    /// 包装用户代码为标准的动态库格式
    fn wrap_user_code(&self, user_code: &str) -> Result<String> {
        // 基础的函数包装模板
        let wrapped_code = format!(
            r#"
use std::ffi::{{CStr, CString}};
use std::os::raw::c_char;

// 用户代码
{user_code}

// 导出函数接口
#[no_mangle]
pub extern "C" fn flux_execute(input_ptr: *const c_char) -> *mut c_char {{
    if input_ptr.is_null() {{
        return std::ptr::null_mut();
    }}

    let input_cstr = unsafe {{ CStr::from_ptr(input_ptr) }};
    let input_str = match input_cstr.to_str() {{
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    }};

    // 解析JSON输入
    let input_json: serde_json::Value = match serde_json::from_str(input_str) {{
        Ok(json) => json,
        Err(_) => serde_json::Value::Null,
    }};

    // 调用用户函数（这里需要根据具体的用户代码格式来适配）
    let result = execute_user_function(input_json);

    // 序列化结果
    let result_str = match serde_json::to_string(&result) {{
        Ok(s) => s,
        Err(_) => "{{\"error\": \"Failed to serialize result\"}}".to_string(),
    }};

    // 返回C字符串
    match CString::new(result_str) {{
        Ok(cstring) => cstring.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }}
}}

// 释放内存的函数
#[no_mangle]
pub extern "C" fn flux_free_string(ptr: *mut c_char) {{
    if !ptr.is_null() {{
        unsafe {{
            let _ = CString::from_raw(ptr);
        }}
    }}
}}

// 用户函数执行入口
fn execute_user_function(input: serde_json::Value) -> serde_json::Value {{
    // 简化版本：直接执行字符串形式的代码
    // 实际实现中需要更复杂的代码解析和执行逻辑
    match input {{
        serde_json::Value::Object(ref map) => {{
            // 示例：如果是加法操作
            if let (Some(a), Some(b)) = (map.get("a"), map.get("b")) {{
                if let (Some(a_num), Some(b_num)) = (a.as_f64(), b.as_f64()) {{
                    return serde_json::json!({{ "result": a_num + b_num }});
                }}
            }}

            // 默认行为：返回处理过的输入
            serde_json::json!({{
                "message": "Function executed",
                "input": input,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }})
        }},
        _ => {{
            serde_json::json!({{
                "message": "Hello from compiled function",
                "input": input,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }})
        }}
    }}
}}
"#
        );

        Ok(wrapped_code)
    }

    /// 生成Cargo.toml文件
    fn generate_cargo_toml(&self, function: &FunctionMetadata, work_dir: &Path) -> Result<PathBuf> {
        let clean_name = function.name.replace(['-', ' '], "_");
        let cargo_content = format!(
            r#"[package]
name = "flux_function_{clean_name}"
version = "0.1.0"
edition = "2021"

[lib]
name = "flux_function_{clean_name}"
crate-type = ["cdylib"]

[dependencies]
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
chrono = {{ version = "0.4", features = ["serde"] }}
"#
        );

        let cargo_file = work_dir.join("Cargo.toml");
        fs::write(&cargo_file, cargo_content)
            .with_context(|| format!("Failed to write Cargo.toml: {cargo_file:?}"))?;

        Ok(cargo_file)
    }

    /// 编译为动态库
    async fn compile_to_dylib(
        &self,
        _rustc_path: &Path,
        _source_file: &Path,
        work_dir: &Path,
    ) -> Result<PathBuf> {
        let _output_name = if cfg!(target_os = "windows") {
            "flux_function.dll"
        } else if cfg!(target_os = "macos") {
            "libflux_function.dylib"
        } else {
            "libflux_function.so"
        };

        let _output_path = work_dir.join(_output_name);

        // 使用cargo build而不是直接rustc，以便处理依赖
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--release")
            .current_dir(work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // 配置目标目录
        if let Some(ref custom_target) = self.config.rust_target_dir {
            let expanded_path = shellexpand::tilde(&custom_target.to_string_lossy()).to_string();
            cmd.env("CARGO_TARGET_DIR", expanded_path);
        } else {
            cmd.arg("--target-dir").arg(work_dir.join("target"));
        }

        tracing::debug!("Executing: {:?}", cmd);

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.compile_timeout_secs),
            tokio::task::spawn_blocking(move || cmd.output()),
        )
        .await
        .context("Compilation timeout")?
        .context("Failed to spawn compilation process")?
        .context("Compilation process failed")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow::anyhow!(
                "Compilation failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
                stdout,
                stderr
            ));
        }

        // 查找编译后的动态库
        let target_dir = if let Some(ref custom_target) = self.config.rust_target_dir {
            let expanded_path = shellexpand::tilde(&custom_target.to_string_lossy()).to_string();
            PathBuf::from(expanded_path).join("release")
        } else {
            work_dir.join("target").join("release")
        };

        // 寻找实际生成的库文件
        for entry in fs::read_dir(&target_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "so" || ext == "dylib" || ext == "dll" {
                        return Ok(path);
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Compiled library not found in target directory"
        ))
    }

    /// 将库文件复制到缓存目录
    fn cache_library(
        &self,
        function_name: &str,
        source_hash: &str,
        library_path: &Path,
    ) -> Result<PathBuf> {
        let cache_filename = format!(
            "{}_{}.{}",
            function_name,
            source_hash,
            library_path
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or("so")
        );
        let cached_path = self.config.cache_dir.join(cache_filename);

        fs::copy(library_path, &cached_path)
            .with_context(|| format!("Failed to cache library to: {cached_path:?}"))?;

        Ok(cached_path)
    }

    /// 获取缓存的编译结果
    async fn get_cached_function(
        &self,
        function_name: &str,
        source_hash: &str,
    ) -> Option<CompiledFunction> {
        let compiled_functions = self.compiled_functions.read().await;
        if let Some(compiled) = compiled_functions.get(function_name) {
            if compiled.source_hash == source_hash && compiled.library_path.exists() {
                return Some(compiled.clone());
            }
        }

        // 检查磁盘缓存
        let cache_filename = format!("{function_name}_{source_hash}.so");
        let cached_path = self.config.cache_dir.join(cache_filename);

        if cached_path.exists() {
            // TODO: 从缓存重建CompiledFunction
            None
        } else {
            None
        }
    }

    /// 缓存编译结果
    async fn cache_compiled_function(&self, function_name: &str, compiled: CompiledFunction) {
        let mut compiled_functions = self.compiled_functions.write().await;
        compiled_functions.insert(function_name.to_string(), compiled);

        // 限制缓存大小
        if compiled_functions.len() > self.config.max_cache_entries {
            // 移除最旧的条目（简化版本）
            if let Some(oldest_key) = compiled_functions.keys().next().cloned() {
                compiled_functions.remove(&oldest_key);
            }
        }
    }

    /// 执行编译后的函数
    pub async fn execute_compiled_function(
        &self,
        compiled: &CompiledFunction,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        let start_time = std::time::Instant::now();

        // 加载动态库
        let library = unsafe {
            Library::new(&compiled.library_path)
                .with_context(|| format!("Failed to load library: {:?}", compiled.library_path))?
        };

        // 获取函数符号
        let flux_execute: Symbol<
            unsafe extern "C" fn(*const std::os::raw::c_char) -> *mut std::os::raw::c_char,
        > = unsafe {
            library
                .get(b"flux_execute")
                .context("Function 'flux_execute' not found in library")?
        };

        let flux_free: Symbol<unsafe extern "C" fn(*mut std::os::raw::c_char)> = unsafe {
            library
                .get(b"flux_free_string")
                .context("Function 'flux_free_string' not found in library")?
        };

        // 准备输入
        let input_json =
            serde_json::to_string(&request.input).context("Failed to serialize input")?;
        let input_cstring =
            std::ffi::CString::new(input_json).context("Failed to create input C string")?;

        // 调用函数
        let result_ptr = unsafe { flux_execute(input_cstring.as_ptr()) };

        if result_ptr.is_null() {
            return Ok(InvokeResponse {
                output: serde_json::json!({"error": "Function returned null"}),
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                status: ExecutionStatus::Error("Function execution failed".to_string()),
            });
        }

        // 获取结果
        let result_cstr = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
        let result_str = result_cstr
            .to_str()
            .context("Failed to convert result to string")?;

        let output: serde_json::Value = serde_json::from_str(result_str)
            .unwrap_or_else(|_| serde_json::json!({"result": result_str}));

        // 释放内存
        unsafe { flux_free(result_ptr) };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(InvokeResponse {
            output,
            execution_time_ms,
            status: ExecutionStatus::Success,
        })
    }

    /// 获取编译统计信息
    pub async fn get_stats(&self) -> HashMap<String, serde_json::Value> {
        let compiled_functions = self.compiled_functions.read().await;
        let mut stats = HashMap::new();

        stats.insert(
            "compiled_functions_count".to_string(),
            serde_json::Value::Number(compiled_functions.len().into()),
        );

        let total_compile_time: u64 = compiled_functions.values().map(|f| f.compile_time_ms).sum();
        stats.insert(
            "total_compile_time_ms".to_string(),
            serde_json::Value::Number(total_compile_time.into()),
        );

        let avg_compile_time = if compiled_functions.is_empty() {
            0
        } else {
            total_compile_time / compiled_functions.len() as u64
        };
        stats.insert(
            "avg_compile_time_ms".to_string(),
            serde_json::Value::Number(avg_compile_time.into()),
        );

        stats
    }

    /// 清理缓存
    pub async fn clear_cache(&self) -> Result<()> {
        // 清理内存缓存
        {
            let mut compiled_functions = self.compiled_functions.write().await;
            compiled_functions.clear();
        }

        // 清理临时目录
        {
            let mut temp_dirs = self.temp_dirs.write().await;
            temp_dirs.clear();
        }

        // 清理磁盘缓存（可选）
        if self.config.cache_dir.exists() {
            for entry in fs::read_dir(&self.config.cache_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "so" || ext == "dylib" || ext == "dll" {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
            }
        }

        tracing::info!("Compiler cache cleared");
        Ok(())
    }
}

/// 检查系统是否支持编译
pub fn check_compilation_support() -> Result<()> {
    // 检查rustc
    which::which("rustc").context("rustc not found. Please install Rust toolchain")?;

    // 检查cargo
    which::which("cargo").context("cargo not found. Please install Rust toolchain")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_config_default() {
        let config = CompilerConfig::default();
        assert_eq!(config.opt_level, 2);
        assert!(!config.debug);
        assert_eq!(config.compile_timeout_secs, 30);
    }

    #[tokio::test]
    async fn test_wrap_user_code() {
        let config = CompilerConfig::default();
        let compiler = RustCompiler::new(config).unwrap();

        let user_code = "fn test() -> i32 { 42 }";
        let wrapped = compiler.wrap_user_code(user_code).unwrap();

        assert!(wrapped.contains(user_code));
        assert!(wrapped.contains("flux_execute"));
        assert!(wrapped.contains("flux_free_string"));
    }

    #[test]
    fn test_check_compilation_support() {
        // 这个测试需要系统安装了Rust工具链
        match check_compilation_support() {
            Ok(()) => println!("Compilation support available"),
            Err(e) => println!("Compilation support not available: {e}"),
        }
    }
}

