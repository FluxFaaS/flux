use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::process::{Child, Command as TokioCommand};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;

use crate::functions::{ExecutionStatus, InvokeRequest};
use crate::runtime::compiler::CompiledFunction;

/// 沙箱配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// 是否启用进程隔离
    pub enable_process_isolation: bool,
    /// 是否启用容器隔离
    pub enable_container_isolation: bool,
    /// 执行超时时间（秒）
    pub execution_timeout_secs: u64,
    /// 最大内存使用（MB）
    pub max_memory_mb: u64,
    /// 最大CPU使用率（百分比）
    pub max_cpu_percent: f64,
    /// 是否允许网络访问
    pub allow_network: bool,
    /// 是否允许文件系统访问
    pub allow_filesystem: bool,
    /// 允许访问的目录列表
    pub allowed_dirs: Vec<PathBuf>,
    /// 工作目录
    pub work_dir: Option<PathBuf>,
    /// 环境变量限制
    pub allowed_env_vars: Vec<String>,
    /// 临时目录根路径
    pub temp_root: PathBuf,
    /// 自定义Rust编译目标路径
    pub rust_target_dir: Option<PathBuf>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enable_process_isolation: true,
            enable_container_isolation: false,
            execution_timeout_secs: 30,
            max_memory_mb: 128,
            max_cpu_percent: 50.0,
            allow_network: false,
            allow_filesystem: false,
            allowed_dirs: vec![],
            work_dir: None,
            allowed_env_vars: vec!["PATH".to_string()],
            temp_root: PathBuf::from("/tmp/flux_sandbox"),
            rust_target_dir: None,
        }
    }
}

/// 沙箱执行结果
#[derive(Debug, Clone)]
pub struct SandboxResult {
    /// 执行状态
    pub status: ExecutionStatus,
    /// 函数输出
    pub output: serde_json::Value,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// 内存使用峰值（字节）
    pub peak_memory_bytes: u64,
    /// CPU使用率（百分比）
    pub cpu_usage_percent: f64,
    /// 退出码
    pub exit_code: Option<i32>,
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
}

/// 进程监控信息
#[derive(Debug, Clone)]
pub struct ProcessMonitor {
    /// 进程ID
    pub pid: u32,
    /// 开始时间
    pub start_time: Instant,
    /// 内存使用统计
    pub memory_usage: u64,
    /// CPU使用统计
    pub cpu_usage: f64,
    /// 是否仍在运行
    pub is_running: bool,
}

/// 沙箱隔离执行器
#[derive(Debug)]
pub struct SandboxExecutor {
    /// 配置
    config: SandboxConfig,
    /// 活跃进程监控
    active_processes: Arc<RwLock<HashMap<u32, ProcessMonitor>>>,
    /// 系统信息监控
    system_monitor: Arc<Mutex<sysinfo::System>>,
    /// 临时目录管理
    temp_dirs: Arc<RwLock<Vec<TempDir>>>,
}

