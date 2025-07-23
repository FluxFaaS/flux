use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, sleep};

use crate::functions::{ExecutionStatus, FunctionMetadata, InvokeRequest, InvokeResponse};
use crate::runtime::compiler::{CompiledFunction, RustCompiler};
use crate::runtime::resource::{ResourceManager, ResourceSummary};
use crate::runtime::sandbox::{SandboxExecutor, SandboxResult};

/// 函数实例状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InstanceState {
    /// 创建中
    Creating,
    /// 预热中
    Warming,
    /// 就绪状态
    Ready,
    /// 运行中
    Running,
    /// 空闲状态
    Idle,
    /// 暂停状态
    Paused,
    /// 停止中
    Stopping,
    /// 已停止
    Stopped,
    /// 错误状态
    Error(String),
}

/// 函数实例配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    /// 最大空闲时间（秒）
    pub max_idle_duration_secs: u64,
    /// 预热超时时间（秒）
    pub warm_timeout_secs: u64,
    /// 最大执行时间（秒）
    pub max_execution_duration_secs: u64,
    /// 是否启用自动预热
    pub enable_auto_warm: bool,
    /// 预热并发数
    pub warm_concurrency: u32,
    /// 资源配额名称
    pub resource_quota_name: Option<String>,
    /// 实例标签
    pub labels: HashMap<String, String>,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            max_idle_duration_secs: 300,     // 5分钟
            warm_timeout_secs: 30,           // 30秒
            max_execution_duration_secs: 60, // 1分钟
            enable_auto_warm: true,
            warm_concurrency: 1,
            resource_quota_name: Some("default".to_string()),
            labels: HashMap::new(),
        }
    }
}

/// 函数实例信息
#[derive(Debug, Clone)]
pub struct FunctionInstance {
    /// 实例ID
    pub instance_id: String,
    /// 函数名称
    pub function_name: String,
    /// 函数元数据
    pub function_metadata: FunctionMetadata,
    /// 编译后的函数
    pub compiled_function: Option<CompiledFunction>,
    /// 当前状态
    pub state: InstanceState,
    /// 配置
    pub config: InstanceConfig,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后活动时间
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// 执行统计
    pub execution_stats: InstanceExecutionStats,
    /// 进程ID（如果正在运行）
    pub process_id: Option<u32>,
    /// 版本号
    pub version: u64,
}

/// 实例执行统计
#[derive(Debug, Clone, Default)]
pub struct InstanceExecutionStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 成功执行次数
    pub successful_executions: u64,
    /// 失败执行次数
    pub failed_executions: u64,
    /// 平均执行时间（毫秒）
    pub avg_execution_time_ms: f64,
    /// 最大执行时间（毫秒）
    pub max_execution_time_ms: u64,
    /// 最小执行时间（毫秒）
    pub min_execution_time_ms: u64,
    /// 最后执行时间
    pub last_execution_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 累计CPU使用时间（毫秒）
    pub total_cpu_time_ms: u64,
    /// 峰值内存使用（字节）
    pub peak_memory_bytes: u64,
}

/// 实例生命周期事件
#[derive(Debug, Clone, Serialize)]
pub struct InstanceLifecycleEvent {
    /// 事件ID
    pub event_id: String,
    /// 实例ID
    pub instance_id: String,
    /// 函数名称
    pub function_name: String,
    /// 事件类型
    pub event_type: LifecycleEventType,
    /// 事件时间
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 事件描述
    pub description: String,
    /// 相关数据
    pub metadata: HashMap<String, String>,
}

/// 生命周期事件类型
#[derive(Debug, Clone, Serialize)]
pub enum LifecycleEventType {
    /// 实例创建
    Created,
    /// 开始预热
    WarmingStarted,
    /// 预热完成
    WarmingCompleted,
    /// 预热失败
    WarmingFailed,
    /// 实例就绪
    Ready,
    /// 开始执行
    ExecutionStarted,
    /// 执行完成
    ExecutionCompleted,
    /// 执行失败
    ExecutionFailed,
    /// 实例空闲
    Idle,
    /// 实例暂停
    Paused,
    /// 实例恢复
    Resumed,
    /// 实例停止
    Stopped,
    /// 实例错误
    Error,
}

