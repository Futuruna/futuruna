use futuruna::{eval_source_with_prelude, Lexer, Parser};
use std::path::Path;
use std::process::Command;

#[test]
fn misplaced_language_directives_fail_at_the_declaration() {
    for source in [
        "# Sag(beløb: Heltal)\n@ sprog da\n= s = Sag(5)\n@ print(show(hvis s.beløb > 3 { \"stor\" } ellers { \"lille\" }))\n",
        "= value = 1\n@ language en\n",
        "@ sprog da\n@ sprog da\n",
        "> inner() {\n@ language en\n1\n}\n",
    ] {
        let error = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect_err("a misplaced directive cannot be silently ignored");
        assert!(error.contains("2:3:") && error.contains("first declaration"), "{source}: {error}");
    }
}

#[test]
fn initial_language_declarations_allow_comments_and_blank_lines() {
    for prefix in [
        "\n-- a comment\n@ sprog da\n",
        "----\nquoted provenance\n----\n@ language dansk\n",
    ] {
        let source = format!("{prefix}@ print(show(hvis Sandt {{ 7 }} ellers {{ 8 }}))\n");
        assert_eq!(
            eval_source_with_prelude(&source, false).unwrap().trim(),
            "7"
        );
    }
}

#[test]
fn header_whitespace_and_trailing_comments_do_not_disable_the_language() {
    for prefix in [
        "@sprog da\n",
        "@\tsprog\tda\n",
        "@ language dansk -- Danish syntax\n",
    ] {
        let source = format!("{prefix}@ print(show(hvis Sandt {{ 7 }} ellers {{ 8 }}))\n");
        assert_eq!(
            eval_source_with_prelude(&source, false).unwrap().trim(),
            "7"
        );
    }
}

#[test]
fn danish_connectors_do_not_reserve_names_in_english_sources() {
    let source = "= og = 1\n= eller = 2\n@ print(show(og + eller))\n";
    assert_eq!(eval_source_with_prelude(source, false).unwrap().trim(), "3");
}

const BOOLEAN_SOURCE: &str = include_str!("differential/corpus/danish_boolean_connectors.runa");
const BOOLEAN_OUTPUT: &str = "true\nfalse\ntrue\ntrue\nfalse\naccepted";

#[test]
fn danish_boolean_connectors_preserve_precedence_and_short_circuiting() {
    assert_eq!(
        eval_source_with_prelude(BOOLEAN_SOURCE, false)
            .unwrap()
            .trim(),
        BOOLEAN_OUTPUT
    );
}

#[test]
fn danish_goal_connectors_preserve_query_bindings() {
    let source = r#"@ sprog da
| person(1)
| edge(1, 2)
| wanted(2)
| other(3)
| selected(x) -> person(x) og edge(x, y) og wanted(y) eller other(x)
@ print(show(selected(1)))
@ print(show(selected(2)))
@ print(show(selected(3)))
@ print(show(findall(x, selected(x))))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "true\nfalse\ntrue\n[1, 3]"
    );
}

#[test]
fn native_danish_boolean_connectors_match_interpretation() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/danish_boolean_connectors.runa");
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        BOOLEAN_OUTPUT
    );
}
