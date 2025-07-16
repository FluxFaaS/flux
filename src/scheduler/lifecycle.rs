use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, sleep};

use crate::functions::{FunctionMetadata, InvokeRequest, InvokeResponse};
use crate::runtime::instance::{InstanceManager, InstanceState};

/// 生命周期管理器配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LifecycleConfig {
    /// 预热配置
    pub warmup_config: WarmupConfig,
    /// 闲置管理配置
    pub idle_config: IdleConfig,
    /// 缓存配置
    pub cache_config: CacheConfig,
    /// 清理配置
    pub cleanup_config: CleanupConfig,
    /// 监控配置
    pub monitoring_config: LifecycleMonitoringConfig,
}

/// 预热配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupConfig {
    /// 是否启用预热
    pub enabled: bool,
    /// 预热实例数量
    pub warmup_count: u32,
    /// 预热超时时间（秒）
    pub warmup_timeout_secs: u64,
    /// 预热间隔（毫秒）
    pub warmup_interval_ms: u64,
    /// 预热策略
    pub warmup_strategy: WarmupStrategy,
    /// 预热触发条件
    pub warmup_trigger: WarmupTrigger,
}

impl Default for WarmupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            warmup_count: 2,
            warmup_timeout_secs: 30,
            warmup_interval_ms: 100,
            warmup_strategy: WarmupStrategy::Eager,
            warmup_trigger: WarmupTrigger::OnDemand,
        }
    }
}

/// 预热策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarmupStrategy {
    /// 立即预热
    Eager,
    /// 延迟预热
    Lazy,
    /// 智能预热
    Smart,
    /// 批量预热
    Batch,
}

/// 预热触发条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarmupTrigger {
    /// 按需预热
    OnDemand,
    /// 定时预热
    Scheduled,
    /// 负载预热
    LoadBased,
    /// 预测预热
    Predictive,
}

/// 闲置管理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleConfig {
    /// 闲置超时时间（秒）
    pub idle_timeout_secs: u64,
    /// 最大闲置实例数
    pub max_idle_instances: u32,
    /// 闲置检查间隔（秒）
    pub idle_check_interval_secs: u64,
    /// 闲置处理策略
    pub idle_strategy: IdleStrategy,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 300, // 5分钟
            max_idle_instances: 5,
            idle_check_interval_secs: 60, // 1分钟
            idle_strategy: IdleStrategy::Terminate,
        }
    }
}

/// 闲置处理策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdleStrategy {
    /// 终止实例
    Terminate,
    /// 暂停实例
    Suspend,
    /// 保持实例
    Keep,
    /// 智能处理
    Smart,
}

/// 缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 是否启用缓存
    pub enabled: bool,
    /// 缓存大小限制
    pub max_cache_size: usize,
    /// 缓存过期时间（秒）
    pub cache_ttl_secs: u64,
    /// 缓存策略
    pub cache_strategy: CacheStrategy,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_cache_size: 100,
            cache_ttl_secs: 3600, // 1小时
            cache_strategy: CacheStrategy::Lru,
        }
    }
}

/// 缓存策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStrategy {
    /// 最近最少使用
    Lru,
    /// 先进先出
    Fifo,
    /// 最不常用
    Lfu,
    /// 时间过期
    Ttl,
}

/// 清理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    /// 清理间隔（秒）
    pub cleanup_interval_secs: u64,
    /// 强制清理超时（秒）
    pub force_cleanup_timeout_secs: u64,
    /// 清理策略
    pub cleanup_strategy: CleanupStrategy,
    /// 是否启用优雅关闭
    pub graceful_shutdown: bool,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            cleanup_interval_secs: 300, // 5分钟
            force_cleanup_timeout_secs: 30,
            cleanup_strategy: CleanupStrategy::Graceful,
            graceful_shutdown: true,
        }
    }
}

/// 清理策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupStrategy {
    /// 优雅清理
    Graceful,
    /// 强制清理
    Force,
    /// 智能清理
    Smart,
}

