use crate::functions::{FunctionMetadata, Result};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// 缓存的函数执行结果
#[derive(Debug, Clone)]
pub struct CachedFunction {
    /// 函数元数据
    pub metadata: FunctionMetadata,
    /// 编译后的代码（简化版本，实际应该是编译结果）
    pub compiled_code: CompiledCode,
    /// 创建时间
    pub created_at: Instant,
    /// 最后访问时间
    pub last_accessed: Instant,
    /// 访问次数
    pub access_count: u64,
    /// 内存使用估算（字节）
    pub memory_usage: usize,
}

/// 编译后的代码（简化版本）
#[derive(Debug, Clone)]
pub struct CompiledCode {
    /// 原始代码
    pub source: String,
    /// 解析后的表达式树（简化版本）
    pub parsed_expressions: Vec<String>,
    /// 编译时间戳
    pub compiled_at: u64,
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存大小（条目数）
    pub size: usize,
    /// 当前内存使用（字节）
    pub memory_usage: usize,
    /// 最大内存限制（字节）
    pub max_memory: usize,
    /// 缓存驱逐次数
    pub evictions: u64,
}

/// 函数缓存管理器
#[derive(Debug)]
pub struct FunctionCache {
    /// LRU 缓存
    cache: Arc<RwLock<LruCache<String, CachedFunction>>>,
    /// 缓存统计
    stats: Arc<RwLock<CacheStats>>,
    /// 最大内存使用限制（字节）
    max_memory: usize,
    /// 缓存条目最大存活时间
    max_age: Duration,
}

