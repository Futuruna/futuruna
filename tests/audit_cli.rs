use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audit/reachability.entry.runa")
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("source has multiple entries"));
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
    assert!(stderr.contains("unknown entry `missing_calculation`"));
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
        stdout.contains(
            "0 gap, 2 unreachable clause, 3 shadowed with a different result, 1 overlapping guards"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("6 rule families, 16 clauses: 16 analyzed, 0 not analyzed"),
        "{stdout}"
    );

    // Shadowed by an identical guard of the same tier with another result.
    let status = finding_block(&stdout, "Shadowed with a different result in status/1: ");
    assert!(
        status.contains("clause_reachability.runa:5 never fires"),
        "{status}"
    );
    assert!(status.contains("-> \"god\" under x > 0"), "{status}");
    assert!(
        status.contains("clause_reachability.runa:4 gives \"immortal\"; ")
            && status.contains("clause_reachability.runa:5 would give \"god\""),
        "{status}"
    );
    assert!(
        status.contains("clause_reachability.runa:4 (guarded default)"),
        "{status}"
    );

    // Shadowed by a broader earlier guard with another result.
    let band = finding_block(&stdout, "Shadowed with a different result in band/1: ");
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

    // The ordinary clause below two guards that together cover every Int:
    // a higher tier takes its inputs, so it is only unreachable.
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
    let payout = finding_block(
        &stdout,
        "Shadowed with a different result in Claim.payout/0: ",
    );
    assert!(
        payout.contains("clause_reachability.runa:25 never fires"),
        "{payout}"
    );
    assert!(
        payout.contains("clause_reachability.runa:24 (guarded default)"),
        "{payout}"
    );
    assert!(payout.contains("Witness: self.amount = "), "{payout}");
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
    // The shadowed `status` clause also overlaps with a different result,
    // but an unreachable clause is reported as shadowed, not as an overlap.
    assert!(
        !stdout.contains("Overlapping guards in status/1"),
        "{stdout}"
    );
}

#[test]
fn clause_audit_separates_shadowing_with_a_different_result_from_redundancy() {
    let fixture = topology_fixture("clause_shadowed_results.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains(
            "Findings: 1 unreachable clause, 1 shadowed with a different result, 0 overlapping guards, 0 gap"
        ),
        "{stdout}"
    );

    // Line 6 is taken by line 5, which decides those inputs differently.
    let band = finding_block(&stdout, "Shadowed with a different result in band/1: ");
    assert!(
        band.contains("clause_shadowed_results.runa:6 never fires"),
        "{band}"
    );
    assert!(
        band.contains("clause_shadowed_results.runa:5 (guarded default)"),
        "{band}"
    );
    assert!(
        band.contains("source order selects ") && band.contains("clause_shadowed_results.runa:5."),
        "{band}"
    );
    let witness: i64 = band
        .lines()
        .find_map(|line| line.trim().strip_prefix("Witness: x = "))
        .unwrap_or_else(|| panic!("missing witness:\n{band}"))
        .parse()
        .expect("integer witness");
    assert!(witness > 8, "{band}");
    assert!(
        band.contains("clause_shadowed_results.runa:5 gives 70; ")
            && band.contains("clause_shadowed_results.runa:6 would give 80"),
        "{band}"
    );
    assert!(!stdout.contains("Unreachable clause in band/1"), "{stdout}");

    // Line 10 is taken by line 9 with the same result: only redundant.
    let tier = finding_block(&stdout, "Unreachable clause in tier/1: ");
    assert!(
        tier.contains("clause_shadowed_results.runa:10 never fires"),
        "{tier}"
    );
    assert!(
        tier.contains("clause_shadowed_results.runa:9 (guarded default)"),
        "{tier}"
    );
    assert!(!tier.contains("Witness"), "{tier}");
    assert!(
        !stdout.contains("Shadowed with a different result in tier/1"),
        "{stdout}"
    );

    let output = run(&["audit", fixture.to_str().expect("fixture path"), "--json"]);
    assert!(output.status.success());
    let clauses = &parse_stdout(&output)["clause_reachability"];
    assert_eq!(clauses["summary"]["unreachable_clauses"], 1, "{clauses}");
    assert_eq!(
        clauses["summary"]["shadowed_with_different_result"], 1,
        "{clauses}"
    );
    let findings = clauses["findings"].as_array().expect("findings");
    let band = findings
        .iter()
        .find(|finding| finding["family"] == "band/1")
        .expect("band finding");
    assert_eq!(band["kind"], "shadowed_with_different_result", "{band}");
    assert_eq!(band["clause"]["line"], 6, "{band}");
    assert_eq!(band["selected"]["line"], 5, "{band}");
    assert_eq!(band["shadowed_by"][0]["line"], 5, "{band}");
    assert_eq!(band["results"]["selected"], "70", "{band}");
    assert_eq!(band["results"]["clause"], "80", "{band}");
    let witness: i64 = band["witness"]["x"]
        .as_str()
        .expect("witness value")
        .parse()
        .expect("integer witness");
    assert!(witness > 8, "{band}");
    let tier = findings
        .iter()
        .find(|finding| finding["family"] == "tier/1")
        .expect("tier finding");
    assert_eq!(tier["kind"], "unreachable_clause", "{tier}");
    assert_eq!(tier["cause"], "shadowed", "{tier}");
    assert_eq!(tier["clause"]["line"], 10, "{tier}");
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
        stdout.contains(
            "0 gap, 0 unreachable clause, 0 shadowed with a different result, 0 overlapping guards"
        ),
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
    // The rest of the source is still analyzed: line 8 is taken by line 7,
    // which gives another result.
    let level = finding_block(&stdout, "Shadowed with a different result in level/1: ");
    assert!(
        level.contains("clause_reachability_untranslatable.runa:8 never fires"),
        "{level}"
    );
}

