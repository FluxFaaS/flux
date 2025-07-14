use anyhow::Result;
use flux::functions::{FunctionMetadata, InvokeRequest};
use flux::runtime::compiler::{CompilerConfig, RustCompiler};
use flux::runtime::sandbox::{SandboxConfig, SandboxExecutor};
use serde_json::json;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🔒 FluxFaaS 沙箱执行环境测试");
    println!("================================================");

    // 创建测试函数
    let test_function = FunctionMetadata::new(
        "sandbox_test".to_string(),
        r#"
// 测试函数：计算两个数的乘积
fn multiply(a: f64, b: f64) -> f64 {
    a * b
}

pub fn main() {
    println!("Hello from sandboxed function!");
}
"#
        .to_string(),
    );

    // 创建沙箱配置
    let sandbox_config = SandboxConfig {
        enable_process_isolation: true,
        enable_container_isolation: false,
        execution_timeout_secs: 10,
        max_memory_mb: 64,
        max_cpu_percent: 50.0,
        allow_network: false,
        allow_filesystem: false,
        allowed_dirs: vec![],
        work_dir: None,
        allowed_env_vars: vec!["PATH".to_string()],
        temp_root: std::path::PathBuf::from("/tmp/flux_sandbox_test"),
        rust_target_dir: Some(std::path::PathBuf::from("~/rust_target")),
    };

    println!("📋 沙箱配置:");
    println!("  - 进程隔离: {}", sandbox_config.enable_process_isolation);
    println!("  - 执行超时: {}秒", sandbox_config.execution_timeout_secs);
    println!("  - 内存限制: {}MB", sandbox_config.max_memory_mb);
    println!("  - CPU限制: {}%", sandbox_config.max_cpu_percent);
    if let Some(ref target_dir) = sandbox_config.rust_target_dir {
        println!("  - 自定义编译路径: {}", target_dir.display());
    }
    println!("  - 网络访问: {}", sandbox_config.allow_network);
    println!("  - 文件系统访问: {}", sandbox_config.allow_filesystem);
    println!();

    // 创建沙箱执行器
    let sandbox = SandboxExecutor::new(sandbox_config)?;
    println!("✅ 沙箱执行器创建成功");

    // 创建编译器
    let compiler_config = CompilerConfig {
        rust_target_dir: Some(std::path::PathBuf::from("~/rust_target")),
        ..Default::default()
    };
    let compiler = RustCompiler::new(compiler_config)?;
    println!("✅ 编译器创建成功");

    // 编译函数
    println!("🔨 开始编译函数...");
    let start_time = Instant::now();
    let compiled = compiler.compile_function(&test_function).await?;
    let compile_time = start_time.elapsed();
    println!("✅ 函数编译完成，耗时: {compile_time:?}");
    println!("  - 编译时间: {}ms", compiled.compile_time_ms);
    println!("  - 库文件路径: {:?}", compiled.library_path);
    println!();

    // 创建测试请求
    let test_request = InvokeRequest {
        input: json!({
            "a": 15.5,
            "b": 4.2
        }),
    };

    println!("🚀 在沙箱中执行函数...");
    println!(
        "输入: {}",
        serde_json::to_string_pretty(&test_request.input)?
    );

    // 在沙箱中执行
    let execution_start = Instant::now();
    let result = sandbox.execute_in_sandbox(&compiled, &test_request).await?;
    let execution_time = execution_start.elapsed();

    println!("✅ 沙箱执行完成，耗时: {execution_time:?}");
    println!();

    // 显示执行结果
    println!("📊 执行结果:");
    println!("  - 状态: {:?}", result.status);
    println!("  - 执行时间: {}ms", result.execution_time_ms);
    println!("  - 内存峰值: {}KB", result.peak_memory_bytes / 1024);
    println!("  - CPU使用: {:.2}%", result.cpu_usage_percent);
    println!("  - 退出码: {:?}", result.exit_code);
    println!();

    if !result.stdout.is_empty() {
        println!("📤 标准输出:");
        println!("{}", result.stdout);
    }

    if !result.stderr.is_empty() {
        println!("❌ 标准错误:");
        println!("{}", result.stderr);
    }

    println!("🎯 函数输出:");
    println!("{}", serde_json::to_string_pretty(&result.output)?);

    // 获取系统资源使用情况
    println!();
    println!("💻 系统资源使用:");
    let system_usage = sandbox.get_system_usage().await?;
    println!(
        "  - 总内存: {}MB",
        system_usage.total_memory_bytes / 1024 / 1024
    );
    println!(
        "  - 已用内存: {}MB",
        system_usage.used_memory_bytes / 1024 / 1024
    );
    println!("  - CPU核心数: {}", system_usage.cpu_count);
    println!("  - 系统负载: {}", system_usage.load_average);

    // 清理资源
    println!();
    println!("🧹 清理沙箱资源...");
    sandbox.cleanup().await?;
    println!("✅ 清理完成");

    println!();
    println!("🎉 沙箱执行环境测试完成！");

    Ok(())
}

// 性能基准测试
#[allow(dead_code)]
async fn benchmark_sandbox_performance() -> Result<()> {
    println!("⚡ 开始沙箱性能基准测试...");

    let function = FunctionMetadata::new(
        "benchmark".to_string(),
        r#"
fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
"#
        .to_string(),
    );

    let sandbox_config = SandboxConfig::default();
    let sandbox = SandboxExecutor::new(sandbox_config)?;

    let compiler_config = CompilerConfig::default();
    let compiler = RustCompiler::new(compiler_config)?;

    let compiled = compiler.compile_function(&function).await?;

    let iterations = 10;
    let mut total_time = 0u64;
    let mut total_memory = 0u64;

    for i in 0..iterations {
        let request = InvokeRequest {
            input: json!({"n": 20}),
        };

        let start = Instant::now();
        let result = sandbox.execute_in_sandbox(&compiled, &request).await?;
        let _duration = start.elapsed();

        total_time += result.execution_time_ms;
        total_memory += result.peak_memory_bytes;

        println!(
            "  第{}次: {}ms, {}KB",
            i + 1,
            result.execution_time_ms,
            result.peak_memory_bytes / 1024
        );
    }

    println!("📈 基准测试结果:");
    println!("  - 平均执行时间: {}ms", total_time / iterations);
    println!("  - 平均内存使用: {}KB", total_memory / iterations / 1024);

    sandbox.cleanup().await?;
    Ok(())
}
