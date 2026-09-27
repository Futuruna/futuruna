use futuruna::{eval_source_with_prelude, Lexer, Parser, TypeChecker};

const VALID: &str = include_str!("differential/corpus/function_return_types.runa");

fn diagnostics(source: &str) -> Vec<futuruna::Diagnostic> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap_or_else(|error| panic!("parse {source}: {error}"));
    TypeChecker::check_with_diagnostics(&statements, None, source)
}

fn assert_bad_returns(sources: &[&str]) {
    for source in sources {
        let errors = diagnostics(source);
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("return type mismatch")),
            "expected return diagnostic for {source}: {errors:?}"
        );
    }
}

#[test]
fn annotated_functions_reject_known_wrong_results_without_being_called() {
    assert_bad_returns(&[
        "> bad(a: Int) -> Int { \"x\" }",
        "> bad(a: Int) -> Bool { a + 1 }",
        "> bad(a: Int) -> () { a }",
        "> bad(a: Int) -> Int { () }",
        "> bad(a: Int) -> Int { [] }",
        "> bad(a: Int) -> List(Int) { [\"x\"] }",
        "> bad(a: Int) -> Option(Int) { Some(\"x\") }",
        "> bad(a: Int) -> List(Int) { [1, \"x\"] }",
        "> text(a: Int) -> String { \"x\" }\n> bad(a: Int) -> Int { text(a) }",
    ]);
}

#[test]
fn return_checks_follow_branches_and_lexical_bindings() {
    assert_bad_returns(&[
        "> bad(a: Int) -> Int { if a > 0 { a } else { \"x\" } }",
        "> bad(a: Int) -> Int { = result = \"x\"\n result }",
        "> bad(a: Int) -> Int { = result = if a > 0 { a } else { \"x\" }\n result }",
        "> bad(a: Int) -> Int { = result = \"x\"\n if a > 0 { result } else { 0 } }",
        "> bad(value: Option(String)) -> Int { match value { | Some(text) -> text | None -> \"empty\" } }",
        "> bad(a: Int) -> Int -> Int { (|x: Int| \"wrong\") }",
        "> bad(a: Int) -> Int { (|x: Int| x) }",
    ]);
}

#[test]
fn methods_and_inline_modules_check_their_authored_returns() {
    assert_bad_returns(&[
        "# Color = Red { > bad(self) -> Int { \"x\" } }",
        "# trait Label { > bad(self) -> Int { \"x\" } }",
        "# Color = Red\n# trait Label { > bad(self) -> Int }\n# impl Label for Color { > bad(self) -> Int { \"x\" } }",
        "# Scope(value: String) { > bad() -> Int { value } }",
        "> module Inner { > bad(a: Int) -> Int { \"x\" } }",
    ]);
}

#[test]
fn generic_return_parameters_are_rigid_inside_the_function() {
    assert_bad_returns(&[
        "> bad(value: a) -> a { 0 }",
        "> bad(value: a, other: b) -> a { other }",
        "> bad(value: a) -> List(a) { [0] }",
        "> identity(value: a) -> a { value }\n> bad(n: Int) -> Int { identity(\"x\") }",
    ]);
}

#[test]
fn valid_contextual_and_generic_returns_keep_their_values() {
    assert_eq!(
        eval_source_with_prelude(VALID, false).unwrap().trim(),
        "42\n0\n7\n42\n42\nhello"
    );
    for source in [
        "> identity(value: a) -> a { value }",
        "> wrap(value: a) -> List(a) { [value] }",
        "> empty(value: a) -> List(a) { [] }",
        "> absent(value: a) -> Option(a) { None }",
        "> present(value: a) -> Option(a) { Some(value) }",
        "> apply_fn(value: a, callback: a -> b) -> b { callback(value) }",
        "> identity(value: a) -> a { value }\n> reuse(value: b) -> b { identity(value) }",
        "> optional(value: Int) -> Int? { Some(value) }",
        "> borrowed(value: &Int) -> &Int { value }",
        "# Boxed(a) = Boxed(a)\n> boxed(value: a) -> Boxed(a) { Boxed(value) }",
        "# Color = Red { > same(self) -> Color { self } }",
        "# Scope(value: Int) { > amount() -> Int { value } }",
        "> named(value: Int) -> String { \"outer\" }\n> apply_value(named) -> Int { named }",
        "> adder(offset: Int) -> Int -> Int { = value = []\n (|value: Int| value + offset) }",
        "> maybe(value: Int) -> Option(Int) { Some(value) }\n> increment(value: Int) -> Option(Int) { = n <- maybe(value)\n Some(n + 1) }",
    ] {
        assert!(
            diagnostics(source).is_empty(),
            "{source}: {:?}",
            diagnostics(source)
        );
    }
}

#[test]
fn return_diagnostic_points_at_the_wrong_expression() {
    let source = "> bad(value: Int) -> Int {\n    \"wrong\"\n}\n";
    let errors = diagnostics(source);
    let error = errors
        .iter()
        .find(|error| error.message.contains("return type mismatch"))
        .unwrap();
    assert!(
        error.message.contains("function `bad` expects `Int`"),
        "{error:?}"
    );
    assert!(error.message.contains("`String`"), "{error:?}");
    assert_eq!(error.span.unwrap().start, source.find("\"wrong\"").unwrap());
}

#[test]
fn native_inout_push_returns_unit_and_persistent_push_keeps_its_list() {
    for source in [
        "> add(values: inout List(Int)) -> () { push(values, 2) }",
        "> add(values: inout shared List(Int)) -> () { push(values, 2) }",
        "> add(values: inout List(Int)) -> () { = done = push(values, 2)\n done }",
        "> add(values: List(Int)) -> List(Int) { push(values, 2) }",
    ] {
        let errors = diagnostics(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
    assert_bad_returns(&[
        "> add(values: inout List(Int)) -> Int { push(values, 2) }",
        "> add(values: List(Int)) -> () { push(values, 2) }",
        "> add(values: inout List(Int)) -> () { = values = [1]\n push(values, 2) }",
        "> push(values: List(Int), item: Int) -> Int { item }\n> add(values: inout List(Int)) -> () { push(values, 2) }",
    ]);
}

#[test]
fn a_value_binding_shadows_a_same_named_function_signature() {
    let source = r#"
> selected(value: Int) -> String { "outer" }
= selected = |value| value
> invoke(value: Int) -> Int { selected(value) }
@ print(show(invoke(42)))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "42"
    );
}

#[test]
fn authored_constructor_fields_replace_the_same_prelude_owner() {
    for (body, valid) in [("Pair(7, \"hello\")", true), ("Pair(\"wrong\", 7)", false)] {
        let source = format!("# Pair = Pair(Int, String)\n> make_pair() -> Pair {{ {body} }}\n");
        let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap();
        for statements in [
            statements.clone(),
            futuruna::prepend_prelude(futuruna::parse_prelude(), &statements),
        ] {
            let errors = TypeChecker::check_with_diagnostics(&statements, None, &source);
            if valid {
                assert!(errors.is_empty(), "{errors:?}");
            } else {
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("return type mismatch")),
                    "{errors:?}"
                );
            }
        }
    }
}

#[test]
fn native_valid_returns_compile_and_match_interpretation() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/function_return_types.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "42\n0\n7\n42\n42\nhello"
    );
}
