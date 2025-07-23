use flux::functions::{FunctionMetadata, InvokeRequest, ScriptType};
use flux::runtime::compiler::{CompilerConfig, RustCompiler, check_compilation_support};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🔥 测试FluxFaaS真实Rust代码编译器");
    println!();

    // 检查编译支持
    println!("🔍 检查编译环境...");
    match check_compilation_support() {
        Ok(()) => println!("✅ 编译环境检查通过"),
        Err(e) => {
            println!("❌ 编译环境检查失败: {e}");
            println!(
                "请确保已安装Rust工具链: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
            );
            return Ok(());
        }
    }
    println!();

    // 创建编译器
    println!("🛠️ 创建编译器实例...");
    let config = CompilerConfig::default();
    let compiler = RustCompiler::new(config)?;
    println!("✅ 编译器创建成功");
    println!();

    // 创建测试函数
    println!("📝 创建测试函数...");
    let test_function = FunctionMetadata::new(
        "test_add".to_string(),
        r#"
// 这是一个简单的加法函数
fn add_numbers(a: f64, b: f64) -> f64 {
    a + b
}
"#
        .to_string(),
        Some(ScriptType::Rust), // 明确指定为 Rust 代码
    );
    println!("✅ 测试函数创建: {}", test_function.name);
    println!();

    // 编译函数
    println!("🚀 编译函数中...");
    match compiler.compile_function(&test_function).await {
        Ok(compiled) => {
            println!("✅ 编译成功!");
            println!("   📂 库文件路径: {:?}", compiled.library_path);
            println!("   ⏱️ 编译耗时: {}ms", compiled.compile_time_ms);
            println!("   🔑 源码哈希: {}", compiled.source_hash);
            println!();

            // 测试执行
            println!("🧪 测试函数执行...");
            let request = InvokeRequest {
                input: serde_json::json!({
                    "a": 10.5,
                    "b": 20.3
                }),
            };

            match compiler
                .execute_compiled_function(&compiled, &request)
                .await
            {
                Ok(response) => {
                    println!("✅ 执行成功!");
                    println!(
                        "   📤 输出: {}",
                        serde_json::to_string_pretty(&response.output)?
                    );
                    println!("   ⏱️ 执行耗时: {}ms", response.execution_time_ms);
                    println!("   🔍 状态: {:?}", response.status);
                }
                Err(e) => {
                    println!("❌ 执行失败: {e}");
                }
            }
        }
        Err(e) => {
            println!("❌ 编译失败: {e}");
        }
    }
    println!();

    // 显示编译器统计
    println!("📊 编译器统计信息:");
    let stats = compiler.get_stats().await;
    for (key, value) in stats {
        println!("   {key}: {value}");
    }

    println!();
    println!("🎉 测试完成!");

    Ok(())
}
