use futuruna::{
    parse_prelude, prepend_prelude, Lexer, Parser, RuleDispatchKey, TypeCheckArtifacts, TypeChecker,
};

fn artifacts(source: &str) -> TypeCheckArtifacts {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    TypeChecker::check_with_backend_artifacts(
        &prepend_prelude(parse_prelude(), &statements),
        None,
        source,
    )
}

fn compare(fixture: &str, expected: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus")
        .join(fixture);
    for native in [false, true] {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_runa"));
        if native {
            command.arg("run");
        }
        let output = command.arg(&path).output().unwrap();
        assert!(
            output.status.success(),
            "{fixture} native={native}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "{fixture} native={native}"
        );
    }
}

#[test]
fn collection_values_and_reference_literals_compile_without_display_keys_or_synthetic_captures() {
    compare(
        "native_collection_references.runa",
        "[3, 1, 2]\n4\n2\n3\n2\n2\ntrue",
    );
}

#[test]
fn contextual_option_comparisons_and_map_rule_returns_keep_native_values() {
    compare("contextual_rule_value_types.runa", "true\nfalse\ntrue\n2");
}

#[test]
fn borrowed_function_parameters_pass_owned_values_to_rules() {
    compare("borrowed_arguments_to_rules.runa", "okay\naa\n3\nkept");
}

#[test]
fn runtime_map_types_do_not_manufacture_a_totality_proof() {
    let checked = artifacts(include_str!(
        "differential/corpus/contextual_rule_value_types.runa"
    ));
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let key = RuleDispatchKey {
        scope: Some("Scope".into()),
        name: "result".into(),
        arity: 0,
    };
    assert!(
        !checked
            .rule_dispatch_backend_return_issues
            .contains_key(&key),
        "{:?}",
        checked.rule_dispatch_backend_return_issues
    );
    assert_eq!(
        checked
            .rule_dispatch_backend_return_types
            .get(&key)
            .map(String::as_str),
        Some("Map(String, Int)")
    );
    assert!(!checked.rule_dispatch_total_value_keys.contains(&key));
}

#[test]
fn equality_context_still_rejects_incompatible_payloads_and_owners() {
    for expression in [
        "input.value == Some(\"bad\")",
        "Some(\"bad\") == input.value",
        "input.value == True",
    ] {
        let source =
            format!("# Input(value: Option(Int))\n| wrong(input: Input) -> {expression}\n");
        let checked = artifacts(&source);
        let key = RuleDispatchKey {
            scope: None,
            name: "wrong".into(),
            arity: 1,
        };
        assert!(
            checked
                .rule_dispatch_backend_return_issues
                .contains_key(&key),
            "{source}"
        );
    }
}

#[test]
fn guarded_partial_calls_keep_interpreter_native_parity() {
    compare("guarded_partial_rule_calls.runa", "0\n7\n3\n0\n9");
}

#[test]
fn disjunctions_of_complete_stable_guards_keep_native_values() {
    compare("disjunctive_guard_coverage.runa", "5\n6\n7\n2\n3\n-7");
}
