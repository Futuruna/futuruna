// runa-from-rust: expect-unsupported Result::map_err shape outside integer parse remapping

fn remap(input: Result<i64, String>) -> Result<i64, String> {
    input.map_err(|_| "bad".to_string())
}

fn main() {
    println!("remap={:?}", remap(Err("raw".to_string())));
}
