// Stress: Closures with captures, higher-order functions, chaining
fn apply_n(f: fn(i64) -> i64, x: i64, n: i64) -> i64 {
    if n <= 0 { x } else { apply_n(f, f(x), n - 1) }
}

fn compose(f: fn(i64) -> i64, g: fn(i64) -> i64, x: i64) -> i64 {
    g(f(x))
}

fn main() {
    // Basic HOF
    println!("{}", apply_n(|x| x * 2, 1, 10));
    println!("{}", compose(|x| x + 1, |x| x * 3, 5));

    // Map/filter chains
    let nums: Vec<i64> = (1..=10).collect();
    let result: i64 = nums.iter()
        .filter(|x| **x % 2 == 0)
        .map(|x| x * x)
        .sum();
    println!("{}", result);

    // Fold
    let factorial: i64 = (1..=6).fold(1, |acc, x| acc * x);
    println!("{}", factorial);

    // Nested
    let matrix = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]];
    let flat_sum: i64 = matrix.iter().flat_map(|row| row.iter()).sum();
    println!("{}", flat_sum);
}
