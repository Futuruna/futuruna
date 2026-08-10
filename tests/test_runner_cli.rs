use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIR_ID: AtomicU64 = AtomicU64::new(0);

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn temp_test_dir() -> PathBuf {
    let unique_id = NEXT_TEMP_DIR_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "futuruna-test-runner-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
        unique_id
    ));
    std::fs::create_dir_all(&path).expect("create test runner fixture directory");
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(runa())
        .args(args)
        .output()
        .expect("run Futuruna test runner")
}

#[test]
fn parallel_test_runner_buffers_results_in_filename_order() {
    let dir = temp_test_dir();
    std::fs::write(dir.join("b.runa"), "@ print(\"beta\")\n").expect("write b fixture");
    std::fs::write(dir.join("a.runa"), "@ print(\"alpha\")\n").expect("write a fixture");
    std::fs::write(
        dir.join("c.runa"),
        "-- expect-runtime-error: head: empty list\n\n@ print(show(head([])))\n",
    )
    .expect("write runtime-error fixture");

    let dir_arg = dir.to_str().expect("UTF-8 fixture path");
    let serial_output = run(&["test", "--jobs", "1", dir_arg]);
    let output = run(&["test", "--jobs=2", dir_arg]);
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        serial_output.status.success(),
        "serial stdout:\n{}\nserial stderr:\n{}",
        String::from_utf8_lossy(&serial_output.stdout),
        String::from_utf8_lossy(&serial_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&serial_output.stdout),
        "alpha\nbeta\n"
    );
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "alpha\nbeta\n");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("with 2 jobs"));
    assert!(stderr.contains("c.runa"));
    assert!(stderr.contains("expect-runtime-error"));
    let a_index = stderr.find("a.runa").expect("a result");
    let b_index = stderr.find("b.runa").expect("b result");
    let c_index = stderr.find("c.runa").expect("c result");
    assert!(a_index < b_index && b_index < c_index);
}

#[test]
fn test_runner_rejects_zero_jobs() {
    let output = run(&["test", "--jobs", "0"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("--jobs requires a positive integer, got '0'"));

    let roundtrip = run(&["test", "--jobs", "2", "--roundtrip"]);
    assert!(!roundtrip.status.success());
    assert!(String::from_utf8_lossy(&roundtrip.stderr)
        .contains("--jobs is not supported with runa test --roundtrip"));
}
