// Real-world 3: A mini type checker
// Exercises: recursive enums, environment, pattern matching, error propagation

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum Type {
    TInt,
    TBool,
    TFun(Box<Type>, Box<Type>),
}

#[derive(Debug, Clone)]
enum Expr {
    Lit(i64),
    BoolLit(bool),
    Var(String),
    Add(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Lam(String, Type, Box<Expr>),
    App(Box<Expr>, Box<Expr>),
}

fn type_to_string(t: &Type) -> String {
    match t {
        Type::TInt => "Int".to_string(),
        Type::TBool => "Bool".to_string(),
        Type::TFun(a, b) => format!("({} -> {})", type_to_string(a), type_to_string(b)),
    }
}

fn type_check(expr: &Expr, env: &HashMap<String, Type>) -> Result<Type, String> {
    match expr {
        Expr::Lit(_) => Ok(Type::TInt),
        Expr::BoolLit(_) => Ok(Type::TBool),
        Expr::Var(name) => {
            env.get(name).cloned().ok_or_else(|| format!("unbound variable: {}", name))
        }
        Expr::Add(a, b) => {
            let ta = type_check(a, env)?;
            let tb = type_check(b, env)?;
            if ta == Type::TInt && tb == Type::TInt {
                Ok(Type::TInt)
            } else {
                Err(format!("+ expects Int, got {} and {}", type_to_string(&ta), type_to_string(&tb)))
            }
        }
        Expr::Eq(a, b) => {
            let ta = type_check(a, env)?;
            let tb = type_check(b, env)?;
            if ta == tb {
                Ok(Type::TBool)
            } else {
                Err(format!("== expects same types, got {} and {}", type_to_string(&ta), type_to_string(&tb)))
            }
        }
        Expr::If(cond, then_expr, else_expr) => {
            let tc = type_check(cond, env)?;
            if tc != Type::TBool {
                return Err(format!("if condition must be Bool, got {}", type_to_string(&tc)));
            }
            let tt = type_check(then_expr, env)?;
            let te = type_check(else_expr, env)?;
            if tt == te {
                Ok(tt)
            } else {
                Err(format!("if branches differ: {} vs {}", type_to_string(&tt), type_to_string(&te)))
            }
        }
        Expr::Lam(param, param_ty, body) => {
            let mut new_env = env.clone();
            new_env.insert(param.clone(), param_ty.clone());
            let body_ty = type_check(body, &new_env)?;
            Ok(Type::TFun(Box::new(param_ty.clone()), Box::new(body_ty)))
        }
        Expr::App(func, arg) => {
            let tf = type_check(func, env)?;
            let ta = type_check(arg, env)?;
            match tf {
                Type::TFun(param_ty, ret_ty) => {
                    if *param_ty == ta {
                        Ok(*ret_ty)
                    } else {
                        Err(format!("arg type mismatch: expected {}, got {}",
                            type_to_string(&param_ty), type_to_string(&ta)))
                    }
                }
                _ => Err(format!("not a function: {}", type_to_string(&tf))),
            }
        }
    }
}

fn check_and_show(name: &str, expr: &Expr) {
    let env = HashMap::new();
    match type_check(expr, &env) {
        Ok(ty) => println!("{}: {}", name, type_to_string(&ty)),
        Err(e) => println!("{}: ERROR {}", name, e),
    }
}

fn main() {
    // 1 + 2 : Int
    let e1 = Expr::Add(Box::new(Expr::Lit(1)), Box::new(Expr::Lit(2)));
    check_and_show("1+2", &e1);

    // if true then 1 else 2 : Int
    let e2 = Expr::If(
        Box::new(Expr::BoolLit(true)),
        Box::new(Expr::Lit(1)),
        Box::new(Expr::Lit(2)),
    );
    check_and_show("if-then-else", &e2);

    // (\x:Int. x + 1) : Int -> Int
    let e3 = Expr::Lam(
        "x".to_string(), Type::TInt,
        Box::new(Expr::Add(Box::new(Expr::Var("x".to_string())), Box::new(Expr::Lit(1)))),
    );
    check_and_show("lambda", &e3);

    // (\x:Int. x + 1) 5 : Int
    let e4 = Expr::App(Box::new(e3.clone()), Box::new(Expr::Lit(5)));
    check_and_show("application", &e4);

    // 1 == 2 : Bool
    let e5 = Expr::Eq(Box::new(Expr::Lit(1)), Box::new(Expr::Lit(2)));
    check_and_show("equality", &e5);

    // ERROR: 1 + true
    let e6 = Expr::Add(Box::new(Expr::Lit(1)), Box::new(Expr::BoolLit(true)));
    check_and_show("type-error", &e6);

    // ERROR: unbound variable
    let e7 = Expr::Var("y".to_string());
    check_and_show("unbound", &e7);
}
