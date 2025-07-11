use crate::functions::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 函数性能监控器
#[derive(Debug, Clone)]
pub struct PerformanceMonitor {
    /// 函数执行统计
    stats: Arc<RwLock<HashMap<String, FunctionStats>>>,
    /// 全局统计
    global_stats: Arc<RwLock<GlobalStats>>,
}

/// 单个函数的统计信息
#[derive(Debug, Clone, Default)]
pub struct FunctionStats {
    /// 总调用次数
    pub total_calls: u64,
    /// 成功调用次数
    pub successful_calls: u64,
    /// 失败调用次数
    pub failed_calls: u64,
    /// 总执行时间
    pub total_duration: Duration,
    /// 最快执行时间
    pub min_duration: Option<Duration>,
    /// 最慢执行时间
    pub max_duration: Option<Duration>,
    /// 平均执行时间
    pub avg_duration: Duration,
    /// 最后执行时间
    pub last_execution: Option<Instant>,
    /// 内存使用峰值（字节）
    pub peak_memory: u64,
    /// 平均内存使用（字节）
    pub avg_memory: u64,
}

/// 全局统计信息
#[derive(Debug, Clone, Default)]
pub struct GlobalStats {
    /// 系统启动时间
    pub start_time: Option<Instant>,
    /// 总请求数
    pub total_requests: u64,
    /// 总成功数
    pub total_success: u64,
    /// 总失败数
    pub total_failures: u64,
    /// 活跃函数数量
    #[allow(dead_code)]
    pub active_functions: u64,
    /// 系统峰值内存
    pub peak_system_memory: u64,
    /// 当前系统内存使用
    pub current_system_memory: u64,
    /// 最后重置时间
    #[allow(dead_code)]
    pub last_reset: Option<Instant>,
}

/// 执行结果统计
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 函数名称
    pub function_name: String,
    /// 执行时间
    pub duration: Duration,
    /// 是否成功
    pub success: bool,
    /// 内存使用（字节）
    pub memory_usage: u64,
    /// 错误信息（如果有）
    #[allow(dead_code)]
    pub error_message: Option<String>,
}

/// 性能报告
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    /// 报告生成时间
    pub generated_at: Instant,
    /// 全局统计
    pub global_stats: GlobalStats,
    /// 各函数统计
    pub function_stats: HashMap<String, FunctionStats>,
    /// 性能建议
    pub recommendations: Vec<String>,
    /// 系统健康状态
    pub health_status: HealthStatus,
}

