// runa-from-rust: expect-unsupported borrowed references outside the validation boundary

fn first_label<'a>(labels: &'a Vec<String>) -> &'a String {
    &labels[0]
}

fn main() {
    let labels = vec!["alpha".to_string(), "beta".to_string()];
    println!("first={}", first_label(&labels));
}
