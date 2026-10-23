use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audit/reachability.calculate.runa")
}

fn topology_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/audit")
        .join(name)
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

#[test]
fn topology_audit_does_not_infer_semantics_from_rule_names() {
    let fixture = topology_fixture("topology_names_are_not_semantics.runa");
    let output = run(&["audit", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Findings: 0 contradiction, 0 tension, 0 asymmetry, 0 gap"));
    assert!(stdout.contains("No structural findings discovered."));
    assert!(!stdout.contains("PARADOX"));
}

#[test]
fn topology_audit_finds_conflicting_active_branches_of_one_rule() {
    let fixture = topology_fixture("topology_semantic_contradiction.runa");
    let output = run(&["audit", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Findings: 1 contradiction"));
    assert!(stdout.contains("CONTRADICTIONS (1)"));
    assert!(stdout.contains("Conflicting active exception branches for reduced_rate_applies"));
    assert!(stdout.contains("family_grant -> true"));
    assert!(stdout.contains("statutory_exclusion -> false"));
    assert!(stdout.contains("same rule call"));
}

#[test]
fn topology_audit_reports_conflicting_unconditional_values_and_the_selected_result() {
    let fixture = topology_fixture("topology_unconditional_contradiction.runa");
    let runtime = run(&[fixture.to_str().unwrap()]);
    assert!(runtime.status.success());
    assert_eq!(String::from_utf8_lossy(&runtime.stdout).trim(), "25");

    let output = run(&["audit", fixture.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Findings: 1 contradiction"), "{stdout}");
    assert!(
        stdout.contains("Conflicting active ordinary clause branches for sats"),
        "{stdout}"
    );
    assert!(stdout.contains("-> 25"), "{stdout}");
    assert!(stdout.contains("-> 40"), "{stdout}");
    assert!(
        stdout.contains("Source order resolves sats() to 25"),
        "{stdout}"
    );
}

#[test]
fn topology_audit_keeps_equal_outcomes_boolean_alternatives_and_other_arities_separate() {
    let fixture = topology_fixture("topology_clause_alternatives.runa");
    let output = run(&["audit", fixture.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Findings: 0 contradiction, 1 tension"),
        "{stdout}"
    );
    assert!(stdout.contains("Override: cap"), "{stdout}");
    assert!(stdout.contains("default rule says cap() -> 25"), "{stdout}");
}

fn z3_available() -> bool {
    Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn audit_stdout(fixture: &Path) -> String {
    let output = run(&["audit", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The report block for one finding: its heading line and the indented
/// lines that follow it.
fn finding_block<'a>(stdout: &'a str, heading: &str) -> &'a str {
    let start = stdout
        .find(heading)
        .unwrap_or_else(|| panic!("missing finding `{heading}`:\n{stdout}"));
    let rest = &stdout[start..];
    let end = rest.find("\n\n").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn clause_audit_reports_shadowed_and_unsatisfiable_clauses() {
    let fixture = topology_fixture("clause_reachability.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("0 gap, 5 unreachable clause, 1 overlapping guards"),
        "{stdout}"
    );
    assert!(
        stdout.contains("6 rule families, 16 clauses: 16 analyzed, 0 not analyzed"),
        "{stdout}"
    );

    // Shadowed by an identical guard of the same tier.
    let status = finding_block(&stdout, "Unreachable clause in status/1: ");
    assert!(
        status.contains("clause_reachability.runa:5 never fires"),
        "{status}"
    );
    assert!(status.contains("-> \"god\" under x > 0"), "{status}");
    assert!(
        status.contains("clause_reachability.runa:4 (guarded default)"),
        "{status}"
    );

    // Shadowed by a broader earlier guard.
    let band = finding_block(&stdout, "Unreachable clause in band/1: ");
    assert!(
        band.contains("clause_reachability.runa:9 never fires"),
        "{band}"
    );
    assert!(
        band.contains("clause_reachability.runa:8 (guarded default)"),
        "{band}"
    );

    // A guard that cannot hold on its own.
    let never = finding_block(&stdout, "Unreachable clause in never/1: ");
    assert!(
        never.contains("clause_reachability.runa:16 never fires"),
        "{never}"
    );
    assert!(never.contains("Its guard is unsatisfiable"), "{never}");

    // The ordinary clause below two guards that together cover every Int.
    let rate = finding_block(&stdout, "Unreachable clause in rate/1: ");
    assert!(
        rate.contains("clause_reachability.runa:11 never fires"),
        "{rate}"
    );
    assert!(
        rate.contains("clause_reachability.runa:12 (guarded default)"),
        "{rate}"
    );
    assert!(
        rate.contains("clause_reachability.runa:13 (guarded default)"),
        "{rate}"
    );

    // RuleScope fields and a call to another pure rule in the guard.
    let payout = finding_block(&stdout, "Unreachable clause in Claim.payout/0: ");
    assert!(
        payout.contains("clause_reachability.runa:25 never fires"),
        "{payout}"
    );
    assert!(
        payout.contains("clause_reachability.runa:24 (guarded default)"),
        "{payout}"
    );
}

#[test]
fn clause_audit_reports_overlapping_guards_with_a_witness() {
    let fixture = topology_fixture("clause_reachability.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    let overlap = finding_block(&stdout, "Overlapping guards in rate/1: ");
    assert!(
        overlap.contains("clause_reachability.runa:12 and ")
            && overlap.contains("clause_reachability.runa:13\n"),
        "{overlap}"
    );
    assert!(
        overlap.contains("source order selects ")
            && overlap.contains("clause_reachability.runa:12."),
        "{overlap}"
    );
    let witness = overlap
        .lines()
        .find_map(|line| line.trim().strip_prefix("Witness: x = "))
        .unwrap_or_else(|| panic!("missing witness:\n{overlap}"));
    let witness: i64 = witness.parse().expect("integer witness");
    assert!((11..=19).contains(&witness), "{overlap}");
    // The shadowed `status` clauses also overlap with a different result,
    // but an unreachable clause is reported only as unreachable.
    assert!(
        !stdout.contains("Overlapping guards in status/1"),
        "{stdout}"
    );
}

#[test]
fn clause_audit_accepts_disjoint_guards_and_specific_before_general_clauses() {
    let fixture = topology_fixture("clause_reachability_disjoint.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("0 gap, 0 unreachable clause, 0 overlapping guards"),
        "{stdout}"
    );
    assert!(
        stdout.contains("7 rule families, 17 clauses: 17 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(
        stdout.contains("No structural findings discovered."),
        "{stdout}"
    );
}

#[test]
fn clause_audit_reports_untranslatable_guards_as_not_analyzed() {
    let fixture = topology_fixture("clause_reachability_untranslatable.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("2 rule families, 5 clauses: 4 analyzed, 1 not analyzed"),
        "{stdout}"
    );
    let not_analyzed = finding_block(&stdout, "Not analyzed (reachability unknown):");
    assert!(
        not_analyzed.contains("clause_reachability_untranslatable.runa:4 in advisory/1")
            && not_analyzed.contains("`Float`"),
        "{not_analyzed}"
    );
    assert!(
        !stdout.contains("Unreachable clause in advisory/1"),
        "{stdout}"
    );
    // The rest of the source is still analyzed.
    let level = finding_block(&stdout, "Unreachable clause in level/1: ");
    assert!(
        level.contains("clause_reachability_untranslatable.runa:8 never fires"),
        "{level}"
    );
}

#[test]
fn clause_audit_json_carries_findings_across_imports() {
    let fixture = topology_fixture("clause_reachability.calculate.runa");
    let output = run(&["audit", fixture.to_str().expect("fixture path"), "--json"]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = parse_stdout(&output);
    assert_eq!(report["schema"], "futuruna.audit.reachability.v1");
    let clauses = &report["clause_reachability"];
    if !z3_available() {
        assert_eq!(clauses["status"], "not_run", "{clauses}");
        return;
    }
    assert_eq!(clauses["status"], "ran", "{clauses}");
    assert_eq!(clauses["summary"]["unreachable_clauses"], 2, "{clauses}");
    assert_eq!(clauses["summary"]["overlapping_guards"], 1, "{clauses}");
    assert_eq!(clauses["summary"]["not_analyzed"], 0, "{clauses}");
    let findings = clauses["findings"].as_array().expect("findings");
    let located = |location: &Value, file: &str, line: u64| {
        location["file"]
            .as_str()
            .is_some_and(|path| path.ends_with(file))
            && location["line"] == line
    };

    let surcharge = findings
        .iter()
        .find(|finding| finding["family"] == "surcharge/1")
        .expect("surcharge finding");
    assert_eq!(surcharge["kind"], "unreachable_clause");
    assert_eq!(surcharge["cause"], "shadowed");
    assert_eq!(surcharge["clause"]["tier"], "guarded_default");
    assert!(
        located(
            &surcharge["clause"],
            "clause_reachability.calculate.runa",
            4
        ),
        "{surcharge}"
    );
    let shadows = surcharge["shadowed_by"].as_array().expect("shadowed_by");
    assert_eq!(shadows.len(), 1, "{surcharge}");
    assert!(
        located(&shadows[0], "clause-reachability-base.runa", 4),
        "{surcharge}"
    );

    let overlap = findings
        .iter()
        .find(|finding| finding["kind"] == "overlapping_guards")
        .expect("overlap finding");
    assert_eq!(overlap["family"], "rate/1");
    assert!(
        located(&overlap["clause"], "clause_reachability.calculate.runa", 8),
        "{overlap}"
    );
    assert!(
        located(
            &overlap["overlaps"],
            "clause_reachability.calculate.runa",
            7
        ),
        "{overlap}"
    );
    let witness: i64 = overlap["witness"]["x"]
        .as_str()
        .expect("witness value")
        .parse()
        .expect("integer witness");
    assert!((11..=19).contains(&witness), "{overlap}");
}

#[test]
fn clause_audit_says_it_did_not_run_without_z3() {
    let fixture = topology_fixture("clause_reachability.runa");
    let empty_path = std::env::temp_dir().join("runa-audit-no-z3");
    std::fs::create_dir_all(&empty_path).expect("create empty PATH directory");
    let output = Command::new(runa())
        .args(["audit", fixture.to_str().expect("fixture path")])
        .env("PATH", &empty_path)
        .output()
        .expect("run runa audit");
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
    assert!(
        stdout.contains("0 gap, 0 unreachable clause, 0 overlapping guards"),
        "{stdout}"
    );
    assert!(!stdout.contains("Unreachable clause in"), "{stdout}");
}
