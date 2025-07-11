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
        println!("4. 📊 查看系统状态");
        println!("5. 🆔 演示 SCRU128 功能");
        println!("6. 🚪 退出");

        print!("\nflux> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => list_functions(&scheduler).await?,
            "2" => invoke_function(&scheduler).await?,
            "3" => register_new_function(&scheduler).await?,
            "4" => show_system_status(&scheduler).await?,
            "5" => demonstrate_scru128_features(&scheduler).await?,
            "6" | "q" | "quit" | "exit" => {
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
    println!("💾 内存使用: 轻量级");
    println!("🚀 运行状态: 正常");
    println!("⚡ 执行引擎: SimpleRuntime");
    println!(
        "📅 启动时间: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );

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

/// 演示 SCRU128 功能
async fn demonstrate_scru128_features(_scheduler: &SimpleScheduler) -> anyhow::Result<()> {
    println!("🆔 SCRU128 功能演示");
    println!("===================");

    // 创建几个测试函数来展示 ID 生成
    println!("📦 正在创建测试函数以展示 SCRU128 特性...");

    let test_functions = vec![
        ("demo1", "演示函数 1"),
        ("demo2", "演示函数 2"),
        ("demo3", "演示函数 3"),
    ];

    let mut generated_ids = Vec::new();

    for (name, desc) in test_functions {
        let request = RegisterFunctionRequest {
            name: format!("{}_{}", name, chrono::Utc::now().timestamp_millis()),
            description: Some(desc.to_string()),
            code: "return 'demo'".to_string(),
            timeout_ms: Some(1000),
        };

        let metadata = FunctionMetadata::from_request(request);
        let id = metadata.id;
        generated_ids.push((metadata.name.clone(), id));

        println!("   📦 {} -> ID: {}", metadata.name, id);

        // 短暂延迟确保时间戳不同
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    println!("\n🔍 SCRU128 特性分析:");
    println!(
        "   📏 ID 长度: {} 字符",
        generated_ids[0].1.to_string().len()
    );
    println!("   🔤 编码方式: Base32");
    println!("   ⏰ 包含时间戳: 是");
    println!("   🔀 支持排序: 是");

    // 验证排序特性
    let id_strings: Vec<String> = generated_ids.iter().map(|(_, id)| id.to_string()).collect();
    let mut sorted_ids = id_strings.clone();
    sorted_ids.sort();

    if id_strings == sorted_ids {
        println!("   ✅ 时间有序性: 通过（生成顺序 = 排序顺序）");
    } else {
        println!("   ❗ 时间有序性: 异常");
    }

    println!("\n💡 SCRU128 优势:");
    println!("   • 比 UUID 更短（25 vs 36 字符）");
    println!("   • 时间有序，数据库索引友好");
    println!("   • 分布式环境安全");
    println!("   • URL 友好，无需转义");

    println!("\n🗑️  注意：演示函数不会实际注册到系统中");

    Ok(())
}
