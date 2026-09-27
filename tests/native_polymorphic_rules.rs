use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONSUMER: AtomicU64 = AtomicU64::new(0);

#[test]
fn unconstrained_facts_preserve_types_and_repeated_variable_constraints() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/logic_polymorphic_facts.runa");
    for native in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        if native {
            command.arg("run");
        }
        let output = command.arg(&source).output().unwrap();
        assert!(
            output.status.success(),
            "native={native}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "same(1, 2) (expect false): false\nhas_self_loop (expect false): false\nfindall edge(n, n) (expect []): []\nfirst argument\nsecond argument\nunused argument\npolymorphic rule facts passed\n",
            "native={native}"
        );
    }
}

#[test]
fn exported_generic_facts_stay_in_their_library_module() {
    assert_rust_consumer(
        "@ export\n| same(value, value)\n",
        r#"
mod rules;
fn main() {
    assert!(rules::same(1_i64, 1_i64));
    assert!(!rules::same(1_i64, 2_i64));
    assert!(rules::same("law", "law"));
    assert!(!rules::same(1_i64, "1"));
    println!("embedded rules passed");
}
"#,
    );
}

#[test]
fn query_callbacks_use_their_library_function_despite_consumer_and_parameter_shadowing() {
    assert_rust_consumer(
        r#"
> is_small(value: Int) -> Bool { value < 3 }
| number(1)
| number(2)
| number(3)
| accepted(value) -> number(value), is_small(value)
@ export
> selected(is_small: Int) -> List(Int) { findall(value, accepted(value)) }
"#,
        r#"
mod rules;
#[allow(dead_code)]
fn is_small(_: i64) -> bool { false }
fn main() {
    assert_eq!(rules::selected(99), vec![1_i64, 2_i64]);
    println!("embedded rules passed");
}
"#,
    );
}

#[test]
fn query_override_guards_and_values_keep_their_declaring_library_scope() {
    assert_rust_consumer(
        r#"
> blocked(value: Int) -> Bool { value == 1 }
> decision(value: Int) -> Bool { False }
| accepted(1)
| accepted(2)
| accepted(3)
| exception denial accepted(1) -> decision(1) under blocked(1)
@ export
> selected(blocked: Int) -> List(Int) { findall(value, accepted(value)) }
"#,
        r#"
mod rules;
#[allow(dead_code)]
fn blocked(_: i64) -> bool { false }
#[allow(dead_code)]
fn decision(_: i64) -> bool { true }
fn main() {
    assert_eq!(rules::selected(99), vec![2_i64, 3_i64]);
    println!("embedded rules passed");
}
"#,
    );
}

fn assert_rust_consumer(source: &str, consumer: &str) {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "futuruna-polymorphic-library-{}-{nonce}-{}",
        std::process::id(),
        NEXT_CONSUMER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let source_path = directory.join("rules.runa");
    std::fs::write(&source_path, source).unwrap();
    let emitted = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("lib")
        .arg(&source_path)
        .output()
        .unwrap();
    assert!(
        emitted.status.success(),
        "{}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    std::fs::write(directory.join("rules.rs"), emitted.stdout).unwrap();
    std::fs::write(directory.join("consumer.rs"), consumer).unwrap();
    let compiled = Command::new("rustc")
        .current_dir(&directory)
        .args(["--edition=2021", "consumer.rs", "-o", "consumer"])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}: {}",
        directory.display(),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(directory.join("consumer")).output().unwrap();
    assert!(
        executed.status.success(),
        "{}: {}",
        directory.display(),
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(executed.stdout, b"embedded rules passed\n");
    std::fs::remove_dir_all(directory).unwrap();
}