impl FunctionCache {
    /// 创建新的函数缓存
    pub fn new(capacity: usize, max_memory_mb: usize, max_age_seconds: u64) -> Self {
        let max_memory = max_memory_mb * 1024 * 1024; // 转换为字节
        let max_age = Duration::from_secs(max_age_seconds);

        Self {
            cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap(),
            ))),
            stats: Arc::new(RwLock::new(CacheStats {
                max_memory,
                ..Default::default()
            })),
            max_memory,
            max_age,
        }
    }

    /// 获取缓存的函数
    pub async fn get(&self, function_name: &str) -> Option<CachedFunction> {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if let Some(cached_function) = cache.get_mut(function_name) {
            // 检查是否过期
            if cached_function.created_at.elapsed() > self.max_age {
                cache.pop(function_name);
                stats.misses += 1;
                stats.evictions += 1;
                tracing::debug!("Cache entry expired for function: {}", function_name);
                return None;
            }

            // 更新访问信息
            cached_function.last_accessed = Instant::now();
            cached_function.access_count += 1;
            stats.hits += 1;

            tracing::debug!(
                "Cache hit for function: {} (accessed {} times)",
                function_name,
                cached_function.access_count
            );

            Some(cached_function.clone())
        } else {
            stats.misses += 1;
            tracing::debug!("Cache miss for function: {}", function_name);
            None
        }
    }

    /// 缓存函数
    pub async fn put(&self, function_name: String, function: FunctionMetadata) -> Result<()> {
        // 编译函数代码（简化版本）
        let compiled_code = self.compile_function(&function).await?;
        let memory_usage = self.estimate_memory_usage(&function, &compiled_code);

        let cached_function = CachedFunction {
            metadata: function,
            compiled_code,
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            access_count: 0,
            memory_usage,
        };

        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        // 检查内存限制
        if stats.memory_usage + memory_usage > self.max_memory {
            // 需要清理缓存
            self.evict_to_fit(memory_usage, &mut cache, &mut stats)
                .await;
        }

        // 添加到缓存
        if let Some(old_function) = cache.put(function_name.clone(), cached_function) {
            // 更新内存使用统计
            stats.memory_usage = stats.memory_usage - old_function.memory_usage + memory_usage;
        } else {
            stats.memory_usage += memory_usage;
        }

        stats.size = cache.len();

        tracing::info!(
            "Cached function: {} (memory usage: {} bytes, total: {} bytes)",
            function_name,
            memory_usage,
            stats.memory_usage
        );

        Ok(())
    }

    /// 移除缓存的函数
    pub async fn remove(&self, function_name: &str) -> bool {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if let Some(removed_function) = cache.pop(function_name) {
            stats.memory_usage -= removed_function.memory_usage;
            stats.size = cache.len();
            tracing::info!("Removed function from cache: {}", function_name);
            true
        } else {
            false
        }
    }

    /// 清空缓存
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        cache.clear();
        stats.memory_usage = 0;
        stats.size = 0;
        stats.evictions += 1;

        tracing::info!("Cleared function cache");
    }

    /// 获取缓存统计信息
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let mut stats = self.stats.read().await.clone();
        stats.size = cache.len();
        stats
    }

    /// 获取缓存命中率
    pub async fn hit_rate(&self) -> f64 {
        let stats = self.stats.read().await;
        let total = stats.hits + stats.misses;
        if total == 0 {
            0.0
        } else {
            stats.hits as f64 / total as f64
        }
    }

    /// 清理过期的缓存条目
    pub async fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;
        let mut removed_count = 0;
        let mut removed_memory = 0;

        // 收集过期的键
        let mut expired_keys = Vec::new();
        let now = Instant::now();

        // 注意：LRU 缓存不支持直接迭代，所以我们使用不同的策略
        // 这里我们简化处理，只在 get 方法中检查过期
        let cache_len = cache.len();
        for _ in 0..cache_len {
            if let Some((key, value)) = cache.pop_lru() {
                if now.duration_since(value.created_at) > self.max_age {
                    expired_keys.push(key);
                    removed_memory += value.memory_usage;
                    removed_count += 1;
                } else {
                    // 如果没过期，重新放回缓存
                    cache.put(key, value);
                }
            }
        }

        stats.memory_usage -= removed_memory;
        stats.size = cache.len();
        stats.evictions += removed_count as u64;

        if removed_count > 0 {
            tracing::info!(
                "Cleaned up {} expired cache entries, freed {} bytes",
                removed_count,
                removed_memory
            );
        }

        removed_count
    }

    /// 编译函数代码（简化版本）
    async fn compile_function(&self, function: &FunctionMetadata) -> Result<CompiledCode> {
        // 这是一个简化的编译过程
        // 真实环境中应该进行语法分析、优化等
        let parsed_expressions = self.parse_code(&function.code)?;

        let compiled_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(CompiledCode {
            source: function.code.clone(),
            parsed_expressions,
            compiled_at,
        })
    }

    /// 解析代码为表达式（简化版本）
    fn parse_code(&self, code: &str) -> Result<Vec<String>> {
        // 简化的代码解析
        let mut expressions = Vec::new();

        // 提取 return 语句
        if let Some(return_part) = code.split("return").nth(1) {
            let cleaned = return_part.trim().trim_end_matches(';').trim();
            expressions.push(cleaned.to_string());
        }

        // 提取函数定义
        for line in code.lines() {
            if line.trim().starts_with("fn ") {
                expressions.push(line.trim().to_string());
            }
        }

        if expressions.is_empty() {
            expressions.push("default_expression".to_string());
        }

        Ok(expressions)
    }

    /// 估算内存使用量
    fn estimate_memory_usage(&self, function: &FunctionMetadata, compiled: &CompiledCode) -> usize {
        // 简化的内存使用估算
        let base_size = std::mem::size_of::<CachedFunction>();
        let metadata_size = function.code.len() + function.name.len() + function.description.len();
        let compiled_size = compiled.source.len()
            + compiled
                .parsed_expressions
                .iter()
                .map(|s| s.len())
                .sum::<usize>();

        base_size + metadata_size + compiled_size + 100 // 额外开销
    }

    /// 清理缓存以腾出空间
    async fn evict_to_fit(
        &self,
        needed_memory: usize,
        cache: &mut LruCache<String, CachedFunction>,
        stats: &mut CacheStats,
    ) {
        let mut freed_memory = 0;
        let mut evicted_count = 0;

        while stats.memory_usage + needed_memory > self.max_memory && !cache.is_empty() {
            if let Some((key, value)) = cache.pop_lru() {
                freed_memory += value.memory_usage;
                evicted_count += 1;
                tracing::debug!("Evicted function from cache: {}", key);
            } else {
                break;
            }
        }

        stats.memory_usage -= freed_memory;
        stats.evictions += evicted_count;

        if evicted_count > 0 {
            tracing::info!(
                "Evicted {} functions from cache, freed {} bytes",
                evicted_count,
                freed_memory
            );
        }
    }
}

impl Default for FunctionCache {
    fn default() -> Self {
        // 默认配置：100个条目，50MB内存，1小时过期
        Self::new(100, 50, 3600)
    }
}