/// 函数实例管理器
#[derive(Debug)]
pub struct InstanceManager {
    /// 活跃实例
    active_instances: Arc<RwLock<HashMap<String, FunctionInstance>>>,
    /// 函数名到实例ID的映射
    function_instances: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// 编译器
    compiler: Arc<RustCompiler>,
    /// 沙箱执行器
    sandbox: Arc<SandboxExecutor>,
    /// 资源管理器
    resource_manager: Arc<ResourceManager>,
    /// 生命周期事件历史
    lifecycle_events: Arc<RwLock<Vec<InstanceLifecycleEvent>>>,
    /// 默认配置
    default_config: InstanceConfig,
    /// 清理任务句柄
    cleanup_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl InstanceManager {
    /// 创建新的实例管理器
    pub fn new(
        compiler: Arc<RustCompiler>,
        sandbox: Arc<SandboxExecutor>,
        resource_manager: Arc<ResourceManager>,
        default_config: Option<InstanceConfig>,
    ) -> Self {
        let manager = Self {
            active_instances: Arc::new(RwLock::new(HashMap::new())),
            function_instances: Arc::new(RwLock::new(HashMap::new())),
            compiler,
            sandbox,
            resource_manager,
            lifecycle_events: Arc::new(RwLock::new(Vec::new())),
            default_config: default_config.unwrap_or_default(),
            cleanup_handle: Arc::new(Mutex::new(None)),
        };

        // 启动清理任务
        manager.start_cleanup_task();

        manager
    }

    /// 创建新的函数实例
    pub async fn create_instance(
        &self,
        function_metadata: FunctionMetadata,
        config: Option<InstanceConfig>,
    ) -> Result<String> {
        let instance_id = scru128::new().to_string();
        let config = config.unwrap_or_else(|| self.default_config.clone());

        tracing::info!(
            "Creating new instance for function: {} (ID: {})",
            function_metadata.name,
            instance_id
        );

        // 记录创建事件
        self.emit_lifecycle_event(
            &instance_id,
            &function_metadata.name,
            LifecycleEventType::Created,
            "Instance creation started".to_string(),
            HashMap::new(),
        )
        .await;

        let mut instance = FunctionInstance {
            instance_id: instance_id.clone(),
            function_name: function_metadata.name.clone(),
            function_metadata,
            compiled_function: None,
            state: InstanceState::Creating,
            config,
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
            execution_stats: InstanceExecutionStats::default(),
            process_id: None,
            version: 1,
        };

        // 编译函数
        match self
            .compiler
            .compile_function(&instance.function_metadata)
            .await
        {
            Ok(compiled) => {
                instance.compiled_function = Some(compiled);
                instance.state = InstanceState::Ready;
                tracing::info!("Function compiled successfully for instance: {instance_id}");
            }
            Err(e) => {
                instance.state = InstanceState::Error(format!("Compilation failed: {e}"));
                tracing::error!("Failed to compile function for instance {instance_id}: {e}");
            }
        }

        // 保存函数名用于后续映射
        let function_name = instance.function_name.clone();

        // 注册实例
        {
            let mut instances = self.active_instances.write().await;
            instances.insert(instance_id.clone(), instance);
        }

        // 更新函数到实例的映射
        {
            let mut function_instances = self.function_instances.write().await;
            function_instances
                .entry(function_name)
                .or_insert_with(Vec::new)
                .push(instance_id.clone());
        }

        // 如果启用自动预热，开始预热
        if self.default_config.enable_auto_warm {
            self.warm_instance(&instance_id).await?;
        }

        Ok(instance_id)
    }

    /// 预热实例
    pub async fn warm_instance(&self, instance_id: &str) -> Result<()> {
        let mut instance = {
            let mut instances = self.active_instances.write().await;
            instances
                .get_mut(instance_id)
                .ok_or_else(|| anyhow::anyhow!("Instance not found: {}", instance_id))?
                .clone()
        };

        if instance.state != InstanceState::Ready {
            return Err(anyhow::anyhow!(
                "Instance {} is not ready for warming (current state: {:?})",
                instance_id,
                instance.state
            ));
        }

        tracing::info!("Starting warm-up for instance: {}", instance_id);

        // 记录预热开始事件
        self.emit_lifecycle_event(
            instance_id,
            &instance.function_name,
            LifecycleEventType::WarmingStarted,
            "Instance warming started".to_string(),
            HashMap::new(),
        )
        .await;

        // 更新状态为预热中
        instance.state = InstanceState::Warming;
        self.update_instance(instance.clone()).await?;

        // 执行预热（这里可以实现具体的预热逻辑）
        let warm_result = self.perform_warm_up(&instance).await;

        match warm_result {
            Ok(_) => {
                instance.state = InstanceState::Ready;
                instance.last_activity = chrono::Utc::now();

                self.emit_lifecycle_event(
                    instance_id,
                    &instance.function_name,
                    LifecycleEventType::WarmingCompleted,
                    "Instance warming completed successfully".to_string(),
                    HashMap::new(),
                )
                .await;

                tracing::info!("Instance {} warmed up successfully", instance_id);
            }
            Err(e) => {
                instance.state = InstanceState::Error(format!("Warm-up failed: {e}"));

                self.emit_lifecycle_event(
                    instance_id,
                    &instance.function_name,
                    LifecycleEventType::WarmingFailed,
                    format!("Instance warming failed: {e}"),
                    HashMap::new(),
                )
                .await;

                tracing::error!("Failed to warm up instance {instance_id}: {e}");
            }
        }

        self.update_instance(instance).await?;
        Ok(())
    }

    /// 执行函数实例
    pub async fn execute_instance(
        &self,
        instance_id: &str,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        let start_time = Instant::now();

        // 获取实例
        let mut instance = {
            let instances = self.active_instances.read().await;
            instances
                .get(instance_id)
                .ok_or_else(|| anyhow::anyhow!("Instance not found: {}", instance_id))?
                .clone()
        };

        // 检查实例状态
        if !matches!(instance.state, InstanceState::Ready | InstanceState::Idle) {
            return Err(anyhow::anyhow!(
                "Instance {} is not ready for execution (current state: {:?})",
                instance_id,
                instance.state
            ));
        }

        // 记录执行开始事件
        self.emit_lifecycle_event(
            instance_id,
            &instance.function_name,
            LifecycleEventType::ExecutionStarted,
            "Function execution started".to_string(),
            HashMap::new(),
        )
        .await;

        // 更新状态为运行中
        instance.state = InstanceState::Running;
        instance.last_activity = chrono::Utc::now();
        self.update_instance(instance.clone()).await?;

        // 开始资源监控
        let monitoring_started = if let Some(ref quota_name) = instance.config.resource_quota_name {
            self.resource_manager
                .start_monitoring(0, instance.function_name.clone(), Some(quota_name.clone()))
                .await
                .is_ok()
        } else {
            false
        };

        // 执行函数
        let execution_result = if let Some(ref compiled) = instance.compiled_function {
            self.sandbox.execute_in_sandbox(compiled, request).await
        } else {
            Err(anyhow::anyhow!("Function not compiled"))
        };

        let execution_time = start_time.elapsed();

        // 停止资源监控
        let resource_summary = if monitoring_started {
            self.resource_manager
                .stop_monitoring(0)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        // 处理执行结果
        let response = match execution_result {
            Ok(sandbox_result) => {
                // 更新执行统计
                self.update_execution_stats(
                    instance_id,
                    &sandbox_result,
                    execution_time,
                    resource_summary,
                )
                .await?;

                // 记录执行完成事件
                self.emit_lifecycle_event(
                    instance_id,
                    &instance.function_name,
                    LifecycleEventType::ExecutionCompleted,
                    "Function execution completed successfully".to_string(),
                    HashMap::new(),
                )
                .await;

                InvokeResponse {
                    output: sandbox_result.output,
                    execution_time_ms: sandbox_result.execution_time_ms,
                    status: sandbox_result.status,
                }
            }
            Err(e) => {
                // 记录执行失败事件
                self.emit_lifecycle_event(
                    instance_id,
                    &instance.function_name,
                    LifecycleEventType::ExecutionFailed,
                    format!("Function execution failed: {e}"),
                    HashMap::new(),
                )
                .await;

                InvokeResponse {
                    output: serde_json::json!({"error": e.to_string()}),
                    execution_time_ms: execution_time.as_millis() as u64,
                    status: ExecutionStatus::Failed,
                }
            }
        };

        // 更新实例状态为空闲
        instance.state = InstanceState::Idle;
        instance.last_activity = chrono::Utc::now();
        self.update_instance(instance).await?;

        Ok(response)
    }

    /// 停止实例
    pub async fn stop_instance(&self, instance_id: &str) -> Result<()> {
        let mut instance = {
            let instances = self.active_instances.read().await;
            instances
                .get(instance_id)
                .ok_or_else(|| anyhow::anyhow!("Instance not found: {}", instance_id))?
                .clone()
        };

        tracing::info!("Stopping instance: {}", instance_id);

        // 记录停止事件
        self.emit_lifecycle_event(
            instance_id,
            &instance.function_name,
            LifecycleEventType::Stopped,
            "Instance stopped".to_string(),
            HashMap::new(),
        )
        .await;

        // 如果有进程在运行，停止资源监控
        if let Some(process_id) = instance.process_id {
            let _ = self.resource_manager.stop_monitoring(process_id).await;
        }

        // 更新状态
        instance.state = InstanceState::Stopped;
        self.update_instance(instance.clone()).await?;

        // 从活跃实例中移除
        {
            let mut instances = self.active_instances.write().await;
            instances.remove(instance_id);
        }

        // 从函数映射中移除
        {
            let mut function_instances = self.function_instances.write().await;
            if let Some(instance_ids) = function_instances.get_mut(&instance.function_name) {
                instance_ids.retain(|id| id != instance_id);
                if instance_ids.is_empty() {
                    function_instances.remove(&instance.function_name);
                }
            }
        }

        Ok(())
    }

    /// 获取函数的所有实例
    pub async fn get_function_instances(&self, function_name: &str) -> Vec<String> {
        let function_instances = self.function_instances.read().await;
        function_instances
            .get(function_name)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取实例信息
    pub async fn get_instance(&self, instance_id: &str) -> Option<FunctionInstance> {
        let instances = self.active_instances.read().await;
        instances.get(instance_id).cloned()
    }

    /// 获取所有活跃实例
    pub async fn get_all_instances(&self) -> Vec<FunctionInstance> {
        let instances = self.active_instances.read().await;
        instances.values().cloned().collect()
    }

    /// 获取实例统计信息
    pub async fn get_instance_stats(&self) -> InstanceManagerStats {
        let instances = self.active_instances.read().await;
        let function_instances = self.function_instances.read().await;

        let mut stats = InstanceManagerStats {
            total_instances: instances.len() as u64,
            active_functions: function_instances.len() as u64,
            ..Default::default()
        };

        for instance in instances.values() {
            match instance.state {
                InstanceState::Ready => stats.ready_instances += 1,
                InstanceState::Running => stats.running_instances += 1,
                InstanceState::Idle => stats.idle_instances += 1,
                InstanceState::Warming => stats.warming_instances += 1,
                InstanceState::Error(_) => stats.error_instances += 1,
                _ => {}
            }

            stats.total_executions += instance.execution_stats.total_executions;
            stats.successful_executions += instance.execution_stats.successful_executions;
            stats.failed_executions += instance.execution_stats.failed_executions;
        }

        stats
    }

    /// 清理空闲实例
    pub async fn cleanup_idle_instances(&self) -> Result<u64> {
        let now = chrono::Utc::now();
        let mut cleaned_count = 0u64;

        let instances_to_cleanup: Vec<String> = {
            let instances = self.active_instances.read().await;
            instances
                .values()
                .filter(|instance| {
                    matches!(instance.state, InstanceState::Idle)
                        && (now - instance.last_activity).num_seconds() as u64
                            > instance.config.max_idle_duration_secs
                })
                .map(|instance| instance.instance_id.clone())
                .collect()
        };

        for instance_id in instances_to_cleanup {
            if let Err(e) = self.stop_instance(&instance_id).await {
                tracing::warn!("Failed to cleanup idle instance {instance_id}: {e}");
            } else {
                cleaned_count += 1;
                tracing::info!("Cleaned up idle instance: {instance_id}");
            }
        }

        Ok(cleaned_count)
    }

    /// 更新实例
    async fn update_instance(&self, instance: FunctionInstance) -> Result<()> {
        let mut instances = self.active_instances.write().await;
        instances.insert(instance.instance_id.clone(), instance);
        Ok(())
    }

    /// 执行预热操作
    async fn perform_warm_up(&self, instance: &FunctionInstance) -> Result<()> {
        // 这里可以实现具体的预热逻辑
        // 例如：预编译、预加载依赖、初始化连接等

        // 模拟预热过程
        sleep(Duration::from_millis(100)).await;

        tracing::debug!("Warm-up completed for instance: {}", instance.instance_id);
        Ok(())
    }

    /// 更新执行统计
    async fn update_execution_stats(
        &self,
        instance_id: &str,
        sandbox_result: &SandboxResult,
        execution_time: Duration,
        resource_summary: Option<ResourceSummary>,
    ) -> Result<()> {
        let mut instances = self.active_instances.write().await;

        if let Some(instance) = instances.get_mut(instance_id) {
            let stats = &mut instance.execution_stats;

            stats.total_executions += 1;
            stats.last_execution_time = Some(chrono::Utc::now());

            let execution_time_ms = execution_time.as_millis() as u64;

            if matches!(
                sandbox_result.status,
                ExecutionStatus::Success | ExecutionStatus::Completed
            ) {
                stats.successful_executions += 1;
            } else {
                stats.failed_executions += 1;
            }

            // 更新执行时间统计
            if stats.min_execution_time_ms == 0 || execution_time_ms < stats.min_execution_time_ms {
                stats.min_execution_time_ms = execution_time_ms;
            }

            if execution_time_ms > stats.max_execution_time_ms {
                stats.max_execution_time_ms = execution_time_ms;
            }

            // 更新平均执行时间
            stats.avg_execution_time_ms = (stats.avg_execution_time_ms
                * (stats.total_executions - 1) as f64
                + execution_time_ms as f64)
                / stats.total_executions as f64;

            // 更新资源使用统计
            if let Some(resource_summary) = resource_summary {
                stats.total_cpu_time_ms += resource_summary.total_duration_ms;

                // 更新峰值内存使用
                for (_, usage) in resource_summary.resource_usage {
                    if usage.peak_usage > stats.peak_memory_bytes {
                        stats.peak_memory_bytes = usage.peak_usage;
                    }
                }
            }
        }

        Ok(())
    }

    /// 发出生命周期事件
    async fn emit_lifecycle_event(
        &self,
        instance_id: &str,
        function_name: &str,
        event_type: LifecycleEventType,
        description: String,
        metadata: HashMap<String, String>,
    ) {
        let event = InstanceLifecycleEvent {
            event_id: scru128::new().to_string(),
            instance_id: instance_id.to_string(),
            function_name: function_name.to_string(),
            event_type,
            timestamp: chrono::Utc::now(),
            description,
            metadata,
        };

        let mut events = self.lifecycle_events.write().await;
        events.push(event);

        // 限制事件历史大小
        if events.len() > 10000 {
            events.remove(0);
        }
    }

    /// 启动清理任务
    fn start_cleanup_task(&self) {
        let instances = self.active_instances.clone();
        let function_instances = self.function_instances.clone();
        let resource_manager = self.resource_manager.clone();

        let cleanup_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // 每分钟清理一次

            loop {
                interval.tick().await;

                let now = chrono::Utc::now();
                let instances_to_cleanup: Vec<String> = {
                    let instances_guard = instances.read().await;
                    instances_guard
                        .values()
                        .filter(|instance| {
                            matches!(instance.state, InstanceState::Idle)
                                && (now - instance.last_activity).num_seconds() as u64
                                    > instance.config.max_idle_duration_secs
                        })
                        .map(|instance| instance.instance_id.clone())
                        .collect()
                };

                for instance_id in instances_to_cleanup {
                    // 停止资源监控
                    if let Some(instance) = {
                        let instances_guard = instances.read().await;
                        instances_guard.get(&instance_id).cloned()
                    } {
                        if let Some(process_id) = instance.process_id {
                            let _ = resource_manager.stop_monitoring(process_id).await;
                        }

                        // 从活跃实例中移除
                        {
                            let mut instances_guard = instances.write().await;
                            instances_guard.remove(&instance_id);
                        }

                        // 从函数映射中移除
                        {
                            let mut function_instances_guard = function_instances.write().await;
                            if let Some(instance_ids) =
                                function_instances_guard.get_mut(&instance.function_name)
                            {
                                instance_ids.retain(|id| id != &instance_id);
                                if instance_ids.is_empty() {
                                    function_instances_guard.remove(&instance.function_name);
                                }
                            }
                        }

                        tracing::info!("Cleaned up idle instance: {}", instance_id);
                    }
                }
            }
        });

        // 保存清理任务句柄
        let cleanup_handle = self.cleanup_handle.clone();
        tokio::spawn(async move {
            let mut handle = cleanup_handle.lock().await;
            *handle = Some(cleanup_task);
        });
    }

    /// 获取生命周期事件
    pub async fn get_lifecycle_events(&self, limit: Option<usize>) -> Vec<InstanceLifecycleEvent> {
        let events = self.lifecycle_events.read().await;
        let limit = limit.unwrap_or(100);

        if events.len() <= limit {
            events.clone()
        } else {
            events[events.len() - limit..].to_vec()
        }
    }

    /// 清理资源
    pub async fn cleanup(&self) -> Result<()> {
        // 停止清理任务
        {
            let mut handle = self.cleanup_handle.lock().await;
            if let Some(task) = handle.take() {
                task.abort();
            }
        }

        // 停止所有实例
        let instance_ids: Vec<String> = {
            let instances = self.active_instances.read().await;
            instances.keys().cloned().collect()
        };

        for instance_id in instance_ids {
            if let Err(e) = self.stop_instance(&instance_id).await {
                tracing::warn!(
                    "Failed to stop instance {} during cleanup: {}",
                    instance_id,
                    e
                );
            }
        }

        tracing::info!("Instance manager cleanup completed");
        Ok(())
    }
}

/// 实例管理器统计信息
#[derive(Debug, Clone, Default, Serialize)]
pub struct InstanceManagerStats {
    /// 总实例数
    pub total_instances: u64,
    /// 活跃函数数
    pub active_functions: u64,
    /// 就绪实例数
    pub ready_instances: u64,
    /// 运行中实例数
    pub running_instances: u64,
    /// 空闲实例数
    pub idle_instances: u64,
    /// 预热中实例数
    pub warming_instances: u64,
    /// 错误实例数
    pub error_instances: u64,
    /// 总执行次数
    pub total_executions: u64,
    /// 成功执行次数
    pub successful_executions: u64,
    /// 失败执行次数
    pub failed_executions: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functions::ScriptType;
    use crate::runtime::compiler::CompilerConfig;
    use crate::runtime::sandbox::SandboxConfig;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_instance_creation() {
        let temp_dir = TempDir::new().unwrap();
        let compiler_config = CompilerConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let compiler = Arc::new(RustCompiler::new(compiler_config).unwrap());
        let sandbox = Arc::new(SandboxExecutor::new(SandboxConfig::default()).unwrap());
        let resource_manager = Arc::new(ResourceManager::new());

        let config = InstanceConfig {
            enable_auto_warm: false, // 禁用自动预热
            ..Default::default()
        };
        let manager = InstanceManager::new(compiler, sandbox, resource_manager, Some(config));

        let function_metadata = FunctionMetadata {
            id: scru128::new(),
            name: "test_function".to_string(),
            description: "Test function".to_string(),
            code: "fn test_function() -> i32 { 42 }".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            timeout_ms: 5000,
            version: "1.0.0".to_string(),
            dependencies: vec![],
            parameters: vec![],
            return_type: crate::functions::ReturnType::Integer,
            script_type: ScriptType::Rust,
        };

        let instance_id = manager
            .create_instance(function_metadata, None)
            .await
            .unwrap();
        assert!(!instance_id.is_empty());

        let instance = manager.get_instance(&instance_id).await.unwrap();
        assert_eq!(instance.function_name, "test_function");
        assert_eq!(instance.instance_id, instance_id);
    }

    #[tokio::test]
    async fn test_instance_stats() {
        let temp_dir = TempDir::new().unwrap();
        let compiler_config = CompilerConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let compiler = Arc::new(RustCompiler::new(compiler_config).unwrap());
        let sandbox = Arc::new(SandboxExecutor::new(SandboxConfig::default()).unwrap());
        let resource_manager = Arc::new(ResourceManager::new());

        let manager = InstanceManager::new(compiler, sandbox, resource_manager, None);

        let stats = manager.get_instance_stats().await;
        assert_eq!(stats.total_instances, 0);
        assert_eq!(stats.active_functions, 0);
    }
}
