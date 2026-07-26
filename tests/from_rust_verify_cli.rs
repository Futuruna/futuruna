use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct TempRustFile {
    path: PathBuf,
}

impl TempRustFile {
    fn new(stem: &str, source: &str) -> Self {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "futuruna_from_rust_verify_{}_{}_{}.rs",
            stem,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        path.push(unique);
        fs::write(&path, source).expect("write temp Rust fixture");
        Self { path }
    }
}

impl Drop for TempRustFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn run_from_rust_verify(source: &str) -> Output {
    let fixture = TempRustFile::new("fixture", source);
    Command::new(runa())
        .args([
            "from-rust",
            "--verify",
            fixture.path.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run runa from-rust --verify")
}

#[test]
fn from_rust_verify_reports_stable_match_line() {
    let output = run_from_rust_verify(
        r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    println!("{}", add(2, 5));
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected supported verify success, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: translated "),
        "missing stable translated line:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: match "),
        "missing stable match line:\n{}",
        stderr
    );
    assert!(
        stderr.contains(" lines=1"),
        "stable match line should include line count:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_reports_stable_unsupported_line() {
    let output = run_from_rust_verify(
        r#"
fn main() {
    unsafe {
        println!("{}", 1);
    }
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected unsupported verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported unsafe-rust:"),
        "missing stable unsupported category line:\n{}",
        stderr
    );
}

#[test]
fn from_rust_help_names_frss_v0_and_verify_summaries() {
    let output = Command::new(runa())
        .args(["from-rust", "--help"])
        .output()
        .expect("run runa from-rust --help");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr:\n{}", stderr);
    assert!(
        stderr.contains("FRSS-v0 preview contract"),
        "stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: match <file> lines=<n>"),
        "stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported <category>: <message>"),
        "stderr:\n{}",
        stderr
    );
}
