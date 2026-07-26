// Downstream-shaped fixture: text normalization and classification.

fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    let lowered = trimmed.to_lowercase();
    lowered.replace("-", "_")
}

fn classify(raw: &str) -> String {
    let token = normalize(raw);
    if token.starts_with("error") {
        format!("alert:{}", token)
    } else if token.ends_with("_ok") {
        format!("ready:{}", token)
    } else {
        format!("note:{}", token)
    }
}

fn main() {
    println!("{}", normalize("  User-Login  "));
    println!("{}", classify("ERROR-timeout"));
    println!("{}", classify("batch-ok"));
    println!("{}", classify(" status-pending "));
}
