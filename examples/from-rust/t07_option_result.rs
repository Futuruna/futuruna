// T07: Option, Result, error handling
fn safe_div(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        Err("division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

fn find_first_positive(xs: &[i64]) -> Option<i64> {
    for x in xs {
        if *x > 0 {
            return Some(*x);
        }
    }
    None
}

fn main() {
    match safe_div(10, 3) {
        Ok(v) => println!("{}", v),
        Err(e) => println!("{}", e),
    }
    match safe_div(10, 0) {
        Ok(v) => println!("{}", v),
        Err(e) => println!("{}", e),
    }
    match find_first_positive(&[-3, -1, 0, 4, 7]) {
        Some(v) => println!("{}", v),
        None => println!("none"),
    }
    match find_first_positive(&[-3, -1]) {
        Some(v) => println!("{}", v),
        None => println!("none"),
    }
}
