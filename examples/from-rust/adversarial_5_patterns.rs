// Adversarial 5: Pattern matching edge cases, nested enums, guards

#[derive(Debug, Clone)]
enum Expr {
    Lit(i64),
    Var(String),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Let(String, Box<Expr>, Box<Expr>),
}

fn eval(expr: &Expr, env: &HashMap<String, i64>) -> i64 {
    match expr {
        Expr::Lit(n) => *n,
        Expr::Var(name) => *env.get(name).unwrap_or(&0),
        Expr::Add(a, b) => eval(a, env) + eval(b, env),
        Expr::Mul(a, b) => eval(a, env) * eval(b, env),
        Expr::If(cond, then, els) => {
            if eval(cond, env) != 0 {
                eval(then, env)
            } else {
                eval(els, env)
            }
        }
        Expr::Let(name, value, body) => {
            let mut new_env = env.clone();
            new_env.insert(name.clone(), eval(value, env));
            eval(body, &new_env)
        }
    }
}

fn pretty(expr: &Expr) -> String {
    match expr {
        Expr::Lit(n) => n.to_string(),
        Expr::Var(name) => name.clone(),
        Expr::Add(a, b) => format!("({} + {})", pretty(a), pretty(b)),
        Expr::Mul(a, b) => format!("({} * {})", pretty(a), pretty(b)),
        Expr::If(c, t, e) => format!("if {} then {} else {}", pretty(c), pretty(t), pretty(e)),
        Expr::Let(name, val, body) => format!("let {} = {} in {}", name, pretty(val), pretty(body)),
    }
}

fn simplify(expr: &Expr) -> Expr {
    match expr {
        Expr::Add(a, b) => {
            let a = simplify(a);
            let b = simplify(b);
            match (&a, &b) {
                (Expr::Lit(0), _) => b,
                (_, Expr::Lit(0)) => a,
                (Expr::Lit(x), Expr::Lit(y)) => Expr::Lit(x + y),
                _ => Expr::Add(Box::new(a), Box::new(b)),
            }
        }
        Expr::Mul(a, b) => {
            let a = simplify(a);
            let b = simplify(b);
            match (&a, &b) {
                (Expr::Lit(0), _) | (_, Expr::Lit(0)) => Expr::Lit(0),
                (Expr::Lit(1), _) => b,
                (_, Expr::Lit(1)) => a,
                (Expr::Lit(x), Expr::Lit(y)) => Expr::Lit(x * y),
                _ => Expr::Mul(Box::new(a), Box::new(b)),
            }
        }
        other => other.clone(),
    }
}

use std::collections::HashMap;

fn main() {
    // let x = 5 in (x + 3) * 2
    let expr = Expr::Let(
        "x".to_string(),
        Box::new(Expr::Lit(5)),
        Box::new(Expr::Mul(
            Box::new(Expr::Add(
                Box::new(Expr::Var("x".to_string())),
                Box::new(Expr::Lit(3)),
            )),
            Box::new(Expr::Lit(2)),
        )),
    );

    println!("Expr: {}", pretty(&expr));
    println!("Eval: {}", eval(&expr, &HashMap::new()));

    // (0 + x) * (1 * y)  → should simplify to x * y
    let complex = Expr::Mul(
        Box::new(Expr::Add(
            Box::new(Expr::Lit(0)),
            Box::new(Expr::Var("x".to_string())),
        )),
        Box::new(Expr::Mul(
            Box::new(Expr::Lit(1)),
            Box::new(Expr::Var("y".to_string())),
        )),
    );
    println!("Before: {}", pretty(&complex));
    println!("After:  {}", pretty(&simplify(&complex)));
}
