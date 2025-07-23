use anyhow::Result;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::{Duration, sleep};

use flux::functions::{FunctionMetadata, InvokeRequest, ReturnType, ScriptType};
use flux::runtime::compiler::{CompilerConfig, RustCompiler};
use flux::runtime::instance::{InstanceConfig, InstanceManager};
use flux::runtime::resource::ResourceManager;
use flux::runtime::sandbox::{SandboxConfig, SandboxExecutor};
use flux::scheduler::pool::{LoadBalanceStrategy, PoolConfig, PoolManager};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🚀 FluxFaaS 函数实例池管理器测试");
    println!("==========================================");

    // 创建临时目录
    let temp_dir = TempDir::new()?;
    println!("📁 临时目录: {:?}", temp_dir.path());

    // 配置编译器
    let compiler_config = CompilerConfig {
        cache_dir: temp_dir.path().join("cache"),
        opt_level: 2,
        ..Default::default()
    };

    // 配置沙箱
    let sandbox_config = SandboxConfig {
        execution_timeout_secs: 30,
        max_memory_mb: 64,
        temp_root: temp_dir.path().join("sandbox").to_path_buf(),
        ..Default::default()
    };

    // 创建组件
    let compiler = Arc::new(RustCompiler::new(compiler_config)?);
    let sandbox = Arc::new(SandboxExecutor::new(sandbox_config)?);
    let resource_manager = Arc::new(ResourceManager::new());

    // 创建实例管理器
    let instance_config = InstanceConfig {
        max_idle_duration_secs: 120,
        enable_auto_warm: true,
        ..Default::default()
    };

    let instance_manager = Arc::new(InstanceManager::new(
        compiler,
        sandbox,
        resource_manager,
        Some(instance_config.clone()),
    ));

    // 创建池管理器
    let pool_config = PoolConfig {
        min_instances: 1,
        max_instances: 5,
        target_instances: 2,
        scale_up_threshold: 0.7,
        scale_down_threshold: 0.2,
        scale_up_cooldown_secs: 30,
        scale_down_cooldown_secs: 60,
        warm_new_instances: true,
        health_check_interval_secs: 15,
        load_balance_strategy: LoadBalanceStrategy::RoundRobin,
        instance_config,
    };

    let pool_manager = PoolManager::new(instance_manager, Some(pool_config.clone()));

    // 测试1: 创建函数池
    println!("\n🔧 测试1: 创建函数池");
    let function_metadata = FunctionMetadata {
        id: scru128::new(),
        name: "hello_pool".to_string(),
        description: "Hello Pool 函数".to_string(),
        code: r#"
fn hello_pool(input: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "message": "Hello from pool!",
        "input": input,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}
"#
        .to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        timeout_ms: 5000,
        version: "1.0.0".to_string(),
        dependencies: vec![],
        parameters: vec![],
        return_type: ReturnType::Any,
        script_type: ScriptType::Rust,
    };

    let pool = pool_manager
        .create_pool(function_metadata.clone(), None)
        .await?;
    println!("✅ 函数池创建成功");

    // 等待池初始化完成
    sleep(Duration::from_secs(5)).await;

    // 测试2: 获取池状态
    println!("\n📊 测试2: 获取池状态");
    let state = pool.get_state().await;
    println!("📋 池状态: {state:?}");

    // 测试3: 获取池统计信息
    println!("\n📈 测试3: 获取池统计信息");
    let stats = pool.get_stats().await;
    println!("📊 池统计信息:");
    println!("  - 总实例数: {}", stats.total_instances);
    println!("  - 健康实例数: {}", stats.healthy_instances);
    println!("  - 当前负载: {:.2}", stats.current_load);
    println!("  - 活跃连接数: {}", stats.active_connections);
    println!("  - 平均响应时间: {:.2}ms", stats.avg_response_time_ms);

    // 测试4: 执行函数请求
    println!("\n⚡ 测试4: 执行函数请求");
    let request = InvokeRequest {
        input: serde_json::json!({"name": "Pool Test", "iteration": 1}),
    };

    match pool.execute(&request).await {
        Ok(response) => {
            println!("✅ 执行成功:");
            println!("  - 输出: {output}", output = response.output);
            println!(
                "  - 执行时间: {execution_time}ms",
                execution_time = response.execution_time_ms
            );
            println!("  - 状态: {:?}", response.status);
        }
        Err(e) => {
            println!("❌ 执行失败: {e}");
        }
    }

    // 测试5: 多次并发执行测试负载均衡
    println!("\n🔄 测试5: 多次并发执行测试负载均衡");
    let concurrent_requests = 8;
    println!("发起 {concurrent_requests} 个并发请求:");

    let mut handles = Vec::new();
    for i in 0..concurrent_requests {
        let pool_clone = pool.clone();
        let request = InvokeRequest {
            input: serde_json::json!({"name": "Concurrent Test", "iteration": i + 1}),
        };

        let handle = tokio::spawn(async move {
            let start = std::time::Instant::now();
            let result = pool_clone.execute(&request).await;
            let duration = start.elapsed();
            (i + 1, result, duration)
        });

        handles.push(handle);
    }

    // 等待所有请求完成
    for handle in handles {
        match handle.await {
            Ok((iteration, result, duration)) => match result {
                Ok(response) => {
                    println!(
                        "  请求{iteration}: ✅ 成功 ({response}ms, 总时间: {duration}ms)",
                        response = response.execution_time_ms,
                        duration = duration.as_millis()
                    );
                }
                Err(e) => {
                    println!("  请求{iteration}: ❌ 失败 - {e}");
                }
            },
            Err(e) => {
                println!("  任务失败: {e}");
            }
        }
    }

    // 测试6: 检查负载均衡后的统计信息
    println!("\n📊 测试6: 检查负载均衡后的统计信息");
    let stats_after = pool.get_stats().await;
    println!("📈 更新后的统计信息:");
    println!("  - 总实例数: {}", stats_after.total_instances);
    println!("  - 健康实例数: {}", stats_after.healthy_instances);
    println!("  - 当前负载: {:.2}", stats_after.current_load);
    println!("  - 活跃连接数: {}", stats_after.active_connections);
    println!(
        "  - 平均响应时间: {:.2}ms",
        stats_after.avg_response_time_ms
    );

    // 测试7: 创建另一个函数池
    println!("\n🔄 测试7: 创建另一个函数池");
    let calculator_function = FunctionMetadata {
        id: scru128::new(),
        name: "calculator_pool".to_string(),
        description: "Calculator Pool 函数".to_string(),
        code: r#"
fn calculator_pool(input: serde_json::Value) -> serde_json::Value {
    let operation = input["operation"].as_str().unwrap_or("add");
    let a = input["a"].as_f64().unwrap_or(0.0);
    let b = input["b"].as_f64().unwrap_or(0.0);

    let result = match operation {
        "add" => a + b,
        "subtract" => a - b,
        "multiply" => a * b,
        "divide" => if b != 0.0 { a / b } else { f64::NAN },
        _ => f64::NAN,
    };

    serde_json::json!({
        "operation": operation,
        "operands": [a, b],
        "result": result,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}
"#
        .to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        timeout_ms: 5000,
        version: "1.0.0".to_string(),
        dependencies: vec![],
        parameters: vec![],
        return_type: ReturnType::Any,
        script_type: ScriptType::Rust,
    };

    let calculator_pool_config = PoolConfig {
        min_instances: 1,
        max_instances: 3,
        target_instances: 1,
        load_balance_strategy: LoadBalanceStrategy::LeastConnections,
        ..pool_config.clone()
    };

    let calculator_pool = pool_manager
        .create_pool(calculator_function, Some(calculator_pool_config))
        .await?;
    println!("✅ 计算器函数池创建成功");

    // 等待池初始化
    sleep(Duration::from_secs(3)).await;

    // 测试8: 执行计算器函数
    println!("\n🧮 测试8: 执行计算器函数");
    let calc_requests = vec![
        ("add", 15.0, 27.0),
        ("subtract", 100.0, 25.0),
        ("multiply", 6.0, 7.0),
        ("divide", 84.0, 12.0),
    ];

    for (op, a, b) in calc_requests {
        let calc_request = InvokeRequest {
            input: serde_json::json!({
                "operation": op,
                "a": a,
                "b": b
            }),
        };

        match calculator_pool.execute(&calc_request).await {
            Ok(response) => {
                println!(
                    "  {op}({a}, {b}): ✅ 结果 = {result}",
                    result = response.output["result"]
                );
            }
            Err(e) => {
                println!("  {op}({a}, {b}): ❌ 失败 - {e}");
            }
        }
    }

    // 测试9: 获取所有池的统计信息
    println!("\n📊 测试9: 获取所有池的统计信息");
    let all_stats = pool_manager.get_all_stats().await;
    println!("📈 所有池的统计信息:");
    for (name, stats) in &all_stats {
        println!(
            "  池 '{name}': 实例数={total_instances}, 健康数={healthy_instances}, 负载={current_load:.2}",
            total_instances = stats.total_instances,
            healthy_instances = stats.healthy_instances,
            current_load = stats.current_load
        );
    }

    // 测试10: 测试手动扩容
    println!("\n📈 测试10: 测试手动扩容");
    println!(
        "当前hello_pool实例数: {}",
        pool.get_stats().await.total_instances
    );

    let scaled_count = pool.scale_up(4).await?;
    println!("✅ 扩容完成，新增实例数: {scaled_count}");

    sleep(Duration::from_secs(2)).await;
    println!("扩容后实例数: {}", pool.get_stats().await.total_instances);

    // 测试11: 测试手动缩容
    println!("\n📉 测试11: 测试手动缩容");
    let scaled_down_count = pool.scale_down(2).await?;
    println!("✅ 缩容完成，移除实例数: {scaled_down_count}");

    sleep(Duration::from_secs(2)).await;
    println!("缩容后实例数: {}", pool.get_stats().await.total_instances);

    // 测试12: 获取扩缩容历史
    println!("\n📜 测试12: 获取扩缩容历史");
    let scaling_history = pool.get_scaling_history(Some(10)).await;
    println!("🔄 扩缩容历史 ({} 个事件):", scaling_history.len());
    for event in &scaling_history {
        println!(
            "  - [{}] {:?}: {} -> {} ({})",
            event.timestamp.format("%H:%M:%S"),
            event.event_type,
            event.before_count,
            event.after_count,
            event.reason
        );
    }

    // 测试13: 暂停和恢复池
    println!("\n⏸️ 测试13: 暂停和恢复池");
    pool.pause().await?;
    println!("✅ 池已暂停，状态: {:?}", pool.get_state().await);

    sleep(Duration::from_secs(1)).await;

    pool.resume().await?;
    println!("✅ 池已恢复，状态: {:?}", pool.get_state().await);

    // 测试14: 性能测试
    println!("\n🏃 测试14: 性能测试");
    let performance_requests = 20;
    println!("执行 {performance_requests} 个连续请求测试性能:");

    let mut total_time = Duration::ZERO;
    let mut success_count = 0;

    for i in 1..=performance_requests {
        let perf_request = InvokeRequest {
            input: serde_json::json!({"name": "Performance Test", "iteration": i}),
        };

        let start = std::time::Instant::now();
        match pool.execute(&perf_request).await {
            Ok(response) => {
                let duration = start.elapsed();
                total_time += duration;
                success_count += 1;

                if i % 5 == 0 {
                    println!(
                        "  第{i}次: ✅ 成功 ({response}ms, 总时间: {duration}ms)",
                        response = response.execution_time_ms,
                        duration = duration.as_millis()
                    );
                }
            }
            Err(e) => {
                println!("  第{i}次: ❌ 失败 - {e}");
            }
        }
    }

    if success_count > 0 {
        let avg_time = total_time / success_count;
        println!("📊 性能测试结果:");
        println!("  - 成功请求数: {success_count}/{performance_requests}");
        println!(
            "  - 平均响应时间: {avg_time}ms",
            avg_time = avg_time.as_millis()
        );
        println!(
            "  - 总执行时间: {total_time}ms",
            total_time = total_time.as_millis()
        );
    }

    // 清理
    println!("\n🧹 清理资源...");
    pool_manager.cleanup().await?;
    println!("✅ 清理完成");

    println!("\n🎉 函数实例池管理器测试完成!");
    Ok(())
}
