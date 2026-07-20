// T14: String processing
fn word_count(s: &str) -> i64 {
    if s.is_empty() { 0 } else { s.split(' ').count() as i64 }
}

fn repeat_string(s: &str, n: i64) -> String {
    if n <= 0 {
        String::new()
    } else {
        format!("{}{}", s, repeat_string(s, n - 1))
    }
}

fn ends_with_excl(s: &str) -> bool {
    s.ends_with("!")
}

fn main() {
    println!("{}", word_count("hello world foo bar"));
    println!("{}", word_count(""));
    println!("{}", repeat_string("ab", 3));
    println!("{}", repeat_string("x", 5));
    println!("{}", ends_with_excl("hello!"));
    println!("{}", ends_with_excl("hello"));
}
