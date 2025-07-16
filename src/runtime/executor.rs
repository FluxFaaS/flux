use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::functions::{ExecutionStatus, FunctionMetadata, InvokeRequest, InvokeResponse};
use crate::runtime::compiler::{CompilerConfig, RustCompiler};
use crate::runtime::resource::{ResourceManager, ResourceQuota};
use crate::runtime::sandbox::{SandboxConfig, SandboxExecutor, SandboxResult};

/// 进程级隔离执行器
///
/// 相比基础的ProcessExecutor，提供更强的进程隔离和资源管理功能：
/// - 独立进程沙箱执行
/// - 细粒度资源监控和限制
/// - 安全策略执行
/// - 生命周期管理
#[derive(Debug)]
pub struct IsolatedProcessExecutor {
    /// 编译器实例
    compiler: Arc<RustCompiler>,
    /// 沙箱执行器
    sandbox: Arc<SandboxExecutor>,
    /// 资源管理器
    resource_manager: Arc<ResourceManager>,
    /// 活跃的执行实例
    active_executions: Arc<RwLock<HashMap<String, ExecutionInstance>>>,
    /// 执行统计
    execution_stats: Arc<RwLock<ExecutionStatistics>>,
}

/// 执行实例信息
#[derive(Debug, Clone)]
pub struct ExecutionInstance {
    /// 执行ID
    pub execution_id: String,
    /// 函数名称
    pub function_name: String,
    /// 进程ID
    pub process_id: Option<u32>,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 开始时间
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// 结束时间
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 资源使用配额
    pub quota_name: Option<String>,
}

/// 执行统计信息
#[derive(Debug, Clone, Default)]
pub struct ExecutionStatistics {
    /// 总执行次数
    pub total_executions: u64,
    /// 成功执行次数
    pub successful_executions: u64,
    /// 失败执行次数
    pub failed_executions: u64,
    /// 超时执行次数
    pub timeout_executions: u64,
    /// 平均执行时间（毫秒）
    pub average_execution_time_ms: f64,
    /// 最长执行时间（毫秒）
    pub max_execution_time_ms: u64,
    /// 最短执行时间（毫秒）
    pub min_execution_time_ms: u64,
}

/// 进程级隔离配置
#[derive(Debug, Clone)]
pub struct IsolatedExecutorConfig {
    /// 编译器配置
    pub compiler_config: CompilerConfig,
    /// 沙箱配置
    pub sandbox_config: SandboxConfig,
    /// 默认资源配额名称
    pub default_quota_name: Option<String>,
    /// 最大并发执行数
    pub max_concurrent_executions: usize,
    /// 执行实例清理间隔（秒）
    pub cleanup_interval_secs: u64,
}

impl Default for IsolatedExecutorConfig {
    fn default() -> Self {
        Self {
            compiler_config: CompilerConfig::default(),
            sandbox_config: SandboxConfig::default(),
            default_quota_name: Some("default".to_string()),
            max_concurrent_executions: 100,
            cleanup_interval_secs: 300, // 5分钟
        }
    }
}

impl IsolatedProcessExecutor {
    /// 创建新的进程级隔离执行器
    pub fn new(config: IsolatedExecutorConfig) -> Result<Self> {
        let compiler = Arc::new(RustCompiler::new(config.compiler_config)?);
        let sandbox = Arc::new(SandboxExecutor::new(config.sandbox_config)?);
        let resource_manager = Arc::new(ResourceManager::new());

        // 设置默认资源配额
        if let Some(quota_name) = &config.default_quota_name {
            tokio::spawn({
                let resource_manager = resource_manager.clone();
                let quota_name = quota_name.clone();
                async move {
                    let default_quota = ResourceQuota::default();
                    resource_manager.set_quota(default_quota).await;
                    info!("Set default resource quota: {}", quota_name);
                }
            });
        }

        Ok(Self {
            compiler,
            sandbox,
            resource_manager,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            execution_stats: Arc::new(RwLock::new(ExecutionStatistics::default())),
        })
    }