/// 生命周期监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleMonitoringConfig {
    /// 是否启用监控
    pub enabled: bool,
    /// 监控间隔（秒）
    pub monitoring_interval_secs: u64,
    /// 保留监控数据时间（秒）
    pub retention_secs: u64,
}

impl Default for LifecycleMonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            monitoring_interval_secs: 10,
            retention_secs: 86400, // 24小时
        }
    }
}

/// 生命周期阶段
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LifecyclePhase {
    /// 创建中
    Creating,
    /// 预热中
    Warming,
    /// 就绪
    Ready,
    /// 执行中
    Executing,
    /// 闲置
    Idle,
    /// 暂停
    Suspended,
    /// 清理中
    Cleaning,
    /// 已终止
    Terminated,
    /// 错误状态
    Error(String),
}

/// 生命周期事件
#[derive(Debug, Clone, Serialize)]
pub struct LifecycleEvent {
    /// 事件ID
    pub event_id: String,
    /// 实例ID
    pub instance_id: String,
    /// 函数名
    pub function_name: String,
    /// 事件类型
    pub event_type: LifecycleEventType,
    /// 事件时间
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 阶段转换
    pub phase_transition: Option<(LifecyclePhase, LifecyclePhase)>,
    /// 事件数据
    pub data: HashMap<String, serde_json::Value>,
    /// 耗时（毫秒）
    pub duration_ms: Option<u64>,
}

/// 生命周期事件类型
#[derive(Debug, Clone, Serialize)]
pub enum LifecycleEventType {
    /// 实例创建
    InstanceCreated,
    /// 实例预热开始
    WarmupStarted,
    /// 实例预热完成
    WarmupCompleted,
    /// 实例预热失败
    WarmupFailed,
    /// 实例就绪
    InstanceReady,
    /// 执行开始
    ExecutionStarted,
    /// 执行完成
    ExecutionCompleted,
    /// 执行失败
    ExecutionFailed,
    /// 实例闲置
    InstanceIdle,
    /// 实例暂停
    InstanceSuspended,
    /// 实例恢复
    InstanceResumed,
    /// 清理开始
    CleanupStarted,
    /// 清理完成
    CleanupCompleted,
    /// 实例终止
    InstanceTerminated,
    /// 生命周期错误
    LifecycleError,
}

/// 生命周期统计
#[derive(Debug, Clone, Default, Serialize)]
pub struct LifecycleStatistics {
    /// 总实例数
    pub total_instances: u64,
    /// 活跃实例数
    pub active_instances: u64,
    /// 闲置实例数
    pub idle_instances: u64,
    /// 预热实例数
    pub warming_instances: u64,
    /// 平均创建时间（毫秒）
    pub avg_creation_time_ms: f64,
    /// 平均预热时间（毫秒）
    pub avg_warmup_time_ms: f64,
    /// 平均执行时间（毫秒）
    pub avg_execution_time_ms: f64,
    /// 平均生命周期时间（毫秒）
    pub avg_lifecycle_time_ms: f64,
    /// 成功率
    pub success_rate: f64,
    /// 预热成功率
    pub warmup_success_rate: f64,
    /// 各阶段实例数统计
    pub phase_counts: HashMap<String, u64>,
}

/// 实例生命周期信息
#[derive(Debug, Clone)]
pub struct InstanceLifecycle {
    /// 实例ID
    pub instance_id: String,
    /// 函数名
    pub function_name: String,
    /// 当前阶段
    pub current_phase: LifecyclePhase,
    /// 创建时间
    pub created_at: Instant,
    /// 最后活动时间
    pub last_activity: Instant,
    /// 预热开始时间
    pub warmup_started_at: Option<Instant>,
    /// 预热完成时间
    pub warmup_completed_at: Option<Instant>,
    /// 执行次数
    pub execution_count: u64,
    /// 总执行时间
    pub total_execution_time: Duration,
    /// 生命周期事件历史
    pub event_history: Vec<LifecycleEvent>,
    /// 实例元数据
    pub metadata: HashMap<String, String>,
}

