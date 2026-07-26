// runa-from-rust: expect-unsupported thread spawning outside the validation boundary

use std::thread;

fn main() {
    let handle = thread::spawn(|| 7);
    println!("thread={}", handle.join().unwrap());
}
