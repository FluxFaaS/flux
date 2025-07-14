use super::{Scheduler, SimpleScheduler};
use crate::functions::{InvokeRequest, InvokeResponse, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 负载均衡策略

#[derive(Debug, Clone)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    Random,
    LeastConnections,
}

/// 高级调度器（为将来扩展预留）
#[derive(Debug)]
pub struct AdvancedScheduler {
    scheduler: SimpleScheduler,
    strategy: LoadBalanceStrategy,
    active_executions: Arc<Mutex<u64>>,
}

impl AdvancedScheduler {
    pub fn new(strategy: LoadBalanceStrategy) -> Self {
        Self {
            scheduler: SimpleScheduler::new(),
            strategy,
            active_executions: Arc::new(Mutex::new(0)),
        }
    }

    /// 获取当前活跃执行数
    pub async fn active_executions(&self) -> u64 {
        *self.active_executions.lock().await
    }

    /// 增加活跃执行计数
    async fn increment_executions(&self) {
        let mut count = self.active_executions.lock().await;
        *count += 1;
    }

    /// 减少活跃执行计数
    async fn decrement_executions(&self) {
        let mut count = self.active_executions.lock().await;
        if *count > 0 {
            *count -= 1;
        }
    }
}

#[async_trait::async_trait]
impl Scheduler for AdvancedScheduler {
    async fn schedule(
        &self,
        function_name: &str,
        request: InvokeRequest,
    ) -> Result<InvokeResponse> {
        self.increment_executions().await;

        tracing::info!(
            "Advanced scheduling function: {} with strategy: {:?}",
            function_name,
            self.strategy
        );

        let result = self.scheduler.schedule(function_name, request).await;

        self.decrement_executions().await;

        result
    }
}

/// 调度器统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchedulerStats {
    pub total_executions: u64,
    pub active_executions: u64,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
}

impl Default for SchedulerStats {
    fn default() -> Self {
        Self {
            total_executions: 0,
            active_executions: 0,
            avg_execution_time_ms: 0.0,
            success_rate: 100.0,
        }
    }
}
