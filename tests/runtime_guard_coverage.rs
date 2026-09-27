use futuruna::{
    parse_prelude, prepend_prelude, Lexer, Parser, RuleDispatchKey, TypeCheckArtifacts, TypeChecker,
};

fn artifacts(source: &str) -> TypeCheckArtifacts {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("guard fixture parses");
    let statements = prepend_prelude(parse_prelude(), &statements);
    let artifacts = TypeChecker::check_with_backend_artifacts(&statements, None, source);
    assert!(
        artifacts.diagnostics.is_empty(),
        "{source}: {:?}",
        artifacts.diagnostics
    );
    artifacts
}

fn key(scope: Option<&str>, name: &str, arity: usize) -> RuleDispatchKey {
    RuleDispatchKey {
        scope: scope.map(str::to_owned),
        name: name.into(),
        arity,
    }
}

#[test]
fn complementary_guards_authorize_runtime_dispatch_without_a_value_proof() {
    let source = include_str!("differential/corpus/complementary_scoped_guards.runa");
    let checked = artifacts(source);
    for key in [
        key(Some("Assessment"), "amount", 0),
        key(Some("Assessment"), "selected", 0),
        key(None, "sign", 1),
        key(None, "chosen", 1),
    ] {
        assert!(
            checked
                .rule_dispatch_runtime_irrefutable_keys
                .contains(&key),
            "{key:?}"
        );
        assert!(
            !checked.rule_dispatch_total_value_keys.contains(&key),
            "guard coverage must not manufacture a total-value proof: {key:?}"
        );
    }
}

#[test]
fn stable_boolean_and_integer_complements_cover_both_outcomes() {
    for (field, left, right) in [
        ("Int", "input < 0", "input >= 0"),
        ("Int", "input <= 0", "input > 0"),
        ("Int", "input == 0", "input != 0"),
        ("Int", "0 > input", "input >= 0"),
        ("Bool", "input", "input == False"),
        ("Bool", "input == True", "!input"),
        ("Bool", "input != False", "input == False"),
    ] {
        let source = format!("# Branch(input: {field}) {{ | value() -> 1 under {left}\n| exception other value() -> 2 under {right} }}\n");
        assert!(
            artifacts(&source)
                .rule_dispatch_runtime_irrefutable_keys
                .contains(&key(Some("Branch"), "value", 0)),
            "{source}"
        );
    }
}

#[test]
fn gaps_changing_expressions_and_refutable_heads_receive_no_certificate() {
    for source in [
        "# Branch(input: Int) { | value() -> 1 under input < 0\n| exception other value() -> 2 under input > 0 }\n",
        "# Branch(input: Float) { | value() -> 1 under input < 0.0\n| exception other value() -> 2 under input >= 0.0 }\n",
        "# Branch(input: Int, other: Int) { | value() -> 1 under input < 0\n| exception other value() -> 2 under other >= 0 }\n",
        "> probe() -> Int { 1 }\n# Branch(input: Int) { | value() -> 1 under probe() < 0\n| exception other value() -> 2 under probe() >= 0 }\n",
        "# Branch(input: Int) { | value(0) -> 1 under input < 0\n| exception other value(0) -> 2 under input >= 0 }\n",
        "# Branch(input: Int) { | value(x: Int, x: Int) -> 1 under input < 0\n| exception other value(x: Int, x: Int) -> 2 under input >= 0 }\n",
        "# Branch(input: Int) { | value() -> 1 under input / 0 < 0\n| exception other value() -> 2 under input / 0 >= 0 }\n",
        "# Branch(input: Int) { | value(input) -> 1 under input < 0\n| exception other value(input) -> 2 under input >= 0 }\n",
        "# Branch(input: Int) { | value(input: Int) -> 1 under input < 0\n| exception other value(other: Int) -> 2 under input >= 0 }\n",
    ] {
        let checked = artifacts(source);
        assert!(!checked.rule_dispatch_runtime_irrefutable_keys.iter().any(|key| key.scope.as_deref() == Some("Branch") && key.name == "value"), "{source}");
    }
}

#[test]
fn erased_type_wrappers_cannot_authorize_guard_coverage() {
    for ty in ["Int?", "&Int", "&mut Int", "shared Int"] {
        for (declarations, parameter, expression) in [
            (String::new(), ty.to_owned(), "input"),
            (
                format!("# Input(value: {ty})\n"),
                "Input".into(),
                "input.value",
            ),
            (
                format!(
                    "# Leaf(value: Int)\n# Input(leaf: {})\n",
                    ty.replace("Int", "Leaf")
                ),
                "Input".into(),
                "input.leaf.value",
            ),
        ] {
            let source = format!("{declarations}# Branch(input: {parameter}) {{ | value() -> 1 under {expression} < 0\n| exception other value() -> 2 under {expression} >= 0 }}\n");
            let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
                .parse_program()
                .expect("wrapper fixture parses");
            // Some wrapped projections are rejected by the frontend as well.
            // Even its diagnostic artifacts must not carry runtime authority.
            let checked = TypeChecker::check_with_backend_artifacts(&statements, None, &source);
            assert!(
                !checked
                    .rule_dispatch_runtime_irrefutable_keys
                    .contains(&key(Some("Branch"), "value", 0)),
                "{source}"
            );
        }
    }
}

#[test]
fn enumeration_coverage_requires_every_nullary_member_of_the_same_owner() {
    for source in [
        "# Choice = First | Second | Third\n| value(x: Choice) -> 1 under x == First\n| value(x: Choice) -> 2 under x == Second\n",
        "# Choice = First | Second(Int)\n| value(x: Choice) -> 1 under x == First\n| value(x: Choice) -> 2 under x == Second(1)\n",
        "# Choice = First | Second\n# Other = First | Second\n| value(x: Choice) -> 1 under x == First\n| value(x: Choice) -> 2 under x == Second\n",
        "# Choice = First | Second\n| value(x: Choice, other: Choice) -> 1 under x == First\n| value(x: Choice, other: Choice) -> 2 under other == Second\n",
    ] {
        let checked = artifacts(source);
        assert!(!checked.rule_dispatch_runtime_irrefutable_keys.iter().any(|key| key.scope.is_none() && key.name == "value"), "{source}");
    }
}

#[test]
fn guard_coverage_does_not_certify_failing_rule_bodies() {
    let checked = artifacts("# Branch(input: Int) { | value() -> 1 / input under input < 0\n| exception other value() -> 1 / input under input >= 0 }\n");
    let key = key(Some("Branch"), "value", 0);
    assert!(checked
        .rule_dispatch_runtime_irrefutable_keys
        .contains(&key));
    assert!(!checked.rule_dispatch_total_value_keys.contains(&key));
}

#[test]
fn native_complementary_scoped_guards_keep_interpreted_values() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/complementary_scoped_guards.runa");
    for args in [
        vec![fixture.as_os_str()],
        vec![std::ffi::OsStr::new("run"), fixture.as_os_str()],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "10\n20\nfalse\ntrue\n-1\n1\n1\n2\n3"
        );
    }
}
