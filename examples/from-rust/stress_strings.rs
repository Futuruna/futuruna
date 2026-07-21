// Stress: String operations
fn repeat_join(s: &str, n: i64, sep: &str) -> String {
    if n <= 0 {
        String::new()
    } else if n == 1 {
        s.to_string()
    } else {
        format!("{}{}{}", s, sep, repeat_join(s, n - 1, sep))
    }
}

fn is_prefix(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

fn main() {
    println!("{}", repeat_join("ha", 3, "-"));
    println!("{}", repeat_join("x", 1, ","));
    println!("{}", repeat_join("ab", 0, " "));
    println!("{}", is_prefix("hello world", "hello"));
    println!("{}", is_prefix("hello world", "world"));

    let msg = format!("{} has {} chars", "hello", 5);
    println!("{}", msg);
}
