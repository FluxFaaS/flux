// 问候函数
fn greet(name: &str) -> String {
    return format!("Hello, {}! Welcome to FluxFaaS!", name);
}

fn main() {
    let result = greet("World");
    println!("{}", result);
}
