// runa-from-rust: expect-unsupported async Rust outside the validation boundary

async fn load_count() -> i64 {
    42
}

fn main() {
    println!("async fixture declared");
}