impl SandboxExecutor {
    /// 创建新的沙箱执行器
    pub fn new(config: SandboxConfig) -> Result<Self> {
        // 创建临时目录根路径
        std::fs::create_dir_all(&config.temp_root)
            .with_context(|| format!("Failed to create temp root: {:?}", config.temp_root))?;

        let mut system = sysinfo::System::new_all();
        system.refresh_all();

        Ok(Self {
            config,
            active_processes: Arc::new(RwLock::new(HashMap::new())),
            system_monitor: Arc::new(Mutex::new(system)),
            temp_dirs: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// 在沙箱中执行编译后的函数
    pub async fn execute_in_sandbox(
        &self,
        compiled: &CompiledFunction,
        request: &InvokeRequest,
    ) -> Result<SandboxResult> {
        let start_time = Instant::now();

        if self.config.enable_container_isolation {
            // 容器化执行
            self.execute_in_container(compiled, request, start_time)
                .await
        } else if self.config.enable_process_isolation {
            // 进程隔离执行
            self.execute_in_process(compiled, request, start_time).await
        } else {
            return Err(anyhow::anyhow!("No isolation method enabled"));
        }
    }

    /// 在独立进程中执行函数
    async fn execute_in_process(
        &self,
        compiled: &CompiledFunction,
        request: &InvokeRequest,
        start_time: Instant,
    ) -> Result<SandboxResult> {
        // 创建安全的临时工作目录
        let temp_dir = self.create_secure_temp_dir().await?;
        let work_dir = temp_dir.path();

        // 复制动态库到安全目录
        let lib_name = compiled
            .library_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid library path"))?;
        let secure_lib_path = work_dir.join(lib_name);

        tokio::fs::copy(&compiled.library_path, &secure_lib_path)
            .await
            .context("Failed to copy library to secure directory")?;

        // 创建执行器可执行文件
        let executor_path = self
            .create_function_executor(&secure_lib_path, work_dir)
            .await?;

        // 准备输入数据
        let input_json =
            serde_json::to_string(&request.input).context("Failed to serialize input")?;

        // 构建安全的执行命令
        let mut cmd = TokioCommand::new(&executor_path);
        cmd.arg(&input_json)
            .current_dir(work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // 设置安全的环境变量
        cmd.env_clear();
        for env_var in &self.config.allowed_env_vars {
            if let Ok(value) = std::env::var(env_var) {
                cmd.env(env_var, value);
            }
        }

        // 设置工作目录权限限制
        self.set_directory_permissions(work_dir).await?;

        tracing::info!(
            "Starting sandboxed process for function: {}",
            compiled.metadata.name
        );

        // 启动进程
        let child = cmd.spawn().context("Failed to spawn sandboxed process")?;

        let pid = child.id().unwrap_or(0);

        // 注册进程监控
        self.register_process_monitor(pid).await;

        // 等待执行完成（带超时）
        let timeout_duration = Duration::from_secs(self.config.execution_timeout_secs);
        let execution_result =
            timeout(timeout_duration, self.monitor_process_execution(child, pid)).await;

        // 清理进程监控
        self.unregister_process_monitor(pid).await;

        // 保持临时目录引用
        {
            let mut temp_dirs = self.temp_dirs.write().await;
            temp_dirs.push(temp_dir);
            if temp_dirs.len() > 20 {
                temp_dirs.remove(0);
            }
        }

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match execution_result {
            Ok(result) => result,
            Err(_) => {
                // 超时，强制终止进程
                self.kill_process(pid).await?;
                Ok(SandboxResult {
                    status: ExecutionStatus::Failed,
                    output: serde_json::json!({"error": "Execution timeout"}),
                    execution_time_ms,
                    peak_memory_bytes: 0,
                    cpu_usage_percent: 0.0,
                    exit_code: Some(-1),
                    stdout: String::new(),
                    stderr: "Execution timeout".to_string(),
                })
            }
        }
    }

    /// 在容器中执行函数（Docker支持）
    async fn execute_in_container(
        &self,
        compiled: &CompiledFunction,
        request: &InvokeRequest,
        start_time: Instant,
    ) -> Result<SandboxResult> {
        // TODO: 实现Docker容器执行
        // 这需要集成bollard crate或直接调用docker命令

        tracing::warn!(
            "Container execution not implemented yet, falling back to process isolation"
        );
        self.execute_in_process(compiled, request, start_time).await
    }

    /// 创建安全的临时目录
    async fn create_secure_temp_dir(&self) -> Result<TempDir> {
        let temp_dir = TempDir::new_in(&self.config.temp_root)
            .context("Failed to create secure temporary directory")?;

        // 设置严格的权限（仅所有者可读写执行）
        #[cfg(unix)]
        {
            let permissions = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(temp_dir.path(), permissions)
                .context("Failed to set directory permissions")?;
        }

        Ok(temp_dir)
    }

    /// 创建函数执行器可执行文件
    async fn create_function_executor(
        &self,
        library_path: &Path,
        work_dir: &Path,
    ) -> Result<PathBuf> {
        // 创建一个简单的执行器程序
        let executor_source = self.generate_executor_source(library_path)?;

        // 创建src目录
        let src_dir = work_dir.join("src");
        tokio::fs::create_dir_all(&src_dir)
            .await
            .context("Failed to create src directory")?;

        let executor_src_path = src_dir.join("main.rs");

        tokio::fs::write(&executor_src_path, executor_source)
            .await
            .context("Failed to write executor source")?;

        // 创建Cargo.toml为执行器
        let executor_cargo_toml = work_dir.join("Cargo.toml");
        let cargo_content = r#"[package]
name = "executor"
version = "0.1.0"
edition = "2021"

[dependencies]
libloading = "0.8"
"#;
        tokio::fs::write(&executor_cargo_toml, cargo_content)
            .await
            .context("Failed to write executor Cargo.toml")?;

        // 使用cargo构建执行器
        let output = tokio::task::spawn_blocking({
            let work_dir = work_dir.to_path_buf();
            let rust_target_dir = self.config.rust_target_dir.clone();
            move || {
                let mut command = Command::new("cargo");
                command.arg("build").current_dir(&work_dir);

                // 如果配置了自定义编译路径，设置环境变量
                if let Some(target_dir) = rust_target_dir {
                    let expanded_path =
                        shellexpand::tilde(&target_dir.to_string_lossy()).to_string();
                    command.env("CARGO_TARGET_DIR", expanded_path);
                }

                command.output()
            }
        })
        .await
        .context("Failed to spawn cargo build task")?
        .context("Failed to execute cargo build")?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        tracing::debug!(
            "Cargo build output:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout,
            stderr
        );

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Executor compilation failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
                stdout,
                stderr
            ));
        }

        // 找到构建后的可执行文件
        let target_dir = if let Some(ref custom_target) = self.config.rust_target_dir {
            // 使用自定义编译路径
            let expanded_path = shellexpand::tilde(&custom_target.to_string_lossy()).to_string();
            PathBuf::from(expanded_path)
        } else {
            // 使用默认路径
            work_dir.join("target")
        };

        // 首先尝试列出target目录的内容
        let mut executor_path = None;
        if target_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        let exe_path = entry_path.join("executor");
                        if exe_path.exists() {
                            executor_path = Some(exe_path);
                            break;
                        }
                    }
                }
            }
        }

        // 如果没找到，检查常见路径
        if executor_path.is_none() {
            let possible_paths = vec![
                target_dir.join("debug/executor"),
                target_dir.join("release/executor"),
                work_dir.join("target/debug/executor"),
                work_dir.join("target/release/executor"),
            ];

            for path in possible_paths {
                if path.exists() {
                    executor_path = Some(path);
                    break;
                }
            }
        }

        let executor_path = executor_path.ok_or_else(|| {
            // 列出实际存在的文件
            let mut debug_info = String::new();
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                debug_info.push_str("Target directory contents:\n");
                for entry in entries.flatten() {
                    debug_info.push_str(&format!("  {:?}\n", entry.path()));
                    if entry.path().is_dir() {
                        if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                            for sub_entry in sub_entries.flatten() {
                                debug_info.push_str(&format!("    {:?}\n", sub_entry.path()));
                            }
                        }
                    }
                }
            }
            anyhow::anyhow!("Executor binary not found.\n{}", debug_info)
        })?;

        // 设置执行权限
        #[cfg(unix)]
        {
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&executor_path, permissions)
                .context("Failed to set executor permissions")?;
        }

        Ok(executor_path)
    }

    /// 生成执行器源代码（使用libloading）
    fn generate_executor_source(&self, library_path: &Path) -> Result<String> {
        let lib_name = library_path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid library name"))?;

        let source = format!(
            r#"
use std::ffi::{{CStr, CString}};
use std::os::raw::c_char;

fn main() {{
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {{
        eprintln!("Usage: executor <input_json>");
        std::process::exit(1);
    }}

    let input = &args[1];

    // 动态加载库
    unsafe {{
        let lib = match libloading::Library::new("./{lib_name}") {{
            Ok(lib) => lib,
            Err(e) => {{
                eprintln!("Failed to load library: {{e}}");
                std::process::exit(1);
            }}
        }};

        // 获取函数符号
        let flux_execute: libloading::Symbol<unsafe extern "C" fn(*const c_char) -> *mut c_char> =
            match lib.get(b"flux_execute") {{
                Ok(func) => func,
                Err(e) => {{
                    eprintln!("Failed to get flux_execute symbol: {{e}}");
                    std::process::exit(1);
                }}
            }};

        let flux_free: libloading::Symbol<unsafe extern "C" fn(*mut c_char)> =
            match lib.get(b"flux_free_string") {{
                Ok(func) => func,
                Err(e) => {{
                    eprintln!("Failed to get flux_free_string symbol: {{e}}");
                    std::process::exit(1);
                }}
            }};

        // 准备输入 - 修复类型转换
        let input_cstring = match CString::new(input.clone()) {{
            Ok(s) => s,
            Err(e) => {{
                eprintln!("Failed to create input CString: {{e}}");
                std::process::exit(1);
            }}
        }};

        // 调用函数
        let result_ptr = flux_execute(input_cstring.as_ptr());

        if result_ptr.is_null() {{
            eprintln!("Function returned null");
            std::process::exit(1);
        }}

        // 读取结果
        let result_cstr = CStr::from_ptr(result_ptr);
        let result_str = match result_cstr.to_str() {{
            Ok(s) => s,
            Err(e) => {{
                eprintln!("Failed to convert result to string: {{e}}");
                flux_free(result_ptr);
                std::process::exit(1);
            }}
        }};

        println!("{{}}", result_str);

        // 释放内存
        flux_free(result_ptr);
    }}
}}
"#
        );

        Ok(source)
    }

    /// 设置目录权限限制
    async fn set_directory_permissions(&self, dir: &Path) -> Result<()> {
        // 设置严格的目录权限
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(dir, permissions)
                .context("Failed to set directory permissions")?;
        }

        Ok(())
    }

    /// 注册进程监控
    async fn register_process_monitor(&self, pid: u32) {
        let monitor = ProcessMonitor {
            pid,
            start_time: Instant::now(),
            memory_usage: 0,
            cpu_usage: 0.0,
            is_running: true,
        };

        let mut processes = self.active_processes.write().await;
        processes.insert(pid, monitor);

        tracing::debug!("Registered process monitor for PID: {}", pid);
    }

    /// 取消注册进程监控
    async fn unregister_process_monitor(&self, pid: u32) {
        let mut processes = self.active_processes.write().await;
        processes.remove(&pid);
        tracing::debug!("Unregistered process monitor for PID: {}", pid);
    }

    /// 监控进程执行
    async fn monitor_process_execution(&self, child: Child, pid: u32) -> Result<SandboxResult> {
        let start_time = Instant::now();
        let mut peak_memory = 0u64;
        let mut cpu_usage = 0.0f64;

        // 启动资源监控任务
        let monitor_handle = {
            let system_monitor = self.system_monitor.clone();
            let active_processes = self.active_processes.clone();
            let max_memory = self.config.max_memory_mb * 1024 * 1024; // 转换为字节
            let max_cpu = self.config.max_cpu_percent;

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(100));

                loop {
                    interval.tick().await;

                    // 更新系统信息
                    {
                        let mut system = system_monitor.lock().await;
                        system.refresh_processes();
                    }

                    // 检查进程状态
                    let should_continue = {
                        let mut processes = active_processes.write().await;
                        if let Some(monitor) = processes.get_mut(&pid) {
                            if !monitor.is_running {
                                break;
                            }

                            // 获取进程资源使用情况
                            let system = system_monitor.lock().await;
                            if let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) {
                                monitor.memory_usage = process.memory();
                                monitor.cpu_usage = process.cpu_usage() as f64;

                                // 检查资源限制
                                if process.memory() > max_memory {
                                    tracing::warn!(
                                        "Process {} exceeded memory limit: {} > {}",
                                        pid,
                                        process.memory(),
                                        max_memory
                                    );
                                    monitor.is_running = false;
                                    return Err(anyhow::anyhow!("Memory limit exceeded"));
                                }

                                if process.cpu_usage() as f64 > max_cpu {
                                    tracing::warn!(
                                        "Process {} exceeded CPU limit: {}% > {}%",
                                        pid,
                                        process.cpu_usage(),
                                        max_cpu
                                    );
                                    // CPU超限警告但不立即终止
                                }

                                true
                            } else {
                                // 进程已不存在
                                monitor.is_running = false;
                                false
                            }
                        } else {
                            false
                        }
                    };

                    if !should_continue {
                        break;
                    }
                }

                Ok::<(), anyhow::Error>(())
            })
        };

        // 等待进程完成
        let child = child;
        let output = child
            .wait_with_output()
            .await
            .context("Failed to wait for process")?;

        // 停止监控
        monitor_handle.abort();

        // 更新最终统计
        {
            let processes = self.active_processes.read().await;
            if let Some(monitor) = processes.get(&pid) {
                peak_memory = monitor.memory_usage;
                cpu_usage = monitor.cpu_usage;
            }
        }

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let exit_code = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let status = if output.status.success() {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };

        let output_json = if output.status.success() && !stdout.trim().is_empty() {
            serde_json::from_str(&stdout)
                .unwrap_or_else(|_| serde_json::json!({"result": stdout.trim()}))
        } else {
            serde_json::json!({"error": stderr.trim()})
        };

        Ok(SandboxResult {
            status,
            output: output_json,
            execution_time_ms,
            peak_memory_bytes: peak_memory,
            cpu_usage_percent: cpu_usage,
            exit_code,
            stdout,
            stderr,
        })
    }

    /// 强制终止进程
    async fn kill_process(&self, pid: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            let nix_pid = Pid::from_raw(pid as i32);

            // 首先尝试优雅终止
            if let Err(e) = kill(nix_pid, Signal::SIGTERM) {
                tracing::warn!("Failed to send SIGTERM to process {}: {}", pid, e);
            } else {
                // 等待一段时间让进程优雅退出
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }

            // 检查进程是否仍然存在，如果存在则强制终止
            if let Err(e) = kill(nix_pid, Signal::SIGKILL) {
                tracing::warn!("Failed to send SIGKILL to process {}: {}", pid, e);
            }
        }

        #[cfg(windows)]
        {
            // Windows 版本的进程终止
            use std::process::Command;
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
        }

        // 更新监控状态
        {
            let mut processes = self.active_processes.write().await;
            if let Some(monitor) = processes.get_mut(&pid) {
                monitor.is_running = false;
            }
        }

        tracing::info!("Terminated process: {}", pid);
        Ok(())
    }

    /// 获取当前活跃进程数量
    pub async fn get_active_process_count(&self) -> usize {
        let processes = self.active_processes.read().await;
        processes.len()
    }

    /// 获取系统资源使用情况
    pub async fn get_system_usage(&self) -> Result<SystemUsage> {
        let mut system = self.system_monitor.lock().await;
        system.refresh_all();

        Ok(SystemUsage {
            total_memory_bytes: system.total_memory(),
            used_memory_bytes: system.used_memory(),
            total_swap_bytes: system.total_swap(),
            used_swap_bytes: system.used_swap(),
            cpu_count: system.cpus().len() as u32,
            load_average: format!("{:.2}", sysinfo::System::load_average().one),
        })
    }

    /// 清理所有资源
    pub async fn cleanup(&self) -> Result<()> {
        // 终止所有活跃进程
        let pids: Vec<u32> = {
            let processes = self.active_processes.read().await;
            processes.keys().cloned().collect()
        };

        for pid in pids {
            let _ = self.kill_process(pid).await;
        }

        // 清理临时目录
        {
            let mut temp_dirs = self.temp_dirs.write().await;
            temp_dirs.clear();
        }

        tracing::info!("Sandbox executor cleanup completed");
        Ok(())
    }
}

