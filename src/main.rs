#![allow(clippy::uninlined_format_args)]

use std::io::{self, Write};
use tracing::info;

mod functions;
// mod gateway;  // 暂时禁用，等待 Silent API 研究
mod runtime;
mod scheduler;

use functions::registry::FunctionRegistry;
use functions::{FunctionMetadata, InvokeRequest, RegisterFunctionRequest};
use scheduler::{Scheduler, SimpleScheduler};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    info!("🚀 Starting FluxFaaS MVP with SCRU128 ID...");

    // 测试基础功能
    test_basic_functionality().await?;

    // 启动交互式 CLI
    run_interactive_cli().await?;

    Ok(())
}

/// 测试基础功能模块
async fn test_basic_functionality() -> anyhow::Result<()> {
    info!("🧪 Running basic functionality tests...");

    // 测试函数注册表
    let registry = FunctionRegistry::new();

    let hello_fn = FunctionMetadata::from_request(RegisterFunctionRequest {
        name: "hello".to_string(),
        description: Some("Hello world function".to_string()),
        code: "return \"Hello, World!\"".to_string(),
        timeout_ms: Some(1000),
    });

    registry.register(hello_fn).await?;
    info!("✅ Function registry test passed");

    // 测试函数执行
    let runtime = runtime::SimpleRuntime::new();
    let function = registry.get("hello").await?;
    let request = InvokeRequest {
        input: serde_json::json!({"test": "data"}),
    };

    let _response = runtime.execute(&function, &request).await?;
    info!("✅ Function execution test passed");

    // 测试调度器
    let scheduler = SimpleScheduler::new();
    let scheduler_registry = scheduler.registry();

    scheduler_registry.register(function).await?;
    let _response = scheduler.schedule("hello", request).await?;
    info!("✅ Scheduler test passed");

    info!("🎉 All core components working perfectly!");
    info!("💡 Now using SCRU128 IDs for better performance and ordering!");

    Ok(())
}

/// 运行交互式 CLI
async fn run_interactive_cli() -> anyhow::Result<()> {
    let scheduler = SimpleScheduler::new();

    // 预注册示例函数
    register_sample_functions(&scheduler).await?;

    loop {
        println!("\n🌟 FluxFaaS 交互式界面 (SCRU128 ID)");
        println!("====================================");
        println!("1. 📋 查看所有函数");
        println!("2. 🚀 调用函数");
        println!("3. ➕ 注册新函数");
        println!("4. 📁 从文件加载函数");
        println!("5. 📂 从目录批量加载函数");
        println!("6. 📊 查看系统状态");
        println!("7. 🎯 查看缓存统计");
        println!("8. 📊 查看性能监控");
        println!("9. 🔄 重置监控数据");
        println!("10. 🚪 退出");

        print!("\nflux> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => list_functions(&scheduler).await?,
            "2" => invoke_function(&scheduler).await?,
            "3" => register_new_function(&scheduler).await?,
            "4" => load_function_from_file(&scheduler).await?,
            "5" => load_functions_from_directory(&scheduler).await?,
            "6" => show_system_status(&scheduler).await?,
            "7" => show_cache_statistics(&scheduler).await?,
            "8" => show_performance_monitor(&scheduler).await?,
            "9" => reset_performance_data(&scheduler).await?,
            "10" | "q" | "quit" | "exit" => {
                println!("👋 再见！");
                break;
            }
            _ => println!("❌ 无效选项，请重新选择"),
        }
    }

    Ok(())
}

/// 预注册示例函数
async fn register_sample_functions(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    let registry = scheduler.registry();

    let sample_functions = vec![
        RegisterFunctionRequest {
            name: "hello".to_string(),
            description: Some("Hello World 函数".to_string()),
            code: "return \"Hello, World!\"".to_string(),
            timeout_ms: Some(5000),
        },
        RegisterFunctionRequest {
            name: "echo".to_string(),
            description: Some("回声函数".to_string()),
            code: "return input".to_string(),
            timeout_ms: Some(3000),
        },
        RegisterFunctionRequest {
            name: "add".to_string(),
            description: Some("加法函数".to_string()),
            code: "const {a, b} = JSON.parse(input); return (a + b).toString();".to_string(),
            timeout_ms: Some(2000),
        },
    ];

    for func_req in sample_functions {
        let metadata = FunctionMetadata::from_request(func_req);
        registry.register(metadata).await?;
    }

    info!("📚 Sample functions registered with SCRU128 IDs");
    Ok(())
}

