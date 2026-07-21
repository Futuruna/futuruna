// Stress: Result chains, Option
fn safe_div(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 { Err("division by zero".to_string()) } else { Ok(a / b) }
}

fn chain(a: i64, b: i64, c: i64) -> Result<i64, String> {
    let x = safe_div(a, b)?;
    let y = safe_div(x, c)?;
    Ok(y)
}

fn first_positive(xs: &[i64]) -> Option<i64> {
    for x in xs {
        if *x > 0 {
            return Some(*x);
        }
    }
    None
}

fn main() {
    match chain(100, 5, 2) {
        Ok(v) => println!("{}", v),
        Err(e) => println!("err: {}", e),
    }
    match chain(100, 0, 2) {
        Ok(v) => println!("{}", v),
        Err(e) => println!("err: {}", e),
    }
    match first_positive(&[-3, -1, 0, 4, 7]) {
        Some(v) => println!("{}", v),
        None => println!("none"),
    }
    match first_positive(&[-3, -1]) {
        Some(v) => println!("{}", v),
        None => println!("none"),
    }
}
