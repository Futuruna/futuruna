// T03: String operations
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn repeat(s: &str, n: i64) -> String {
    if n <= 0 {
        String::new()
    } else {
        format!("{}{}", s, repeat(s, n - 1))
    }
}

fn main() {
    println!("{}", greet("World"));
    println!("{}", greet("Futuruna"));
    println!("{}", repeat("ab", 3));
    println!("{}", repeat("x", 5));
}
