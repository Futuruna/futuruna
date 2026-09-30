use futuruna::{parse_prelude, prepend_prelude, Lexer, Parser, TypeChecker};

fn diagnostics(source: &str) -> Vec<futuruna::Diagnostic> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("declaration fixture parses");
    TypeChecker::check_with_diagnostics(
        &prepend_prelude(parse_prelude(), &statements),
        None,
        source,
    )
}

#[test]
fn ordinary_function_names_are_unique_within_their_lexical_scope() {
    for source in [
        "> f(a: Int) -> Int { a }\n> f(a: Int) -> Int { a + 1 }\n",
        "> f(a: Int) -> Int { a }\n> f() -> Int { 1 }\n",
        "> module M {\n> f() -> Int { 1 }\n> f() -> Int { 2 }\n}\n",
        "> outer() -> Int {\n> f() -> Int { 1 }\n> f() -> Int { 2 }\nf()\n}\n",
    ] {
        let errors = diagnostics(source);
        let error = errors
            .iter()
            .find(|error| error.message.contains("duplicate function declaration `f`"))
            .unwrap_or_else(|| panic!("{source}: {errors:?}"));
        let expected_line = if source.starts_with("> f") { 2 } else { 3 };
        assert_eq!(error.span.unwrap().start_line_col(source).0, expected_line);
    }
}

#[test]
fn separate_scopes_prelude_overrides_and_rule_clauses_keep_their_names() {
    let source = include_str!("differential/corpus/declaration_scope_controls.runa");
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        futuruna::eval_source_with_prelude(source, false)
            .unwrap()
            .trim(),
        "10"
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/declaration_scope_controls.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "10");
}
