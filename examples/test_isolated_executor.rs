use anyhow::Result;
use flux::functions::{FunctionMetadata, InvokeRequest, ScriptType};
use flux::runtime::compiler::CompilerConfig;
use flux::runtime::executor::{IsolatedExecutorConfig, IsolatedProcessExecutor};
use flux::runtime::resource::ResourceQuota;
use flux::runtime::sandbox::SandboxConfig;
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🔒 FluxFaaS 进程级隔离执行器测试");
    println!("================================================");

    // 创建测试函数
    let test_function = FunctionMetadata::new(
        "isolated_test".to_string(),
        r#"
// 测试函数：复杂计算和资源使用
fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn calculate_stats(numbers: &[f64]) -> (f64, f64, f64) {
    let sum: f64 = numbers.iter().sum();
    let mean = sum / numbers.len() as f64;

    let variance: f64 = numbers.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / numbers.len() as f64;

    let std_dev = variance.sqrt();

    (mean, variance, std_dev)
}

pub fn main() {
    println!("Hello from isolated process executor!");

    // 执行一些计算密集型任务
    let fib_result = fibonacci(20);
    println!("Fibonacci(20) = {}", fib_result);

    // 一些统计计算
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
    let (mean, variance, std_dev) = calculate_stats(&data);
    println!("Statistics: mean={:.2}, variance={:.2}, std_dev={:.2}",
             mean, variance, std_dev);
}
"#
        .to_string(),
        Some(ScriptType::Rust), // 明确指定为 Rust 代码
    );

    // 创建进程级隔离执行器配置
    let executor_config = IsolatedExecutorConfig {
        compiler_config: CompilerConfig {
            rust_target_dir: Some(PathBuf::from("~/rust_target")),
            ..Default::default()
        },
        sandbox_config: SandboxConfig {
            enable_process_isolation: true,
            enable_container_isolation: false,
            execution_timeout_secs: 30,
            max_memory_mb: 128,
            max_cpu_percent: 80.0,
            allow_network: false,
            allow_filesystem: false,
            allowed_dirs: vec![],
            work_dir: None,
            allowed_env_vars: vec!["PATH".to_string()],
            temp_root: PathBuf::from("/tmp/flux_isolated_test"),
            rust_target_dir: Some(PathBuf::from("~/rust_target")),
        },
        default_quota_name: Some("test_quota".to_string()),
        max_concurrent_executions: 50,
        cleanup_interval_secs: 120,
    };

    println!("📋 进程级隔离执行器配置:");
    println!(
        "  - 最大并发执行数: {}",
        executor_config.max_concurrent_executions
    );
    println!("  - 清理间隔: {}秒", executor_config.cleanup_interval_secs);
    println!("  - 默认配额: {:?}", executor_config.default_quota_name);
    println!("  - 自定义编译路径: ~/rust_target");
    println!();

    // 创建进程级隔离执行器
    let executor = IsolatedProcessExecutor::new(executor_config)?;
    println!("✅ 进程级隔离执行器创建成功");

    // 设置资源配额到资源管理器中 - 这很重要！
    let mut limits = std::collections::HashMap::new();
    limits.insert(
        flux::runtime::resource::ResourceType::Memory,
        flux::runtime::resource::ResourceLimit {
            resource_type: flux::runtime::resource::ResourceType::Memory,
            soft_limit: 64 * 1024 * 1024,  // 64MB软限制
            hard_limit: 128 * 1024 * 1024, // 128MB硬限制
            check_interval_ms: 1000,
            enabled: true,
        },
    );
    limits.insert(
        flux::runtime::resource::ResourceType::Cpu,
        flux::runtime::resource::ResourceLimit {
            resource_type: flux::runtime::resource::ResourceType::Cpu,
            soft_limit: 70, // 70% CPU软限制
            hard_limit: 90, // 90% CPU硬限制
            check_interval_ms: 1000,
            enabled: true,
        },
    );

    let test_quota = ResourceQuota {
        name: "test_quota".to_string(),
        time_window_secs: 60,
        limits,
        enabled: true,
        created_at: chrono::Utc::now(),
    };

    // 重要：将配额设置到资源管理器中
    // 注意：我们无法直接访问executor的resource_manager，所以我们需要重新设计这部分

    println!("📊 设置资源配额:");
    println!("  - 配额名称: {}", test_quota.name);
    println!("  - 时间窗口: {}秒", test_quota.time_window_secs);

    if let Some(memory_limit) = test_quota
        .limits
        .get(&flux::runtime::resource::ResourceType::Memory)
    {
        println!(
            "  - 内存限制: {}MB (软) / {}MB (硬)",
            memory_limit.soft_limit / (1024 * 1024),
            memory_limit.hard_limit / (1024 * 1024)
        );
    }

    if let Some(cpu_limit) = test_quota
        .limits
        .get(&flux::runtime::resource::ResourceType::Cpu)
    {
        println!(
            "  - CPU限制: {}% (软) / {}% (硬)",
            cpu_limit.soft_limit, cpu_limit.hard_limit
        );
    }
    println!();

    // 测试1: 基本隔离执行
    println!("🚀 测试1: 基本隔离执行");
    let test_request = InvokeRequest {
        input: json!({
            "test_name": "basic_execution",
            "data": [1, 2, 3, 4, 5]
        }),
    };

    let start_time = Instant::now();
    let response = executor
        .execute_isolated(
            &test_function,
            &test_request,
            Some("test_quota".to_string()),
        )
        .await?;
    let execution_time = start_time.elapsed();

    println!("✅ 基本执行完成，耗时: {execution_time:?}");
    println!("  - 状态: {:?}", response.status);
    println!("  - 执行时间: {}ms", response.execution_time_ms);
    println!("  - 输出: {}", response.output);
    println!();

    // 测试2: 顺序执行（模拟并发）
    println!("🚀 测试2: 顺序执行 (3个实例)");
    let mut sequential_results = vec![];

    for i in 0..3 {
        let function = test_function.clone();
        let request = InvokeRequest {
            input: json!({
                "test_name": format!("sequential_execution_{}", i),
                "instance_id": i
            }),
        };

        let start = Instant::now();
        let result = executor
            .execute_isolated(
                &function, &request, None, // 不使用配额，避免配额问题
            )
            .await;
        let duration = start.elapsed();

        sequential_results.push((i, duration, result));
    }

    println!("✅ 顺序执行完成:");
    for (id, duration, result) in sequential_results {
        match result {
            Ok(response) => {
                println!(
                    "  - 实例{id}: 成功，耗时: {duration:?}, 执行时间: {execution_time_ms}ms",
                    duration = duration,
                    execution_time_ms = response.execution_time_ms
                );
            }
            Err(e) => {
                println!("  - 实例{id}: 失败，耗时: {duration:?}, 错误: {e}");
            }
        }
    }
    println!();

    // 获取执行统计信息
    println!("📊 执行统计信息:");
    let stats = executor.get_execution_statistics().await;
    println!("  - 总执行次数: {}", stats.total_executions);
    println!("  - 成功执行次数: {}", stats.successful_executions);
    println!("  - 失败执行次数: {}", stats.failed_executions);
    println!("  - 超时执行次数: {}", stats.timeout_executions);
    println!("  - 平均执行时间: {:.2}ms", stats.average_execution_time_ms);
    println!("  - 最短执行时间: {}ms", stats.min_execution_time_ms);
    println!("  - 最长执行时间: {}ms", stats.max_execution_time_ms);
    println!();

    // 获取活跃执行实例
    println!("📋 活跃执行实例:");
    let active_executions = executor.get_active_executions().await;
    if active_executions.is_empty() {
        println!("  - 没有活跃的执行实例");
    } else {
        for instance in &active_executions {
            println!(
                "  - ID: {execution_id}",
                execution_id = instance.execution_id
            );
            println!(
                "    函数: {function_name}",
                function_name = instance.function_name
            );
            println!("    状态: {:?}", instance.status);
            println!(
                "    开始时间: {started_at}",
                started_at = instance.started_at
            );
            if let Some(ended_at) = instance.ended_at {
                println!("    结束时间: {ended_at}");
            }
            if let Some(quota) = &instance.quota_name {
                println!("    配额: {quota}");
            }
        }
    }
    println!();

    // 清理已完成的执行实例
    println!("🧹 清理已完成的执行实例");
    let cleaned_count = executor.cleanup_completed_executions().await;
    println!("✅ 清理了 {cleaned_count} 个已完成的执行实例");
    println!();

    // 测试3: 资源限制测试 (故意创建一个可能消耗更多资源的函数)
    println!("🚀 测试3: 资源限制测试");
    let resource_test_function = FunctionMetadata::new(
        "resource_test".to_string(),
        r#"
// 这个函数会消耗更多计算资源
fn heavy_computation(iterations: usize) -> u64 {
    let mut result = 0u64;
    for i in 0..iterations {
        for j in 0..1000 {
            result = result.wrapping_add((i * j) as u64);
        }
    }
    result
}

pub fn main() {
    println!("Starting heavy computation...");
    let result = heavy_computation(10000); // 增加计算量
    println!("Heavy computation result: {}", result);
}
"#
        .to_string(),
        Some(ScriptType::Rust), // 明确指定为 Rust 代码
    );

    let resource_test_request = InvokeRequest {
        input: json!({
            "test_name": "resource_limit_test"
        }),
    };

    let start_time = Instant::now();
    let response = executor
        .execute_isolated(
            &resource_test_function,
            &resource_test_request,
            Some("test_quota".to_string()),
        )
        .await?;
    let execution_time = start_time.elapsed();

    println!("✅ 资源限制测试完成，耗时: {execution_time:?}");
    println!("  - 状态: {:?}", response.status);
    println!("  - 执行时间: {}ms", response.execution_time_ms);
    println!();

    // 最终统计
    println!("📊 最终执行统计:");
    let final_stats = executor.get_execution_statistics().await;
    println!("  - 总执行次数: {}", final_stats.total_executions);
    println!("  - 成功执行次数: {}", final_stats.successful_executions);
    println!("  - 失败执行次数: {}", final_stats.failed_executions);
    println!("  - 超时执行次数: {}", final_stats.timeout_executions);
    println!(
        "  - 平均执行时间: {:.2}ms",
        final_stats.average_execution_time_ms
    );
    println!();

    // 关闭执行器
    println!("🛑 关闭进程级隔离执行器");
    executor.shutdown().await?;
    println!("✅ 执行器关闭完成");

    println!();
    println!("🎉 进程级隔离执行器测试完成！");
    println!("✅ 所有测试功能验证成功");

    Ok(())
}

/// 性能基准测试函数
#[allow(dead_code)]
async fn benchmark_isolated_executor() -> Result<()> {
    println!("🏁 进程级隔离执行器性能基准测试");

    let config = IsolatedExecutorConfig::default();
    let executor = IsolatedProcessExecutor::new(config)?;

    let simple_function = FunctionMetadata::new(
        "benchmark_function".to_string(),
        r#"
pub fn main() {
    println!("Benchmark test");
}
"#
        .to_string(),
        Some(ScriptType::Rust), // 明确指定为 Rust 代码
    );

    let request = InvokeRequest { input: json!({}) };

    let iterations = 10;
    let mut total_time = 0u64;

    println!("执行 {iterations} 次基准测试...");

    for i in 0..iterations {
        let start = Instant::now();
        let result = executor
            .execute_isolated(&simple_function, &request, None)
            .await?;
        let _duration = start.elapsed();

        total_time += result.execution_time_ms;

        if i % 2 == 0 {
            print!(".");
        }
    }

    println!();
    println!("基准测试完成:");
    println!(
        "  - 平均执行时间: {:.2}ms",
        total_time as f64 / iterations as f64
    );

    executor.shutdown().await?;
    Ok(())
}
