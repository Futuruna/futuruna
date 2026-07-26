// Downstream-shaped fixture: application config validation.

#[derive(Clone, Debug)]
enum ConfigError {
    Missing(String),
    Invalid(String),
}

fn parse_positive(name: &str, raw: Option<String>) -> Result<i64, ConfigError> {
    match raw {
        Some(value) => {
            let n: i64 = value
                .parse()
                .map_err(|_| ConfigError::Invalid(name.to_string()))?;
            if n > 0 {
                Ok(n)
            } else {
                Err(ConfigError::Invalid(name.to_string()))
            }
        }
        None => Err(ConfigError::Missing(name.to_string())),
    }
}

fn summarize(name: &str, raw: Option<String>) -> String {
    match parse_positive(name, raw) {
        Ok(value) => format!("{}={}", name, value),
        Err(ConfigError::Missing(field)) => format!("{} missing", field),
        Err(ConfigError::Invalid(field)) => format!("{} invalid", field),
    }
}

fn main() {
    println!("{}", summarize("port", Some("8080".to_string())));
    println!("{}", summarize("workers", Some("-2".to_string())));
    println!("{}", summarize("timeout", None));
}
