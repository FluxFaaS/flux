use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 负载均衡器配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoadBalancerConfig {
    /// 负载均衡策略
    pub strategy: LoadBalanceStrategy,
    /// 健康检查配置
    pub health_check: HealthCheckConfig,
    /// 权重配置
    pub weight_config: WeightConfig,
    /// 故障转移配置
    pub failover_config: FailoverConfig,
    /// 性能监控配置
    pub monitoring_config: MonitoringConfig,
}

/// 负载均衡策略
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum LoadBalanceStrategy {
    #[default]
    /// 轮询
    RoundRobin,
    /// 加权轮询
    WeightedRoundRobin,
    /// 随机
    Random,
    /// 加权随机
    WeightedRandom,
    /// 最少连接
    LeastConnections,
    /// 加权最少连接
    WeightedLeastConnections,
    /// 最少负载
    LeastLoad,
    /// 响应时间最短
    FastestResponse,
    /// 一致性哈希
    ConsistentHash,
    /// 自适应负载均衡
    Adaptive,
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// 检查间隔（秒）
    pub interval_secs: u64,
    /// 超时时间（秒）
    pub timeout_secs: u64,
    /// 连续失败次数阈值
    pub failure_threshold: u32,
    /// 连续成功次数阈值
    pub success_threshold: u32,
    /// 是否启用健康检查
    pub enabled: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval_secs: 30,
            timeout_secs: 5,
            failure_threshold: 3,
            success_threshold: 2,
            enabled: true,
        }
    }
}

/// 权重配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightConfig {
    /// 默认权重
    pub default_weight: u32,
    /// 最小权重
    pub min_weight: u32,
    /// 最大权重
    pub max_weight: u32,
    /// 权重调整因子
    pub adjustment_factor: f64,
    /// 是否启用动态权重调整
    pub dynamic_adjustment: bool,
}

impl Default for WeightConfig {
    fn default() -> Self {
        Self {
            default_weight: 100,
            min_weight: 1,
            max_weight: 1000,
            adjustment_factor: 0.1,
            dynamic_adjustment: true,
        }
    }
}

/// 故障转移配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔（毫秒）
    pub retry_interval_ms: u64,
    /// 断路器开启阈值
    pub circuit_breaker_threshold: f64,
    /// 断路器恢复时间（秒）
    pub circuit_breaker_recovery_secs: u64,
    /// 连续失败次数阈值
    pub failure_threshold: u32,
    /// 连续成功次数阈值
    pub success_threshold: u32,
    /// 是否启用故障转移
    pub enabled: bool,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_interval_ms: 100,
            circuit_breaker_threshold: 0.5,
            circuit_breaker_recovery_secs: 60,
            failure_threshold: 3,
            success_threshold: 2,
            enabled: true,
        }
    }
}

/// 性能监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// 监控窗口大小（秒）
    pub window_size_secs: u64,
    /// 采样间隔（毫秒）
    pub sampling_interval_ms: u64,
    /// 是否启用性能监控
    pub enabled: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            window_size_secs: 300,
            sampling_interval_ms: 1000,
            enabled: true,
        }
    }
}

/// 负载均衡目标
#[derive(Debug, Clone)]
pub struct LoadBalanceTarget {
    /// 目标ID
    pub id: String,
    /// 目标名称
    pub name: String,
    /// 权重
    pub weight: u32,
    /// 当前负载（0.0-1.0）
    pub current_load: f64,
    /// 活跃连接数
    pub active_connections: u32,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// 健康状态
    pub is_healthy: bool,
    /// 最后活动时间
    pub last_activity: Instant,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 连续成功次数
    pub consecutive_successes: u32,
    /// 断路器状态
    pub circuit_breaker_state: CircuitBreakerState,
    /// 性能统计
    pub performance_stats: PerformanceStats,
}

/// 断路器状态
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    /// 关闭状态（正常）
    Closed,
    /// 开启状态（故障）
    Open,
    /// 半开状态（恢复中）
    HalfOpen,
}

