// runa-from-rust: expect-unsupported impl Trait outside the checked compose fixture

fn numbers() -> impl Iterator<Item = i64> {
    vec![1, 2, 3].into_iter()
}

fn main() {
    println!("sum={}", numbers().sum::<i64>());
}
