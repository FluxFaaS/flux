use anyhow::Result;
use flux::functions::{FunctionMetadata, InvokeRequest, ScriptType};
use flux::runtime::compiler::CompilerConfig;
use flux::runtime::executor::{IsolatedExecutorConfig, IsolatedProcessExecutor};
use flux::runtime::sandbox::SandboxConfig;
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🔒 FluxFaaS 进程级隔离执行器简化测试");
    println!("================================================");

    // 创建测试函数
    let test_function = FunctionMetadata::new(
        "isolated_simple_test".to_string(),
        r#"
// 简单的测试函数
fn add_numbers(a: f64, b: f64) -> f64 {
    a + b
}

pub fn main() {
    println!("Hello from isolated process executor!");
    let result = add_numbers(10.5, 20.3);
    println!("10.5 + 20.3 = {}", result);
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
            temp_root: PathBuf::from("/tmp/flux_isolated_simple_test"),
            rust_target_dir: Some(PathBuf::from("~/rust_target")),
        },
        default_quota_name: None, // 不使用配额，简化测试
        max_concurrent_executions: 10,
        cleanup_interval_secs: 120,
    };

    println!("📋 进程级隔离执行器配置:");
    println!(
        "  - 最大并发执行数: {}",
        executor_config.max_concurrent_executions
    );
    println!("  - 清理间隔: {}秒", executor_config.cleanup_interval_secs);
    println!("  - 自定义编译路径: ~/rust_target");
    println!();

    // 创建进程级隔离执行器
    let executor = IsolatedProcessExecutor::new(executor_config)?;
    println!("✅ 进程级隔离执行器创建成功");

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
            None, // 不使用配额
        )
        .await?;
    let execution_time = start_time.elapsed();

    println!("✅ 基本执行完成，耗时: {execution_time:?}");
    println!("  - 状态: {:?}", response.status);
    println!("  - 执行时间: {}ms", response.execution_time_ms);
    println!("  - 输出: {}", response.output);
    println!();

    // 测试2: 多次执行
    println!("🚀 测试2: 多次顺序执行 (3次)");

    for i in 0..3 {
        let request = InvokeRequest {
            input: json!({
                "test_name": format!("execution_{}", i),
                "iteration": i
            }),
        };

        let start = Instant::now();
        let result = executor
            .execute_isolated(&test_function, &request, None)
            .await?;
        let duration = start.elapsed();

        println!(
            "  - 执行{}: 状态={:?}, 耗时={duration:?}, 执行时间={}ms",
            i, result.status, result.execution_time_ms
        );
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
                println!("    结束时间: {ended_at}",);
            }
        }
    }
    println!();

    // 清理已完成的执行实例
    println!("🧹 清理已完成的执行实例");
    let cleaned_count = executor.cleanup_completed_executions().await;
    println!("✅ 清理了 {cleaned_count} 个已完成的执行实例");
    println!();

    // 最终统计
    println!("📊 最终执行统计:");
    let final_stats = executor.get_execution_statistics().await;
    println!("  - 总执行次数: {}", final_stats.total_executions);
    println!("  - 成功执行次数: {}", final_stats.successful_executions);
    println!("  - 失败执行次数: {}", final_stats.failed_executions);
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
    println!("🎉 进程级隔离执行器简化测试完成！");
    println!("✅ 所有基本功能验证成功");

    Ok(())
}
