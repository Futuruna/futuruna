// Stress: HashMap operations
use std::collections::HashMap;

fn build_map() -> HashMap<String, i64> {
    let mut m = HashMap::new();
    m.insert("alice".to_string(), 30);
    m.insert("bob".to_string(), 25);
    m.insert("charlie".to_string(), 35);
    m
}

fn lookup(m: &HashMap<String, i64>, key: &str) -> i64 {
    m.get(key).cloned().unwrap_or(0)
}

fn has_key(m: &HashMap<String, i64>, key: &str) -> bool {
    m.contains_key(key)
}

fn main() {
    let m = build_map();
    println!("{}", lookup(&m, "alice"));
    println!("{}", lookup(&m, "missing"));
    println!("{}", has_key(&m, "bob"));
    println!("{}", has_key(&m, "nobody"));
}
