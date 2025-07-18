use serde_json::{Value, Value::Null, json};
use std::env;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 获取服务器配置
    let base_url =
        env::var("FLUX_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

    let client = reqwest::Client::new();

    // 检查服务器连接
    if (check_server_health(&client, &base_url).await).is_err() {
        eprintln!("❌ 无法连接到FluxFaaS服务器: {base_url}");
        eprintln!("💡 请确保服务器正在运行: cargo run --bin flux");
        return Ok(());
    }

    println!("✅ 已连接到FluxFaaS服务器: {base_url}");

    loop {
        println!("\n🌟 FluxFaaS CLI客户端 (HTTP模式)");
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

        print!("\nflux-cli> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => list_functions(&client, &base_url).await?,
            "2" => invoke_function(&client, &base_url).await?,
            "3" => register_new_function(&client, &base_url).await?,
            "4" => load_function_from_file(&client, &base_url).await?,
            "5" => load_functions_from_directory(&client, &base_url).await?,
            "6" => show_system_status(&client, &base_url).await?,
            "7" => show_cache_statistics(&client, &base_url).await?,
            "8" => show_performance_monitor(&client, &base_url).await?,
            "9" => reset_performance_data(&client, &base_url).await?,
            "10" | "q" | "quit" | "exit" => {
                println!("👋 再见！");
                break;
            }
            _ => println!("❌ 无效选项，请重新选择"),
        }
    }

    Ok(())
}

/// 检查服务器健康状态
async fn check_server_health(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
    let response = client.get(format!("{base_url}/health")).send().await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Server not healthy"))
    }
}

/// 列出所有函数
async fn list_functions(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
    let response = client.get(format!("{base_url}/functions")).send().await?;

    if !response.status().is_success() {
        println!("❌ 获取函数列表失败: {}", response.status());
        return Ok(());
    }

    let data: Value = response.json().await?;

    if let Some(functions) = data["data"].as_array() {
        if functions.is_empty() {
            println!("📭 没有注册的函数");
            return Ok(());
        }

        println!("📋 已注册的函数:");
        println!("------------------------");
        for (i, func) in functions.iter().enumerate() {
            let name = func["name"].as_str().unwrap_or("Unknown");
            let description = func["description"].as_str().unwrap_or("No description");
            let id = func["id"].as_str().unwrap_or("Unknown");
            let timeout = func["timeout_ms"].as_u64().unwrap_or(0);

            println!("{}. {} - {}", i + 1, name, description);
            println!("   🆔 SCRU128 ID: {id}");
            println!("   ⏱️  Timeout: {timeout}ms");
            println!();
        }
        println!("总计: {} 个函数", functions.len());
    }

    Ok(())
}

