use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;

/// 资源类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// CPU使用率（百分比）
    Cpu,
    /// 内存使用（字节）
    Memory,
    /// 磁盘IO（字节/秒）
    DiskIo,
    /// 网络IO（字节/秒）
    NetworkIo,
    /// 文件描述符数量
    FileDescriptors,
    /// 线程数量
    Threads,
    /// 进程数量
    Processes,
}

/// 资源限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimit {
    /// 资源类型
    pub resource_type: ResourceType,
    /// 软限制（警告阈值）
    pub soft_limit: u64,
    /// 硬限制（强制终止阈值）
    pub hard_limit: u64,
    /// 检查间隔（毫秒）
    pub check_interval_ms: u64,
    /// 是否启用
    pub enabled: bool,
}

impl Default for ResourceLimit {
    fn default() -> Self {
        Self {
            resource_type: ResourceType::Memory,
            soft_limit: 64 * 1024 * 1024,  // 64MB
            hard_limit: 128 * 1024 * 1024, // 128MB
            check_interval_ms: 1000,
            enabled: true,
        }
    }
}

/// 资源使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// 资源类型
    pub resource_type: ResourceType,
    /// 当前使用量
    pub current_usage: u64,
    /// 峰值使用量
    pub peak_usage: u64,
    /// 平均使用量
    pub average_usage: f64,
    /// 最后更新时间
    pub last_updated: chrono::DateTime<chrono::Utc>,
    /// 是否超限
    pub is_exceeded: bool,
    /// 超限类型（软限制或硬限制）
    pub exceeded_type: Option<LimitType>,
}

/// 限制类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LimitType {
    /// 软限制（警告）
    Soft,
    /// 硬限制（强制终止）
    Hard,
}

/// 资源快照类型
pub type ResourceSnapshot = (chrono::DateTime<chrono::Utc>, HashMap<ResourceType, u64>);

/// 资源配额管理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// 配额名称
    pub name: String,
    /// 时间窗口（秒）
    pub time_window_secs: u64,
    /// 资源限制集合
    pub limits: HashMap<ResourceType, ResourceLimit>,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        let mut limits = HashMap::new();

        // 默认内存限制
        limits.insert(
            ResourceType::Memory,
            ResourceLimit {
                resource_type: ResourceType::Memory,
                soft_limit: 64 * 1024 * 1024,  // 64MB
                hard_limit: 128 * 1024 * 1024, // 128MB
                check_interval_ms: 1000,
                enabled: true,
            },
        );

        // 默认CPU限制
        limits.insert(
            ResourceType::Cpu,
            ResourceLimit {
                resource_type: ResourceType::Cpu,
                soft_limit: 50, // 50%
                hard_limit: 80, // 80%
                check_interval_ms: 1000,
                enabled: true,
            },
        );

        // 默认文件描述符限制
        limits.insert(
            ResourceType::FileDescriptors,
            ResourceLimit {
                resource_type: ResourceType::FileDescriptors,
                soft_limit: 100,
                hard_limit: 200,
                check_interval_ms: 5000,
                enabled: true,
            },
        );

        Self {
            name: "default".to_string(),
            time_window_secs: 300, // 5分钟窗口
            limits,
            enabled: true,
            created_at: chrono::Utc::now(),
        }
    }
}

/// 资源监控事件
#[derive(Debug, Clone, Serialize)]
pub struct ResourceEvent {
    /// 事件ID
    pub event_id: String,
    /// 进程ID
    pub process_id: u32,
    /// 函数名称
    pub function_name: String,
    /// 资源类型
    pub resource_type: ResourceType,
    /// 事件类型
    pub event_type: ResourceEventType,
    /// 当前使用量
    pub current_usage: u64,
    /// 限制值
    pub limit_value: u64,
    /// 事件时间
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 事件描述
    pub description: String,
}

/// 资源事件类型
#[derive(Debug, Clone, Serialize)]
pub enum ResourceEventType {
    /// 资源警告（软限制）
    Warning,
    /// 资源超限（硬限制）
    Violation,
    /// 资源恢复正常
    Recovery,
    /// 资源监控开始
    MonitoringStarted,
    /// 资源监控结束
    MonitoringEnded,
}