/// 系统资源使用情况
#[derive(Debug, Clone, Serialize)]
pub struct SystemUsage {
    /// 总内存（字节）
    pub total_memory_bytes: u64,
    /// 已使用内存（字节）
    pub used_memory_bytes: u64,
    /// 总交换空间（字节）
    pub total_swap_bytes: u64,
    /// 已使用交换空间（字节）
    pub used_swap_bytes: u64,
    /// CPU核心数
    pub cpu_count: u32,
    /// 系统负载平均值（简化为字符串）
    pub load_average: String,
}

impl Drop for SandboxExecutor {
    fn drop(&mut self) {
        // 注意：在Drop中不能使用async，所以这里只是记录日志
        tracing::info!("SandboxExecutor dropped, cleanup should be called explicitly");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(config.enable_process_isolation);
        assert!(!config.enable_container_isolation);
        assert_eq!(config.execution_timeout_secs, 30);
        assert_eq!(config.max_memory_mb, 128);
    }

    #[tokio::test]
    async fn test_sandbox_executor_creation() {
        let config = SandboxConfig::default();
        let result = SandboxExecutor::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_executor_source_generation() {
        let config = SandboxConfig::default();
        let executor = SandboxExecutor::new(config).unwrap();
        let lib_path = Path::new("test.so");
        let source = executor.generate_executor_source(lib_path).unwrap();

        assert!(source.contains("flux_execute"));
        assert!(source.contains("flux_free_string"));
        assert!(source.contains("test.so"));
    }
}