/// 性能统计
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 总响应时间（毫秒）
    pub total_response_time_ms: u64,
    /// 最小响应时间（毫秒）
    pub min_response_time_ms: u64,
    /// 最大响应时间（毫秒）
    pub max_response_time_ms: u64,
    /// 最近更新时间
    pub last_updated: Instant,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_response_time_ms: 0,
            min_response_time_ms: u64::MAX,
            max_response_time_ms: 0,
            last_updated: Instant::now(),
        }
    }
}

/// 负载均衡器
#[derive(Debug)]
pub struct LoadBalancer {
    /// 配置
    config: LoadBalancerConfig,
    /// 目标列表
    targets: Arc<RwLock<HashMap<String, LoadBalanceTarget>>>,
    /// 负载均衡状态
    state: Arc<RwLock<LoadBalancerState>>,
    /// 性能监控器
    performance_monitor: Arc<PerformanceMonitor>,
}

/// 负载均衡器状态
#[derive(Debug, Clone, Default)]
pub struct LoadBalancerState {
    /// 轮询计数器
    pub round_robin_counter: usize,
    /// 加权轮询当前权重
    pub weighted_round_robin_weights: HashMap<String, i32>,
    /// 随机种子
    pub random_seed: u64,
    /// 一致性哈希环
    pub consistent_hash_ring: Vec<(u64, String)>,
    /// 自适应权重
    pub adaptive_weights: HashMap<String, f64>,
}

/// 性能监控器
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// 监控配置
    config: MonitoringConfig,
    /// 监控数据
    metrics: Arc<RwLock<HashMap<String, Vec<MetricPoint>>>>,
}

/// 监控数据点
#[derive(Debug, Clone)]
pub struct MetricPoint {
    /// 时间戳
    pub timestamp: Instant,
    /// 响应时间（毫秒）
    pub response_time_ms: u64,
    /// 是否成功
    pub success: bool,
    /// 负载值
    pub load: f64,
}

/// 负载均衡结果
#[derive(Debug, Clone)]
pub struct LoadBalanceResult {
    /// 选中的目标ID
    pub target_id: String,
    /// 选择原因
    pub reason: String,
    /// 选择时的负载情况
    pub load_snapshot: HashMap<String, f64>,
    /// 选择耗时（微秒）
    pub selection_time_us: u64,
}

