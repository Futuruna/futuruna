//! Model-level native/interpreter parity; no fabricated rates or partial-rule
//! codegen-policy exemption. All cases are source tables or fictional reports.
use std::process::{Command, Output};

fn execute(native: bool, path: &str) -> Output {
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let mut command = Command::new(binary);
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS", "1");
    if native {
        command.arg("run");
    }
    command.arg(path).output().expect("execute parameter case")
}

#[test]
fn source_parameters_and_report_results_match_native_execution() {
    for path in [
        "tests/tax_parameter_domain_test.runa",
        "tests/tax_report_native_test.runa",
    ] {
        let interpreted = execute(false, path);
        let native = execute(true, path);
        for (lane, output) in [("interpreter", &interpreted), ("native", &native)] {
            assert!(
                output.status.success(),
                "{lane} {path}: {}\n{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(!output.stdout.is_empty(), "{lane} {path}: no results");
        }
        assert_eq!(
            interpreted.stdout, native.stdout,
            "{path}: differing results"
        );
    }
}

#[test]
fn strict_parameter_helpers_fail_instead_of_fabricating_missing_rates() {
    for path in [
        "tests/fixtures/tax_parameter_domain/unsupported_national.runa",
        "tests/fixtures/tax_parameter_domain/unsupported_property.runa",
    ] {
        for native in [false, true] {
            let output = execute(native, path);
            assert!(
                !output.status.success(),
                "{path}: unsupported domain succeeded"
            );
            assert!(
                output.stdout.is_empty(),
                "{path}: emitted a fabricated result"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("head: empty list"),
                "{path}: expected runtime domain failure, not a compilation error: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
