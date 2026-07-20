// T01: Basic functions, arithmetic, println
fn add(a: i64, b: i64) -> i64 { a + b }
fn mul(a: i64, b: i64) -> i64 { a * b }

fn main() {
    println!("{}", add(3, 4));
    println!("{}", mul(6, 7));
    println!("{}", add(mul(2, 3), 4));
}