/// 系统健康状态
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 危险
    Critical,
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new() -> Self {
        let global_stats = GlobalStats {
            start_time: Some(Instant::now()),
            ..Default::default()
        };

        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            global_stats: Arc::new(RwLock::new(global_stats)),
        }
    }

    /// 记录函数执行结果
    pub async fn record_execution(&self, result: ExecutionResult) -> Result<()> {
        // 更新函数统计
        self.update_function_stats(&result).await;

        // 更新全局统计
        self.update_global_stats(&result).await;

        Ok(())
    }

    /// 获取函数统计信息
    #[allow(dead_code)]
    pub async fn get_function_stats(&self, function_name: &str) -> Option<FunctionStats> {
        let stats = self.stats.read().await;
        stats.get(function_name).cloned()
    }

    /// 获取全局统计信息
    pub async fn get_global_stats(&self) -> GlobalStats {
        self.global_stats.read().await.clone()
    }

    /// 生成性能报告
    pub async fn generate_report(&self) -> PerformanceReport {
        let global_stats = self.get_global_stats().await;
        let function_stats = {
            let stats = self.stats.read().await;
            stats.clone()
        };

        let recommendations = self.generate_recommendations(&global_stats, &function_stats);
        let health_status = self.assess_health(&global_stats, &function_stats);

        PerformanceReport {
            generated_at: Instant::now(),
            global_stats,
            function_stats,
            recommendations,
            health_status,
        }
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) -> Result<()> {
        {
            let mut stats = self.stats.write().await;
            stats.clear();
        }

        {
            let mut global_stats = self.global_stats.write().await;
            *global_stats = GlobalStats {
                start_time: Some(Instant::now()),
                last_reset: Some(Instant::now()),
                ..Default::default()
            };
        }

        tracing::info!("Performance statistics have been reset");
        Ok(())
    }

    /// 获取热点函数（调用次数最多的函数）
    pub async fn get_hottest_functions(&self, limit: usize) -> Vec<(String, u64)> {
        let stats = self.stats.read().await;
        let mut functions: Vec<_> = stats
            .iter()
            .map(|(name, stats)| (name.clone(), stats.total_calls))
            .collect();

        functions.sort_by(|a, b| b.1.cmp(&a.1));
        functions.truncate(limit);
        functions
    }

    /// 获取最慢的函数
    pub async fn get_slowest_functions(&self, limit: usize) -> Vec<(String, Duration)> {
        let stats = self.stats.read().await;
        let mut functions: Vec<_> = stats
            .iter()
            .map(|(name, stats)| (name.clone(), stats.avg_duration))
            .collect();

        functions.sort_by(|a, b| b.1.cmp(&a.1));
        functions.truncate(limit);
        functions
    }

    /// 获取错误率最高的函数
    pub async fn get_error_prone_functions(&self, limit: usize) -> Vec<(String, f64)> {
        let stats = self.stats.read().await;
        let mut functions: Vec<_> = stats
            .iter()
            .filter_map(|(name, stats)| {
                if stats.total_calls > 0 {
                    let error_rate = stats.failed_calls as f64 / stats.total_calls as f64;
                    Some((name.clone(), error_rate))
                } else {
                    None
                }
            })
            .collect();

        functions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        functions.truncate(limit);
        functions
    }

    /// 更新函数统计
    async fn update_function_stats(&self, result: &ExecutionResult) {
        let mut stats = self.stats.write().await;
        let function_stats = stats
            .entry(result.function_name.clone())
            .or_insert_with(FunctionStats::default);

        function_stats.total_calls += 1;
        if result.success {
            function_stats.successful_calls += 1;
        } else {
            function_stats.failed_calls += 1;
        }

        function_stats.total_duration += result.duration;
        function_stats.last_execution = Some(Instant::now());

        // 更新最小/最大执行时间
        match function_stats.min_duration {
            None => function_stats.min_duration = Some(result.duration),
            Some(min) if result.duration < min => {
                function_stats.min_duration = Some(result.duration)
            }
            _ => {}
        }

        match function_stats.max_duration {
            None => function_stats.max_duration = Some(result.duration),
            Some(max) if result.duration > max => {
                function_stats.max_duration = Some(result.duration)
            }
            _ => {}
        }

        // 更新平均执行时间
        function_stats.avg_duration =
            function_stats.total_duration / function_stats.total_calls as u32;

        // 更新内存统计
        if result.memory_usage > function_stats.peak_memory {
            function_stats.peak_memory = result.memory_usage;
        }

        function_stats.avg_memory =
            ((function_stats.avg_memory * (function_stats.total_calls - 1)) + result.memory_usage)
                / function_stats.total_calls;
    }

    /// 更新全局统计
    async fn update_global_stats(&self, result: &ExecutionResult) {
        let mut global_stats = self.global_stats.write().await;

        global_stats.total_requests += 1;
        if result.success {
            global_stats.total_success += 1;
        } else {
            global_stats.total_failures += 1;
        }

        // 估算系统内存使用
        global_stats.current_system_memory += result.memory_usage;
        if global_stats.current_system_memory > global_stats.peak_system_memory {
            global_stats.peak_system_memory = global_stats.current_system_memory;
        }
    }

    /// 生成性能建议
    fn generate_recommendations(
        &self,
        global_stats: &GlobalStats,
        function_stats: &HashMap<String, FunctionStats>,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        // 检查总体成功率
        if global_stats.total_requests > 0 {
            let success_rate =
                global_stats.total_success as f64 / global_stats.total_requests as f64;
            if success_rate < 0.95 {
                recommendations.push(format!(
                    "系统成功率较低 ({:.1}%)，建议检查错误日志并优化错误处理",
                    success_rate * 100.0
                ));
            }
        }

        // 检查内存使用
        if global_stats.peak_system_memory > 100 * 1024 * 1024 {
            // 100MB
            recommendations.push("系统峰值内存使用较高，建议优化内存管理".to_string());
        }

        // 检查慢函数
        for (name, stats) in function_stats {
            if stats.avg_duration > Duration::from_millis(1000) {
                recommendations.push(format!(
                    "函数 '{}' 平均执行时间较长 ({:.2}ms)，建议优化性能",
                    name,
                    stats.avg_duration.as_millis()
                ));
            }

            // 检查错误率
            if stats.total_calls > 0 {
                let error_rate = stats.failed_calls as f64 / stats.total_calls as f64;
                if error_rate > 0.1 {
                    recommendations.push(format!(
                        "函数 '{}' 错误率较高 ({:.1}%)，建议检查实现",
                        name,
                        error_rate * 100.0
                    ));
                }
            }
        }

        if recommendations.is_empty() {
            recommendations.push("系统运行良好，无需特别优化".to_string());
        }

        recommendations
    }

    /// 评估系统健康状态
    fn assess_health(
        &self,
        global_stats: &GlobalStats,
        function_stats: &HashMap<String, FunctionStats>,
    ) -> HealthStatus {
        let mut warning_count = 0;
        let mut critical_count = 0;

        // 检查总体成功率
        if global_stats.total_requests > 0 {
            let success_rate =
                global_stats.total_success as f64 / global_stats.total_requests as f64;
            if success_rate < 0.8 {
                critical_count += 1;
            } else if success_rate < 0.95 {
                warning_count += 1;
            }
        }

        // 检查函数状态
        for stats in function_stats.values() {
            if stats.total_calls > 0 {
                let error_rate = stats.failed_calls as f64 / stats.total_calls as f64;
                if error_rate > 0.5 {
                    critical_count += 1;
                } else if error_rate > 0.1 {
                    warning_count += 1;
                }
            }

            if stats.avg_duration > Duration::from_millis(5000) {
                critical_count += 1;
            } else if stats.avg_duration > Duration::from_millis(1000) {
                warning_count += 1;
            }
        }

        if critical_count > 0 {
            HealthStatus::Critical
        } else if warning_count > 0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}
