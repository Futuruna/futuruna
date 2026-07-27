use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_RUST_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRustFile {
    path: PathBuf,
}

impl TempRustFile {
    fn new(stem: &str, source: &str) -> Self {
        let mut path = std::env::temp_dir();
        let seq = TEMP_RUST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let unique = format!(
            "futuruna_from_rust_verify_{}_{}_{}_{}.rs",
            stem,
            std::process::id(),
            seq,
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
    run_from_rust_verify_with_env(source, &[])
}

fn run_from_rust_verify_with_env(source: &str, envs: &[(&str, &str)]) -> Output {
    let fixture = TempRustFile::new("fixture", source);
    let mut command = Command::new(runa());
    command.args([
        "from-rust",
        "--verify",
        fixture.path.to_str().expect("utf8 path"),
    ]);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run runa from-rust --verify")
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
fn from_rust_verify_reports_stable_rust_parse_failure_line() {
    let output = run_from_rust_verify(
        r#"
fn main( {
    println!("bad");
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected Rust parse verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: rust-parse error:"),
        "missing stable Rust parse failure line:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_reports_stable_rust_compile_failure_line() {
    let output = run_from_rust_verify(
        r#"
fn main() {
    println!("{}", missing_value);
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected Rust compile verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: translated "),
        "compile failure path should still report translation first:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: rust-compile-failed "),
        "missing stable Rust compile failure line:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_reports_translated_parse_failed_harness_line() {
    let output = run_from_rust_verify_with_env(
        r#"
fn main() {
    println!("rust");
}
"#,
        &[
            ("FUTURUNA_FROM_RUST_VERIFY_HARNESS", "1"),
            ("FUTURUNA_FROM_RUST_VERIFY_RUNA_SOURCE", "> broken("),
        ],
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected translated parse verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: translated-parse-failed "),
        "missing stable translated parse failure line:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: mismatch "),
        "parse failure should stop before output comparison:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_reports_mismatch_harness_line() {
    let output = run_from_rust_verify_with_env(
        r#"
fn main() {
    println!("rust");
}
"#,
        &[
            ("FUTURUNA_FROM_RUST_VERIFY_HARNESS", "1"),
            (
                "FUTURUNA_FROM_RUST_VERIFY_RUNA_SOURCE",
                "-- harness translated source\n@ print(\"futuruna\")\n",
            ),
        ],
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected mismatch verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: mismatch "),
        "missing stable mismatch line:\n{}",
        stderr
    );
    assert!(
        stderr.contains(" rust_lines=1 futuruna_lines=1"),
        "mismatch line should include line counts:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_rejects_unsupported_format_spec_before_translation() {
    let output = run_from_rust_verify(
        r#"
fn main() {
    println!("{:x}", 10);
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected unsupported format-spec verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported unsupported-format-spec:"),
        "missing stable unsupported format-spec line:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: translated "),
        "unsupported format spec should fail before translation:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: mismatch "),
        "unsupported format spec should fail before output comparison:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_rejects_unsupported_macro_before_translation() {
    let output = run_from_rust_verify(
        r#"
fn main() {
    print!("hi");
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected unsupported macro verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported unsupported-macro:"),
        "missing stable unsupported macro line:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: translated "),
        "unsupported macro should fail before translation:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: mismatch "),
        "unsupported macro should fail before output comparison:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_rejects_unsupported_expr_fallback_before_translation() {
    let output = run_from_rust_verify(
        r#"
fn main() {
    let xs = [0; 3];
    println!("{}", xs[0]);
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected unsupported expression verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported unsupported-rust-expr:"),
        "missing stable unsupported expression line:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: translated "),
        "unsupported expression fallback should fail before translation:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: mismatch "),
        "unsupported expression fallback should fail before output comparison:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_reports_additional_unsupported_category() {
    let output = run_from_rust_verify(
        r#"
use std::thread;

fn main() {
    let handle = thread::spawn(|| 7);
    println!("{}", handle.join().unwrap());
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected async/threading unsupported verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported async-threading:"),
        "missing stable async-threading unsupported line:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_rejects_effectful_std_api_before_translation() {
    let output = run_from_rust_verify(
        r#"
fn main() {
    let _ = std::env::var("HOME");
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected effectful std unsupported verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported unsupported-effect:"),
        "missing stable unsupported-effect line:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: translated "),
        "unsupported effect should fail before translation:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: mismatch "),
        "unsupported effect should fail before output comparison:\n{}",
        stderr
    );
}

#[test]
fn from_rust_verify_rejects_module_declaration_before_translation() {
    let output = run_from_rust_verify(
        r#"
mod helper;

fn main() {
    println!("module fixture declared");
}
"#,
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected unsupported module verify failure, stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: unsupported unsupported-module:"),
        "missing stable unsupported-module line:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: translated "),
        "unsupported module should fail before translation:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("from-rust verify: mismatch "),
        "unsupported module should fail before output comparison:\n{}",
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
    assert!(
        stderr.contains("from-rust verify: translated-parse-failed <file>: <message>"),
        "stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("from-rust verify: mismatch <file> rust_lines=<n> futuruna_lines=<n>"),
        "stderr:\n{}",
        stderr
    );
}
