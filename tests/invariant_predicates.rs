use futuruna::eval_source_with_prelude;

#[test]
fn known_non_boolean_invariants_fail_before_any_runtime_check() {
    for predicate in [
        "limit + 1",
        "\"yes\"",
        "[1, 2]",
        "[]",
        "(1, 2)",
        "()",
        "next(limit)",
    ] {
        let source = format!(
            "> next(value: Int) -> Int {{ value + 1 }}\n= limit = 10\n| invalid: limit -> {predicate}\n@ print(\"initialized\")\n"
        );
        // There is deliberately no `?`: rejection must happen at declaration,
        // not merely when the interpreter happens to evaluate the predicate.
        let error = eval_source_with_prelude(&source, false)
            .expect_err("frontend must reject a known non-Boolean invariant");
        assert!(error.starts_with("3:"), "{predicate}: {error}");
        assert!(
            error.contains("invariant `invalid` predicate must return Bool"),
            "{error}"
        );
    }
}

#[test]
fn boolean_invariants_retain_forward_checks_and_failure_branches() {
    let source = r#"
> positive(value: Int) -> Bool { value > 0 }
= limit = 10
= balance = -5
? limit_ok -> { @ print("limit OK") } else { @ print("limit VIOLATION") }
| limit_ok: limit -> positive(limit)
| balance_ok: balance -> balance >= 0
? balance_ok -> { @ print("balance OK") } else { @ print("balance VIOLATION") }
? all -> { @ print("ALL OK") } else { @ print("SOME FAILED") }
"#;
    let output = eval_source_with_prelude(source, false).expect("valid Boolean invariants");
    assert_eq!(output.trim(), "limit OK\nbalance VIOLATION\nSOME FAILED");
}
