use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audit/reachability.calculate.runa")
}

fn run(args: &[&str]) -> Output {
    Command::new(runa())
        .args(args)
        .output()
        .expect("run runa audit command")
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn callable_status<'a>(report: &'a Value, qualified_name: &str) -> &'a str {
    report["callables"]
        .as_array()
        .expect("callables")
        .iter()
        .find(|callable| callable["qualified_name"] == qualified_name)
        .unwrap_or_else(|| panic!("missing callable {qualified_name}"))["status"]
        .as_str()
        .expect("callable status")
}

#[test]
fn calculation_reachability_json_tracks_imports_scopes_and_bindings() {
    let fixture = fixture();
    let output = run(&[
        "audit",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "calculate_reachability",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let report = parse_stdout(&output);
    assert_eq!(report["schema"], "futuruna.audit.reachability.v1");
    assert_eq!(report["entry"], "calculate_reachability");
    assert_eq!(report["input_type"], "ReachabilityInput");
    assert_eq!(report["output_type"], "ReachabilityOutput");
    assert_eq!(
        callable_status(&report, "calculate_reachability"),
        "reachable"
    );
    assert_eq!(callable_status(&report, "imported_helper"), "reachable");
    assert_eq!(
        callable_status(&report, "UsedCalculationScope"),
        "reachable"
    );
    assert_eq!(
        callable_status(&report, "UsedCalculationScope.adjusted"),
        "reachable"
    );
    assert_eq!(
        callable_status(&report, "UsedCalculationScope.result"),
        "reachable"
    );
    assert_eq!(
        callable_status(&report, "calculate_alternate"),
        "not_reached"
    );
    assert_eq!(
        callable_status(&report, "imported_unused_rule"),
        "not_reached"
    );
    assert_eq!(
        callable_status(&report, "UnusedCalculationScope"),
        "not_reached"
    );
    assert_eq!(
        callable_status(&report, "UnusedCalculationScope.result"),
        "not_reached"
    );
    let required_bindings = report["required_bindings"]
        .as_array()
        .expect("required bindings");
    assert!(required_bindings
        .iter()
        .any(|binding| binding == "imported_base"));
    assert!(!required_bindings
        .iter()
        .any(|binding| binding == "imported_unused_binding"));
    assert!(report["loaded_sources"]
        .as_array()
        .expect("loaded sources")
        .iter()
        .filter_map(Value::as_str)
        .any(|source| source.ends_with("tests/fixtures/audit/reachability-helper.runa")));
}

#[test]
fn calculation_reachability_requires_entry_for_multiple_calculations() {
    let fixture = fixture();
    let output = run(&["audit", fixture.to_str().expect("fixture path"), "--json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source has multiple calculations"));
}

#[test]
fn calculation_reachability_rejects_unknown_entry() {
    let fixture = fixture();
    let output = run(&[
        "audit",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "missing_calculation",
        "--json",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown calculation entry `missing_calculation`"));
    assert!(stderr.contains("`calculate_reachability`"));
    assert!(stderr.contains("`calculate_alternate`"));
}

#[test]
fn audit_without_entry_keeps_the_existing_topology_report() {
    let fixture = fixture();
    let output = run(&["audit", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("automated gap discovery"));
}
