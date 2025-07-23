use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;

use crate::functions::{FunctionMetadata, InvokeRequest, InvokeResponse};
use crate::runtime::instance::{InstanceConfig, InstanceManager, InstanceState};

/// 实例池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// 最小实例数
    pub min_instances: u32,
    /// 最大实例数
    pub max_instances: u32,
    /// 目标实例数
    pub target_instances: u32,
    /// 扩容阈值（百分比）
    pub scale_up_threshold: f64,
    /// 缩容阈值（百分比）
    pub scale_down_threshold: f64,
    /// 扩容冷却时间（秒）
    pub scale_up_cooldown_secs: u64,
    /// 缩容冷却时间（秒）
    pub scale_down_cooldown_secs: u64,
    /// 预热新实例
    pub warm_new_instances: bool,
    /// 健康检查间隔（秒）
    pub health_check_interval_secs: u64,
    /// 负载均衡策略
    pub load_balance_strategy: LoadBalanceStrategy,
    /// 实例配置
    pub instance_config: InstanceConfig,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_instances: 1,
            max_instances: 10,
            target_instances: 2,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.3,
            scale_up_cooldown_secs: 60,
            scale_down_cooldown_secs: 300,
            warm_new_instances: true,
            health_check_interval_secs: 30,
            load_balance_strategy: LoadBalanceStrategy::RoundRobin,
            instance_config: InstanceConfig::default(),
        }
    }
}

/// 负载均衡策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalanceStrategy {
    /// 轮询
    RoundRobin,
    /// 随机
    Random,
    /// 最少连接
    LeastConnections,
    /// 最少负载
    LeastLoad,
    /// 响应时间最短
    FastestResponse,
}

/// 实例池状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PoolState {
    /// 初始化中
    Initializing,
    /// 运行中
    Running,
    /// 扩容中
    ScalingUp,
    /// 缩容中
    ScalingDown,
    /// 暂停
    Paused,
    /// 停止中
    Stopping,
    /// 已停止
    Stopped,
    /// 错误状态
    Error(String),
}

/// 池实例信息
#[derive(Debug, Clone)]
pub struct PoolInstance {
    /// 实例ID
    pub instance_id: String,
    /// 当前负载（0.0-1.0）
    pub current_load: f64,
    /// 活跃连接数
    pub active_connections: u32,
    /// 最后活动时间
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// 健康状态
    pub is_healthy: bool,
    /// 实例创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 函数实例池
#[derive(Debug)]
pub struct FunctionPool {
    /// 函数元数据
    function_metadata: FunctionMetadata,
    /// 池配置
    config: PoolConfig,
    /// 池状态
    state: Arc<RwLock<PoolState>>,
    /// 池中的实例
    instances: Arc<RwLock<HashMap<String, PoolInstance>>>,
    /// 实例管理器
    instance_manager: Arc<InstanceManager>,
    /// 负载均衡器状态
    load_balancer_state: Arc<RwLock<LoadBalancerState>>,
    /// 扩缩容历史
    scaling_history: Arc<RwLock<VecDeque<ScalingEvent>>>,
    /// 最后扩容时间
    last_scale_up: Arc<RwLock<Option<Instant>>>,
    /// 最后缩容时间
    last_scale_down: Arc<RwLock<Option<Instant>>>,
    /// 健康检查任务句柄
    health_check_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 自动扩缩容任务句柄
    auto_scaling_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// 负载均衡器状态
#[derive(Debug, Clone, Default)]
pub struct LoadBalancerState {
    /// 轮询计数器
    pub round_robin_counter: usize,
    /// 随机种子
    pub random_seed: u64,
}

/// 扩缩容事件
#[derive(Debug, Clone, Serialize)]
pub struct ScalingEvent {
    /// 事件ID
    pub event_id: String,
    /// 事件类型
    pub event_type: ScalingEventType,
    /// 事件时间
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 扩缩容前实例数
    pub before_count: u32,
    /// 扩缩容后实例数
    pub after_count: u32,
    /// 触发原因
    pub reason: String,
    /// 相关指标
    pub metrics: HashMap<String, f64>,
}

/// 扩缩容事件类型
#[derive(Debug, Clone, Serialize)]
pub enum ScalingEventType {
    /// 扩容
    ScaleUp,
    /// 缩容
    ScaleDown,
    /// 手动调整
    ManualScale,
    /// 健康检查触发
    HealthCheck,
}

/// 池执行统计
#[derive(Debug, Clone, Default, Serialize)]
pub struct PoolExecutionStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// 最大响应时间（毫秒）
    pub max_response_time_ms: u64,
    /// 最小响应时间（毫秒）
    pub min_response_time_ms: u64,
    /// 当前负载
    pub current_load: f64,
    /// 活跃连接数
    pub active_connections: u32,
    /// 健康实例数
    pub healthy_instances: u32,
    /// 总实例数
    pub total_instances: u32,
}

impl FunctionPool {
    /// 创建新的函数实例池
    pub async fn new(
        function_metadata: FunctionMetadata,
        config: PoolConfig,
        instance_manager: Arc<InstanceManager>,
    ) -> Result<Self> {
        let pool = Self {
            function_metadata,
            config,
            state: Arc::new(RwLock::new(PoolState::Initializing)),
            instances: Arc::new(RwLock::new(HashMap::new())),
            instance_manager,
            load_balancer_state: Arc::new(RwLock::new(LoadBalancerState::default())),
            scaling_history: Arc::new(RwLock::new(VecDeque::new())),
            last_scale_up: Arc::new(RwLock::new(None)),
            last_scale_down: Arc::new(RwLock::new(None)),
            health_check_handle: Arc::new(Mutex::new(None)),
            auto_scaling_handle: Arc::new(Mutex::new(None)),
        };

        // 初始化池
        pool.initialize().await?;

        Ok(pool)
    }

