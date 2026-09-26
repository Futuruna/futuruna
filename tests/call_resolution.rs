use futuruna::{eval_source_with_prelude, Lexer, Parser, TypeChecker};

#[test]
fn known_rules_reject_calls_with_undeclared_arities() {
    for arguments in ["\"alice\"", "\"alice\", \"bob\", \"x\""] {
        let source = format!("| parent(\"alice\", \"bob\")\n@ print(show(parent({arguments})))\n");
        let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap();
        let errors = TypeChecker::check_with_diagnostics(&statements, None, &source);
        let error = errors
            .iter()
            .find(|error| error.message.contains("arity") || error.message.contains("arities"))
            .unwrap_or_else(|| panic!("{source}: {errors:?}"));
        assert!(
            error.message.contains("parent") && error.message.contains('2'),
            "{error:?}"
        );
        assert_eq!(error.span.unwrap().start_line_col(&source), (2, 14));
        assert!(eval_source_with_prelude(&source, false).is_err());
    }
}

#[test]
fn overloaded_rule_arities_and_lexical_callable_parameters_remain_valid() {
    let source = r#"
| related("alice", "bob")
| related("alice")
> apply(related: Int -> Int) -> Int { related(2) }
> untyped(related) { related(2) }
@ print(show(related("alice")))
@ print(show(related("alice", "bob")))
@ print(show(related("bob")))
@ print(show(apply(|value| value + 1)))
@ print(show(untyped(|value| value + 2)))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "true\ntrue\nfalse\n3\n4"
    );
}

#[test]
fn local_and_imported_rule_arities_do_not_borrow_other_declarations() {
    for source in [
        "| related(1)\n> probe() { | related(1, 2); related(1) }\n",
        "| related(1, 2)\n> probe() { | related(1); related(1, 2) }\n",
        "| related(1, 2)\n> probe() { = callback = |value| value + 1; callback(1); related(1) }\n",
    ] {
        assert!(eval_source_with_prelude(source, false).is_err(), "{source}");
    }
    let root = std::env::temp_dir().join(format!("futuruna-call-arity-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("relations.runa"), "| related(1, 2)\n").unwrap();
    let source = "@ import ./relations\n@ print(show(related(1)))\n";
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    let errors = TypeChecker::check_with_diagnostics(
        &statements,
        Some(root.to_string_lossy().into()),
        source,
    );
    assert!(
        errors.iter().any(|error| error.message.contains("arities")),
        "{errors:?}"
    );
    std::fs::remove_file(root.join("relations.runa")).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn untyped_rule_parameters_and_local_closures_can_shadow_rules() {
    let source = r#"
| related(1, 2)
| delegated(related) -> related(2)
> probe() -> Int {
    = related = |value| value + 5
    related(2)
}
@ print(show(delegated(|value| value + 1)))
@ print(show(probe()))
@ print(show(related(1, 2)))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "3\n7\ntrue"
    );
}