/// 调用函数
async fn invoke_function(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
    // 先获取函数列表
    let response = client.get(format!("{base_url}/functions")).send().await?;

    if !response.status().is_success() {
        println!("❌ 获取函数列表失败");
        return Ok(());
    }

    let data: Value = response.json().await?;
    let empty_vec = vec![];
    let functions = data["data"].as_array().unwrap_or(&empty_vec);

    if functions.is_empty() {
        println!("❌ 没有可用的函数");
        return Ok(());
    }

    println!("📋 可用函数:");
    for (i, func) in functions.iter().enumerate() {
        let name = func["name"].as_str().unwrap_or("Unknown");
        let id = func["id"].as_str().unwrap_or("Unknown");
        println!("{}. {} (ID: {})", i + 1, name, id);
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
    let function_name = selected_func["name"].as_str().unwrap_or("unknown");

    print!("请输入函数参数 (JSON 格式，例如: {{\"a\": 1, \"b\": 2}}): ");
    io::stdout().flush()?;

    let mut input_data = String::new();
    io::stdin().read_line(&mut input_data)?;

    let input_json: Value = match serde_json::from_str(input_data.trim()) {
        Ok(json) => json,
        Err(_) => json!(input_data.trim()),
    };

    println!("🚀 正在调用函数 '{function_name}'...");

    let invoke_request = json!({
        "input": input_json
    });

    let response = client
        .post(format!("{base_url}/invoke/{function_name}"))
        .json(&invoke_request)
        .send()
        .await?;

    if response.status().is_success() {
        let result: Value = response.json().await?;
        println!("✅ 函数执行成功!");
        println!("📤 输出: {}", serde_json::to_string_pretty(&result)?);
    } else {
        let error_text = response.text().await?;
        println!("❌ 函数执行失败: {error_text}");
    }

    Ok(())
}

/// 注册新函数
async fn register_new_function(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
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

    let register_request = json!({
        "name": name,
        "description": if description.is_empty() { Null } else { json!(description) },
        "code": code,
        "timeout_ms": timeout_ms
    });

    let response = client
        .post(format!("{base_url}/functions"))
        .json(&register_request)
        .send()
        .await?;

    if response.status().is_success() {
        let result: Value = response.json().await?;
        println!("✅ 函数 '{name}' 注册成功!");
        if let Some(function_data) = result.get("function") {
            if let Some(id) = function_data["id"].as_str() {
                println!("   🆔 分配的 SCRU128 ID: {id}");
                println!("   💡 此 ID 包含时间信息，支持自然排序");
            }
        }
    } else {
        let error_text = response.text().await?;
        println!("❌ 函数注册失败: {error_text}");
    }

    Ok(())
}

/// 从文件加载函数
async fn load_function_from_file(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
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
    let timeout_ms: Option<u64> = timeout_input.trim().parse().ok();

    let load_request = json!({
        "file_path": file_path,
        "function_name": if custom_name.is_empty() { Null } else { json!(custom_name) },
        "description": if description.is_empty() { Null } else { json!(description) },
        "timeout_ms": timeout_ms
    });

    println!("📤 正在从文件加载函数...");

    let response = client
        .post(format!("{base_url}/load/file"))
        .json(&load_request)
        .send()
        .await?;

    if response.status().is_success() {
        println!("✅ 函数从文件 '{file_path}' 加载成功!");
        println!("💡 函数已通过安全验证并注册到系统中");
    } else {
        let error_text = response.text().await?;
        println!("❌ 函数加载失败: {error_text}");
        println!("💡 请检查文件路径和文件内容是否正确");
    }

    Ok(())
}

/// 从目录批量加载函数
async fn load_functions_from_directory(
    client: &reqwest::Client,
    base_url: &str,
) -> anyhow::Result<()> {
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

    let load_request = json!({
        "directory_path": dir_path
    });

    let response = client
        .post(format!("{base_url}/load/directory"))
        .json(&load_request)
        .send()
        .await?;

    if response.status().is_success() {
        let result: Value = response.json().await?;
        if let Some(data) = result.get("data") {
            if let Some(data_str) = data.as_str() {
                println!("✅ {data_str}");
                println!("💡 所有函数都已通过安全验证并注册到系统中");
            }
        }
        if let Some(message) = result.get("message") {
            if let Some(msg) = message.as_str() {
                println!("📝 {msg}");
            }
        }
    } else {
        let error_text = response.text().await?;
        println!("❌ 批量加载失败: {error_text}");
        println!("💡 请检查目录路径是否正确");
    }

    Ok(())
}

/// 显示系统状态
async fn show_system_status(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
    let response = client.get(format!("{base_url}/status")).send().await?;

    if !response.status().is_success() {
        println!("❌ 获取系统状态失败");
        return Ok(());
    }

    let data: Value = response.json().await?;

    println!("📊 FluxFaaS 系统状态");
    println!("==================");

    if let Some(functions) = data["functions"]["total"].as_u64() {
        println!("🔧 已注册函数数量: {functions}");
    }

    println!("🆔 ID 系统: SCRU128 (时间有序)");
    println!(
        "🚀 运行状态: {}",
        data["status"].as_str().unwrap_or("unknown")
    );

    if let Some(runtime) = data["runtime"].as_object() {
        println!(
            "⚡ 执行引擎: {}",
            runtime["type"].as_str().unwrap_or("unknown")
        );
    }

    if let Some(timestamp) = data["timestamp"].as_str() {
        println!("📅 状态时间: {timestamp}");
    }

    Ok(())
}

/// 显示缓存统计信息
async fn show_cache_statistics(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
    let response = client.get(format!("{base_url}/cache/stats")).send().await?;

    if !response.status().is_success() {
        println!("❌ 获取缓存统计失败");
        return Ok(());
    }

    let data: Value = response.json().await?;

    println!("🎯 FluxFaaS 缓存统计");
    println!("===================");

    if let Some(stats) = data["data"]["basic_stats"].as_object() {
        println!("📊 基本统计:");
        if let Some(hit_rate) = stats["hit_rate"].as_f64() {
            println!("  🎯 缓存命中率: {:.2}%", hit_rate * 100.0);
        }
        if let Some(hits) = stats["hits"].as_u64() {
            println!("  ✅ 命中次数: {hits}");
        }
        if let Some(misses) = stats["misses"].as_u64() {
            println!("  ❌ 未命中次数: {misses}");
        }
        if let Some(size) = stats["size"].as_u64() {
            println!("  📏 当前缓存大小: {size} 个函数");
        }
    }

    if let Some(memory) = data["data"]["memory_usage"].as_object() {
        println!("\n💾 内存使用:");
        if let Some(current_kb) = memory["current_kb"].as_f64() {
            println!("  🔍 当前使用: {current_kb:.2} KB");
        }
        if let Some(max_mb) = memory["max_mb"].as_f64() {
            println!("  📏 最大限制: {max_mb:.2} MB");
        }
        if let Some(usage_percent) = memory["usage_percent"].as_f64() {
            println!("  📊 使用率: {usage_percent:.2}%");
        }
    }

    Ok(())
}

/// 显示性能监控信息
async fn show_performance_monitor(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
    let response = client
        .get(format!("{base_url}/performance/stats"))
        .send()
        .await?;

    if !response.status().is_success() {
        println!("❌ 获取性能监控失败");
        return Ok(());
    }

    let data: Value = response.json().await?;

    println!("📊 FluxFaaS 性能监控");
    println!("===================");

    if let Some(monitor_data) = data["data"].as_object() {
        if let Some(health) = monitor_data["health_status"].as_str() {
            let health_icon = match health {
                "healthy" => "💚",
                "warning" => "💛",
                "critical" => "❤️",
                _ => "❓",
            };
            println!("🏥 系统健康状态: {health_icon} {health}");
        }

        if let Some(global) = monitor_data["global_stats"].as_object() {
            println!("\n📈 全局统计:");
            if let Some(total) = global["total_requests"].as_u64() {
                println!("  📝 总请求数: {total}");
            }
            if let Some(success) = global["total_success"].as_u64() {
                println!("  ✅ 成功次数: {success}");
            }
            if let Some(failures) = global["total_failures"].as_u64() {
                println!("  ❌ 失败次数: {failures}");
            }
            if let Some(rate) = global["success_rate"].as_f64() {
                println!("  🎯 成功率: {rate:.2}%");
            }
        }

        if let Some(hottest) = monitor_data["hottest_functions"].as_array() {
            if !hottest.is_empty() {
                println!("\n🔥 热点函数 (调用最频繁):");
                for (i, func) in hottest.iter().enumerate() {
                    if let (Some(name), Some(calls)) =
                        (func["name"].as_str(), func["calls"].as_u64())
                    {
                        println!("  {}. {} ({} 次调用)", i + 1, name, calls);
                    }
                }
            }
        }
    }

    Ok(())
}

/// 重置性能监控数据
async fn reset_performance_data(client: &reqwest::Client, base_url: &str) -> anyhow::Result<()> {
    println!("🔄 重置性能监控数据");

    print!("确认要重置所有性能监控数据吗？(y/N): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let confirmation = input.trim().to_lowercase();

    if confirmation == "y" || confirmation == "yes" {
        let response = client.post(format!("{base_url}/reset")).send().await?;

        if response.status().is_success() {
            println!("✅ 性能监控数据已重置");
            println!("💡 新的统计将从现在开始重新计算");
        } else {
            let error_text = response.text().await?;
            println!("❌ 重置失败: {error_text}");
        }
    } else {
        println!("❌ 重置操作已取消");
    }

    Ok(())
}