    /// 初始化实例池
    async fn initialize(&self) -> Result<()> {
        tracing::info!(
            "Initializing function pool for: {} (target: {} instances)",
            self.function_metadata.name,
            self.config.target_instances
        );

        // 创建初始实例
        for i in 0..self.config.target_instances {
            let instance_id = self
                .instance_manager
                .create_instance(
                    self.function_metadata.clone(),
                    Some(self.config.instance_config.clone()),
                )
                .await?;

            let pool_instance = PoolInstance {
                instance_id: instance_id.clone(),
                current_load: 0.0,
                active_connections: 0,
                last_activity: chrono::Utc::now(),
                avg_response_time_ms: 0.0,
                is_healthy: true,
                created_at: chrono::Utc::now(),
            };

            let mut instances = self.instances.write().await;
            instances.insert(instance_id, pool_instance);

            tracing::info!(
                "Created initial instance {}/{} for function: {}",
                i + 1,
                self.config.target_instances,
                self.function_metadata.name
            );
        }

        // 更新状态为运行中
        {
            let mut state = self.state.write().await;
            *state = PoolState::Running;
        }

        // 启动健康检查
        self.start_health_check().await;

        // 启动自动扩缩容
        self.start_auto_scaling().await;

        tracing::info!(
            "Function pool initialized successfully for: {}",
            self.function_metadata.name
        );

        Ok(())
    }

    /// 执行函数请求
    pub async fn execute(&self, request: &InvokeRequest) -> Result<InvokeResponse> {
        let start_time = Instant::now();

        // 选择实例
        let instance_id = self.select_instance().await?;

        // 更新实例负载
        self.update_instance_load(&instance_id, true).await;

        // 执行请求
        let result = self
            .instance_manager
            .execute_instance(&instance_id, request)
            .await;

        // 更新实例负载
        self.update_instance_load(&instance_id, false).await;

        // 更新统计信息
        let execution_time = start_time.elapsed();
        self.update_execution_stats(&result, execution_time).await;

        result
    }

