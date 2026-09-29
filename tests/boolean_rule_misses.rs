use futuruna::{eval_source_with_prelude, Interpreter, Lexer, Parser, TypeChecker, Value};
use std::path::Path;
use std::process::Command;

const SOURCE: &str = include_str!("differential/corpus/untyped_boolean_rule_misses.runa");
const EXPECTED: &str = "false\ntrue\nno\ntrue\ntrue\nfalse\nfalse\ntrue\nfalse\nfalse\ntrue\nfalse\ntrue\nfalse\nfalse";

#[test]
fn untyped_boolean_rule_hits_and_misses_are_boolean_values() {
    assert_eq!(
        eval_source_with_prelude(SOURCE, false).unwrap().trim(),
        EXPECTED
    );
}

#[test]
fn native_untyped_boolean_rules_infer_parameters_from_guards() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/untyped_boolean_rule_misses.runa");
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
}

#[test]
fn boolean_misses_do_not_change_other_arities_or_partial_integer_rules() {
    let rules = "| eligible(age) -> True under age >= 18\n| eligible(age, threshold) -> 7 under age >= threshold\n";
    let source =
        format!("{rules}= predicate_miss = eligible(10)\n= amount_hit = eligible(20, 18)\n");
    let source = source.as_str();
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
    assert!(
        artifacts.diagnostics.is_empty(),
        "{:?}",
        artifacts.diagnostics
    );
    for install_checked in [false, true] {
        let mut interpreter = Interpreter::new();
        if install_checked {
            interpreter.install_rule_dispatch_metadata(&artifacts);
        } else {
            interpreter.install_rule_dispatch_metadata_for_program(&statements, None);
        }
        let mut env = interpreter.default_env();
        interpreter.run_program(&statements, &mut env);
        assert!(matches!(
            env.get("predicate_miss"),
            Some(Value::Bool(false))
        ));
        assert!(matches!(env.get("amount_hit"), Some(Value::Int(7))));
    }
    let error =
        eval_source_with_prelude(&format!("{rules}@ print(show(eligible(10, 18)))\n"), false)
            .expect_err("an Int rule miss has no Boolean answer");
    assert!(
        error.contains("no value rule matched `eligible/2`"),
        "{error}"
    );
}

#[test]
fn a_guard_error_never_becomes_a_boolean_miss() {
    for (source, expected) in [
        (
            "| eligible(value) -> True under value\n@ print(show(eligible(7)))\n",
            "guard must return Bool",
        ),
        (
            "| eligible(value) -> True under 1 / (value - 1) > 0\n@ print(show(eligible(1)))\n",
            "division by zero",
        ),
        (
            "| eligible(value) -> True under value >= 18\n@ print(show(eligible(\"young\")))\n",
            "unsupported operands",
        ),
    ] {
        let error =
            eval_source_with_prelude(source, false).expect_err("a failed guard is not False");
        assert!(error.contains(expected), "{source}: {error}");
    }
}
