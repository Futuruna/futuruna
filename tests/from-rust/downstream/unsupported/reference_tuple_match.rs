// runa-from-rust: expect-unsupported tuple-of-references match outside the checked simplification subset

fn compare(a: &(i64, i64), b: &(i64, i64)) -> i64 {
    match (&a, &b) {
        ((x, _), _) if *x > 0 => *x,
        _ => 0,
    }
}

fn main() {
    println!("value={}", compare(&(3, 4), &(5, 6)));
}
