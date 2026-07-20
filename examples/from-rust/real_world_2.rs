// Real-world 2: Expression evaluator (recursive, no mutation)
// Exercises: recursive enums, pattern matching, Result error handling

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
}

fn eval(e: &Expr) -> Result<f64, String> {
    match e {
        Expr::Num(n) => Ok(*n),
        Expr::Neg(inner) => {
            let v = eval(inner)?;
            Ok(0.0 - v)
        }
        Expr::Add(a, b) => {
            let va = eval(a)?;
            let vb = eval(b)?;
            Ok(va + vb)
        }
        Expr::Sub(a, b) => {
            let va = eval(a)?;
            let vb = eval(b)?;
            Ok(va - vb)
        }
        Expr::Mul(a, b) => {
            let va = eval(a)?;
            let vb = eval(b)?;
            Ok(va * vb)
        }
        Expr::Div(a, b) => {
            let va = eval(a)?;
            let vb = eval(b)?;
            if vb == 0.0 {
                Err("division by zero".to_string())
            } else {
                Ok(va / vb)
            }
        }
    }
}

fn show_result(r: &Result<f64, String>) -> String {
    match r {
        Ok(v) => format!("{}", v),
        Err(e) => format!("error: {}", e),
    }
}

fn main() {
    // 3 + 4
    let e1 = Expr::Add(Box::new(Expr::Num(3.0)), Box::new(Expr::Num(4.0)));
    println!("{}", show_result(&eval(&e1)));

    // (10 - 3) * 2
    let e2 = Expr::Mul(
        Box::new(Expr::Sub(Box::new(Expr::Num(10.0)), Box::new(Expr::Num(3.0)))),
        Box::new(Expr::Num(2.0)),
    );
    println!("{}", show_result(&eval(&e2)));

    // 5 * 5
    let e3 = Expr::Mul(Box::new(Expr::Num(5.0)), Box::new(Expr::Num(5.0)));
    println!("{}", show_result(&eval(&e3)));

    // 10 / 0
    let e4 = Expr::Div(Box::new(Expr::Num(10.0)), Box::new(Expr::Num(0.0)));
    println!("{}", show_result(&eval(&e4)));

    // -(3 + 4)
    let e5 = Expr::Neg(Box::new(Expr::Add(
        Box::new(Expr::Num(3.0)),
        Box::new(Expr::Num(4.0)),
    )));
    println!("{}", show_result(&eval(&e5)));
}