/// 列出所有函数
async fn list_functions(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    let functions = scheduler.registry().list().await;

    if functions.is_empty() {
        println!("📭 No functions registered");
        return Ok(());
    }

    println!("📋 Registered Functions:");
    println!("------------------------");
    for (i, func) in functions.iter().enumerate() {
        let description = if func.description.is_empty() {
            "No description"
        } else {
            &func.description
        };
        println!("{}. {} - {}", i + 1, func.name, description);
        println!("   🆔 SCRU128 ID: {}", func.id);
        println!(
            "   📅 Created: {}",
            func.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("   ⏱️  Timeout: {}ms", func.timeout_ms);
        println!();
    }
    println!("Total: {} functions", functions.len());

    Ok(())
}

/// 调用函数
async fn invoke_function(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    let functions = scheduler.registry().list().await;

    if functions.is_empty() {
        println!("❌ 没有可用的函数");
        return Ok(());
    }

    println!("📋 可用函数:");
    for (i, func) in functions.iter().enumerate() {
        println!("{}. {} (ID: {})", i + 1, func.name, func.id);
    }

    print!("请选择要调用的函数 (输入编号): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let choice: usize = input.trim().parse().unwrap_or(0);
    if choice == 0 || choice > functions.len() {
        println!("❌ 无效选择");
        return Ok(());
    }

    let selected_func = &functions[choice - 1];

    print!("请输入函数参数 (JSON 格式，例如: {{\"a\": 1, \"b\": 2}}): ");
    io::stdout().flush()?;

    let mut input_data = String::new();
    io::stdin().read_line(&mut input_data)?;

    let input_json: serde_json::Value = match serde_json::from_str(input_data.trim()) {
        Ok(json) => json,
        Err(_) => serde_json::json!(input_data.trim()),
    };

    let request = InvokeRequest { input: input_json };

    println!("🚀 正在调用函数 '{}'...", selected_func.name);
    println!("   🆔 函数 ID: {}", selected_func.id);

    match scheduler.schedule(&selected_func.name, request).await {
        Ok(response) => {
            println!("✅ 函数执行成功!");
            println!(
                "📤 输出: {}",
                serde_json::to_string_pretty(&response.output)?
            );
            println!("⏱️  执行时间: {}ms", response.execution_time_ms);
            println!("📊 状态: {:?}", response.status);
        }
        Err(e) => {
            println!("❌ 函数执行失败: {e}");
        }
    }

    Ok(())
}

/// 注册新函数
async fn register_new_function(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    println!("➕ 注册新函数（将生成 SCRU128 ID）");

    print!("函数名称: ");
    io::stdout().flush()?;
    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();

    if name.is_empty() {
        println!("❌ 函数名称不能为空");
        return Ok(());
    }

    print!("函数描述 (可选): ");
    io::stdout().flush()?;
    let mut description = String::new();
    io::stdin().read_line(&mut description)?;
    let description = description.trim();

    print!("函数代码: ");
    io::stdout().flush()?;
    let mut code = String::new();
    io::stdin().read_line(&mut code)?;
    let code = code.trim().to_string();

    if code.is_empty() {
        println!("❌ 函数代码不能为空");
        return Ok(());
    }

    print!("超时时间 (毫秒，默认 5000): ");
    io::stdout().flush()?;
    let mut timeout_input = String::new();
    io::stdin().read_line(&mut timeout_input)?;
    let timeout_ms = timeout_input.trim().parse().unwrap_or(5000);

    let request = RegisterFunctionRequest {
        name: name.clone(),
        description: if description.is_empty() {
            None
        } else {
            Some(description.to_string())
        },
        code,
        timeout_ms: Some(timeout_ms),
    };

    let metadata = FunctionMetadata::from_request(request);
    let function_id = metadata.id;

    match scheduler.registry().register(metadata).await {
        Ok(_) => {
            println!("✅ 函数 '{name}' 注册成功!");
            println!("   🆔 分配的 SCRU128 ID: {function_id}");
            println!("   💡 此 ID 包含时间信息，支持自然排序");
        }
        Err(e) => println!("❌ 函数注册失败: {e}"),
    }

    Ok(())
}

/// 显示系统状态
async fn show_system_status(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    let functions = scheduler.registry().list().await;

    println!("📊 FluxFaaS 系统状态");
    println!("==================");
    println!("🔧 已注册函数数量: {}", functions.len());
    println!("🆔 ID 系统: SCRU128 (时间有序)");
    println!("🚀 运行状态: 正常");
    println!("⚡ 执行引擎: SimpleRuntime (带缓存)");
    println!(
        "📅 启动时间: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );

    // 显示缓存统计信息
    println!("\n🎯 缓存系统状态:");
    println!("================");

    // 这里需要从 runtime 获取缓存统计
    // 但由于架构限制，我们暂时显示静态信息
    println!("💾 缓存引擎: LRU Cache");
    println!("📏 最大容量: 100 个函数");
    println!("💽 内存限制: 50 MB");
    println!("⏰ 过期时间: 1 小时");
    println!("✅ 缓存状态: 已启用");

    if !functions.is_empty() {
        println!("\n📝 函数概览 (按创建时间排序):");
        for func in &functions {
            println!(
                "  • {} ({}ms timeout) - ID: {}",
                func.name, func.timeout_ms, func.id
            );
        }

        // 展示 SCRU128 的时间有序性
        println!("\n🔍 SCRU128 特性验证:");
        let mut ids: Vec<_> = functions.iter().map(|f| f.id.to_string()).collect();
        let original_order = ids.clone();
        ids.sort();

        if original_order == ids {
            println!("   ✅ ID 具有时间有序性（自然排序 = 创建时间排序）");
        } else {
            println!("   ❗ ID 顺序异常");
        }
    }

    Ok(())
}

/// 从文件加载函数
async fn load_function_from_file(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    println!("📁 从文件加载函数");

    print!("请输入函数文件路径 (例如: examples/functions/greet.rs): ");
    io::stdout().flush()?;
    let mut file_path = String::new();
    io::stdin().read_line(&mut file_path)?;
    let file_path = file_path.trim();

    if file_path.is_empty() {
        println!("❌ 文件路径不能为空");
        return Ok(());
    }

    print!("自定义函数名称 (可选，回车使用文件名): ");
    io::stdout().flush()?;
    let mut custom_name = String::new();
    io::stdin().read_line(&mut custom_name)?;
    let custom_name = custom_name.trim();

    print!("函数描述 (可选): ");
    io::stdout().flush()?;
    let mut description = String::new();
    io::stdin().read_line(&mut description)?;
    let description = description.trim();

    print!("超时时间 (毫秒，默认 5000): ");
    io::stdout().flush()?;
    let mut timeout_input = String::new();
    io::stdin().read_line(&mut timeout_input)?;
    let timeout_ms = timeout_input.trim().parse().ok();

    let name = if custom_name.is_empty() {
        None
    } else {
        Some(custom_name.to_string())
    };

    let desc = if description.is_empty() {
        None
    } else {
        Some(description.to_string())
    };

    println!("📤 正在从文件加载函数...");

    match scheduler
        .registry()
        .register_from_file(file_path, name, desc, timeout_ms)
        .await
    {
        Ok(_) => {
            println!("✅ 函数从文件 '{file_path}' 加载成功!");
            println!("💡 函数已通过安全验证并注册到系统中");
        }
        Err(e) => {
            println!("❌ 函数加载失败: {e}");
            println!("💡 请检查文件路径和文件内容是否正确");
        }
    }

    Ok(())
}

/// 从目录批量加载函数
async fn load_functions_from_directory(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    println!("📂 从目录批量加载函数");

    print!("请输入函数目录路径 (例如: examples/functions): ");
    io::stdout().flush()?;
    let mut dir_path = String::new();
    io::stdin().read_line(&mut dir_path)?;
    let dir_path = dir_path.trim();

    if dir_path.is_empty() {
        println!("❌ 目录路径不能为空");
        return Ok(());
    }

    println!("📤 正在从目录批量加载函数...");

    match scheduler.registry().register_from_directory(dir_path).await {
        Ok(count) => {
            if count > 0 {
                println!("✅ 成功从目录 '{dir_path}' 加载了 {count} 个函数!");
                println!("💡 所有函数都已通过安全验证并注册到系统中");
            } else {
                println!("⚠️  目录 '{dir_path}' 中没有找到有效的函数文件");
                println!("💡 请确保目录中包含 .rs 文件且格式正确");
            }
        }
        Err(e) => {
            println!("❌ 批量加载失败: {e}");
            println!("💡 请检查目录路径是否正确");
        }
    }

    Ok(())
}

/// 显示缓存统计信息
async fn show_cache_statistics(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    println!("🎯 FluxFaaS 缓存统计");
    println!("===================");

    let cache = scheduler.runtime().cache();
    let stats = cache.stats().await;
    let hit_rate = cache.hit_rate().await;

    println!("📊 基本统计:");
    println!("  🎯 缓存命中率: {:.2}%", hit_rate * 100.0);
    println!("  ✅ 命中次数: {}", stats.hits);
    println!("  ❌ 未命中次数: {}", stats.misses);
    println!("  📏 当前缓存大小: {} 个函数", stats.size);
    println!("  🔄 驱逐次数: {}", stats.evictions);

    println!("\n💾 内存使用:");
    println!(
        "  🔍 当前使用: {} 字节 ({:.2} KB)",
        stats.memory_usage,
        stats.memory_usage as f64 / 1024.0
    );
    println!(
        "  📏 最大限制: {} 字节 ({:.2} MB)",
        stats.max_memory,
        stats.max_memory as f64 / (1024.0 * 1024.0)
    );
    let usage_percent = if stats.max_memory > 0 {
        (stats.memory_usage as f64 / stats.max_memory as f64) * 100.0
    } else {
        0.0
    };
    println!("  📊 使用率: {usage_percent:.2}%");

    println!("\n⚙️  配置信息:");
    println!("  📦 缓存类型: LRU (Least Recently Used)");
    println!("  🔄 最大容量: 100 个函数");
    println!("  ⏰ 过期时间: 1 小时");
    println!("  🚀 状态: 活跃");

    if stats.size > 0 {
        println!("\n💡 性能建议:");
        if hit_rate < 0.5 {
            println!("  ⚠️  缓存命中率较低，考虑增加缓存容量");
        } else if hit_rate > 0.8 {
            println!("  ✅ 缓存效果良好！");
        }

        if usage_percent > 80.0 {
            println!("  ⚠️  内存使用率较高，可能需要清理缓存");
        }
    } else {
        println!("\n💡 提示: 缓存为空，执行一些函数后可以看到缓存效果");
    }

    Ok(())
}

/// 显示性能监控信息
async fn show_performance_monitor(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    println!("📊 FluxFaaS 性能监控");
    println!("===================");

    let monitor = scheduler.runtime().monitor();
    let report = monitor.generate_report().await;

    // 显示系统健康状态
    let health_icon = match report.health_status {
        crate::runtime::monitor::HealthStatus::Healthy => "💚",
        crate::runtime::monitor::HealthStatus::Warning => "💛",
        crate::runtime::monitor::HealthStatus::Critical => "❤️",
    };
    println!(
        "🏥 系统健康状态: {} {:?}",
        health_icon, report.health_status
    );

    // 显示全局统计
    println!("\n📈 全局统计:");
    println!("  📝 总请求数: {}", report.global_stats.total_requests);
    println!("  ✅ 成功次数: {}", report.global_stats.total_success);
    println!("  ❌ 失败次数: {}", report.global_stats.total_failures);

    if report.global_stats.total_requests > 0 {
        let success_rate = (report.global_stats.total_success as f64
            / report.global_stats.total_requests as f64)
            * 100.0;
        println!("  🎯 成功率: {:.2}%", success_rate);
    }

    if let Some(start_time) = report.global_stats.start_time {
        let uptime = report.generated_at.duration_since(start_time);
        println!("  ⏰ 系统运行时间: {:.2}秒", uptime.as_secs_f64());
    }

    println!(
        "  💾 峰值内存: {:.2} KB",
        report.global_stats.peak_system_memory as f64 / 1024.0
    );
    println!(
        "  🔄 当前内存: {:.2} KB",
        report.global_stats.current_system_memory as f64 / 1024.0
    );

    // 显示函数统计
    if !report.function_stats.is_empty() {
        println!("\n📊 函数执行统计:");
        for (name, stats) in report.function_stats.iter() {
            println!("  📦 函数: {}", name);
            println!("    📝 调用次数: {}", stats.total_calls);
            println!("    ✅ 成功: {} 次", stats.successful_calls);
            println!("    ❌ 失败: {} 次", stats.failed_calls);

            if stats.total_calls > 0 {
                let error_rate = (stats.failed_calls as f64 / stats.total_calls as f64) * 100.0;
                println!("    🎯 错误率: {:.2}%", error_rate);
            }

            println!(
                "    ⏱️  平均执行时间: {:.2}ms",
                stats.avg_duration.as_millis()
            );

            if let Some(min) = stats.min_duration {
                println!("    ⚡ 最快: {:.2}ms", min.as_millis());
            }

            if let Some(max) = stats.max_duration {
                println!("    🐌 最慢: {:.2}ms", max.as_millis());
            }

            println!(
                "    💾 峰值内存: {:.2} KB",
                stats.peak_memory as f64 / 1024.0
            );
            println!(
                "    📊 平均内存: {:.2} KB",
                stats.avg_memory as f64 / 1024.0
            );
            println!();
        }

        // 显示热点函数
        let hottest = monitor.get_hottest_functions(3).await;
        if !hottest.is_empty() {
            println!("🔥 热点函数 (调用最频繁):");
            for (i, (name, calls)) in hottest.iter().enumerate() {
                println!("  {}. {} ({} 次调用)", i + 1, name, calls);
            }
            println!();
        }

        // 显示最慢函数
        let slowest = monitor.get_slowest_functions(3).await;
        if !slowest.is_empty() {
            println!("🐌 最慢函数:");
            for (i, (name, duration)) in slowest.iter().enumerate() {
                println!("  {}. {} ({:.2}ms 平均)", i + 1, name, duration.as_millis());
            }
            println!();
        }

        // 显示错误率高的函数
        let error_prone = monitor.get_error_prone_functions(3).await;
        if !error_prone.is_empty() {
            println!("⚠️  高错误率函数:");
            for (i, (name, error_rate)) in error_prone.iter().enumerate() {
                println!("  {}. {} ({:.2}% 错误率)", i + 1, name, error_rate * 100.0);
            }
            println!();
        }
    } else {
        println!("\n💡 还没有函数执行记录，调用一些函数后再查看统计");
    }

    // 显示性能建议
    println!("💡 性能建议:");
    for recommendation in &report.recommendations {
        println!("  • {}", recommendation);
    }

    Ok(())
}

/// 重置性能监控数据
async fn reset_performance_data(scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    println!("🔄 重置性能监控数据");

    print!("确认要重置所有性能监控数据吗？(y/N): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let confirmation = input.trim().to_lowercase();

    if confirmation == "y" || confirmation == "yes" {
        let monitor = scheduler.runtime().monitor();

        match monitor.reset_stats().await {
            Ok(_) => {
                println!("✅ 性能监控数据已重置");
                println!("💡 新的统计将从现在开始重新计算");
            }
            Err(e) => {
                println!("❌ 重置失败: {}", e);
            }
        }
    } else {
        println!("❌ 重置操作已取消");
    }

    Ok(())
}
