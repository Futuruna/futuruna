use futuruna::{
    eval_source_with_prelude, parse_prelude, prepend_prelude, Diagnostic, Interpreter, Lexer,
    Parser, Stmt, TypeChecker,
};
use std::path::Path;
use std::process::Command;

fn parse(source: &str) -> Vec<Stmt> {
    Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap()
}

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let statements = prepend_prelude(parse_prelude(), &parse(source));
    TypeChecker::check_with_diagnostics(&statements, None, source)
}

#[test]
fn unsupported_effects_report_names_hints_and_authored_positions() {
    for (marked, expected) in [
        (
            "= intro = \"æøå println\"\n@ «println»(\"hi\")",
            "did you mean `print`",
        ),
        ("@ «printf»(\"hi\")", "did you mean `print`"),
        ("@ «log»(\"audit trail\")", "unknown effect `log`"),
        ("@ «assert»(False)", "without `@`"),
        (
            "> probe() { @ «assert_with_message»(False, \"failed\") }",
            "without `@`",
        ),
        ("= løn = @ «println»(\"hi\")", "did you mean `print`"),
    ] {
        let before = marked.split_once('«').unwrap().0;
        let expected_position = (
            before.chars().filter(|c| *c == '\n').count() + 1,
            before.rsplit('\n').next().unwrap().chars().count() + 1,
        );
        let source = marked.replace(['«', '»'], "");
        let errors = diagnostics(&source);
        let error = errors
            .iter()
            .find(|d| d.message.contains("unknown effect"))
            .unwrap_or_else(|| panic!("{source}: {errors:?}"));
        assert!(error.message.contains(expected), "{source}: {error:?}");
        assert_eq!(
            error.span.unwrap().start_line_col(&source),
            expected_position
        );
    }
}

#[test]
fn unsupported_effects_stop_all_cli_modes_before_prior_effects() {
    let path = std::env::temp_dir().join(format!("futuruna-effects-{}.runa", std::process::id()));
    for effect in [
        "println(\"hi\")",
        "log(\"audit trail\")",
        "assert(False)",
        "printf(\"hi\")",
    ] {
        std::fs::write(&path, format!("@ print(\"must not run\")\n@ {effect}\n")).unwrap();
        for args in [
            &[][..],
            &["check", "--frontend"],
            &["check"],
            &["emit"],
            &["run"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_runa"))
                .args(args)
                .arg(&path)
                .env("NO_COLOR", "1")
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("unknown effect"),
                "{args:?}: {output:?}"
            );
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unchecked_unknown_effects_stop_execution_instead_of_returning_unit() {
    let source = "@ print(\"before\")\n@ log(\"audit trail\")\n@ print(\"after\")\n";
    let mut interpreter = Interpreter::new();
    interpreter.suppress_output = true;
    let mut env = interpreter.default_env();
    let error = interpreter
        .run_program_with_diagnostics(&parse(source), &mut env, Path::new("effects.runa"), source)
        .expect_err("unchecked hosts must receive an explicit failure");
    assert!(error.message.contains("unknown effect `log`"), "{error:?}");
    assert_eq!(interpreter.output, vec!["before"]);
}

#[test]
fn real_effects_declaration_markers_and_algebraic_calls_keep_working() {
    let source = r#"
# effect Console { > say(message: String) -> () }
@ pure
> ordinary() -> Int { 7 }
> greet() -> () with Console { say("hello") }
= result = | handle Console {
    | say(message) -> { @ print(message); resume(()) }
} in greet()
@ skriv("world")
assert(ordinary() == 7)
# EffectInput(value: Int)
@ calculate
> calculation(input: EffectInput) -> Int { input.value }
"#;
    assert!(diagnostics(source).is_empty(), "{:?}", diagnostics(source));
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "hello\nworld"
    );
    for source in [
        "@ time\n@ random()\n@ input\n",
        "@ export\n> named() -> Int { 7 }\n",
        "@ test\n> tested() -> Bool { True }\n",
    ] {
        assert!(
            diagnostics(source).is_empty(),
            "{source}: {:?}",
            diagnostics(source)
        );
    }
}

#[test]
fn ordinary_assertions_cannot_be_silently_ignored() {
    let error = eval_source_with_prelude("assert(1 == 2)\n", false).unwrap_err();
    assert!(error.to_lowercase().contains("assertion failed"), "{error}");
    let path = std::env::temp_dir().join(format!(
        "futuruna-assert-effect-{}.runa",
        std::process::id()
    ));
    std::fs::write(&path, "assert(1 == 2)\n@ print(\"must not run\")\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .to_lowercase()
            .contains("assertion failed"),
        "{output:?}"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn supported_effects_execute_in_interpreted_and_native_programs() {
    let source = include_str!("differential/corpus/supported_effect_calls.runa");
    let expected = "supported\naliases\n1\n2\ndone";
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        expected
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/supported_effect_calls.runa");
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    assert!(diagnostics("= stream = subject()\n@ complete(stream)\n").is_empty());
}