#[test]
fn clause_audit_json_carries_findings_across_imports() {
    let fixture = topology_fixture("clause_reachability.entry.runa");
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
    assert_eq!(clauses["summary"]["unreachable_clauses"], 1, "{clauses}");
    assert_eq!(
        clauses["summary"]["shadowed_with_different_result"], 1,
        "{clauses}"
    );
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
    // The imported clause selects 5 where the local one would give 7.
    assert_eq!(surcharge["kind"], "shadowed_with_different_result");
    assert!(
        located(&surcharge["selected"], "clause-reachability-base.runa", 4),
        "{surcharge}"
    );
    assert_eq!(surcharge["results"]["selected"], "5", "{surcharge}");
    assert_eq!(surcharge["results"]["clause"], "7", "{surcharge}");
    assert_eq!(surcharge["clause"]["tier"], "guarded_default");
    assert!(
        located(&surcharge["clause"], "clause_reachability.entry.runa", 4),
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
        located(&overlap["clause"], "clause_reachability.entry.runa", 8),
        "{overlap}"
    );
    assert!(
        located(&overlap["overlaps"], "clause_reachability.entry.runa", 7),
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
        stdout.contains(
            "0 gap, 0 unreachable clause, 0 shadowed with a different result, 0 overlapping guards"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("Unreachable clause in"), "{stdout}");
}

#[test]
fn clause_audit_reports_gaps_with_a_witness() {
    let fixture = topology_fixture("clause_gaps.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("0 overlapping guards, 3 clause gap"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "Findings: 0 unreachable clause, 0 shadowed with a different result, 0 overlapping guards, 3 gap"
        ),
        "{stdout}"
    );

    // `> 0` and `< 0` leave exactly 0.
    let sign = finding_block(&stdout, "Gap in sign/1: ");
    assert!(
        sign.contains("clause_gaps.runa:4 (guarded default)")
            && sign.contains("clause_gaps.runa:5 (guarded default)"),
        "{sign}"
    );
    assert!(sign.contains("Witness: x = 0\n"), "{sign}");
    assert!(sign.contains("no value rule matched"), "{sign}");

    // A RuleScope witness names each field.
    let tax = finding_block(&stdout, "Gap in Share.tax/0: ");
    assert!(
        tax.contains("clause_gaps.runa:10 (exception)")
            && tax.contains("clause_gaps.runa:9 (guarded default)"),
        "{tax}"
    );
    let witness = tax
        .lines()
        .find_map(|line| line.trim().strip_prefix("Witness: "))
        .unwrap_or_else(|| panic!("missing witness:\n{tax}"));
    assert!(witness.starts_with("self.own = 0, self.spouse = "), "{tax}");

    // A variant without a clause.
    let allowance = finding_block(&stdout, "Gap in allowance/1: ");
    assert!(allowance.contains("= Divorced\n"), "{allowance}");
}

