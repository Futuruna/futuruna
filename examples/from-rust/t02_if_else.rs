// T02: If/else, recursion, string returns
fn max(a: i64, b: i64) -> i64 {
    if a > b { a } else { b }
}

fn abs(x: i64) -> i64 {
    if x < 0 { -x } else { x }
}

fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}

fn fibonacci(n: i64) -> i64 {
    if n <= 0 { 0 } else if n == 1 { 1 } else { fibonacci(n - 1) + fibonacci(n - 2) }
}

fn main() {
    println!("{}", max(10, 20));
    println!("{}", abs(-42));
    println!("{}", factorial(10));
    println!("{}", fibonacci(10));
}
