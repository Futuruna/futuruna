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
        "tests/tax_deduction_parameter_domain_test.runa",
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
fn ordinary_deductions_match_external_skat_calculator_boundaries() {
    let path = "examples/danish-income-tax/skatdk-arbejdsfradrag-ekstern.scenario.runa";
    let interpreted = execute(false, path);
    let native = execute(true, path);
    for (lane, output) in [("interpreter", &interpreted), ("native", &native)] {
        assert!(
            output.status.success(),
            "{lane}: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("11 fiktive"));
    }
    assert_eq!(interpreted.stdout, native.stdout);
}

#[test]
fn extra_pension_deduction_matches_external_rounding_and_native_execution() {
    let path = "examples/danish-income-tax/ligningsloven-par9l.scenario.runa";
    let interpreted = execute(false, path);
    let native = execute(true, path);
    for (lane, output) in [("interpreter", &interpreted), ("native", &native)] {
        assert!(
            output.status.success(),
            "{lane}: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("8 fiktive pensionsfradragscases"));
    }
    assert_eq!(interpreted.stdout, native.stdout);
}

#[test]
fn personal_allowance_transfer_matches_native_execution() {
    let path = "tests/personfradrag_transfer_test.runa";
    let interpreted = execute(false, path);
    let native = execute(true, path);
    for (lane, output) in [("interpreter", &interpreted), ("native", &native)] {
        assert!(
            output.status.success(),
            "{lane}: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("7 samordnings-"));
    }
    assert_eq!(interpreted.stdout, native.stdout);
}

#[test]
fn personal_allowance_recipient_offsets_obey_cohabitation() {
    // The surrounding PSL §10 source still has pre-existing native guarded-
    // lookup failures (td-124b83). The isolated new §12 kernel has parity above;
    // this checks its integration into §10 without claiming full native support.
    let output = execute(
        false,
        "tests/fixtures/tax_parameter_domain/personfradrag_recipient.runa",
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("2 modtagerkontroller"));
}

#[test]
fn spouse_loss_recipient_rates_preserve_capacity_and_priority() {
    // Full wage/tax native generation has separately tracked guarded-lookup
    // failures (td-124b83); this model change claims interpreter coverage only.
    for path in [
        "tests/fixtures/tax_parameter_domain/spouse_loss_recipient.runa",
        "examples/danish-income-tax/loenmodtager-par13-spouse.audit.runa",
        "examples/danish-income-tax/loenmodtager-par13-priority.audit.runa",
    ] {
        let output = execute(false, path);
        assert!(
            output.status.success(),
            "{path}: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(!output.stdout.is_empty(), "{path}: no results");
    }
}

#[test]
fn deduction_ore_projection_preserves_external_cases_and_known_disagreements() {
    for path in [
        "examples/danish-income-tax/skatdk-fradrag-oere-ekstern.scenario.runa",
        "tests/personskat_senior_test.runa",
        "tests/personskat_single_parent_test.runa",
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
        }
        assert_eq!(interpreted.stdout, native.stdout, "{path}");
    }
}

#[test]
fn strict_parameter_helpers_fail_instead_of_fabricating_missing_rates() {
    for path in [
        "tests/fixtures/tax_parameter_domain/unsupported_national.runa",
        "tests/fixtures/tax_parameter_domain/unsupported_property.runa",
        "tests/fixtures/tax_parameter_domain/unsupported_employment.runa",
        "tests/fixtures/tax_parameter_domain/unsupported_pension_ceiling.runa",
        "tests/fixtures/tax_parameter_domain/invalid_part_year_divisor.runa",
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

#[test]
fn state_tax_domains_preserve_native_results_and_reject_missing_inputs() {
    // Compile this large shared import graph once, then exercise runtime edges
    // against that exact binary rather than recompiling for every input.
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let binary = std::fs::canonicalize(binary).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/tax_state_parameter_domain_test.runa");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "futuruna-state-tax-native-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let build = Command::new(&binary)
        .current_dir(&directory)
        .env("FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS", "1")
        .args(["build", source.to_str().unwrap()])
        .output()
        .unwrap();
    if !build.status.success() {
        let diagnostic_path = directory.join("build.stderr");
        std::fs::write(&diagnostic_path, &build.stderr).unwrap();
        panic!(
            "state-tax build failed (complete diagnostics at {}):\n{}",
            diagnostic_path.display(),
            String::from_utf8_lossy(&build.stderr)
                .lines()
                .take(90)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    let executable = directory.join(format!(
        "tax_state_parameter_domain_test{}",
        std::env::consts::EXE_SUFFIX
    ));
    let run_case = |native: bool, case: &str| {
        let mut command = Command::new(if native { &executable } else { &binary });
        command
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS", "1")
            .env("FUTURUNA_STATE_PARAMETER_CASE", case);
        if !native {
            command.arg(&source);
        }
        command.output().unwrap()
    };
    let interpreted = run_case(false, "supported");
    let native = run_case(true, "supported");
    for output in [&interpreted, &native] {
        assert!(
            output.status.success(),
            "state-tax cases: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
    }
    assert_eq!(interpreted.stdout, native.stdout, "state-tax native parity");
    for case in [
        "missing-reform",
        "pre-reform",
        "post-historical",
        "missing-bundskat",
        "missing-par8c",
        "missing-al",
        "timely-interest",
        "cross-year-interest",
    ] {
        let output = run_case(true, case);
        assert!(
            !output.status.success(),
            "{case}: unsupported native result"
        );
        assert!(output.stdout.is_empty(), "{case}: emitted a result");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("head: empty list"),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for case in ["missing-reform", "timely-interest"] {
        let output = run_case(false, case);
        assert!(
            !output.status.success(),
            "{case}: unsupported interpreted result"
        );
        assert!(output.stdout.is_empty(), "{case}: emitted a result");
        assert!(String::from_utf8_lossy(&output.stderr).contains("head: empty list"));
    }
    std::fs::remove_file(executable).unwrap();
    std::fs::remove_dir(directory).unwrap();
    println!("State-tax native parity, 16 invariants and eight runtime rejection edges passed");
}
