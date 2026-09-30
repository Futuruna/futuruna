use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "futuruna-cli-diagnostics-{}-{}-{}.runa",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }

    fn run(&self, arguments: &[&str], no_color: bool) -> Output {
        let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        let mut command = Command::new(runa);
        command
            .args(arguments)
            .arg(&self.0)
            .env("TERM", "xterm-256color");
        if no_color {
            command.env("NO_COLOR", "1");
        } else {
            command.env_remove("NO_COLOR");
        }
        command.output().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        }
    }
}

#[test]
fn runtime_errors_report_the_user_source_and_stop_later_effects() {
    for (source, message, line) in [
        (
            "= xs = []\n@ print(show(head(xs)))\n@ print(\"after\")\n",
            "head: empty list",
            2,
        ),
        (
            "assert(False)\n@ print(\"after\")\n",
            "Assertion failed!",
            1,
        ),
    ] {
        let fixture = Fixture::new(source);
        let output = fixture.run(&[], true);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(stderr.contains(message), "{stderr}");
        assert!(
            stderr.contains(&format!("{}:{line}:", fixture.0.display())),
            "{stderr}"
        );
        assert!(
            !stderr.contains("panicked at") && !stderr.contains("RUST_BACKTRACE"),
            "{stderr}"
        );
    }
}

#[test]
fn helper_faults_point_to_the_invocation_and_keep_completed_effects() {
    for definition in ["> broken() -> Int { head([]) }", "| broken() -> head([])"] {
        let fixture = Fixture::new(&format!(
            "{definition}\n@ print(\"before\")\n@ print(show(broken()))\n@ print(\"after\")\n"
        ));
        let output = fixture.run(&[], true);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert_eq!(output.stdout, b"before\n", "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("head: empty list")
                && stderr.contains(&format!("{}:3:", fixture.0.display())),
            "{stderr}"
        );
        assert!(!stderr.contains("panicked at"), "{stderr}");
    }
}

#[test]
fn successful_checks_respect_no_color_and_redirected_streams() {
    let fixture = Fixture::new("-- expect-command: check\n-- expect-status: pass\n> identity(value: Int) -> Int { value }\n");
    for no_color in [true, false] {
        for arguments in [
            &["check", "--frontend"][..],
            &["check"][..],
            &["expect"][..],
            &["audit"][..],
        ] {
            let output = fixture.run(arguments, no_color);
            assert!(output.status.success(), "{output:?}");
            assert!(
                !output.stdout.contains(&0x1b) && !output.stderr.contains(&0x1b),
                "{output:?}"
            );
        }
    }
}

#[test]
fn no_color_does_not_rewrite_authored_program_output() {
    let fixture = Fixture::new("@ print(\"\x1b[31mauthored\x1b[0m\")\n");
    let output = fixture.run(&[], true);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"\x1b[31mauthored\x1b[0m\n");
}

#[test]
fn json_parse_errors_are_separate_with_scalar_columns_and_eof_ranges() {
    let fixture = Fixture::new("= ok = 1\r\n@ print(\"æ😀\" 2)\r\n@ print(\"x\" 3)\r\n");
    let output = fixture.run(&["check", "--json", "--frontend"], true);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let errors = report["diagnostics"].as_array().unwrap();
    assert_eq!(errors.len(), 2);
    for (error, line, column) in [(&errors[0], 2, 14), (&errors[1], 3, 13)] {
        assert_eq!(
            error["location"]["range"]["start"],
            serde_json::json!({"line": line, "column": column})
        );
        assert_eq!(
            error["location"]["range"]["end"],
            serde_json::json!({"line": line, "column": column + 1})
        );
    }

    std::fs::write(&fixture.0, "= ok = 1\n> f(").unwrap();
    let output = fixture.run(&["check", "--json", "--frontend"], true);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let range = &report["diagnostics"][0]["location"]["range"];
    assert_eq!(range["start"], serde_json::json!({"line": 2, "column": 5}));
    assert_eq!(range["end"], range["start"]);
}

#[test]
fn json_distinguishes_backend_and_file_read_failures() {
    let fixture = Fixture::new("@ rust { fn bad() { let value: i64 = \"wrong\"; } }\n");
    let output = fixture.run(&["check", "--json"], false);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["phase"], "backend", "{report}");
    assert_eq!(report["backend_validated"], false, "{report}");
    assert!(
        report["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("mismatched types"),
        "{report}"
    );
    assert_eq!(
        report["diagnostics"][0]["location"]["range"]["start"]["line"], 1,
        "{report}"
    );
    assert!(output.stderr.is_empty(), "{output:?}");

    std::fs::remove_file(&fixture.0).unwrap();
    let output = fixture.run(&["check", "--json"], false);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["phase"], "read", "{report}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn json_backend_success_and_cached_success_retain_validation_status() {
    let fixture = Fixture::new("> identity(value: Int) -> Int { value }\n");
    for _ in 0..2 {
        let output = fixture.run(&["check", "--json"], false);
        assert!(output.status.success(), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["ok"], true);
        assert_eq!(report["backend_validated"], true);
        assert_eq!(report["phase"], "backend");
        assert_eq!(report["schema_version"], 1);
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

#[test]
fn check_json_reports_frontend_and_parse_errors_as_structured_output() {
    for (source, expected) in [
        (
            "> unused(value: Intt) -> Int { 1 }\n",
            "unknown type `Intt`",
        ),
        ("= value = [\n", "expected"),
    ] {
        let fixture = Fixture::new(source);
        let output = fixture.run(&["check", "--json"], false);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{error}: {output:?}"));
        assert_eq!(report["ok"], false, "{report}");
        assert!(
            report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| {
                    diagnostic["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(expected))
                }),
            "{report}"
        );
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

#[test]
fn check_json_success_is_a_single_machine_readable_report() {
    let fixture = Fixture::new("> identity(value: Int) -> Int { value }\n");
    let output = fixture.run(&["check", "--frontend", "--json"], false);
    assert!(output.status.success(), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true, "{report}");
    assert!(
        report["diagnostics"].as_array().unwrap().is_empty(),
        "{report}"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
}
