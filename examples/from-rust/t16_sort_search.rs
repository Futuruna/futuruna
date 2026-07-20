// T16: Recursive algorithms
fn sum_range(lo: i64, hi: i64) -> i64 {
    if lo > hi { 0 } else { lo + sum_range(lo + 1, hi) }
}

fn power(base: i64, exp: i64) -> i64 {
    if exp <= 0 { 1 } else { base * power(base, exp - 1) }
}

fn digit_sum(n: i64) -> i64 {
    if n < 10 { n } else { n % 10 + digit_sum(n / 10) }
}

fn count_digits(n: i64) -> i64 {
    if n < 10 { 1 } else { 1 + count_digits(n / 10) }
}

fn ackermann(m: i64, n: i64) -> i64 {
    if m == 0 { n + 1 }
    else if n == 0 { ackermann(m - 1, 1) }
    else { ackermann(m - 1, ackermann(m, n - 1)) }
}

fn main() {
    println!("{}", sum_range(1, 100));
    println!("{}", power(2, 10));
    println!("{}", digit_sum(12345));
    println!("{}", count_digits(9876543));
    println!("{}", ackermann(3, 4));
}
