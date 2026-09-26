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

#[test]
fn test_runner_selects_scenarios_and_audits_with_parallel_jobs() {
    let dir = temp_test_dir();
    std::fs::write(dir.join("b.scenario.runa"), "@ print(\"scenario-beta\")\n")
        .expect("write scenario fixture");
    std::fs::write(dir.join("a.scenario.runa"), "@ print(\"scenario-alpha\")\n")
        .expect("write scenario fixture");
    std::fs::write(dir.join("c.audit.runa"), "@ print(\"audit\")\n").expect("write audit fixture");
    std::fs::write(dir.join("library.runa"), "@ print(\"library\")\n")
        .expect("write regular fixture");

    let dir_arg = dir.to_str().expect("UTF-8 fixture path");
    let scenarios = run(&["test", "--kind", "scenario", "--jobs=2", dir_arg]);
    let audits = run(&["test", "--kind=audit", "--jobs", "2", dir_arg]);
    let all = run(&["test", "--kind", "all", "--jobs", "2", dir_arg]);
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        scenarios.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&scenarios.stdout),
        String::from_utf8_lossy(&scenarios.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&scenarios.stdout),
        "scenario-alpha\nscenario-beta\n"
    );
    assert!(
        audits.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&audits.stdout),
        String::from_utf8_lossy(&audits.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&audits.stdout), "audit\n");
    assert!(
        all.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&all.stdout),
        String::from_utf8_lossy(&all.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&all.stdout),
        "scenario-alpha\nscenario-beta\naudit\nlibrary\n"
    );

    let scenario_stderr = String::from_utf8_lossy(&scenarios.stderr);
    assert!(scenario_stderr.contains("running 2 tests"));
    assert!(scenario_stderr.contains("a.scenario.runa"));
    assert!(scenario_stderr.contains("b.scenario.runa"));
    assert!(!scenario_stderr.contains("c.audit.runa"));
    assert!(!scenario_stderr.contains("library.runa"));
}

#[test]
fn test_runner_rejects_unknown_test_kind() {
    let output = run(&["test", "--kind", "benchmark"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("--kind requires all, scenario, or audit, got 'benchmark'"));
}

#[test]
fn test_runner_reports_every_failure_and_keeps_invariant_details() {
    let dir = temp_test_dir();
    for (name, source) in [
        ("a.runa", "@ print(\"before\")\n"),
        (
            "b.scenario.runa",
            "= value = 3\n| value_ok: value -> value < 2\n? value_ok\n",
        ),
        ("c.runa", "@ print(show(head([])))\n"),
        (
            "d.runa",
            "= other = 7\n| other_ok: other -> other < 2\n? other_ok -> { @ print(\"unreachable\") }\n",
        ),
        ("e.runa", "@ print(\"after\")\n"),
    ] {
        std::fs::write(dir.join(name), source).expect("write test fixture");
    }
    let dir_arg = dir.to_str().expect("UTF-8 fixture path");
    for compiled in [false, true] {
        for jobs in ["1", "2"] {
            let mut args = vec!["test", "--jobs", jobs, dir_arg];
            if compiled {
                args.push("--run");
            }
            let output = run(&args);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let details = format!("args: {args:?}\nstdout:\n{stdout}\nstderr:\n{stderr}");
            assert!(!output.status.success(), "{details}");
            assert!(stderr.contains("3 of 5 tests failed"), "{details}");
            for name in ["b.scenario.runa", "c.runa", "d.runa"] {
                assert!(stderr.contains(&format!("FAIL  {name}")), "{details}");
                assert!(stderr.contains(&format!("  - {name}")), "{details}");
            }
            assert!(stderr.contains("PASS  e.runa"), "{details}");
            assert!(details.contains("value_ok"), "{details}");
            assert!(details.contains("value: 3"), "{details}");
            assert!(details.contains("other_ok"), "{details}");
            assert!(stderr.contains("head: empty list"), "{details}");
            if !compiled {
                assert!(stdout.starts_with("before\n"), "{details}");
                assert!(stdout.ends_with("after\n"), "{details}");
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("remove owned test fixtures");
}

#[test]
fn test_runner_recognizes_expected_invariant_exits_and_rejects_missing_errors() {
    let dir = temp_test_dir();
    std::fs::write(
        dir.join("a.runa"),
        "-- expect-runtime-error: value_ok\n-- expect-runtime-error: value: [1, 2, 3]\n= value = [1, 2, 3]\n| value_ok: value -> length(value) < 2\n? value_ok\n",
    )
    .expect("write expected invariant failure");
    std::fs::write(
        dir.join("b.runa"),
        "-- expect-runtime-error: missing_error\n@ print(\"success\")\n",
    )
    .expect("write missing runtime error");
    std::fs::write(
        dir.join("c.runa"),
        "-- expect-runtime-error: wrong_message\n@ print(show(head([])))\n",
    )
    .expect("write mismatched runtime error");
    let dir_arg = dir.to_str().expect("UTF-8 fixture path");
    for compiled in [false, true] {
        for jobs in ["1", "2"] {
            let mut args = vec!["test", "--jobs", jobs, dir_arg];
            if compiled {
                args.push("--run");
            }
            let output = run(&args);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let details = format!("args: {args:?}\n{stderr}");
            assert!(!output.status.success(), "{details}");
            assert!(stderr.contains("2 of 3 tests failed"), "{details}");
            assert!(stderr.contains("PASS  a.runa"), "{details}");
            assert!(stderr.contains("expect-runtime-error"), "{details}");
            assert!(stderr.contains("FAIL  b.runa"), "{details}");
            assert!(
                stderr.contains("expected runtime error but program succeeded"),
                "{details}"
            );
            assert!(stderr.contains("FAIL  c.runa"), "{details}");
            assert!(stderr.contains("wrong_message"), "{details}");
            assert!(stderr.contains("head: empty list"), "{details}");
        }
    }
    std::fs::remove_dir_all(dir).expect("remove owned test fixtures");
}
