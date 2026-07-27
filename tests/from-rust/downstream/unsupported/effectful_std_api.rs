// runa-from-rust: expect-unsupported effectful std APIs outside the pure/core boundary

use std::fs;
use std::env;
use std::net::TcpStream;
use std::process::Command;
use std::time::SystemTime;
use std::collections::hash_map::RandomState;

fn main() {
    if false {
        let _ = fs::read_to_string("config.txt");
        let _ = env::var("HOME");
        let _ = Command::new("echo");
        let _ = SystemTime::now();
        let _ = TcpStream::connect("127.0.0.1:9");
        let _ = RandomState::new();
    }
    println!("effectful std fixture declared");
}
