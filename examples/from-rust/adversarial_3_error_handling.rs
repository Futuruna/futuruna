// Adversarial 3: Error handling chains, ?, early returns
// The kind of Rust where ? does heavy lifting
// runa-from-rust: expect-unsupported map_err, From conversions, and rich error enums

use std::collections::HashMap;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    NotFound(String),
    ParseError(String),
    ValidationError { field: String, message: String },
}

impl From<ParseIntError> for AppError {
    fn from(e: ParseIntError) -> Self {
        AppError::ParseError(e.to_string())
    }
}

fn parse_age(s: &str) -> Result<i64, AppError> {
    let age: i64 = s.parse().map_err(|_| AppError::ParseError(format!("invalid age: {}", s)))?;
    if age < 0 || age > 150 {
        return Err(AppError::ValidationError {
            field: "age".to_string(),
            message: format!("age {} out of range", age),
        });
    }
    Ok(age)
}

fn lookup_user(db: &HashMap<String, String>, name: &str) -> Result<String, AppError> {
    db.get(name)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("user '{}' not found", name)))
}

fn process_user(db: &HashMap<String, String>, name: &str) -> Result<String, AppError> {
    let age_str = lookup_user(db, name)?;
    let age = parse_age(&age_str)?;
    let category = if age < 18 {
        "minor"
    } else if age < 65 {
        "adult"
    } else {
        "senior"
    };
    Ok(format!("{} is a {} (age {})", name, category, age))
}

fn process_all(db: &HashMap<String, String>, names: &[&str]) -> Vec<Result<String, AppError>> {
    names.iter().map(|name| process_user(db, name)).collect()
}

fn main() {
    let mut db = HashMap::new();
    db.insert("alice".to_string(), "30".to_string());
    db.insert("bob".to_string(), "17".to_string());
    db.insert("charlie".to_string(), "not_a_number".to_string());
    db.insert("diana".to_string(), "200".to_string());

    let names = ["alice", "bob", "charlie", "diana", "eve"];
    let results = process_all(&db, &names);

    for (name, result) in names.iter().zip(results.iter()) {
        match result {
            Ok(msg) => println!("  OK: {}", msg),
            Err(AppError::NotFound(msg)) => println!("  NOT FOUND: {}", msg),
            Err(AppError::ParseError(msg)) => println!("  PARSE ERROR: {}", msg),
            Err(AppError::ValidationError { field, message }) => {
                println!("  VALIDATION: {} - {}", field, message);
            }
        }
    }
}