    /// 选择实例进行负载均衡
    async fn select_instance(&self) -> Result<String> {
        let instances = self.instances.read().await;

        if instances.is_empty() {
            return Err(anyhow::anyhow!("No instances available in pool"));
        }

        let healthy_instances: Vec<_> = instances
            .iter()
            .filter(|(_, instance)| instance.is_healthy)
            .collect();

        if healthy_instances.is_empty() {
            return Err(anyhow::anyhow!("No healthy instances available in pool"));
        }

        let selected_id = match self.config.load_balance_strategy {
            LoadBalanceStrategy::RoundRobin => self.select_round_robin(&healthy_instances).await,
            LoadBalanceStrategy::Random => self.select_random(&healthy_instances).await,
            LoadBalanceStrategy::LeastConnections => {
                self.select_least_connections(&healthy_instances).await
            }
            LoadBalanceStrategy::LeastLoad => self.select_least_load(&healthy_instances).await,
            LoadBalanceStrategy::FastestResponse => {
                self.select_fastest_response(&healthy_instances).await
            }
        };

        Ok(selected_id)
    }

    /// 轮询选择实例
    async fn select_round_robin(&self, instances: &[(&String, &PoolInstance)]) -> String {
        let mut state = self.load_balancer_state.write().await;
        let index = state.round_robin_counter % instances.len();
        state.round_robin_counter = (state.round_robin_counter + 1) % instances.len();
        instances[index].0.clone()
    }

    /// 随机选择实例
    async fn select_random(&self, instances: &[(&String, &PoolInstance)]) -> String {
        let mut state = self.load_balancer_state.write().await;
        // 简单的线性同余生成器
        state.random_seed = (state
            .random_seed
            .wrapping_mul(1103515245)
            .wrapping_add(12345))
            & 0x7fffffff;
        let index = (state.random_seed as usize) % instances.len();
        instances[index].0.clone()
    }

    /// 选择连接数最少的实例
    async fn select_least_connections(&self, instances: &[(&String, &PoolInstance)]) -> String {
        let min_instance = instances
            .iter()
            .min_by_key(|(_, instance)| instance.active_connections)
            .unwrap();
        min_instance.0.clone()
    }

    /// 选择负载最小的实例
    async fn select_least_load(&self, instances: &[(&String, &PoolInstance)]) -> String {
        let min_instance = instances
            .iter()
            .min_by(|(_, a), (_, b)| a.current_load.partial_cmp(&b.current_load).unwrap())
            .unwrap();
        min_instance.0.clone()
    }

    /// 选择响应时间最短的实例
    async fn select_fastest_response(&self, instances: &[(&String, &PoolInstance)]) -> String {
        let fastest_instance = instances
            .iter()
            .min_by(|(_, a), (_, b)| {
                a.avg_response_time_ms
                    .partial_cmp(&b.avg_response_time_ms)
                    .unwrap()
            })
            .unwrap();
        fastest_instance.0.clone()
    }

    /// 更新实例负载
    async fn update_instance_load(&self, instance_id: &str, increment: bool) {
        let mut instances = self.instances.write().await;
        if let Some(instance) = instances.get_mut(instance_id) {
            if increment {
                instance.active_connections += 1;
            } else {
                instance.active_connections = instance.active_connections.saturating_sub(1);
            }
            instance.last_activity = chrono::Utc::now();

            // 简单的负载计算（基于活跃连接数）
            instance.current_load = instance.active_connections as f64 / 10.0; // 假设最大10个并发
        }
    }

    /// 更新执行统计信息
    async fn update_execution_stats(
        &self,
        result: &Result<InvokeResponse>,
        execution_time: Duration,
    ) {
        // 这里可以实现详细的统计信息更新
        // 目前简化处理
        tracing::debug!(
            "Execution completed in {}ms, success: {}",
            execution_time.as_millis(),
            result.is_ok()
        );
    }

