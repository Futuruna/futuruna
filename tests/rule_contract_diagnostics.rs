use futuruna::{
    eval_source_with_prelude, parse_prelude, prepend_prelude, Lexer, Parser, TypeChecker,
};

fn diagnostics(source: &str) -> Vec<String> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("rule contract fixture parses");
    let statements = prepend_prelude(parse_prelude(), &statements);
    TypeChecker::check_with_source(&statements, None, source)
}

#[test]
fn non_boolean_rule_guards_fail_at_declaration_without_a_call() {
    for guard in ["person.income", "\"yes\"", "[1]", "[]", "()"] {
        for rule in [
            format!("| tax(person: Person) -> person.income / 4 under {guard}"),
            format!("| tax(person: Person) -> person.income / 4\n| exception special tax(person: Person) -> 0 under {guard}"),
        ] {
            let source = format!("# Person(income: Int)\n{rule}\n");
            let errors = diagnostics(&source);
            assert!(errors.iter().any(|error| error.contains("Bool") && (error.contains("guard") || error.contains("condition"))), "{source}: {errors:?}");
        }
    }
}

#[test]
fn conflicting_rule_results_fail_before_the_family_is_called() {
    for source in [
        "| tax(income: Int) -> income / 4\n| tax(income: Int) -> \"exempt\" under income < 10\n",
        "| tax(income: Int) -> income / 4\n| exception exempt tax(income: Int) -> \"exempt\" under income < 10\n",
        "# Case(income: Int) {\n    | tax() -> income / 4\n    | tax() -> \"exempt\" under income < 10\n}\n",
        "> module Local {\n| tax(income: Int) -> income / 4\n| tax(income: Int) -> \"exempt\" under income < 10\n}\n",
        "| tax(income) -> income\n| tax(0) -> 0\n| tax(1) -> \"exempt\"\n",
        "| tax(income: Int) -> [income]\n| tax(income: Int) -> [\"exempt\"] under income < 10\n",
    ] {
        let errors = diagnostics(source);
        assert!(errors.iter().any(|error| error.contains("tax") && error.contains("Int") && error.contains("String")), "{source}: {errors:?}");
    }
}

#[test]
fn result_contracts_distinguish_arities_scopes_and_supported_exception_only_rules() {
    let source = r#"
| answer(value: Int) -> value
| answer(left: String, right: String) -> left + right
# Numeric(value: Int) { | result() -> value }
# Textual(value: String) { | result() -> value }
| exception positive approved(value: Int) -> True under value > 0
@ print(show(answer(42)))
@ print(answer("a", "b"))
@ print(show(Numeric(value = 7).result()))
@ print(Textual(value = "okay").result())
@ print(show(approved(1)))
@ print(show(approved(0)))
"#;
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
    let output = eval_source_with_prelude(source, false).expect("supported rule families");
    assert_eq!(output.trim(), "42\nab\n7\nokay\ntrue\nfalse");
}

#[test]
fn untyped_guards_do_not_inherit_shadowed_global_types() {
    let source = "= flag = 7\n| allowed(flag) -> True under flag\n@ print(show(allowed(True)))\n";
    assert!(diagnostics(source).is_empty(), "{:?}", diagnostics(source));
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "true"
    );
}

#[test]
fn independent_inline_modules_and_partial_collection_types_remain_valid() {
    let source = r#"
> module Numeric { | answer() -> 7 }
> module Textual { | answer() -> "okay" }
| values(n: Int) -> [] under n == 0
| values(n: Int) -> [n]
| generic(value: a) -> value
@ print(show(Numeric.answer()))
@ print(Textual.answer())
@ print(show(values(0)))
@ print(show(values(2)))
"#;
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "7\nokay\n[]\n[2]"
    );
}

#[test]
fn cli_rejects_rule_contract_errors_before_execution_or_native_generation() {
    use std::process::Command;
    let root = std::env::temp_dir().join(format!(
        "futuruna-rule-contracts-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("invalid.runa");
    for (source, expected) in [
        ("| tax(income: Int) -> income / 4 under income\n", "Bool"),
        ("| tax(income: Int) -> income / 4\n| tax(income: Int) -> \"exempt\" under income < 10\n", "conflicting return types"),
    ] {
        std::fs::write(&path, format!("{source}@ print(\"must not run\")\n")).unwrap();
        for args in [&[][..], &["check", "--frontend"], &["check"], &["emit"], &["run"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_runa")).args(args).arg(&path)
                .current_dir(&root).env("NO_COLOR", "1").output().unwrap();
            assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
            assert!(String::from_utf8_lossy(&output.stderr).contains(expected), "{args:?}: {output:?}");
        }
    }
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