/// 进程资源监控器
#[derive(Debug)]
pub struct ProcessResourceMonitor {
    /// 进程ID
    process_id: u32,
    /// 函数名称
    function_name: String,
    /// 资源配额
    quota: ResourceQuota,
    /// 当前资源使用统计
    current_usage: Arc<RwLock<HashMap<ResourceType, ResourceUsage>>>,
    /// 历史使用数据（用于计算平均值）
    usage_history: Arc<RwLock<Vec<ResourceSnapshot>>>,
    /// 系统监控器
    system_monitor: Arc<Mutex<sysinfo::System>>,
    /// 监控开始时间
    start_time: Instant,
    /// 是否正在监控
    is_monitoring: Arc<RwLock<bool>>,
}

impl ProcessResourceMonitor {
    /// 创建新的进程资源监控器
    pub fn new(process_id: u32, function_name: String, quota: ResourceQuota) -> Self {
        let mut system = sysinfo::System::new_all();
        system.refresh_all();

        Self {
            process_id,
            function_name,
            quota,
            current_usage: Arc::new(RwLock::new(HashMap::new())),
            usage_history: Arc::new(RwLock::new(Vec::new())),
            system_monitor: Arc::new(Mutex::new(system)),
            start_time: Instant::now(),
            is_monitoring: Arc::new(RwLock::new(false)),
        }
    }

    /// 开始监控进程资源使用
    pub async fn start_monitoring(&self) -> Result<()> {
        let mut is_monitoring = self.is_monitoring.write().await;
        if *is_monitoring {
            return Ok(());
        }
        *is_monitoring = true;

        tracing::info!(
            "Starting resource monitoring for process {} ({})",
            self.process_id,
            self.function_name
        );

        // 为每种资源类型启动监控任务
        for (resource_type, limit) in &self.quota.limits {
            if limit.enabled {
                self.start_resource_monitoring(resource_type.clone(), limit.clone())
                    .await?;
            }
        }

        Ok(())
    }

    /// 停止监控
    pub async fn stop_monitoring(&self) -> Result<ResourceSummary> {
        let mut is_monitoring = self.is_monitoring.write().await;
        *is_monitoring = false;

        tracing::info!(
            "Stopping resource monitoring for process {} ({})",
            self.process_id,
            self.function_name
        );

        // 生成资源使用摘要
        let summary = self.generate_summary().await;

        Ok(summary)
    }

    /// 启动特定资源类型的监控
    async fn start_resource_monitoring(
        &self,
        resource_type: ResourceType,
        limit: ResourceLimit,
    ) -> Result<()> {
        let process_id = self.process_id;
        let function_name = self.function_name.clone();
        let current_usage = self.current_usage.clone();
        let usage_history = self.usage_history.clone();
        let system_monitor = self.system_monitor.clone();
        let is_monitoring = self.is_monitoring.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(limit.check_interval_ms));

            loop {
                interval.tick().await;

                // 检查是否还在监控
                {
                    let monitoring = is_monitoring.read().await;
                    if !*monitoring {
                        break;
                    }
                }

                // 获取当前资源使用情况
                if let Ok(usage_value) =
                    Self::get_resource_usage(&system_monitor, process_id, &resource_type).await
                {
                    // 更新当前使用统计
                    {
                        let mut usage_map = current_usage.write().await;
                        let usage_stats =
                            usage_map.entry(resource_type.clone()).or_insert_with(|| {
                                ResourceUsage {
                                    resource_type: resource_type.clone(),
                                    current_usage: 0,
                                    peak_usage: 0,
                                    average_usage: 0.0,
                                    last_updated: chrono::Utc::now(),
                                    is_exceeded: false,
                                    exceeded_type: None,
                                }
                            });

                        usage_stats.current_usage = usage_value;
                        usage_stats.peak_usage = usage_stats.peak_usage.max(usage_value);
                        usage_stats.last_updated = chrono::Utc::now();

                        // 检查限制
                        if usage_value > limit.hard_limit {
                            usage_stats.is_exceeded = true;
                            usage_stats.exceeded_type = Some(LimitType::Hard);

                            tracing::error!(
                                "Process {} ({}) exceeded hard limit for {:?}: {} > {}",
                                process_id,
                                function_name,
                                resource_type,
                                usage_value,
                                limit.hard_limit
                            );
                        } else if usage_value > limit.soft_limit {
                            usage_stats.is_exceeded = true;
                            usage_stats.exceeded_type = Some(LimitType::Soft);

                            tracing::warn!(
                                "Process {} ({}) exceeded soft limit for {:?}: {} > {}",
                                process_id,
                                function_name,
                                resource_type,
                                usage_value,
                                limit.soft_limit
                            );
                        } else {
                            usage_stats.is_exceeded = false;
                            usage_stats.exceeded_type = None;
                        }
                    }

                    // 记录历史数据
                    {
                        let mut history = usage_history.write().await;
                        let timestamp = chrono::Utc::now();

                        // 创建或更新当前时间点的数据
                        let mut current_data = HashMap::new();
                        current_data.insert(resource_type.clone(), usage_value);

                        history.push((timestamp, current_data));

                        // 限制历史数据大小（保留最近1小时的数据，每秒一个数据点）
                        if history.len() > 3600 {
                            history.remove(0);
                        }
                    }
                }
            }

