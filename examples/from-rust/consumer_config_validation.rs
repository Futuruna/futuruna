// Consumer-shaped fixture: config parsing and validation

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

fn main() {
    match parse_positive("port", Some("8080".to_string())) {
        Ok(port) => println!("port={}", port),
        Err(_) => println!("port invalid"),
    }

    match parse_positive("workers", Some("-2".to_string())) {
        Ok(workers) => println!("workers={}", workers),
        Err(_) => println!("workers invalid"),
    }

    match parse_positive("timeout", None) {
        Ok(timeout) => println!("timeout={}", timeout),
        Err(_) => println!("timeout missing"),
    }
}
