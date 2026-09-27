use futuruna::{eval_source_with_prelude, Lexer, Parser};

fn parse(source: &str) -> Result<Vec<futuruna::Stmt>, String> {
    Parser::new(Lexer::new(source).tokenize(), source).parse_program()
}

fn assert_hints(cases: &[(&str, &str)]) {
    let mut missing = Vec::new();
    for (source, hint) in cases {
        let error = parse(source).expect_err("foreign syntax must still be rejected");
        if !error.contains(hint) {
            missing.push(format!("{source}\nexpected {hint:?}: {error}"));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n\n"));
}

#[test]
fn lexical_mistakes_explain_comments_strings_numbers_and_negation() {
    assert_hints(&[
        ("# This is a comment", "comments use `--`"),
        ("// note", "comments use `--`"),
        ("/* note */", "block comments use `----"),
        ("= text = 'hello'", "double quotes"),
        ("= count = 1_000_000", "digit separators"),
        ("= amount = 1.25_000", "digit separators"),
        ("| allowed(x) -> \\+ denied(x)", "not(...)"),
        ("= rate = 25%", "remainder"),
    ]);
}

#[test]
fn foreign_conditionals_bindings_and_return_have_contextual_hints() {
    assert_hints(&[
        ("= amount = when True { 1 }", "if condition"),
        ("| fee(x) -> 1 where x > 0", "under"),
        (
            "= selected = if True { 1 } elif False { 2 } else { 3 }",
            "else if",
        ),
        ("> identity(x: Int) -> Int { return x }", "last expression"),
        ("> answer() -> Int { let x = 1; x }", "= name = value"),
    ]);
}

#[test]
fn rule_guards_ranges_and_lambdas_show_the_native_construct() {
    assert_hints(&[
        ("| fee(x) -> 1 under x > 0, x < 10", "`and` or `&&`"),
        ("| parent(1) :- True", "`->`"),
        ("= items = 0..3", "range(start, end)"),
        ("= increment = x => x + 1", "|x| expression"),
        ("= values = map([1], x => x + 1)", "|x| expression"),
        ("= value = (when True { 1 })", "if condition"),
    ]);
}

#[test]
fn malformed_character_strings_have_one_error_at_the_opening_quote() {
    let error = parse("= prís = 'hello'\n").unwrap_err();
    assert_eq!(error.lines().count(), 1, "{error}");
    assert!(error.starts_with("1:10:"), "{error}");
    assert!(error.contains("double quotes"), "{error}");
}

#[test]
fn valid_similar_spellings_keep_their_meaning() {
    let source = include_str!("differential/corpus/foreign_syntax_controls.runa");
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "15\n4\n4\nHello {name}\n105\nHello Ada\ntrue\ntrue\n1"
    );
    for source in [
        "# TODO\n",
        "= _000 = 1\n",
        "= s = \"// /* */ \\+ 1_000_000 'hello'\"\n",
        "@ rust { fn foo() { let x = 1_000; /* note */ // note\n } }\n",
        "| positive(x) -> x > 0\n| accepted(x) -> positive(x), x < 10\n",
    ] {
        parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn native_controls_keep_the_same_strings_arithmetic_and_identifiers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/foreign_syntax_controls.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "15\n4\n4\nHello {name}\n105\nHello Ada\ntrue\ntrue\n1"
    );
}