            tracing::debug!(
                "Resource monitoring stopped for {:?} of process {}",
                resource_type,
                process_id
            );
        });

        Ok(())
    }

    /// 获取特定资源的使用情况
    async fn get_resource_usage(
        system_monitor: &Arc<Mutex<sysinfo::System>>,
        process_id: u32,
        resource_type: &ResourceType,
    ) -> Result<u64> {
        let mut system = system_monitor.lock().await;
        system.refresh_processes();

        let pid = sysinfo::Pid::from_u32(process_id);

        if let Some(process) = system.process(pid) {
            let usage = match resource_type {
                ResourceType::Memory => process.memory(),
                ResourceType::Cpu => (process.cpu_usage() * 100.0) as u64,
                ResourceType::DiskIo => {
                    // sysinfo 在某些平台上可能不支持磁盘IO统计
                    0 // 暂时返回0，后续可以通过其他方式获取
                }
                ResourceType::NetworkIo => {
                    // 网络IO统计需要额外的实现
                    0 // 暂时返回0
                }
                ResourceType::FileDescriptors => {
                    // 文件描述符统计需要平台特定的实现
                    0 // 暂时返回0
                }
                ResourceType::Threads => {
                    // sysinfo 可能不直接提供线程数，需要其他方式获取
                    1 // 暂时返回1
                }
                ResourceType::Processes => 1, // 单个进程
            };

            Ok(usage)
        } else {
            Err(anyhow::anyhow!("Process {} not found", process_id))
        }
    }

    /// 生成资源使用摘要
    async fn generate_summary(&self) -> ResourceSummary {
        let usage_map = self.current_usage.read().await;
        let history = self.usage_history.read().await;

        let mut summaries = HashMap::new();
        let total_duration = self.start_time.elapsed();

        for (resource_type, usage) in usage_map.iter() {
            // 计算平均值
            let average = if !history.is_empty() {
                let values: Vec<u64> = history
                    .iter()
                    .filter_map(|(_, data)| data.get(resource_type))
                    .cloned()
                    .collect();

                if !values.is_empty() {
                    values.iter().sum::<u64>() as f64 / values.len() as f64
                } else {
                    usage.current_usage as f64
                }
            } else {
                usage.current_usage as f64
            };

            let mut summary = usage.clone();
            summary.average_usage = average;
            summaries.insert(resource_type.clone(), summary);
        }

        ResourceSummary {
            process_id: self.process_id,
            function_name: self.function_name.clone(),
            total_duration_ms: total_duration.as_millis() as u64,
            resource_usage: summaries,
            quota_name: self.quota.name.clone(),
            monitoring_ended_at: chrono::Utc::now(),
        }
    }

    /// 获取当前资源使用情况
    pub async fn get_current_usage(&self) -> HashMap<ResourceType, ResourceUsage> {
        let usage_map = self.current_usage.read().await;
        usage_map.clone()
    }

    /// 检查是否有资源超限
    pub async fn has_violations(&self) -> bool {
        let usage_map = self.current_usage.read().await;
        usage_map
            .values()
            .any(|usage| matches!(usage.exceeded_type, Some(LimitType::Hard)))
    }

    /// 检查是否有软限制警告
    pub async fn has_warnings(&self) -> bool {
        let usage_map = self.current_usage.read().await;
        usage_map
            .values()
            .any(|usage| matches!(usage.exceeded_type, Some(LimitType::Soft)))
    }
}

/// 资源使用摘要
#[derive(Debug, Clone, Serialize)]
pub struct ResourceSummary {
    /// 进程ID
    pub process_id: u32,
    /// 函数名称
    pub function_name: String,
    /// 总执行时间（毫秒）
    pub total_duration_ms: u64,
    /// 各种资源的使用情况
    pub resource_usage: HashMap<ResourceType, ResourceUsage>,
    /// 使用的配额名称
    pub quota_name: String,
    /// 监控结束时间
    pub monitoring_ended_at: chrono::DateTime<chrono::Utc>,
}