#[test]
fn clause_audit_reports_no_gap_for_covered_families() {
    let fixture = topology_fixture("clause_gaps_covered.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    // An unguarded default, guards that cover every Int, an exception over
    // covering clauses, an exception that covers the rest, every variant,
    // and Boolean rules (false on a miss).
    assert!(
        stdout.contains("7 rule families, 13 clauses: 13 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "Findings: 0 unreachable clause, 0 shadowed with a different result, 0 overlapping guards, 0 gap"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("Gap in "), "{stdout}");
    assert!(!stdout.contains("Gaps not checked"), "{stdout}");
    assert!(
        stdout.contains("No structural findings discovered."),
        "{stdout}"
    );
}

#[test]
fn clause_audit_does_not_check_gaps_past_an_untranslatable_guard() {
    let fixture = topology_fixture("clause_gaps_untranslatable.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("1 rule family, 2 clauses: 1 analyzed, 1 not analyzed"),
        "{stdout}"
    );
    assert!(!stdout.contains("Gap in mixed/2"), "{stdout}");
    let not_checked = finding_block(&stdout, "Gaps not checked (coverage unknown):");
    assert!(
        not_checked.contains("mixed/2: the guard of ")
            && not_checked.contains("clause_gaps_untranslatable.runa:6 is not translatable"),
        "{not_checked}"
    );
}

#[test]
fn clause_audit_json_reports_gaps() {
    let fixture = topology_fixture("clause_gaps.entry.runa");
    let output = run(&["audit", fixture.to_str().expect("fixture path"), "--json"]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = parse_stdout(&output);
    let clauses = &report["clause_reachability"];
    if !z3_available() {
        assert_eq!(clauses["status"], "not_run", "{clauses}");
        return;
    }
    assert_eq!(clauses["summary"]["gaps"], 1, "{clauses}");
    assert_eq!(clauses["summary"]["gap_not_checked"], 0, "{clauses}");
    let findings = clauses["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1, "{clauses}");
    let gap = &findings[0];
    assert_eq!(gap["kind"], "gap");
    assert_eq!(gap["family"], "ShareTax.tax_ore/0");
    assert_eq!(gap["witness"], serde_json::json!({ "self.own_ore": "0" }));
    let locations = gap["clauses"].as_array().expect("clauses");
    assert_eq!(locations.len(), 2, "{gap}");
    assert_eq!(locations[0]["tier"], "exception", "{gap}");
    assert_eq!(locations[0]["line"], 5, "{gap}");
    assert_eq!(locations[1]["tier"], "guarded_default", "{gap}");
    assert_eq!(locations[1]["line"], 4, "{gap}");
}

#[test]
fn audit_json_works_for_a_file_without_a_calculation() {
    let fixture = topology_fixture("clause_gaps.runa");
    let output = run(&["audit", fixture.to_str().expect("fixture path"), "--json"]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = parse_stdout(&output);
    assert_eq!(report["schema"], "futuruna.audit.reachability.v1");
    // No calculation: the entry sections are null, the rest is present.
    for key in ["entry", "callables", "summary", "loaded_sources"] {
        assert!(report[key].is_null(), "{key}: {report}");
    }
    assert!(report["topology"]["summary"]["rules"].is_u64(), "{report}");
    assert!(report["topology"]["findings"].is_array(), "{report}");
    let clauses = &report["clause_reachability"];
    if !z3_available() {
        assert_eq!(clauses["status"], "not_run", "{clauses}");
        return;
    }
    assert_eq!(clauses["scope"]["kind"], "declared_in_file", "{clauses}");
    assert_eq!(clauses["summary"]["families"], 3, "{clauses}");
    assert_eq!(clauses["summary"]["clauses"], 6, "{clauses}");
    assert_eq!(clauses["summary"]["gaps"], 3, "{clauses}");
    for finding in clauses["findings"].as_array().expect("findings") {
        assert!(
            finding["file"]
                .as_str()
                .is_some_and(|file| file.ends_with("clause_gaps.runa")),
            "{finding}"
        );
    }
}

#[test]
fn calculation_audit_covers_the_families_it_reaches_through_imports() {
    let fixture = topology_fixture("clause_gaps_import.entry.runa");
    let output = run(&["audit", fixture.to_str().expect("fixture path"), "--json"]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = parse_stdout(&output);
    let clauses = &report["clause_reachability"];
    if !z3_available() {
        assert_eq!(clauses["status"], "not_run", "{clauses}");
        return;
    }
    assert_eq!(clauses["scope"]["kind"], "reached_from_entry", "{clauses}");
    assert_eq!(clauses["scope"]["entry"], "share_tax", "{clauses}");
    // The entry rule and the imported `ShareTax.tax_ore`; the imported but
    // unreached `unused_sign` is outside the scope.
    assert_eq!(clauses["summary"]["families"], 2, "{clauses}");
    assert_eq!(clauses["summary"]["files"], 2, "{clauses}");
    let families = clauses["families"].as_array().expect("families");
    let tax = families
        .iter()
        .find(|family| family["family"] == "ShareTax.tax_ore/0")
        .expect("imported family in scope");
    assert!(
        tax["files"][0]
            .as_str()
            .is_some_and(|file| file.ends_with("clause-gaps-module.runa")),
        "{tax}"
    );
    assert!(
        !families
            .iter()
            .any(|family| family["family"] == "unused_sign/1"),
        "{clauses}"
    );

    let findings = clauses["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1, "{clauses}");
    let gap = &findings[0];
    assert_eq!(gap["kind"], "gap");
    assert_eq!(gap["family"], "ShareTax.tax_ore/0");
    assert!(
        gap["file"]
            .as_str()
            .is_some_and(|file| file.ends_with("clause-gaps-module.runa")),
        "{gap}"
    );
    assert_eq!(gap["witness"], serde_json::json!({ "self.own_ore": "0" }));

    // The text report states the same scope and finds the same gap.
    let stdout = audit_stdout(&fixture);
    assert!(
        stdout.contains(
            "Scope: rule families calculation `share_tax` reaches, imports included (2 files)"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("Gap in ShareTax.tax_ore/0: "), "{stdout}");
    assert!(!stdout.contains("unused_sign"), "{stdout}");
}

#[test]
fn clause_audit_translates_not_string_length_option_results_and_arithmetic() {
    let fixture = topology_fixture("clause_reachability_widened.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("6 rule families, 14 clauses: 14 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "1 unreachable clause, 2 shadowed with a different result, 1 overlapping guards, 0 gap"
        ),
        "{stdout}"
    );
    // `not` in a guard.
    let held = finding_block(&stdout, "Shadowed with a different result in held/2: ");
    assert!(
        held.contains("clause_reachability_widened.runa:4 never fires"),
        "{held}"
    );
    // `string_length` counts scalar values: the "å" clause stays reachable.
    assert!(
        !stdout.contains("Unreachable clause in label/1"),
        "{stdout}"
    );
    // Option results are compared, with a witness.
    let overlap = finding_block(&stdout, "Overlapping guards in parameters/1: ");
    assert!(overlap.contains("Witness: year = 2025"), "{overlap}");
    // Arithmetic in a called RuleScope rule.
    let rate = finding_block(&stdout, "Shadowed with a different result in Case.rate/0: ");
    assert!(
        rate.contains("clause_reachability_widened.runa:26 never fires"),
        "{rate}"
    );
    // An irrefutable predicate head answers False: no gap.
    assert!(!stdout.contains("Gap in in_bounds/1"), "{stdout}");
}

/// The clause audit re-checks the program it assembled for the solver, where
/// a shared constructor carries its internal name (`AskEvent`'s `Trade`).
/// The key declarations on its fields name the constructor as written; the
/// solver's check reads no calculation contract, so they do not stop the
/// audit ("does not type-check: `AskEvent` has no such field").
#[test]
fn clause_audit_runs_with_keys_on_shared_constructors() {
    let dir = std::env::temp_dir().join(format!("runa-audit-keyed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let path = dir.join("keyed.entry.runa");
    std::fs::write(
        &path,
        "# Event = Trade(@ field(label = \"Id\", key = True) id: String, amount: Int) | Fee(@ field(label = \"Id\", key = True) id: String)\n\
         # AskEvent = Trade(@ field(label = \"Id\", key = True) id: String, amount: Int) | Fee(@ field(label = \"Id\", key = True) id: String)\n\
         | sign(amount: Int) -> 1 under amount > 0\n\
         | sign(amount) -> 0 under amount <= 0\n\
         # Input(events: List(AskEvent), amount: Int)\n\
         @ entry(\"Keyed shared constructors\")\n\
         > count(input: Input) -> Int { length(input.events) + sign(input.amount) }\n",
    )
    .expect("write fixture");
    let output = run(&["audit", path.to_str().expect("fixture path"), "--json"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let clauses = &parse_stdout(&output)["clause_reachability"];
    if !z3_available() {
        assert_eq!(clauses["status"], "not_run", "{clauses}");
        return;
    }
    assert_eq!(clauses["status"], "ran", "{clauses}");
    assert_eq!(clauses["summary"]["clauses"], 2, "{clauses}");
    assert_eq!(clauses["summary"]["analyzed"], 2, "{clauses}");
}

/// Dates are ordered chronologically in the SMT encoding as at run time.
#[test]
fn clause_audit_orders_dates() {
    let fixture = topology_fixture("clause_dates.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("3 rule families, 8 clauses: 8 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "2 unreachable clause, 0 shadowed with a different result, 1 overlapping guards, 0 gap"
        ),
        "{stdout}"
    );
    // `<=` and `>=` meet on equal dates and together cover every pair, so
    // the default below them never fires.
    let overlap = finding_block(&stdout, "Overlapping guards in band/2: ");
    assert!(overlap.contains("Witness: on = "), "{overlap}");
    let band = finding_block(&stdout, "Unreachable clause in band/2: ");
    assert!(band.contains("clause_dates.runa:9 never fires"), "{band}");
    let never = finding_block(&stdout, "Unreachable clause in never/1: ");
    assert!(
        never.contains("clause_dates.runa:15 never fires"),
        "{never}"
    );
    // Before, on and after the deadline: each clause of `filed` fires.
    assert!(!stdout.contains("in filed/2"), "{stdout}");
}

/// An Option argument is analysed: `is_some`, `is_none`, `unwrap_or` and a
/// match on `Some`/`None` read its presence and value at its own type.
#[test]
fn clause_audit_analyses_option_guards() {
    let fixture = topology_fixture("clause_options.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("5 rule families, 12 clauses: 12 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "0 unreachable clause, 1 shadowed with a different result, 1 overlapping guards, 1 gap"
        ),
        "{stdout}"
    );
    // `is_some` takes every closed account first.
    let levy = finding_block(&stdout, "Shadowed with a different result in levy_days/2: ");
    assert!(
        levy.contains("clause_options.runa:11 never fires"),
        "{levy}"
    );
    let overlap = finding_block(&stdout, "Overlapping guards in rate/2: ");
    assert!(overlap.contains("closed = (Some (Date "), "{overlap}");
    let gap = finding_block(&stdout, "Gap in draw_by/2: ");
    assert!(gap.contains("closed = (Some (Date "), "{gap}");
    assert!(!stdout.contains("in period_end/2"), "{stdout}");
    assert!(!stdout.contains("in status/1"), "{stdout}");
}

/// A match on a tuple of two Options is analysed element by element.
#[test]
fn clause_audit_analyses_tuple_matches() {
    let fixture = topology_fixture("clause_tuples.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("2 rule families, 6 clauses: 6 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "0 unreachable clause, 1 shadowed with a different result, 0 overlapping guards, 1 gap"
        ),
        "{stdout}"
    );
    // `is_none(lost_right)` takes every pair `(None, Some(_))` first.
    let levy = finding_block(&stdout, "Shadowed with a different result in levy/2: ");
    assert!(levy.contains("clause_tuples.runa:10 never fires"), "{levy}");
    // A right lost after the account closed: both are present.
    let gap = finding_block(&stdout, "Gap in levy/2: ");
    assert!(
        gap.contains("lost_right = (Some (Date ") && gap.contains("closed = (Some (Date "),
        "{gap}"
    );
    assert!(!stdout.contains("in state/2"), "{stdout}");
}

/// The rule that decides each running state of a `scan` takes a scalar and is
/// analysed; a guard that scans a list itself is not, and says why.
#[test]
fn clause_audit_analyses_rules_on_scanned_states() {
    let fixture = topology_fixture("clause_scan.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("3 rule families, 3 clauses: 2 analyzed, 1 not analyzed"),
        "{stdout}"
    );
    let not_analyzed = finding_block(&stdout, "Not analyzed (reachability unknown):");
    assert!(
        not_analyzed.contains("clause_scan.runa:8 in ever_short/2: guard not translatable: parameter `bookings` has type `List(Int)`, which the SMT encoding does not represent"),
        "{not_analyzed}"
    );
    assert!(!not_analyzed.contains("overdrawn/1"), "{not_analyzed}");
}

/// A family over two Option sorts: `None` in a head and as a result is
/// unambiguous to the solver, so every clause is analysed.
#[test]
fn clause_audit_analyses_families_mixing_option_sorts() {
    let fixture = topology_fixture("clause_mixed_options.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("1 rule family, 3 clauses: 3 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(!stdout.contains("ambiguous"), "{stdout}");
}

/// `None` compared with (`x == None`, `x != None`) or inside a value (a
/// record field) in families over two Option sorts is unambiguous to the
/// solver, so every clause is analysed.
#[test]
fn clause_audit_analyses_none_in_values_and_comparisons() {
    let fixture = topology_fixture("clause_none_in_values.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("2 rule families, 6 clauses: 6 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(!stdout.contains("ambiguous"), "{stdout}");
}

/// A field two constructors give different sorts (`purpose: DepositPurpose?`
/// and `purpose: WithdrawalPurpose?`) has no SMT reader by name; a head
/// pattern reads it by its constructor, so every clause is analysed (before,
/// Z3 rejected the query: unknown constant `runa_field_purpose`). The family
/// is a Boolean one, so its `-> False` default only marks the head and is
/// not a clause the audit counts.
#[test]
fn clause_audit_reads_head_pattern_fields_by_constructor() {
    let fixture = topology_fixture("clause_field_sorts_by_constructor.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("1 rule family, 2 clauses: 2 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    assert!(!stdout.contains("unknown constant"), "{stdout}");
}

/// A family whose clause heads match every variant has no gap.
#[test]
fn clause_audit_reads_constructor_heads_as_total() {
    let fixture = topology_fixture("clause_constructors.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(!stdout.contains("in due/2"), "{stdout}");
}

/// Guards over a set of a payload-free enum are analysed over every subset,
/// and a witness set is shown as `show` prints it.
#[test]
fn clause_audit_analyses_guards_over_enum_sets() {
    let fixture = topology_fixture("clause_sets.runa");
    let stdout = audit_stdout(&fixture);
    if !z3_available() {
        assert!(stdout.contains("Not run: Z3 not found"), "{stdout}");
        return;
    }
    assert!(
        stdout.contains("1 rule family, 4 clauses: 4 analyzed, 0 not analyzed"),
        "{stdout}"
    );
    // Both plaintiff factors and D1: the first two guards hold.
    let overlap = finding_block(&stdout, "Overlapping guards in decide/1: ");
    let witness = overlap
        .lines()
        .find_map(|line| line.trim().strip_prefix("Witness: facts = {"))
        .and_then(|members| members.strip_suffix('}'))
        .unwrap_or_else(|| panic!("no set witness:\n{overlap}"));
    let members: Vec<&str> = witness.split(", ").collect();
    assert!(
        ["D1", "P1", "P2"]
            .iter()
            .all(|member| members.contains(member)),
        "{overlap}"
    );
    // Four factors make no set of five.
    let unreachable = finding_block(&stdout, "Unreachable clause in decide/1: ");
    assert!(
        unreachable.contains("clause_sets.runa:9 never fires"),
        "{unreachable}"
    );
}
