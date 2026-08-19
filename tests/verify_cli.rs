use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify/rule_dispatch_consumer.runa")
}

fn parity_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verify/rule_dispatch_parity.runa")
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