    /// 在隔离进程中执行函数
    pub async fn execute_isolated(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
        quota_name: Option<String>,
    ) -> Result<InvokeResponse> {
        let execution_id = scru128::new().to_string();
        let start_time = std::time::Instant::now();

        info!(
            "Starting isolated execution for function: {} (ID: {})",
            function.name, execution_id
        );

        // 检查并发限制
        self.check_concurrent_limit().await?;

        // 编译函数
        let compiled = self
            .compiler
            .compile_function(function)
            .await
            .context("Failed to compile function for isolated execution")?;

        // 创建执行实例记录
        let execution_instance = ExecutionInstance {
            execution_id: execution_id.clone(),
            function_name: function.name.clone(),
            process_id: None,
            status: ExecutionStatus::Success, // 临时状态，会在执行完成后更新
            started_at: chrono::Utc::now(),
            ended_at: None,
            quota_name: quota_name.clone(),
        };

        // 注册执行实例
        {
            let mut active = self.active_executions.write().await;
            active.insert(execution_id.clone(), execution_instance);
        }

        // 开始资源监控
        let monitoring_future = if let Some(ref quota_name) = quota_name {
            Some(
                self.resource_manager
                    .start_monitoring(
                        0, // PID将在执行开始后更新
                        function.name.clone(),
                        Some(quota_name.clone()),
                    )
                    .await,
            )
        } else {
            None
        };

        if let Some(Err(e)) = monitoring_future {
            warn!("Failed to start resource monitoring: {}", e);
        }

        // 在沙箱中执行
        let execution_result = self.sandbox.execute_in_sandbox(&compiled, request).await;

        let execution_time = start_time.elapsed();

        // 更新执行实例状态
        {
            let mut active = self.active_executions.write().await;
            if let Some(instance) = active.get_mut(&execution_id) {
                instance.ended_at = Some(chrono::Utc::now());
                instance.status = match &execution_result {
                    Ok(result) => result.status.clone(),
                    Err(_) => ExecutionStatus::Failed,
                };
                if let Ok(result) = &execution_result {
                    // 更新进程ID如果有的话
                    if result.exit_code.is_some() {
                        // 这里可以从沙箱结果中获取实际的进程ID
                        // instance.process_id = Some(actual_pid);
                    }
                }
            }
        }

        // 停止资源监控
        if quota_name.is_some() {
            // 注意：这里我们使用0作为占位符PID，实际应该使用真实的PID
            let _ = self.resource_manager.stop_monitoring(0).await;
        }

        // 更新统计信息
        self.update_execution_stats(&execution_result, execution_time.as_millis() as u64)
            .await;

        // 处理执行结果
        match execution_result {
            Ok(sandbox_result) => {
                info!(
                    "Isolated execution completed successfully for function: {} (ID: {})",
                    function.name, execution_id
                );

                Ok(InvokeResponse {
                    output: sandbox_result.output,
                    status: sandbox_result.status,
                    execution_time_ms: sandbox_result.execution_time_ms,
                })
            }
            Err(e) => {
                error!(
                    "Isolated execution failed for function: {} (ID: {}): {}",
                    function.name, execution_id, e
                );

                Ok(InvokeResponse {
                    output: serde_json::json!(null),
                    status: ExecutionStatus::Failed,
                    execution_time_ms: execution_time.as_millis() as u64,
                })
            }
        }
    }

    /// 检查并发执行限制
    async fn check_concurrent_limit(&self) -> Result<()> {
        let active_count = self.active_executions.read().await.len();
        if active_count >= 100 {
            // 从配置中获取，这里暂时硬编码
            return Err(anyhow::anyhow!(
                "Maximum concurrent executions limit reached: {}",
                active_count
            ));
        }
        Ok(())
    }

    /// 更新执行统计信息
    async fn update_execution_stats(&self, result: &Result<SandboxResult>, execution_time_ms: u64) {
        let mut stats = self.execution_stats.write().await;

        stats.total_executions += 1;

        match result {
            Ok(sandbox_result) => match sandbox_result.status {
                ExecutionStatus::Completed | ExecutionStatus::Success => {
                    stats.successful_executions += 1
                }
                ExecutionStatus::Failed | ExecutionStatus::Error(_) => stats.failed_executions += 1,
                ExecutionStatus::Timeout => stats.timeout_executions += 1,
            },
            Err(_) => stats.failed_executions += 1,
        }

        // 更新执行时间统计
        if stats.total_executions == 1 {
            stats.min_execution_time_ms = execution_time_ms;
            stats.max_execution_time_ms = execution_time_ms;
            stats.average_execution_time_ms = execution_time_ms as f64;
        } else {
            stats.min_execution_time_ms = stats.min_execution_time_ms.min(execution_time_ms);
            stats.max_execution_time_ms = stats.max_execution_time_ms.max(execution_time_ms);

            // 计算移动平均值
            let prev_avg = stats.average_execution_time_ms;
            let count = stats.total_executions as f64;
            stats.average_execution_time_ms =
                (prev_avg * (count - 1.0) + execution_time_ms as f64) / count;
        }
    }

