// T18: Recursive descent expression evaluator
// Parses "(2 + 3) * 4" style expressions using recursive ADT

#[derive(Debug, Clone)]
enum Expr {
    Num(i64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

fn eval(e: &Expr) -> i64 {
    match e {
        Expr::Num(n) => *n,
        Expr::Add(a, b) => eval(a) + eval(b),
        Expr::Sub(a, b) => eval(a) - eval(b),
        Expr::Mul(a, b) => eval(a) * eval(b),
    }
}

fn show_expr(e: &Expr) -> String {
    match e {
        Expr::Num(n) => format!("{}", n),
        Expr::Add(a, b) => format!("({} + {})", show_expr(a), show_expr(b)),
        Expr::Sub(a, b) => format!("({} - {})", show_expr(a), show_expr(b)),
        Expr::Mul(a, b) => format!("({} * {})", show_expr(a), show_expr(b)),
    }
}

fn is_num(e: &Expr, n: i64) -> bool {
    match e { Expr::Num(v) => *v == n, _ => false }
}

fn get_num(e: &Expr) -> Option<i64> {
    match e { Expr::Num(v) => Some(*v), _ => None }
}

fn simplify(e: &Expr) -> Expr {
    match e {
        Expr::Add(a, b) => {
            let sa = simplify(a);
            let sb = simplify(b);
            if is_num(&sa, 0) { sb }
            else if is_num(&sb, 0) { sa }
            else if let (Some(x), Some(y)) = (get_num(&sa), get_num(&sb)) { Expr::Num(x + y) }
            else { Expr::Add(Box::new(sa), Box::new(sb)) }
        }
        Expr::Mul(a, b) => {
            let sa = simplify(a);
            let sb = simplify(b);
            if is_num(&sa, 0) || is_num(&sb, 0) { Expr::Num(0) }
            else if is_num(&sa, 1) { sb }
            else if is_num(&sb, 1) { sa }
            else if let (Some(x), Some(y)) = (get_num(&sa), get_num(&sb)) { Expr::Num(x * y) }
            else { Expr::Mul(Box::new(sa), Box::new(sb)) }
        }
        other => other.clone(),
    }
}

fn main() {
    // 2 + 3
    let e1 = Expr::Add(Box::new(Expr::Num(2)), Box::new(Expr::Num(3)));
    println!("{} = {}", show_expr(&e1), eval(&e1));

    // (2 + 3) * 4
    let e2 = Expr::Mul(
        Box::new(Expr::Add(Box::new(Expr::Num(2)), Box::new(Expr::Num(3)))),
        Box::new(Expr::Num(4)),
    );
    println!("{} = {}", show_expr(&e2), eval(&e2));

    // 0 + (x * 1) where x = 7 → simplify to 7
    let e3 = Expr::Add(
        Box::new(Expr::Num(0)),
        Box::new(Expr::Mul(Box::new(Expr::Num(7)), Box::new(Expr::Num(1)))),
    );
    println!("{} -> {} = {}", show_expr(&e3), show_expr(&simplify(&e3)), eval(&e3));

    // 10 - 3 - 2
    let e4 = Expr::Sub(
        Box::new(Expr::Sub(Box::new(Expr::Num(10)), Box::new(Expr::Num(3)))),
        Box::new(Expr::Num(2)),
    );
    println!("{} = {}", show_expr(&e4), eval(&e4));
}
