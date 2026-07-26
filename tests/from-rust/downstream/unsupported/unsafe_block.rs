// runa-from-rust: expect-unsupported unsafe Rust outside the validation boundary

fn read_pointer(ptr: *const i64) -> i64 {
    unsafe { *ptr }
}

fn main() {
    let value = 9;
    println!("unsafe={}", read_pointer(&value));
}
