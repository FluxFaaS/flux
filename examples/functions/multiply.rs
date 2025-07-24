// 乘法函数
fn multiply(a: f64, b: f64) -> f64 {
    return a * b;
}

fn calculate_area(width: f64, height: f64) -> f64 {
    return multiply(width, height);
}

fn main() {
    let area = calculate_area(10.0, 5.0);
    println!("Area: {}", area);
}

