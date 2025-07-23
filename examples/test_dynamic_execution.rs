use flux::functions::{FunctionMetadata, InvokeRequest};
use flux::runtime::SimpleRuntime;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🚀 测试FluxFaaS动态代码执行");
    println!("================================");

    // 创建运行时
    let runtime = SimpleRuntime::new();

    // 测试JavaScript代码
    println!("\n📝 测试JavaScript代码执行:");
    let js_function = FunctionMetadata::new(
        "js_add".to_string(),
        r#"
const {a, b} = input;
console.log("正在计算:", a, "+", b);
return a + b;
"#
        .to_string(),
        None, // 使用自动检测
    );

    let js_request = InvokeRequest {
        input: json!({
            "a": 15,
            "b": 25
        }),
    };

    match runtime.execute(&js_function, &js_request).await {
        Ok(response) => {
            println!("✅ JavaScript执行成功:");
            println!(
                "   输出: {}",
                serde_json::to_string_pretty(&response.output)?
            );
            println!("   耗时: {}ms", response.execution_time_ms);
        }
        Err(e) => {
            println!("❌ JavaScript执行失败: {e}");
        }
    }

    // 测试Python代码
    println!("\n🐍 测试Python代码执行:");
    let py_function = FunctionMetadata::new(
        "py_multiply".to_string(),
        r#"
def multiply(x, y):
    print(f"正在计算: {x} * {y}")
    return x * y

a = input.get('a', 0)
b = input.get('b', 0)
result = multiply(a, b)
"#
        .to_string(),
        None, // 使用自动检测
    );

    let py_request = InvokeRequest {
        input: json!({
            "a": 7,
            "b": 8
        }),
    };

    match runtime.execute(&py_function, &py_request).await {
        Ok(response) => {
            println!("✅ Python执行成功:");
            println!(
                "   输出: {}",
                serde_json::to_string_pretty(&response.output)?
            );
            println!("   耗时: {}ms", response.execution_time_ms);
        }
        Err(e) => {
            println!("❌ Python执行失败: {e}");
        }
    }

    // 测试简单表达式
    println!("\n🧮 测试简单表达式:");
    let expr_function =
        FunctionMetadata::new("simple_calc".to_string(), "return a * 2 + b".to_string(), None);

    println!("   表达式: {expr_function:?}", );

    let expr_request = InvokeRequest {
        input: json!({
            "a": 10,
            "b": 5
        }),
    };

    match runtime.execute(&expr_function, &expr_request).await {
        Ok(response) => {
            println!("✅ 表达式执行成功:");
            println!(
                "   输出: {}",
                serde_json::to_string_pretty(&response.output)?
            );
            println!("   耗时: {}ms", response.execution_time_ms);
        }
        Err(e) => {
            println!("❌ 表达式执行失败: {e}");
        }
    }

    // 测试复杂JavaScript
    println!("\n🔧 测试复杂JavaScript:");
    let complex_js = FunctionMetadata::new(
        "complex_js".to_string(),
        r#"
function fibonacci(n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}

const n = input.n || 10;
console.log(`正在计算斐波那契数列第${n}项...`);
const result = fibonacci(n);
console.log(`计算完成!`);
return result;
"#
        .to_string(),
        None, // 使用自动检测
    );

    let complex_request = InvokeRequest {
        input: json!({
            "n": 8
        }),
    };

    match runtime.execute(&complex_js, &complex_request).await {
        Ok(response) => {
            println!("✅ 复杂JavaScript执行成功:");
            println!(
                "   输出: {}",
                serde_json::to_string_pretty(&response.output)?
            );
            println!("   耗时: {}ms", response.execution_time_ms);
        }
        Err(e) => {
            println!("❌ 复杂JavaScript执行失败: {e}");
        }
    }

    println!("\n🎉 动态执行测试完成!");
    Ok(())
}