    /// 扩容实例池
    pub async fn scale_up(&self, target_count: u32) -> Result<u32> {
        let current_count = self.instances.read().await.len() as u32;

        if target_count <= current_count {
            return Ok(0);
        }

        let add_count = target_count - current_count;
        let max_add = self.config.max_instances - current_count;
        let actual_add = add_count.min(max_add);

        if actual_add == 0 {
            return Ok(0);
        }

        tracing::info!(
            "Scaling up function pool: {} -> {} instances (adding {})",
            current_count,
            current_count + actual_add,
            actual_add
        );

        // 更新状态
        {
            let mut state = self.state.write().await;
            *state = PoolState::ScalingUp;
        }

        let mut created_count = 0;
        for _ in 0..actual_add {
            match self
                .instance_manager
                .create_instance(
                    self.function_metadata.clone(),
                    Some(self.config.instance_config.clone()),
                )
                .await
            {
                Ok(instance_id) => {
                    let pool_instance = PoolInstance {
                        instance_id: instance_id.clone(),
                        current_load: 0.0,
                        active_connections: 0,
                        last_activity: chrono::Utc::now(),
                        avg_response_time_ms: 0.0,
                        is_healthy: true,
                        created_at: chrono::Utc::now(),
                    };

                    let mut instances = self.instances.write().await;
                    instances.insert(instance_id, pool_instance);
                    created_count += 1;
                }
                Err(e) => {
                    tracing::error!("Failed to create instance during scale up: {}", e);
                    break;
                }
            }
        }

        // 记录扩容事件
        self.record_scaling_event(
            ScalingEventType::ScaleUp,
            current_count,
            current_count + created_count,
            "Auto scaling triggered".to_string(),
        )
        .await;

        // 更新最后扩容时间
        {
            let mut last_scale_up = self.last_scale_up.write().await;
            *last_scale_up = Some(Instant::now());
        }

        // 恢复运行状态
        {
            let mut state = self.state.write().await;
            *state = PoolState::Running;
        }

        tracing::info!(
            "Scale up completed: created {} instances for function: {}",
            created_count,
            self.function_metadata.name
        );

        Ok(created_count)
    }

    /// 缩容实例池
    pub async fn scale_down(&self, target_count: u32) -> Result<u32> {
        let current_count = self.instances.read().await.len() as u32;

        if target_count >= current_count {
            return Ok(0);
        }

        let remove_count = current_count - target_count;
        let min_remove = current_count - self.config.min_instances;
        let actual_remove = remove_count.min(min_remove);

        if actual_remove == 0 {
            return Ok(0);
        }

        tracing::info!(
            "Scaling down function pool: {} -> {} instances (removing {})",
            current_count,
            current_count - actual_remove,
            actual_remove
        );

        // 更新状态
        {
            let mut state = self.state.write().await;
            *state = PoolState::ScalingDown;
        }

        // 选择要移除的实例（优先移除负载最低的）
        let instances_to_remove: Vec<String> = {
            let instances = self.instances.read().await;
            let mut sorted_instances: Vec<_> = instances.iter().collect();
            sorted_instances
                .sort_by(|a, b| a.1.current_load.partial_cmp(&b.1.current_load).unwrap());

            sorted_instances
                .iter()
                .take(actual_remove as usize)
                .map(|(id, _)| (*id).clone())
                .collect()
        };

        let mut removed_count = 0;
        for instance_id in instances_to_remove {
            match self.instance_manager.stop_instance(&instance_id).await {
                Ok(_) => {
                    let mut instances = self.instances.write().await;
                    instances.remove(&instance_id);
                    removed_count += 1;
                }
                Err(e) => {
                    tracing::error!("Failed to stop instance during scale down: {}", e);
                }
            }
        }

        // 记录缩容事件
        self.record_scaling_event(
            ScalingEventType::ScaleDown,
            current_count,
            current_count - removed_count,
            "Auto scaling triggered".to_string(),
        )
        .await;

        // 更新最后缩容时间
        {
            let mut last_scale_down = self.last_scale_down.write().await;
            *last_scale_down = Some(Instant::now());
        }

        // 恢复运行状态
        {
            let mut state = self.state.write().await;
            *state = PoolState::Running;
        }

        tracing::info!(
            "Scale down completed: removed {} instances for function: {}",
            removed_count,
            self.function_metadata.name
        );

        Ok(removed_count)
    }