    /// 获取活跃执行实例列表
    pub async fn get_active_executions(&self) -> Vec<ExecutionInstance> {
        self.active_executions
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// 获取执行统计信息
    pub async fn get_execution_statistics(&self) -> ExecutionStatistics {
        self.execution_stats.read().await.clone()
    }

    /// 根据执行ID获取执行实例
    pub async fn get_execution_instance(&self, execution_id: &str) -> Option<ExecutionInstance> {
        self.active_executions
            .read()
            .await
            .get(execution_id)
            .cloned()
    }

    /// 终止指定的执行实例
    pub async fn terminate_execution(&self, execution_id: &str) -> Result<bool> {
        let mut active = self.active_executions.write().await;

        if let Some(instance) = active.get_mut(execution_id) {
            if let Some(process_id) = instance.process_id {
                // 尝试终止进程
                #[cfg(unix)]
                {
                    use nix::sys::signal::{self, Signal};
                    use nix::unistd::Pid;

                    match signal::kill(Pid::from_raw(process_id as i32), Signal::SIGTERM) {
                        Ok(_) => {
                            instance.status = ExecutionStatus::Failed;
                            instance.ended_at = Some(chrono::Utc::now());
                            info!(
                                "Terminated execution {} (PID: {})",
                                execution_id, process_id
                            );
                            return Ok(true);
                        }
                        Err(e) => {
                            warn!("Failed to terminate process {}: {}", process_id, e);
                            return Ok(false);
                        }
                    }
                }

                #[cfg(not(unix))]
                {
                    warn!("Process termination not supported on this platform");
                    return Ok(false);
                }
            } else {
                warn!("No process ID available for execution {}", execution_id);
                return Ok(false);
            }
        }

        Ok(false)
    }

    /// 清理已完成的执行实例
    pub async fn cleanup_completed_executions(&self) -> usize {
        let mut active = self.active_executions.write().await;
        let initial_count = active.len();

        let now = chrono::Utc::now();
        let cleanup_threshold = chrono::Duration::minutes(5); // 5分钟后清理

        active.retain(|_, instance| {
            // 检查是否已经结束并且超过清理阈值
            if let Some(ended_at) = instance.ended_at {
                now.signed_duration_since(ended_at) < cleanup_threshold
            } else {
                // 没有结束时间的实例认为还在运行，保留
                true
            }
        });

        let cleaned_count = initial_count - active.len();
        if cleaned_count > 0 {
            info!("Cleaned up {} completed execution instances", cleaned_count);
        }

        cleaned_count
    }

    /// 关闭执行器，清理所有资源
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down isolated process executor");

        // 终止所有活跃的执行
        let active_executions = self.get_active_executions().await;
        for instance in active_executions {
            // 如果实例还没有结束时间，认为它还在运行，需要终止
            if instance.ended_at.is_none() {
                let _ = self.terminate_execution(&instance.execution_id).await;
            }
        }

        // 清理资源管理器
        let _ = self.resource_manager.cleanup_all().await;

        // 清理沙箱资源
        let _ = self.sandbox.cleanup().await;

        info!("Isolated process executor shutdown completed");
        Ok(())
    }
}

/// 兼容性：保持原有的ProcessExecutor，但标记为已弃用
#[deprecated(
    note = "Use IsolatedProcessExecutor instead for better isolation and resource management"
)]
pub struct ProcessExecutor {
    isolated_executor: IsolatedProcessExecutor,
}

#[allow(deprecated)]
impl ProcessExecutor {
    pub fn new() -> Result<Self> {
        let config = IsolatedExecutorConfig::default();
        let isolated_executor = IsolatedProcessExecutor::new(config)?;
        Ok(Self { isolated_executor })
    }

    pub async fn execute_function(
        &self,
        function: &FunctionMetadata,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        self.isolated_executor
            .execute_isolated(function, request, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_isolated_executor_creation() {
        let config = IsolatedExecutorConfig::default();
        let executor = IsolatedProcessExecutor::new(config);
        assert!(executor.is_ok());
    }

    #[tokio::test]
    async fn test_execution_statistics() {
        let config = IsolatedExecutorConfig::default();
        let executor = IsolatedProcessExecutor::new(config).unwrap();

        let stats = executor.get_execution_statistics().await;
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.successful_executions, 0);
        assert_eq!(stats.failed_executions, 0);
    }

    #[tokio::test]
    async fn test_concurrent_limit_check() {
        let config = IsolatedExecutorConfig::default();
        let executor = IsolatedProcessExecutor::new(config).unwrap();

        let result = executor.check_concurrent_limit().await;
        assert!(result.is_ok());
    }
}