/// 资源管理器
#[derive(Debug)]
pub struct ResourceManager {
    /// 预定义的资源配额
    quotas: Arc<RwLock<HashMap<String, ResourceQuota>>>,
    /// 活跃的进程监控器
    active_monitors: Arc<RwLock<HashMap<u32, ProcessResourceMonitor>>>,
    /// 资源事件历史
    event_history: Arc<RwLock<Vec<ResourceEvent>>>,
}

impl ResourceManager {
    /// 创建新的资源管理器
    pub fn new() -> Self {
        let mut quotas = HashMap::new();
        quotas.insert("default".to_string(), ResourceQuota::default());

        Self {
            quotas: Arc::new(RwLock::new(quotas)),
            active_monitors: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 添加或更新资源配额
    pub async fn set_quota(&self, quota: ResourceQuota) {
        let mut quotas = self.quotas.write().await;
        quotas.insert(quota.name.clone(), quota);
    }

    /// 获取资源配额
    pub async fn get_quota(&self, name: &str) -> Option<ResourceQuota> {
        let quotas = self.quotas.read().await;
        quotas.get(name).cloned()
    }

    /// 开始监控进程
    pub async fn start_monitoring(
        &self,
        process_id: u32,
        function_name: String,
        quota_name: Option<String>,
    ) -> Result<()> {
        let quota_name = quota_name.unwrap_or_else(|| "default".to_string());
        let quota = self
            .get_quota(&quota_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Quota '{}' not found", quota_name))?;

        let monitor = ProcessResourceMonitor::new(process_id, function_name, quota);
        monitor.start_monitoring().await?;

        let mut monitors = self.active_monitors.write().await;
        monitors.insert(process_id, monitor);

        Ok(())
    }

    /// 停止监控进程
    pub async fn stop_monitoring(&self, process_id: u32) -> Result<Option<ResourceSummary>> {
        let mut monitors = self.active_monitors.write().await;

        if let Some(monitor) = monitors.remove(&process_id) {
            let summary = monitor.stop_monitoring().await?;
            Ok(Some(summary))
        } else {
            Ok(None)
        }
    }

    /// 获取所有活跃监控
    pub async fn get_active_monitors(&self) -> Vec<u32> {
        let monitors = self.active_monitors.read().await;
        monitors.keys().cloned().collect()
    }

    /// 检查特定进程是否有违规
    pub async fn check_violations(&self, process_id: u32) -> Option<bool> {
        let monitors = self.active_monitors.read().await;
        if let Some(monitor) = monitors.get(&process_id) {
            Some(monitor.has_violations().await)
        } else {
            None
        }
    }

    /// 清理所有监控
    pub async fn cleanup_all(&self) -> Result<Vec<ResourceSummary>> {
        let mut monitors = self.active_monitors.write().await;
        let mut summaries = Vec::new();

        for (process_id, monitor) in monitors.drain() {
            match monitor.stop_monitoring().await {
                Ok(summary) => summaries.push(summary),
                Err(e) => tracing::warn!(
                    "Failed to stop monitoring for process {}: {}",
                    process_id,
                    e
                ),
            }
        }

        Ok(summaries)
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_quota_default() {
        let quota = ResourceQuota::default();
        assert_eq!(quota.name, "default");
        assert!(quota.enabled);
        assert!(quota.limits.contains_key(&ResourceType::Memory));
        assert!(quota.limits.contains_key(&ResourceType::Cpu));
    }

    #[test]
    fn test_resource_limit_default() {
        let limit = ResourceLimit::default();
        assert_eq!(limit.resource_type, ResourceType::Memory);
        assert!(limit.enabled);
        assert!(limit.soft_limit < limit.hard_limit);
    }

    #[tokio::test]
    async fn test_resource_manager_creation() {
        let manager = ResourceManager::new();
        let quota = manager.get_quota("default").await;
        assert!(quota.is_some());
    }

    #[tokio::test]
    async fn test_quota_management() {
        let manager = ResourceManager::new();

        let custom_quota = ResourceQuota {
            name: "high-memory".to_string(),
            ..Default::default()
        };

        manager.set_quota(custom_quota.clone()).await;
        let retrieved = manager.get_quota("high-memory").await;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "high-memory");
    }
}