    /// 记录扩缩容事件
    async fn record_scaling_event(
        &self,
        event_type: ScalingEventType,
        before_count: u32,
        after_count: u32,
        reason: String,
    ) {
        let event = ScalingEvent {
            event_id: scru128::new().to_string(),
            event_type,
            timestamp: chrono::Utc::now(),
            before_count,
            after_count,
            reason,
            metrics: HashMap::new(), // 可以添加更多指标
        };

        let mut history = self.scaling_history.write().await;
        history.push_back(event);

        // 限制历史记录大小
        if history.len() > 100 {
            history.pop_front();
        }
    }

    /// 启动健康检查
    async fn start_health_check(&self) {
        let instances = self.instances.clone();
        let instance_manager = self.instance_manager.clone();
        let interval_secs = self.config.health_check_interval_secs;
        let function_name = self.function_metadata.name.clone();

        let health_check_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                let instance_ids: Vec<String> = {
                    let instances_guard = instances.read().await;
                    instances_guard.keys().cloned().collect()
                };

                for instance_id in instance_ids {
                    // 检查实例健康状态
                    if let Some(instance) = instance_manager.get_instance(&instance_id).await {
                        let is_healthy = match instance.state {
                            InstanceState::Ready | InstanceState::Idle => true,
                            InstanceState::Error(_) => false,
                            _ => true, // 其他状态暂时认为是健康的
                        };

                        // 更新健康状态
                        {
                            let mut instances_guard = instances.write().await;
                            if let Some(pool_instance) = instances_guard.get_mut(&instance_id) {
                                pool_instance.is_healthy = is_healthy;
                            }
                        }

                        if !is_healthy {
                            tracing::warn!(
                                "Instance {} of function {} is unhealthy: {:?}",
                                instance_id,
                                function_name,
                                instance.state
                            );
                        }
                    }
                }
            }
        });

        let mut handle = self.health_check_handle.lock().await;
        *handle = Some(health_check_task);
    }

    /// 启动自动扩缩容
    async fn start_auto_scaling(&self) {
        let instances = self.instances.clone();
        let config = self.config.clone();
        let last_scale_up = self.last_scale_up.clone();
        let last_scale_down = self.last_scale_down.clone();
        let function_name = self.function_metadata.name.clone();

        let auto_scaling_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // 每30秒检查一次

            loop {
                interval.tick().await;

                // 计算当前负载
                let (current_load, healthy_count) = {
                    let instances_guard = instances.read().await;
                    let healthy_instances: Vec<_> = instances_guard
                        .values()
                        .filter(|instance| instance.is_healthy)
                        .collect();

                    let total_load: f64 = healthy_instances
                        .iter()
                        .map(|instance| instance.current_load)
                        .sum();

                    let avg_load = if healthy_instances.is_empty() {
                        0.0
                    } else {
                        total_load / healthy_instances.len() as f64
                    };

                    (avg_load, healthy_instances.len() as u32)
                };

                // 检查是否需要扩容
                if current_load > config.scale_up_threshold && healthy_count < config.max_instances
                {
                    let last_scale_up_time = last_scale_up.read().await;
                    let can_scale_up = last_scale_up_time
                        .map(|time| time.elapsed().as_secs() >= config.scale_up_cooldown_secs)
                        .unwrap_or(true);

                    if can_scale_up {
                        let target_count = (healthy_count + 1).min(config.max_instances);
                        tracing::info!(
                            "Auto scaling up function {} from {} to {} instances (load: {:.2})",
                            function_name,
                            healthy_count,
                            target_count,
                            current_load
                        );

                        // 这里暂时跳过实际的扩容操作，因为需要池的引用
                        // 在实际实现中，可以通过消息传递或其他机制来触发扩容
                        tracing::debug!("Auto scaling up would be triggered here");
                    }
                }
                // 检查是否需要缩容
                else if current_load < config.scale_down_threshold
                    && healthy_count > config.min_instances
                {
                    let last_scale_down_time = last_scale_down.read().await;
                    let can_scale_down = last_scale_down_time
                        .map(|time| time.elapsed().as_secs() >= config.scale_down_cooldown_secs)
                        .unwrap_or(true);

                    if can_scale_down {
                        let target_count = (healthy_count - 1).max(config.min_instances);
                        tracing::info!(
                            "Auto scaling down function {} from {} to {} instances (load: {:.2})",
                            function_name,
                            healthy_count,
                            target_count,
                            current_load
                        );

                        // 这里暂时跳过实际的缩容操作，因为需要池的引用
                        // 在实际实现中，可以通过消息传递或其他机制来触发缩容
                        tracing::debug!("Auto scaling down would be triggered here");
                    }
                }
            }
        });

        let mut handle = self.auto_scaling_handle.lock().await;
        *handle = Some(auto_scaling_task);
    }

    /// 获取池统计信息
    pub async fn get_stats(&self) -> PoolExecutionStats {
        let instances = self.instances.read().await;

        let healthy_instances: Vec<_> = instances
            .values()
            .filter(|instance| instance.is_healthy)
            .collect();

        let total_connections: u32 = healthy_instances
            .iter()
            .map(|instance| instance.active_connections)
            .sum();

        let avg_load = if healthy_instances.is_empty() {
            0.0
        } else {
            healthy_instances
                .iter()
                .map(|instance| instance.current_load)
                .sum::<f64>()
                / healthy_instances.len() as f64
        };

        let avg_response_time = if healthy_instances.is_empty() {
            0.0
        } else {
            healthy_instances
                .iter()
                .map(|instance| instance.avg_response_time_ms)
                .sum::<f64>()
                / healthy_instances.len() as f64
        };

        PoolExecutionStats {
            total_requests: 0, // 需要实现请求计数
            successful_requests: 0,
            failed_requests: 0,
            avg_response_time_ms: avg_response_time,
            max_response_time_ms: 0,
            min_response_time_ms: 0,
            current_load: avg_load,
            active_connections: total_connections,
            healthy_instances: healthy_instances.len() as u32,
            total_instances: instances.len() as u32,
        }
    }

    /// 获取池状态
    pub async fn get_state(&self) -> PoolState {
        let state = self.state.read().await;
        state.clone()
    }

    /// 获取扩缩容历史
    pub async fn get_scaling_history(&self, limit: Option<usize>) -> Vec<ScalingEvent> {
        let history = self.scaling_history.read().await;
        let limit = limit.unwrap_or(50);

        if history.len() <= limit {
            history.iter().cloned().collect()
        } else {
            history.iter().rev().take(limit).cloned().collect()
        }
    }

    /// 暂停池
    pub async fn pause(&self) -> Result<()> {
        let mut state = self.state.write().await;
        *state = PoolState::Paused;
        tracing::info!("Function pool paused: {}", self.function_metadata.name);
        Ok(())
    }

    /// 恢复池
    pub async fn resume(&self) -> Result<()> {
        let mut state = self.state.write().await;
        *state = PoolState::Running;
        tracing::info!("Function pool resumed: {}", self.function_metadata.name);
        Ok(())
    }

    /// 停止池
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping function pool: {}", self.function_metadata.name);

        // 更新状态
        {
            let mut state = self.state.write().await;
            *state = PoolState::Stopping;
        }

        // 停止健康检查任务
        {
            let mut handle = self.health_check_handle.lock().await;
            if let Some(task) = handle.take() {
                task.abort();
            }
        }

        // 停止自动扩缩容任务
        {
            let mut handle = self.auto_scaling_handle.lock().await;
            if let Some(task) = handle.take() {
                task.abort();
            }
        }

        // 停止所有实例
        let instance_ids: Vec<String> = {
            let instances = self.instances.read().await;
            instances.keys().cloned().collect()
        };

        for instance_id in instance_ids {
            if let Err(e) = self.instance_manager.stop_instance(&instance_id).await {
                tracing::warn!("Failed to stop instance {}: {}", instance_id, e);
            }
        }

        // 清空实例列表
        {
            let mut instances = self.instances.write().await;
            instances.clear();
        }

        // 更新状态
        {
            let mut state = self.state.write().await;
            *state = PoolState::Stopped;
        }

        tracing::info!("Function pool stopped: {}", self.function_metadata.name);
        Ok(())
    }
}

