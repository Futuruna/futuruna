// Real-world 2: A complete stack-based calculator
// Exercises: enums, Vec as stack, match, error handling, loops

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Add,
    Sub,
    Mul,
    Div,
    Dup,
    Swap,
    Pop,
}

fn tokenize(input: &str) -> Vec<Token> {
    input.split_whitespace()
        .map(|word| match word {
            "+" => Token::Add,
            "-" => Token::Sub,
            "*" => Token::Mul,
            "/" => Token::Div,
            "dup" => Token::Dup,
            "swap" => Token::Swap,
            "pop" => Token::Pop,
            n => Token::Num(n.parse().unwrap_or(0.0)),
        })
        .collect()
}

fn eval(tokens: &[Token]) -> Result<Vec<f64>, String> {
    let mut stack: Vec<f64> = Vec::new();

    for token in tokens {
        match token {
            Token::Num(n) => stack.push(*n),
            Token::Add => {
                if stack.len() < 2 { return Err("stack underflow on +".to_string()); }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(a + b);
            }
            Token::Sub => {
                if stack.len() < 2 { return Err("stack underflow on -".to_string()); }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(a - b);
            }
            Token::Mul => {
                if stack.len() < 2 { return Err("stack underflow on *".to_string()); }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(a * b);
            }
            Token::Div => {
                if stack.len() < 2 { return Err("stack underflow on /".to_string()); }
                let b = stack.pop().unwrap();
                if b == 0.0 { return Err("division by zero".to_string()); }
                let a = stack.pop().unwrap();
                stack.push(a / b);
            }
            Token::Dup => {
                if stack.is_empty() { return Err("stack underflow on dup".to_string()); }
                let top = *stack.last().unwrap();
                stack.push(top);
            }
            Token::Swap => {
                if stack.len() < 2 { return Err("stack underflow on swap".to_string()); }
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            Token::Pop => {
                if stack.is_empty() { return Err("stack underflow on pop".to_string()); }
                stack.pop();
            }
        }
    }
    Ok(stack)
}

fn run_calc(input: &str) -> String {
    let tokens = tokenize(input);
    match eval(&tokens) {
        Ok(stack) => {
            if stack.is_empty() {
                "empty stack".to_string()
            } else {
                format!("{}", stack.last().unwrap())
            }
        }
        Err(e) => format!("error: {}", e),
    }
}

fn main() {
    println!("{}", run_calc("3 4 +"));
    println!("{}", run_calc("10 3 - 2 *"));
    println!("{}", run_calc("5 dup *"));
    println!("{}", run_calc("1 2 3 swap +"));
    println!("{}", run_calc("42 pop"));
    println!("{}", run_calc("10 0 /"));
    println!("{}", run_calc("+"));
}
