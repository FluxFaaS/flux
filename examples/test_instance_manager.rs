use anyhow::Result;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::{Duration, sleep};

use flux::functions::{FunctionMetadata, InvokeRequest};
use flux::runtime::compiler::{CompilerConfig, RustCompiler};
use flux::runtime::instance::{InstanceConfig, InstanceManager};
use flux::runtime::resource::ResourceManager;
use flux::runtime::sandbox::{SandboxConfig, SandboxExecutor};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🚀 FluxFaaS 函数实例管理器测试");
    println!("=====================================");

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
        max_idle_duration_secs: 60,
        enable_auto_warm: true,
        ..Default::default()
    };

    let manager = InstanceManager::new(compiler, sandbox, resource_manager, Some(instance_config));

    // 测试1: 创建函数实例
    println!("\n🔧 测试1: 创建函数实例");
    let function_metadata = FunctionMetadata {
        id: scru128::new(),
        name: "hello_world".to_string(),
        description: "Hello World 函数".to_string(),
        code: r#"
fn hello_world(input: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "message": "Hello, World!",
        "input": input
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
        return_type: "serde_json::Value".to_string(),
    };

    let instance_id = manager
        .create_instance(function_metadata.clone(), None)
        .await?;
    println!("✅ 实例创建成功: {instance_id}");

    // 等待实例就绪
    sleep(Duration::from_secs(2)).await;

    // 测试2: 获取实例信息
    println!("\n📊 测试2: 获取实例信息");
    if let Some(instance) = manager.get_instance(&instance_id).await {
        println!("📋 实例信息:");
        println!("  - ID: {}", instance.instance_id);
        println!("  - 函数名: {}", instance.function_name);
        println!("  - 状态: {:?}", instance.state);
        println!("  - 创建时间: {}", instance.created_at);
        println!("  - 版本: {}", instance.version);
    } else {
        println!("❌ 未找到实例");
    }

    // 测试3: 执行函数实例
    println!("\n⚡ 测试3: 执行函数实例");
    let request = InvokeRequest {
        input: serde_json::json!({"name": "FluxFaaS"}),
    };

    match manager.execute_instance(&instance_id, &request).await {
        Ok(response) => {
            println!("✅ 执行成功:");
            println!("  - 输出: {}", response.output);
            println!("  - 执行时间: {}ms", response.execution_time_ms);
            println!("  - 状态: {:?}", response.status);
        }
        Err(e) => {
            println!("❌ 执行失败: {e}");
        }
    }

    // 测试4: 创建更多实例
    println!("\n🔄 测试4: 创建更多实例");
    let add_function = FunctionMetadata {
        id: scru128::new(),
        name: "add_numbers".to_string(),
        description: "数字相加函数".to_string(),
        code: r#"
fn add_numbers(input: serde_json::Value) -> serde_json::Value {
    let a = input["a"].as_i64().unwrap_or(0);
    let b = input["b"].as_i64().unwrap_or(0);
    serde_json::json!({
        "result": a + b,
        "operation": "addition"
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
        return_type: "serde_json::Value".to_string(),
    };

    let add_instance_id = manager.create_instance(add_function, None).await?;
    println!("✅ 加法函数实例创建成功: {add_instance_id}");

    // 等待实例就绪
    sleep(Duration::from_secs(2)).await;

    // 测试5: 执行加法函数
    println!("\n➕ 测试5: 执行加法函数");
    let add_request = InvokeRequest {
        input: serde_json::json!({"a": 15, "b": 27}),
    };

    match manager
        .execute_instance(&add_instance_id, &add_request)
        .await
    {
        Ok(response) => {
            println!("✅ 加法执行成功:");
            println!("  - 输出: {}", response.output);
            println!("  - 执行时间: {}ms", response.execution_time_ms);
        }
        Err(e) => {
            println!("❌ 加法执行失败: {e}");
        }
    }

    // 测试6: 获取管理器统计信息
    println!("\n📈 测试6: 获取管理器统计信息");
    let stats = manager.get_instance_stats().await;
    println!("📊 实例管理器统计:");
    println!("  - 总实例数: {}", stats.total_instances);
    println!("  - 活跃函数数: {}", stats.active_functions);
    println!("  - 就绪实例数: {}", stats.ready_instances);
    println!("  - 运行中实例数: {}", stats.running_instances);
    println!("  - 空闲实例数: {}", stats.idle_instances);
    println!("  - 预热中实例数: {}", stats.warming_instances);
    println!("  - 错误实例数: {}", stats.error_instances);
    println!("  - 总执行次数: {}", stats.total_executions);
    println!("  - 成功执行次数: {}", stats.successful_executions);
    println!("  - 失败执行次数: {}", stats.failed_executions);

    // 测试7: 获取所有实例
    println!("\n📋 测试7: 获取所有实例");
    let all_instances = manager.get_all_instances().await;
    println!("📦 所有活跃实例 ({} 个):", all_instances.len());
    for instance in &all_instances {
        println!(
            "  - {}: {} (状态: {:?})",
            instance.instance_id, instance.function_name, instance.state
        );
        println!(
            "    执行统计: 总数={}, 成功={}, 失败={}",
            instance.execution_stats.total_executions,
            instance.execution_stats.successful_executions,
            instance.execution_stats.failed_executions
        );
    }

    // 测试8: 获取特定函数的实例
    println!("\n🔍 测试8: 获取特定函数的实例");
    let hello_instances = manager.get_function_instances("hello_world").await;
    println!("🎯 hello_world 函数的实例:");
    for instance_id in &hello_instances {
        println!("  - {instance_id}");
    }

    // 测试9: 多次执行测试性能
    println!("\n🏃 测试9: 多次执行测试性能");
    let iterations = 5;
    println!("执行 {iterations} 次 hello_world 函数:");

    for i in 1..=iterations {
        let test_request = InvokeRequest {
            input: serde_json::json!({"iteration": i}),
        };

        let start = std::time::Instant::now();
        match manager.execute_instance(&instance_id, &test_request).await {
            Ok(response) => {
                let duration = start.elapsed();
                println!(
                    "  第{i}次: ✅ 成功 ({response}ms, 总时间: {duration}ms)",
                    response = response.execution_time_ms,
                    duration = duration.as_millis()
                );
            }
            Err(e) => {
                println!("  第{i}次: ❌ 失败 - {e}");
            }
        }
    }

    // 测试10: 获取生命周期事件
    println!("\n📜 测试10: 获取生命周期事件");
    let events = manager.get_lifecycle_events(Some(10)).await;
    println!("🔄 最近的生命周期事件 ({} 个):", events.len());
    for event in &events {
        println!(
            "  - [{}] {}: {:?} - {}",
            event.timestamp.format("%H:%M:%S"),
            event.function_name,
            event.event_type,
            event.description
        );
    }

    // 测试11: 停止实例
    println!("\n🛑 测试11: 停止实例");
    println!("停止 hello_world 实例...");
    match manager.stop_instance(&instance_id).await {
        Ok(_) => println!("✅ 实例停止成功"),
        Err(e) => println!("❌ 实例停止失败: {e}"),
    }

    // 最终统计
    println!("\n📊 最终统计:");
    let final_stats = manager.get_instance_stats().await;
    println!("  - 剩余实例数: {}", final_stats.total_instances);
    println!("  - 活跃函数数: {}", final_stats.active_functions);

    // 清理
    println!("\n🧹 清理资源...");
    manager.cleanup().await?;
    println!("✅ 清理完成");

    println!("\n🎉 函数实例管理器测试完成!");
    Ok(())
}
