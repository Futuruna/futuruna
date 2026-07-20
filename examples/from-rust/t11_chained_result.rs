// T11: Chained Result with ?, monadic error handling
fn safe_div(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        Err("division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

fn chain(a: i64, b: i64, c: i64) -> Result<i64, String> {
    let x = safe_div(a, b)?;
    let y = safe_div(x, c)?;
    Ok(y + 1)
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
    match chain(100, 5, 0) {
        Ok(v) => println!("{}", v),
        Err(e) => println!("err: {}", e),
    }
}
