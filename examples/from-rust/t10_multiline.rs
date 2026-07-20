// T10: Multi-statement functions, let chains, string building
fn classify(n: i64) -> String {
    if n < 0 {
        "negative".to_string()
    } else if n == 0 {
        "zero".to_string()
    } else if n < 10 {
        "small".to_string()
    } else if n < 100 {
        "medium".to_string()
    } else {
        "large".to_string()
    }
}

fn collatz_steps(n: i64) -> i64 {
    if n <= 1 {
        0
    } else if n % 2 == 0 {
        1 + collatz_steps(n / 2)
    } else {
        1 + collatz_steps(3 * n + 1)
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn lcm(a: i64, b: i64) -> i64 {
    a / gcd(a, b) * b
}

fn is_prime(n: i64) -> bool {
    if n < 2 { return false; }
    if n < 4 { return true; }
    if n % 2 == 0 { return false; }
    let mut i = 3;
    while i * i <= n {
        if n % i == 0 { return false; }
        i += 2;
    }
    true
}

fn main() {
    println!("{}", classify(-5));
    println!("{}", classify(0));
    println!("{}", classify(7));
    println!("{}", classify(42));
    println!("{}", classify(999));
    println!("{}", collatz_steps(27));
    println!("{}", gcd(48, 18));
    println!("{}", lcm(12, 8));
}
