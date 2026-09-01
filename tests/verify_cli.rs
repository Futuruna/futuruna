use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify/rule_dispatch_consumer.runa")
}

fn parity_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify/rule_dispatch_parity.runa")
}

fn qualified_namespace_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/verify/qualified_namespace_parity.runa")
}

fn qualified_namespace_private_fail_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/verify/qualified_namespace_private_fail.runa")
}

fn qualified_namespace_shape_fail_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/verify/qualified_namespace_shape_fail.runa")
}

fn qualified_namespace_metadata_isolation_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/verify/qualified_namespace_metadata_isolation.runa")
}

fn run_runa(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(arguments)
        .output()
        .expect("run Futuruna CLI")
}

#[test]
fn finite_rule_dispatch_domain_matches_interpreter_generated_rust_and_smt() {
    let fixture = parity_fixture();
    let fixture = fixture.to_str().expect("UTF-8 fixture path");
    let interpreted = run_runa(&[fixture]);
    let compiled = run_runa(&["run", fixture]);
    let verified = run_runa(&["verify", fixture]);

    for (mode, output) in [("interpreter", &interpreted), ("compiled", &compiled)] {
        assert!(
            output.status.success(),
            "{mode} stdout:\n{}\n{mode} stderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let interpreted_stdout = String::from_utf8_lossy(&interpreted.stdout);
    let compiled_stdout = String::from_utf8_lossy(&compiled.stdout);
    let expected = "10\n70\n80\n0\n70\n80\n0\n70\n70\n99\ntrue\nfalse\ntrue\nfalse";
    assert_eq!(interpreted_stdout.trim(), expected);
    assert_eq!(compiled_stdout.trim(), expected);

    assert!(
        verified.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&verified.stdout),
        String::from_utf8_lossy(&verified.stderr)
    );
    let verified_stdout = String::from_utf8_lossy(&verified.stdout);
    if Command::new("z3").arg("--version").output().is_ok() {
        for invariant in [
            "parity_imported_default",
            "parity_imported_guard",
            "parity_imported_exception",
            "parity_global_default",
            "parity_global_source_order",
            "parity_global_reversed_source_order",
            "parity_scoped_default",
            "parity_scoped_guard",
            "parity_scoped_source_order",
            "parity_scoped_exception",
            "parity_conditional_boolean_hit",
            "parity_conditional_boolean_miss",
            "parity_exception_boolean_hit",
            "parity_exception_boolean_miss",
        ] {
            assert!(
                verified_stdout.contains(&format!("PROVED: |{invariant}| holds for all values")),
                "missing finite-domain proof for {invariant}:\n{verified_stdout}"
            );
        }
    } else {
        assert!(
            verified_stdout.contains("Z3 not found"),
            "missing explicit solver availability diagnostic:\n{verified_stdout}"
        );
    }
}

#[test]
fn qualified_namespace_calls_match_interpreter_and_smt() {
    let fixture = qualified_namespace_fixture();
    let fixture = fixture.to_str().expect("UTF-8 fixture path");
    let interpreted = run_runa(&[fixture]);
    let verified = run_runa(&["verify", fixture]);

    assert!(
        interpreted.status.success(),
        "interpreter stdout:\n{}\ninterpreter stderr:\n{}",
        String::from_utf8_lossy(&interpreted.stdout),
        String::from_utf8_lossy(&interpreted.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&interpreted.stdout).trim(),
        "2\n101\n101\n101\n11\n101\n23\n203\n101\n203"
    );

    assert!(
        verified.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&verified.stdout),
        String::from_utf8_lossy(&verified.stderr)
    );
    let verified_stdout = String::from_utf8_lossy(&verified.stdout);
    assert!(
        !verified_stdout.contains("higher-order function calls are not translatable to SMT")
            && !verified_stdout.contains("calls unsupported function"),
        "qualified calls did not lower directly:\n{verified_stdout}"
    );
    if Command::new("z3").arg("--version").output().is_ok() {
        for invariant in [
            "qualified_root_function_isolated",
            "qualified_module_function_isolated",
            "qualified_private_function_is_internal",
            "qualified_private_rule_is_internal",
            "qualified_root_rule_same_arity",
            "qualified_module_rule_same_arity",
            "qualified_root_rule_different_arity",
            "qualified_module_rule_different_arity",
            "qualified_alias_rule_same_arity",
            "qualified_alias_rule_different_arity",
            "qualified_root_module_families_are_distinct",
            "qualified_alias_families_have_distinct_symbols",
        ] {
            assert!(
                verified_stdout.contains(&format!("PROVED: |{invariant}| holds for all values")),
                "missing qualified namespace proof for {invariant}:\n{verified_stdout}"
            );
        }
        let alias_section = verified_stdout
            .split("--- | qualified_alias_families_have_distinct_symbols ---")
            .nth(1)
            .and_then(|section| section.split("--- |").next())
            .expect("qualified alias SMT section");
        assert_eq!(
            alias_section
                .lines()
                .filter(|line| line.trim_start().starts_with("(define-fun "))
                .count(),
            4,
            "both alias owners and both arities need distinct SMT definitions:\n{alias_section}"
        );
    } else {
        assert!(
            verified_stdout.contains("Z3 not found"),
            "missing explicit solver availability diagnostic:\n{verified_stdout}"
        );
    }
}