/// 函数池管理器
#[derive(Debug)]
pub struct PoolManager {
    /// 函数池映射
    pools: Arc<RwLock<HashMap<String, Arc<FunctionPool>>>>,
    /// 实例管理器
    instance_manager: Arc<InstanceManager>,
    /// 默认池配置
    default_config: PoolConfig,
}

impl PoolManager {
    /// 创建新的池管理器
    pub fn new(instance_manager: Arc<InstanceManager>, default_config: Option<PoolConfig>) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            instance_manager,
            default_config: default_config.unwrap_or_default(),
        }
    }

    /// 创建函数池
    pub async fn create_pool(
        &self,
        function_metadata: FunctionMetadata,
        config: Option<PoolConfig>,
    ) -> Result<Arc<FunctionPool>> {
        let config = config.unwrap_or_else(|| self.default_config.clone());
        let function_name = function_metadata.name.clone();

        tracing::info!("Creating function pool: {}", function_name);

        let pool = Arc::new(
            FunctionPool::new(function_metadata, config, self.instance_manager.clone()).await?,
        );

        let mut pools = self.pools.write().await;
        pools.insert(function_name.clone(), pool.clone());

        tracing::info!("Function pool created successfully: {}", function_name);
        Ok(pool)
    }

    /// 获取函数池
    pub async fn get_pool(&self, function_name: &str) -> Option<Arc<FunctionPool>> {
        let pools = self.pools.read().await;
        pools.get(function_name).cloned()
    }

    /// 移除函数池
    pub async fn remove_pool(&self, function_name: &str) -> Result<()> {
        let pool = {
            let mut pools = self.pools.write().await;
            pools.remove(function_name)
        };

        if let Some(pool) = pool {
            pool.stop().await?;
            tracing::info!("Function pool removed: {}", function_name);
        }

        Ok(())
    }

    /// 获取所有池的统计信息
    pub async fn get_all_stats(&self) -> HashMap<String, PoolExecutionStats> {
        let pools = self.pools.read().await;
        let mut stats = HashMap::new();

        for (name, pool) in pools.iter() {
            stats.insert(name.clone(), pool.get_stats().await);
        }

        stats
    }

    /// 清理所有池
    pub async fn cleanup(&self) -> Result<()> {
        let pools: Vec<_> = {
            let mut pools_guard = self.pools.write().await;
            pools_guard.drain().collect::<Vec<_>>()
        };

        for (name, pool) in pools {
            if let Err(e) = pool.stop().await {
                tracing::warn!("Failed to stop pool {name}: {e}");
            }
        }

        tracing::info!("Pool manager cleanup completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functions::ScriptType;
    use crate::runtime::compiler::{CompilerConfig, RustCompiler};
    use crate::runtime::resource::ResourceManager;
    use crate::runtime::sandbox::{SandboxConfig, SandboxExecutor};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_pool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let compiler_config = CompilerConfig {
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let compiler = Arc::new(RustCompiler::new(compiler_config).unwrap());
        let sandbox = Arc::new(SandboxExecutor::new(SandboxConfig::default()).unwrap());
        let resource_manager = Arc::new(ResourceManager::new());

        let instance_manager = Arc::new(InstanceManager::new(
            compiler,
            sandbox,
            resource_manager,
            None,
        ));

        let pool_manager = PoolManager::new(instance_manager, None);

        let function_metadata = FunctionMetadata {
            id: scru128::new(),
            name: "test_pool_function".to_string(),
            description: "Test pool function".to_string(),
            code: "fn test_pool_function() -> i32 { 42 }".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            timeout_ms: 5000,
            version: "1.0.0".to_string(),
            dependencies: vec![],
            parameters: vec![],
            return_type: crate::functions::ReturnType::Integer,
            script_type: ScriptType::Rust,
        };

        let pool = pool_manager
            .create_pool(function_metadata, None)
            .await
            .unwrap();
        assert_eq!(pool.get_state().await, PoolState::Running);

        let stats = pool.get_stats().await;
        assert!(stats.total_instances > 0);
    }
}