/// 生命周期管理器
#[derive(Debug)]
pub struct LifecycleManager {
    /// 配置
    config: LifecycleConfig,
    /// 实例管理器
    instance_manager: Arc<InstanceManager>,
    /// 生命周期信息
    lifecycles: Arc<RwLock<HashMap<String, InstanceLifecycle>>>,
    /// 统计信息
    statistics: Arc<RwLock<LifecycleStatistics>>,
    /// 事件历史
    event_history: Arc<RwLock<Vec<LifecycleEvent>>>,
    /// 预热队列
    warmup_queue: Arc<Mutex<Vec<String>>>,
    /// 清理队列
    cleanup_queue: Arc<Mutex<Vec<String>>>,
    /// 监控任务句柄
    monitoring_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 清理任务句柄
    cleanup_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl LifecycleManager {
    /// 创建新的生命周期管理器
    pub fn new(config: LifecycleConfig, instance_manager: Arc<InstanceManager>) -> Self {
        Self {
            config,
            instance_manager,
            lifecycles: Arc::new(RwLock::new(HashMap::new())),
            statistics: Arc::new(RwLock::new(LifecycleStatistics::default())),
            event_history: Arc::new(RwLock::new(Vec::new())),
            warmup_queue: Arc::new(Mutex::new(Vec::new())),
            cleanup_queue: Arc::new(Mutex::new(Vec::new())),
            monitoring_handle: Arc::new(Mutex::new(None)),
            cleanup_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// 启动生命周期管理器
    pub async fn start(&self) -> Result<()> {
        // 启动监控任务
        if self.config.monitoring_config.enabled {
            self.start_monitoring().await;
        }

        // 启动清理任务
        self.start_cleanup().await;

        tracing::info!("Lifecycle manager started");
        Ok(())
    }

    /// 停止生命周期管理器
    pub async fn stop(&self) -> Result<()> {
        // 停止监控任务
        if let Some(handle) = self.monitoring_handle.lock().await.take() {
            handle.abort();
        }

        // 停止清理任务
        if let Some(handle) = self.cleanup_handle.lock().await.take() {
            handle.abort();
        }

        // 清理所有实例
        self.cleanup_all_instances().await?;

        tracing::info!("Lifecycle manager stopped");
        Ok(())
    }

    /// 创建实例
    pub async fn create_instance(&self, function_metadata: FunctionMetadata) -> Result<String> {
        let start_time = Instant::now();

        // 创建实例
        let instance_id = self
            .instance_manager
            .create_instance(function_metadata.clone(), None)
            .await?;

        // 初始化生命周期信息
        let lifecycle = InstanceLifecycle {
            instance_id: instance_id.clone(),
            function_name: function_metadata.name.clone(),
            current_phase: LifecyclePhase::Creating,
            created_at: start_time,
            last_activity: start_time,
            warmup_started_at: None,
            warmup_completed_at: None,
            execution_count: 0,
            total_execution_time: Duration::ZERO,
            event_history: Vec::new(),
            metadata: HashMap::new(),
        };

        // 保存生命周期信息
        {
            let mut lifecycles = self.lifecycles.write().await;
            lifecycles.insert(instance_id.clone(), lifecycle);
        }

        // 记录创建事件
        self.emit_lifecycle_event(
            &instance_id,
            &function_metadata.name,
            LifecycleEventType::InstanceCreated,
            None,
            HashMap::new(),
            Some(start_time.elapsed().as_millis() as u64),
        )
        .await;

        // 更新阶段为就绪
        self.update_phase(&instance_id, LifecyclePhase::Ready)
            .await?;

        // 如果启用预热，添加到预热队列
        if self.config.warmup_config.enabled {
            let mut warmup_queue = self.warmup_queue.lock().await;
            warmup_queue.push(instance_id.clone());
        }

        tracing::info!("Instance created: {}", instance_id);
        Ok(instance_id)
    }

    /// 预热实例
    pub async fn warmup_instance(&self, instance_id: &str) -> Result<()> {
        let start_time = Instant::now();

        // 更新阶段为预热中
        self.update_phase(instance_id, LifecyclePhase::Warming)
            .await?;

        // 记录预热开始事件
        self.emit_lifecycle_event(
            instance_id,
            &self.get_function_name(instance_id).await?,
            LifecycleEventType::WarmupStarted,
            None,
            HashMap::new(),
            None,
        )
        .await;

        // 更新预热开始时间
        {
            let mut lifecycles = self.lifecycles.write().await;
            if let Some(lifecycle) = lifecycles.get_mut(instance_id) {
                lifecycle.warmup_started_at = Some(start_time);
            }
        }

        // 执行预热逻辑
        match self.perform_warmup(instance_id).await {
            Ok(()) => {
                // 预热成功
                let warmup_time = start_time.elapsed();

                // 更新预热完成时间
                {
                    let mut lifecycles = self.lifecycles.write().await;
                    if let Some(lifecycle) = lifecycles.get_mut(instance_id) {
                        lifecycle.warmup_completed_at = Some(Instant::now());
                    }
                }

                // 记录预热完成事件
                self.emit_lifecycle_event(
                    instance_id,
                    &self.get_function_name(instance_id).await?,
                    LifecycleEventType::WarmupCompleted,
                    None,
                    HashMap::new(),
                    Some(warmup_time.as_millis() as u64),
                )
                .await;

                // 更新阶段为就绪
                self.update_phase(instance_id, LifecyclePhase::Ready)
                    .await?;

                tracing::info!(
                    "Instance warmed up: {} ({}ms)",
                    instance_id,
                    warmup_time.as_millis()
                );
                Ok(())
            }
            Err(e) => {
                // 预热失败
                let warmup_time = start_time.elapsed();

                // 记录预热失败事件
                self.emit_lifecycle_event(
                    instance_id,
                    &self.get_function_name(instance_id).await?,
                    LifecycleEventType::WarmupFailed,
                    None,
                    [(
                        "error".to_string(),
                        serde_json::Value::String(e.to_string()),
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    Some(warmup_time.as_millis() as u64),
                )
                .await;

                // 更新阶段为错误
                self.update_phase(instance_id, LifecyclePhase::Error(e.to_string()))
                    .await?;

                tracing::error!("Instance warmup failed: {} - {}", instance_id, e);
                Err(e)
            }
        }
    }

    /// 执行函数
    pub async fn execute_instance(
        &self,
        instance_id: &str,
        request: &InvokeRequest,
    ) -> Result<InvokeResponse> {
        let start_time = Instant::now();

        // 更新阶段为执行中
        self.update_phase(instance_id, LifecyclePhase::Executing)
            .await?;

        // 记录执行开始事件
        self.emit_lifecycle_event(
            instance_id,
            &self.get_function_name(instance_id).await?,
            LifecycleEventType::ExecutionStarted,
            None,
            HashMap::new(),
            None,
        )
        .await;

        // 执行函数
        let result = self
            .instance_manager
            .execute_instance(instance_id, request)
            .await;
        let execution_time = start_time.elapsed();

        // 更新执行统计
        {
            let mut lifecycles = self.lifecycles.write().await;
            if let Some(lifecycle) = lifecycles.get_mut(instance_id) {
                lifecycle.execution_count += 1;
                lifecycle.total_execution_time += execution_time;
                lifecycle.last_activity = Instant::now();
            }
        }

        match result {
            Ok(response) => {
                // 执行成功
                self.emit_lifecycle_event(
                    instance_id,
                    &self.get_function_name(instance_id).await?,
                    LifecycleEventType::ExecutionCompleted,
                    None,
                    HashMap::new(),
                    Some(execution_time.as_millis() as u64),
                )
                .await;

                // 更新阶段为就绪
                self.update_phase(instance_id, LifecyclePhase::Ready)
                    .await?;

                Ok(response)
            }
            Err(e) => {
                // 执行失败
                self.emit_lifecycle_event(
                    instance_id,
                    &self.get_function_name(instance_id).await?,
                    LifecycleEventType::ExecutionFailed,
                    None,
                    [(
                        "error".to_string(),
                        serde_json::Value::String(e.to_string()),
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    Some(execution_time.as_millis() as u64),
                )
                .await;

                // 更新阶段为错误
                self.update_phase(instance_id, LifecyclePhase::Error(e.to_string()))
                    .await?;

                Err(e)
            }
        }
    }

    /// 标记实例为闲置
    pub async fn mark_instance_idle(&self, instance_id: &str) -> Result<()> {
        self.update_phase(instance_id, LifecyclePhase::Idle).await?;

        self.emit_lifecycle_event(
            instance_id,
            &self.get_function_name(instance_id).await?,
            LifecycleEventType::InstanceIdle,
            None,
            HashMap::new(),
            None,
        )
        .await;

        tracing::debug!("Instance marked as idle: {}", instance_id);
        Ok(())
    }

    /// 终止实例
    pub async fn terminate_instance(&self, instance_id: &str) -> Result<()> {
        let start_time = Instant::now();

        // 更新阶段为清理中
        self.update_phase(instance_id, LifecyclePhase::Cleaning)
            .await?;

        // 记录清理开始事件
        self.emit_lifecycle_event(
            instance_id,
            &self.get_function_name(instance_id).await?,
            LifecycleEventType::CleanupStarted,
            None,
            HashMap::new(),
            None,
        )
        .await;

        // 停止实例
        match self.instance_manager.stop_instance(instance_id).await {
            Ok(()) => {
                let cleanup_time = start_time.elapsed();

                // 记录清理完成事件
                self.emit_lifecycle_event(
                    instance_id,
                    &self.get_function_name(instance_id).await?,
                    LifecycleEventType::CleanupCompleted,
                    None,
                    HashMap::new(),
                    Some(cleanup_time.as_millis() as u64),
                )
                .await;

                // 更新阶段为已终止
                self.update_phase(instance_id, LifecyclePhase::Terminated)
                    .await?;

                // 记录终止事件
                self.emit_lifecycle_event(
                    instance_id,
                    &self.get_function_name(instance_id).await?,
                    LifecycleEventType::InstanceTerminated,
                    None,
                    HashMap::new(),
                    None,
                )
                .await;

                // 移除生命周期信息
                {
                    let mut lifecycles = self.lifecycles.write().await;
                    lifecycles.remove(instance_id);
                }

                tracing::info!(
                    "Instance terminated: {} ({}ms)",
                    instance_id,
                    cleanup_time.as_millis()
                );
                Ok(())
            }
            Err(e) => {
                // 终止失败
                self.emit_lifecycle_event(
                    instance_id,
                    &self.get_function_name(instance_id).await?,
                    LifecycleEventType::LifecycleError,
                    None,
                    [(
                        "error".to_string(),
                        serde_json::Value::String(e.to_string()),
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    None,
                )
                .await;

                tracing::error!("Failed to terminate instance: {} - {}", instance_id, e);
                Err(e)
            }
        }
    }

    /// 获取生命周期统计信息
    pub async fn get_statistics(&self) -> LifecycleStatistics {
        let mut stats = self.statistics.read().await.clone();

        // 实时计算统计信息
        let lifecycles = self.lifecycles.read().await;
        stats.total_instances = lifecycles.len() as u64;

        // 统计各阶段实例数
        stats.phase_counts.clear();
        for lifecycle in lifecycles.values() {
            let phase_name = match &lifecycle.current_phase {
                LifecyclePhase::Creating => "creating",
                LifecyclePhase::Warming => "warming",
                LifecyclePhase::Ready => "ready",
                LifecyclePhase::Executing => "executing",
                LifecyclePhase::Idle => "idle",
                LifecyclePhase::Suspended => "suspended",
                LifecyclePhase::Cleaning => "cleaning",
                LifecyclePhase::Terminated => "terminated",
                LifecyclePhase::Error(_) => "error",
            };
            *stats
                .phase_counts
                .entry(phase_name.to_string())
                .or_insert(0) += 1;
        }

        stats.active_instances = stats.phase_counts.get("executing").unwrap_or(&0)
            + stats.phase_counts.get("ready").unwrap_or(&0);
        stats.idle_instances = *stats.phase_counts.get("idle").unwrap_or(&0);
        stats.warming_instances = *stats.phase_counts.get("warming").unwrap_or(&0);

        stats
    }

    /// 获取实例生命周期信息
    pub async fn get_instance_lifecycle(&self, instance_id: &str) -> Option<InstanceLifecycle> {
        let lifecycles = self.lifecycles.read().await;
        lifecycles.get(instance_id).cloned()
    }

    /// 获取所有实例生命周期信息
    pub async fn get_all_lifecycles(&self) -> Vec<InstanceLifecycle> {
        let lifecycles = self.lifecycles.read().await;
        lifecycles.values().cloned().collect()
    }

    /// 获取生命周期事件历史
    pub async fn get_event_history(&self, limit: Option<usize>) -> Vec<LifecycleEvent> {
        let event_history = self.event_history.read().await;
        if let Some(limit) = limit {
            event_history.iter().rev().take(limit).cloned().collect()
        } else {
            event_history.clone()
        }
    }

    /// 更新阶段
    async fn update_phase(&self, instance_id: &str, new_phase: LifecyclePhase) -> Result<()> {
        let mut lifecycles = self.lifecycles.write().await;
        if let Some(lifecycle) = lifecycles.get_mut(instance_id) {
            let old_phase = lifecycle.current_phase.clone();
            lifecycle.current_phase = new_phase.clone();
            lifecycle.last_activity = Instant::now();

            tracing::debug!(
                "Instance {} phase changed: {:?} -> {:?}",
                instance_id,
                old_phase,
                new_phase
            );
        }
        Ok(())
    }

    /// 获取函数名
    async fn get_function_name(&self, instance_id: &str) -> Result<String> {
        let lifecycles = self.lifecycles.read().await;
        if let Some(lifecycle) = lifecycles.get(instance_id) {
            Ok(lifecycle.function_name.clone())
        } else {
            Err(anyhow::anyhow!("Instance not found: {}", instance_id))
        }
    }

    /// 发出生命周期事件
    async fn emit_lifecycle_event(
        &self,
        instance_id: &str,
        function_name: &str,
        event_type: LifecycleEventType,
        phase_transition: Option<(LifecyclePhase, LifecyclePhase)>,
        data: HashMap<String, serde_json::Value>,
        duration_ms: Option<u64>,
    ) {
        let event = LifecycleEvent {
            event_id: scru128::new_string(),
            instance_id: instance_id.to_string(),
            function_name: function_name.to_string(),
            event_type,
            timestamp: chrono::Utc::now(),
            phase_transition,
            data,
            duration_ms,
        };

        // 添加到实例事件历史
        {
            let mut lifecycles = self.lifecycles.write().await;
            if let Some(lifecycle) = lifecycles.get_mut(instance_id) {
                lifecycle.event_history.push(event.clone());
            }
        }

        // 添加到全局事件历史
        {
            let mut event_history = self.event_history.write().await;
            event_history.push(event);

            // 限制历史记录数量
            if event_history.len() > 10000 {
                event_history.drain(0..1000);
            }
        }
    }

    /// 执行预热
    async fn perform_warmup(&self, instance_id: &str) -> Result<()> {
        // 这里可以实现具体的预热逻辑
        // 例如：预编译、预加载资源、建立连接等

        // 模拟预热过程
        sleep(Duration::from_millis(
            self.config.warmup_config.warmup_interval_ms,
        ))
        .await;

        // 检查实例状态
        if let Some(instance) = self.instance_manager.get_instance(instance_id).await {
            match instance.state {
                InstanceState::Ready => Ok(()),
                InstanceState::Error(e) => Err(anyhow::anyhow!("Instance in error state: {}", e)),
                _ => Err(anyhow::anyhow!("Instance not ready for warmup")),
            }
        } else {
            Err(anyhow::anyhow!("Instance not found: {}", instance_id))
        }
    }

    /// 启动监控任务
    async fn start_monitoring(&self) {
        let lifecycles = self.lifecycles.clone();
        let statistics = self.statistics.clone();
        let config = self.config.clone();

        let monitoring_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(
                config.monitoring_config.monitoring_interval_secs,
            ));

            loop {
                interval.tick().await;

                // 更新统计信息
                let lifecycles_guard = lifecycles.read().await;
                let mut stats = statistics.write().await;

                // 计算各种统计指标
                let total_instances = lifecycles_guard.len() as u64;
                let mut total_execution_time = 0u64;
                let mut total_warmup_time = 0u64;
                let mut total_lifecycle_time = 0u64;
                let mut successful_executions = 0u64;
                let mut successful_warmups = 0u64;

                for lifecycle in lifecycles_guard.values() {
                    if lifecycle.execution_count > 0 {
                        total_execution_time += lifecycle.total_execution_time.as_millis() as u64;
                        successful_executions += lifecycle.execution_count;
                    }

                    if let Some(warmup_completed) = lifecycle.warmup_completed_at {
                        if let Some(warmup_started) = lifecycle.warmup_started_at {
                            total_warmup_time +=
                                warmup_completed.duration_since(warmup_started).as_millis() as u64;
                            successful_warmups += 1;
                        }
                    }

                    total_lifecycle_time += lifecycle.created_at.elapsed().as_millis() as u64;
                }

                if total_instances > 0 {
                    stats.avg_lifecycle_time_ms =
                        total_lifecycle_time as f64 / total_instances as f64;
                }

                if successful_executions > 0 {
                    stats.avg_execution_time_ms =
                        total_execution_time as f64 / successful_executions as f64;
                }

                if successful_warmups > 0 {
                    stats.avg_warmup_time_ms = total_warmup_time as f64 / successful_warmups as f64;
                }

                // 更新成功率
                stats.success_rate = if total_instances > 0 {
                    successful_executions as f64 / total_instances as f64
                } else {
                    0.0
                };

                stats.warmup_success_rate = if total_instances > 0 {
                    successful_warmups as f64 / total_instances as f64
                } else {
                    0.0
                };
            }
        });

        let mut handle = self.monitoring_handle.lock().await;
        *handle = Some(monitoring_task);
    }

    /// 启动清理任务
    async fn start_cleanup(&self) {
        let lifecycles = self.lifecycles.clone();
        let cleanup_queue = self.cleanup_queue.clone();
        let config = self.config.clone();
        let instance_manager = self.instance_manager.clone();

        let cleanup_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(
                config.cleanup_config.cleanup_interval_secs,
            ));

            loop {
                interval.tick().await;

                // 检查闲置实例
                let idle_instances = {
                    let lifecycles_guard = lifecycles.read().await;
                    let now = Instant::now();
                    let idle_timeout = Duration::from_secs(config.idle_config.idle_timeout_secs);

                    lifecycles_guard
                        .iter()
                        .filter(|(_, lifecycle)| {
                            matches!(
                                lifecycle.current_phase,
                                LifecyclePhase::Idle | LifecyclePhase::Ready
                            ) && now.duration_since(lifecycle.last_activity) > idle_timeout
                        })
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>()
                };

                // 添加到清理队列
                {
                    let mut cleanup_queue_guard = cleanup_queue.lock().await;
                    cleanup_queue_guard.extend(idle_instances);
                }

                // 处理清理队列
                let instances_to_cleanup = {
                    let mut cleanup_queue_guard = cleanup_queue.lock().await;
                    cleanup_queue_guard.drain(..).collect::<Vec<_>>()
                };

                for instance_id in instances_to_cleanup {
                    if let Err(e) = instance_manager.stop_instance(&instance_id).await {
                        tracing::warn!("Failed to cleanup instance {}: {}", instance_id, e);
                    } else {
                        tracing::info!("Cleaned up idle instance: {}", instance_id);
                    }
                }
            }
        });

        let mut handle = self.cleanup_handle.lock().await;
        *handle = Some(cleanup_task);
    }

    /// 清理所有实例
    async fn cleanup_all_instances(&self) -> Result<()> {
        let instance_ids = {
            let lifecycles = self.lifecycles.read().await;
            lifecycles.keys().cloned().collect::<Vec<_>>()
        };

        for instance_id in instance_ids {
            if let Err(e) = self.terminate_instance(&instance_id).await {
                tracing::warn!(
                    "Failed to terminate instance during cleanup: {} - {}",
                    instance_id,
                    e
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::compiler::RustCompiler;
    use crate::runtime::resource::ResourceManager;
    use crate::runtime::sandbox::SandboxExecutor;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_lifecycle_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let compiler_config = crate::runtime::compiler::CompilerConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let compiler = Arc::new(RustCompiler::new(compiler_config).unwrap());
        let sandbox = Arc::new(SandboxExecutor::new(Default::default()).unwrap());
        let resource_manager = Arc::new(ResourceManager::new());

        let instance_manager = Arc::new(InstanceManager::new(
            compiler,
            sandbox,
            resource_manager,
            None,
        ));

        let config = LifecycleConfig::default();
        let lifecycle_manager = LifecycleManager::new(config, instance_manager);

        let stats = lifecycle_manager.get_statistics().await;
        assert_eq!(stats.total_instances, 0);
    }

    #[tokio::test]
    async fn test_instance_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let compiler_config = crate::runtime::compiler::CompilerConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let compiler = Arc::new(RustCompiler::new(compiler_config).unwrap());
        let sandbox = Arc::new(SandboxExecutor::new(Default::default()).unwrap());
        let resource_manager = Arc::new(ResourceManager::new());

        let instance_manager = Arc::new(InstanceManager::new(
            compiler,
            sandbox,
            resource_manager,
            None,
        ));

        let config = LifecycleConfig::default();
        let lifecycle_manager = LifecycleManager::new(config, instance_manager);

        let function_metadata = FunctionMetadata {
            id: scru128::new(),
            name: "test_function".to_string(),
            version: "1.0.0".to_string(),
            description: "Test function".to_string(),
            code: "fn main() { println!(\"Hello, World!\"); }".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            timeout_ms: 5000,
            dependencies: vec![],
            parameters: vec![],
            return_type: "()".to_string(),
        };

        // 创建实例
        let instance_id = lifecycle_manager
            .create_instance(function_metadata)
            .await
            .unwrap();
        assert!(!instance_id.is_empty());

        // 检查生命周期信息
        let lifecycle = lifecycle_manager
            .get_instance_lifecycle(&instance_id)
            .await
            .unwrap();
        assert_eq!(lifecycle.current_phase, LifecyclePhase::Ready);
        assert_eq!(lifecycle.execution_count, 0);

        // 获取统计信息
        let stats = lifecycle_manager.get_statistics().await;
        assert_eq!(stats.total_instances, 1);
        assert_eq!(stats.active_instances, 1);
    }
}
