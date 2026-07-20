// T11: Chained Result with ?, monadic error handling
fn parse_pair(s: &str) -> Result<(i64, i64), String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(format!("expected 2 parts, got {}", parts.len()));
    }
    let a: i64 = parts[0].trim().parse().map_err(|_| format!("bad number: {}", parts[0]))?;
    let b: i64 = parts[1].trim().parse().map_err(|_| format!("bad number: {}", parts[1]))?;
    Ok((a, b))
}

fn add_pairs(s1: &str, s2: &str) -> Result<i64, String> {
    let (a1, b1) = parse_pair(s1)?;
    let (a2, b2) = parse_pair(s2)?;
    Ok(a1 + b1 + a2 + b2)
}

fn main() {
    match add_pairs("1,2", "3,4") {
        Ok(v) => println!("{}", v),
        Err(e) => println!("err: {}", e),
    }
    match add_pairs("1,2", "bad") {
        Ok(v) => println!("{}", v),
        Err(e) => println!("err: {}", e),
    }
    match add_pairs("1,2", "3,4,5") {
        Ok(v) => println!("{}", v),
        Err(e) => println!("err: {}", e),
    }
}
