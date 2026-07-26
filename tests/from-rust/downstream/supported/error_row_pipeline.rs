// Downstream-shaped fixture: Result pipeline with parse and validation errors.

#[derive(Clone, Debug)]
enum RowError {
    Missing(String),
    Invalid(String),
    TooSmall(String),
}

fn parse_required(field: &str, raw: Option<String>) -> Result<i64, RowError> {
    match raw {
        Some(value) => {
            let parsed: i64 = value
                .parse()
                .map_err(|_| RowError::Invalid(field.to_string()))?;
            if parsed > 0 {
                Ok(parsed)
            } else {
                Err(RowError::TooSmall(field.to_string()))
            }
        }
        None => Err(RowError::Missing(field.to_string())),
    }
}

fn row_total(raw_qty: Option<String>, raw_cents: Option<String>) -> Result<i64, RowError> {
    let qty = parse_required("qty", raw_qty)?;
    let cents = parse_required("cents", raw_cents)?;
    Ok(qty * cents)
}

fn describe(result: Result<i64, RowError>) -> String {
    match result {
        Ok(total) => format!("ok:{}", total),
        Err(RowError::Missing(field)) => format!("missing:{}", field),
        Err(RowError::Invalid(field)) => format!("invalid:{}", field),
        Err(RowError::TooSmall(field)) => format!("small:{}", field),
    }
}

fn main() {
    println!(
        "{}",
        describe(row_total(Some("3".to_string()), Some("250".to_string())))
    );
    println!("{}", describe(row_total(None, Some("100".to_string()))));
    println!(
        "{}",
        describe(row_total(Some("two".to_string()), Some("100".to_string())))
    );
    println!(
        "{}",
        describe(row_total(Some("1".to_string()), Some("0".to_string())))
    );
}