#[test]
fn verify_rejects_private_and_non_call_qualified_shapes() {
    let private_fixture = qualified_namespace_private_fail_fixture();
    let private = run_runa(&[
        "verify",
        private_fixture.to_str().expect("UTF-8 fixture path"),
    ]);
    assert!(!private.status.success());
    let private_stderr = String::from_utf8_lossy(&private.stderr);
    assert!(
        private_stderr.contains(
            "qualified import `Policy` has no exported member `hidden_step`; add `@ export`"
        ),
        "private qualified callable did not fail closed:\n{private_stderr}"
    );

    let shape_fixture = qualified_namespace_shape_fail_fixture();
    let shape = run_runa(&[
        "verify",
        shape_fixture.to_str().expect("UTF-8 fixture path"),
    ]);
    assert!(!shape.status.success());
    let shape_stderr = String::from_utf8_lossy(&shape.stderr);
    assert!(
        shape_stderr.contains(
            "qualified member `Policy.collide` is not a direct function or rule call; unsupported qualified shapes fail closed"
        ),
        "qualified function value did not fail closed:\n{shape_stderr}"
    );
}

#[test]
fn unused_qualified_nominal_metadata_does_not_perturb_root_smt() {
    let fixture = qualified_namespace_metadata_isolation_fixture();
    let verified = run_runa(&["verify", fixture.to_str().expect("UTF-8 fixture path")]);

    assert!(
        verified.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&verified.stdout),
        String::from_utf8_lossy(&verified.stderr)
    );
    let stdout = String::from_utf8_lossy(&verified.stdout);
    assert!(
        stdout.contains("(declare-datatype Collision (RootOnly))"),
        "root nominal metadata was not retained:\n{stdout}"
    );
    assert!(
        !stdout.contains("ModuleOnly"),
        "unused qualified nominal metadata leaked into root SMT:\n{stdout}"
    );
    if Command::new("z3").arg("--version").output().is_ok() {
        assert!(
            stdout.contains("PROVED: |unused_qualified_metadata_is_isolated| holds for all values"),
            "missing metadata-isolation proof:\n{stdout}"
        );
    }
}

#[test]
fn verify_lowers_scoped_and_imported_rule_dispatch_to_smt() {
    let fixture = fixture();
    let output = run_runa(&["verify", fixture.to_str().expect("UTF-8 fixture path")]);
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("higher-order function calls are not translatable to SMT"),
        "rule dispatch was not lowered:\n{stdout}"
    );
    assert!(
        !stdout.contains("calls unsupported function `imported_band`"),
        "imported rule chain was not indexed:\n{stdout}"
    );

    if Command::new("z3").arg("--version").output().is_ok() {
        for invariant in [
            "imported_base_is_retained",
            "imported_condition_is_retained",
            "local_exception_extends_imported_chain",
            "first_overlapping_guard_wins",
            "reversing_guards_changes_dispatch",
            "scoped_first_overlapping_guard_wins",
            "scoped_exception_precedes_conditions",
            "boolean_overload_is_well_sorted",
            "integer_overload_is_well_sorted",
            "imported_exception_is_symbolic",
            "first_guard_is_symbolic",
            "constructor_payload_head_is_lowered",
            "named_constructor_head_is_lowered",
            "literal_heads_are_lowered",
            "constructor_payload_is_symbolic",
            "sequential_rule_block_is_symbolic",
        ] {
            assert!(
                stdout.contains(&format!("PROVED: |{invariant}| holds for all values")),
                "missing proof for {invariant}:\n{stdout}"
            );
        }
        assert!(
            stdout.contains("COUNTEREXAMPLE found for |intentional_wrong_order_claim|"),
            "the intentionally false ordering claim should be refuted:\n{stdout}"
        );
        assert!(
            stdout.contains("COUNTEREXAMPLE found for |scoped_intentional_wrong_order_claim|"),
            "the directly scoped false ordering claim should be refuted:\n{stdout}"
        );
        assert!(
            stdout.contains(
                "partial rule dispatch for `partial_value` has no unconditional value for SMT"
            ),
            "partial rules must fail closed with a precise diagnostic:\n{stdout}"
        );
        assert!(
            stdout.contains(
                "recursive rule dispatch for `recursive_value` is not yet translatable to SMT"
            ),
            "recursive rules must fail closed with a precise diagnostic:\n{stdout}"
        );
        assert!(
            stdout.contains(
                "rule `projected` has a higher-order parameter that is not translatable to first-order SMT"
            ),
            "higher-order rule parameters must fail closed with a precise diagnostic:\n{stdout}"
        );
        assert!(
            stdout.contains(
                "rule `conflicting_result` with arity 1 has conflicting return types `Int` and `Bool`"
            ),
            "conflicting exact-arity returns must fail closed:\n{stdout}"
        );
        assert!(
            stdout.contains(
                "cannot infer every return clause of rule `unresolved_result` with arity 1"
            ),
            "unresolved exact-arity returns must fail closed:\n{stdout}"
        );
        assert!(
            stdout
                .lines()
                .filter(|line| line.contains("SMT fallback skipped"))
                .all(|line| !line.contains("runa_rule_")),
            "SMT diagnostics must not expose internal generated symbols:\n{stdout}"
        );
    } else {
        assert!(
            stdout.contains("Z3 not found"),
            "missing explicit solver availability diagnostic:\n{stdout}"
        );
    }
}