impl LoadBalancer {
    /// 创建新的负载均衡器
    pub fn new(config: LoadBalancerConfig) -> Self {
        let performance_monitor =
            Arc::new(PerformanceMonitor::new(config.monitoring_config.clone()));

        Self {
            config,
            targets: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(LoadBalancerState::default())),
            performance_monitor,
        }
    }

    /// 添加目标
    pub async fn add_target(&self, target: LoadBalanceTarget) -> Result<()> {
        let targets_len = {
            let mut targets = self.targets.write().await;
            targets.insert(target.id.clone(), target);
            targets.len()
        }; // 释放写锁

        // 更新一致性哈希环
        self.update_consistent_hash_ring().await;

        tracing::info!("Added load balance target: {}", targets_len);
        Ok(())
    }

    /// 移除目标
    pub async fn remove_target(&self, target_id: &str) -> Result<()> {
        let mut targets = self.targets.write().await;
        if targets.remove(target_id).is_some() {
            // 更新一致性哈希环
            drop(targets);
            self.update_consistent_hash_ring().await;
            tracing::info!("Removed load balance target: {}", target_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Target not found: {}", target_id))
        }
    }

    /// 更新目标状态
    pub async fn update_target_status(
        &self,
        target_id: &str,
        is_healthy: bool,
        load: f64,
        connections: u32,
        response_time_ms: f64,
    ) -> Result<()> {
        let mut targets = self.targets.write().await;
        if let Some(target) = targets.get_mut(target_id) {
            target.is_healthy = is_healthy;
            target.current_load = load;
            target.active_connections = connections;
            target.avg_response_time_ms = response_time_ms;
            target.last_activity = Instant::now();

            // 更新性能统计
            target.performance_stats.total_requests += 1;
            if is_healthy {
                target.performance_stats.successful_requests += 1;
                target.consecutive_successes += 1;
                target.consecutive_failures = 0;
            } else {
                target.performance_stats.failed_requests += 1;
                target.consecutive_failures += 1;
                target.consecutive_successes = 0;
            }

            // 更新断路器状态
            self.update_circuit_breaker_state(target).await;

            Ok(())
        } else {
            Err(anyhow::anyhow!("Target not found: {}", target_id))
        }
    }

    /// 选择目标
    pub async fn select_target(&self, request_key: Option<&str>) -> Result<LoadBalanceResult> {
        let start_time = Instant::now();

        let targets = self.targets.read().await;
        if targets.is_empty() {
            return Err(anyhow::anyhow!("No targets available"));
        }

        // 过滤健康的目标
        let healthy_targets: Vec<_> = targets
            .iter()
            .filter(|(_, target)| {
                target.is_healthy && target.circuit_breaker_state == CircuitBreakerState::Closed
            })
            .collect();

        if healthy_targets.is_empty() {
            return Err(anyhow::anyhow!("No healthy targets available"));
        }

        let selected_id = match self.config.strategy {
            LoadBalanceStrategy::RoundRobin => self.select_round_robin(&healthy_targets).await,
            LoadBalanceStrategy::WeightedRoundRobin => {
                self.select_weighted_round_robin(&healthy_targets).await
            }
            LoadBalanceStrategy::Random => self.select_random(&healthy_targets).await,
            LoadBalanceStrategy::WeightedRandom => {
                self.select_weighted_random(&healthy_targets).await
            }
            LoadBalanceStrategy::LeastConnections => {
                self.select_least_connections(&healthy_targets).await
            }
            LoadBalanceStrategy::WeightedLeastConnections => {
                self.select_weighted_least_connections(&healthy_targets)
                    .await
            }
            LoadBalanceStrategy::LeastLoad => self.select_least_load(&healthy_targets).await,
            LoadBalanceStrategy::FastestResponse => {
                self.select_fastest_response(&healthy_targets).await
            }
            LoadBalanceStrategy::ConsistentHash => {
                self.select_consistent_hash(&healthy_targets, request_key)
                    .await
            }
            LoadBalanceStrategy::Adaptive => self.select_adaptive(&healthy_targets).await,
        };

        let selection_time = start_time.elapsed();

        // 创建负载快照
        let load_snapshot: HashMap<String, f64> = healthy_targets
            .iter()
            .map(|(id, target)| (id.to_string(), target.current_load))
            .collect();

        Ok(LoadBalanceResult {
            target_id: selected_id.clone(),
            reason: format!("Selected by {:?} strategy", self.config.strategy),
            load_snapshot,
            selection_time_us: selection_time.as_micros() as u64,
        })
    }

    /// 轮询选择
    async fn select_round_robin(&self, targets: &[(&String, &LoadBalanceTarget)]) -> String {
        let mut state = self.state.write().await;
        let index = state.round_robin_counter % targets.len();
        state.round_robin_counter = (state.round_robin_counter + 1) % targets.len();
        targets[index].0.clone()
    }

    /// 加权轮询选择
    async fn select_weighted_round_robin(
        &self,
        targets: &[(&String, &LoadBalanceTarget)],
    ) -> String {
        let mut state = self.state.write().await;

        // 初始化权重
        for (id, target) in targets {
            state
                .weighted_round_robin_weights
                .entry(id.to_string())
                .or_insert(-(target.weight as i32));
        }

        // 找到当前权重最高的目标
        let mut selected_id = String::new();
        let mut max_weight = i32::MIN;
        let mut total_weight = 0;

        for (id, target) in targets {
            let current_weight = state
                .weighted_round_robin_weights
                .get_mut(&id.to_string())
                .unwrap();
            *current_weight += target.weight as i32;
            total_weight += target.weight as i32;

            if *current_weight > max_weight {
                max_weight = *current_weight;
                selected_id = id.to_string();
            }
        }

        // 减去总权重
        if let Some(weight) = state.weighted_round_robin_weights.get_mut(&selected_id) {
            *weight -= total_weight;
        }

        selected_id
    }

    /// 随机选择
    async fn select_random(&self, targets: &[(&String, &LoadBalanceTarget)]) -> String {
        let mut state = self.state.write().await;
        state.random_seed = (state
            .random_seed
            .wrapping_mul(1103515245)
            .wrapping_add(12345))
            & 0x7fffffff;
        let index = (state.random_seed as usize) % targets.len();
        targets[index].0.clone()
    }

    /// 加权随机选择
    async fn select_weighted_random(&self, targets: &[(&String, &LoadBalanceTarget)]) -> String {
        let total_weight: u32 = targets.iter().map(|(_, target)| target.weight).sum();

        let mut state = self.state.write().await;
        state.random_seed = (state
            .random_seed
            .wrapping_mul(1103515245)
            .wrapping_add(12345))
            & 0x7fffffff;
        let mut random_weight = (state.random_seed as u32) % total_weight;

        for (id, target) in targets {
            if random_weight < target.weight {
                return id.to_string();
            }
            random_weight -= target.weight;
        }

        targets[0].0.clone()
    }

    /// 最少连接选择
    async fn select_least_connections(&self, targets: &[(&String, &LoadBalanceTarget)]) -> String {
        let min_target = targets
            .iter()
            .min_by_key(|(_, target)| target.active_connections)
            .unwrap();
        min_target.0.clone()
    }

    /// 加权最少连接选择
    async fn select_weighted_least_connections(
        &self,
        targets: &[(&String, &LoadBalanceTarget)],
    ) -> String {
        let min_target = targets
            .iter()
            .min_by(|(_, a), (_, b)| {
                let a_ratio = a.active_connections as f64 / a.weight as f64;
                let b_ratio = b.active_connections as f64 / b.weight as f64;
                a_ratio.partial_cmp(&b_ratio).unwrap()
            })
            .unwrap();
        min_target.0.clone()
    }

    /// 最少负载选择
    async fn select_least_load(&self, targets: &[(&String, &LoadBalanceTarget)]) -> String {
        let min_target = targets
            .iter()
            .min_by(|(_, a), (_, b)| a.current_load.partial_cmp(&b.current_load).unwrap())
            .unwrap();
        min_target.0.clone()
    }

    /// 响应时间最短选择
    async fn select_fastest_response(&self, targets: &[(&String, &LoadBalanceTarget)]) -> String {
        let fastest_target = targets
            .iter()
            .min_by(|(_, a), (_, b)| {
                a.avg_response_time_ms
                    .partial_cmp(&b.avg_response_time_ms)
                    .unwrap()
            })
            .unwrap();
        fastest_target.0.clone()
    }

    /// 一致性哈希选择
    async fn select_consistent_hash(
        &self,
        targets: &[(&String, &LoadBalanceTarget)],
        request_key: Option<&str>,
    ) -> String {
        let key = request_key.unwrap_or("default");
        let hash = self.hash_key(key);

        let state = self.state.read().await;

        // 在哈希环中查找
        for (ring_hash, target_id) in &state.consistent_hash_ring {
            if hash <= *ring_hash {
                return target_id.clone();
            }
        }

        // 如果没找到，返回第一个
        if let Some((_, target_id)) = state.consistent_hash_ring.first() {
            target_id.clone()
        } else {
            targets[0].0.clone()
        }
    }

    /// 自适应选择
    async fn select_adaptive(&self, targets: &[(&String, &LoadBalanceTarget)]) -> String {
        let state = self.state.read().await;

        // 计算自适应权重
        let mut best_target = targets[0].0.clone();
        let mut best_score = f64::MIN;

        for (id, target) in targets {
            let adaptive_weight = state.adaptive_weights.get(&id.to_string()).unwrap_or(&1.0);

            // 综合评分：权重 / (负载 * 响应时间 * 连接数)
            let score = *adaptive_weight
                / (target.current_load.max(0.1)
                    * target.avg_response_time_ms.max(1.0)
                    * (target.active_connections as f64).max(1.0));

            if score > best_score {
                best_score = score;
                best_target = id.to_string();
            }
        }

        best_target
    }

    /// 更新一致性哈希环
    async fn update_consistent_hash_ring(&self) {
        let targets = self.targets.read().await;
        let mut state = self.state.write().await;

        state.consistent_hash_ring.clear();

        for (id, target) in targets.iter() {
            // 每个目标根据权重创建适量的虚拟节点（限制最大数量）
            let virtual_nodes = ((target.weight as usize).max(1) * 3).min(300); // 最多300个虚拟节点

            for i in 0..virtual_nodes {
                let virtual_key = format!("{id}:{i}");
                let hash = self.hash_key(&virtual_key);
                state.consistent_hash_ring.push((hash, id.clone()));
            }
        }

        // 排序哈希环
        state.consistent_hash_ring.sort_by_key(|(hash, _)| *hash);
    }

    /// 哈希函数
    fn hash_key(&self, key: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// 更新断路器状态
    async fn update_circuit_breaker_state(&self, target: &mut LoadBalanceTarget) {
        match target.circuit_breaker_state {
            CircuitBreakerState::Closed => {
                if target.consecutive_failures >= self.config.failover_config.failure_threshold {
                    target.circuit_breaker_state = CircuitBreakerState::Open;
                    tracing::warn!("Circuit breaker opened for target: {}", target.id);
                }
            }
            CircuitBreakerState::Open => {
                // 检查是否可以进入半开状态
                if target.last_activity.elapsed()
                    > Duration::from_secs(self.config.failover_config.circuit_breaker_recovery_secs)
                {
                    target.circuit_breaker_state = CircuitBreakerState::HalfOpen;
                    tracing::info!("Circuit breaker half-opened for target: {}", target.id);
                }
            }
            CircuitBreakerState::HalfOpen => {
                if target.consecutive_successes >= self.config.failover_config.success_threshold {
                    target.circuit_breaker_state = CircuitBreakerState::Closed;
                    tracing::info!("Circuit breaker closed for target: {}", target.id);
                } else if target.consecutive_failures > 0 {
                    target.circuit_breaker_state = CircuitBreakerState::Open;
                    tracing::warn!("Circuit breaker re-opened for target: {}", target.id);
                }
            }
        }
    }

    /// 获取负载均衡统计信息
    pub async fn get_statistics(&self) -> LoadBalancerStatistics {
        let targets = self.targets.read().await;
        let total_targets = targets.len();
        let healthy_targets = targets.values().filter(|t| t.is_healthy).count();

        let total_requests: u64 = targets
            .values()
            .map(|t| t.performance_stats.total_requests)
            .sum();
        let successful_requests: u64 = targets
            .values()
            .map(|t| t.performance_stats.successful_requests)
            .sum();
        let failed_requests: u64 = targets
            .values()
            .map(|t| t.performance_stats.failed_requests)
            .sum();

        let avg_response_time = if total_requests > 0 {
            targets
                .values()
                .map(|t| t.performance_stats.total_response_time_ms)
                .sum::<u64>() as f64
                / total_requests as f64
        } else {
            0.0
        };

        LoadBalancerStatistics {
            total_targets,
            healthy_targets,
            total_requests,
            successful_requests,
            failed_requests,
            success_rate: if total_requests > 0 {
                successful_requests as f64 / total_requests as f64
            } else {
                0.0
            },
            avg_response_time_ms: avg_response_time,
            strategy: self.config.strategy.clone(),
        }
    }

    /// 获取目标列表
    pub async fn get_targets(&self) -> Vec<LoadBalanceTarget> {
        let targets = self.targets.read().await;
        targets.values().cloned().collect()
    }
}

/// 负载均衡统计信息
#[derive(Debug, Clone, Serialize)]
pub struct LoadBalancerStatistics {
    /// 总目标数
    pub total_targets: usize,
    /// 健康目标数
    pub healthy_targets: usize,
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 成功率
    pub success_rate: f64,
    /// 平均响应时间
    pub avg_response_time_ms: f64,
    /// 当前策略
    pub strategy: LoadBalanceStrategy,
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new(config: MonitoringConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 记录性能指标
    pub async fn record_metric(
        &self,
        target_id: &str,
        response_time_ms: u64,
        success: bool,
        load: f64,
    ) {
        if !self.config.enabled {
            return;
        }

        let mut metrics = self.metrics.write().await;
        let target_metrics = metrics
            .entry(target_id.to_string())
            .or_insert_with(Vec::new);

        target_metrics.push(MetricPoint {
            timestamp: Instant::now(),
            response_time_ms,
            success,
            load,
        });

        // 清理过期数据
        let cutoff_time = Instant::now() - Duration::from_secs(self.config.window_size_secs);
        target_metrics.retain(|point| point.timestamp > cutoff_time);
    }

    /// 获取性能指标
    pub async fn get_metrics(&self, target_id: &str) -> Option<Vec<MetricPoint>> {
        let metrics = self.metrics.read().await;
        metrics.get(target_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_balancer_creation() {
        let config = LoadBalancerConfig::default();
        let balancer = LoadBalancer::new(config);

        let stats = balancer.get_statistics().await;
        assert_eq!(stats.total_targets, 0);
        assert_eq!(stats.healthy_targets, 0);
    }

    #[tokio::test]
    async fn test_target_management() {
        let config = LoadBalancerConfig::default();
        let balancer = LoadBalancer::new(config);

        let target = LoadBalanceTarget {
            id: "target1".to_string(),
            name: "Test Target 1".to_string(),
            weight: 100,
            current_load: 0.5,
            active_connections: 10,
            avg_response_time_ms: 50.0,
            is_healthy: true,
            last_activity: Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 0,
            circuit_breaker_state: CircuitBreakerState::Closed,
            performance_stats: PerformanceStats::default(),
        };

        balancer.add_target(target).await.unwrap();

        let stats = balancer.get_statistics().await;
        assert_eq!(stats.total_targets, 1);
        assert_eq!(stats.healthy_targets, 1);

        balancer.remove_target("target1").await.unwrap();

        let stats = balancer.get_statistics().await;
        assert_eq!(stats.total_targets, 0);
    }

    #[tokio::test]
    async fn test_round_robin_selection() {
        let config = LoadBalancerConfig::default();
        let balancer = LoadBalancer::new(config);

        // 添加多个目标
        for i in 1..=3 {
            let target = LoadBalanceTarget {
                id: format!("target{i}"),
                name: format!("Test Target {i}"),
                weight: 100,
                current_load: 0.3,
                active_connections: 5,
                avg_response_time_ms: 30.0,
                is_healthy: true,
                last_activity: Instant::now(),
                consecutive_failures: 0,
                consecutive_successes: 0,
                circuit_breaker_state: CircuitBreakerState::Closed,
                performance_stats: PerformanceStats::default(),
            };
            balancer.add_target(target).await.unwrap();
        }

        // 测试轮询选择
        let mut selections = Vec::new();
        for _ in 0..6 {
            let result = balancer.select_target(None).await.unwrap();
            selections.push(result.target_id);
        }

        // 验证轮询模式
        assert_eq!(selections[0], selections[3]);
        assert_eq!(selections[1], selections[4]);
        assert_eq!(selections[2], selections[5]);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = LoadBalancerConfig::default();
        let balancer = LoadBalancer::new(config);

        let target = LoadBalanceTarget {
            id: "target1".to_string(),
            name: "Test Target 1".to_string(),
            weight: 100,
            current_load: 0.5,
            active_connections: 10,
            avg_response_time_ms: 50.0,
            is_healthy: true,
            last_activity: Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 0,
            circuit_breaker_state: CircuitBreakerState::Closed,
            performance_stats: PerformanceStats::default(),
        };

        balancer.add_target(target).await.unwrap();

        // 模拟连续失败
        for _ in 0..5 {
            balancer
                .update_target_status("target1", false, 0.8, 20, 200.0)
                .await
                .unwrap();
        }

        let targets = balancer.get_targets().await;
        assert_eq!(targets[0].circuit_breaker_state, CircuitBreakerState::Open);
    }
}
