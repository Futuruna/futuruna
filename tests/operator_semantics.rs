use futuruna::{eval_source_with_prelude, Lexer, Parser};
use std::path::Path;
use std::process::Command;

const SOURCE: &str = include_str!("differential/corpus/primitive_operator_results.runa");
const EXPECTED: &str = "zebra<apple: false\nif zebra<apple: else\nx==y: false\nif x==y: else\nPair(1,2)==Pair(1,3): false\n7.5 % 2.0: 1.5\n-7.5 % 2.0: -1.5\nx<y: true\ntuple equal: true\ntuple unequal: false\nunit equal: true\nrecord char equal: true\nlist chars equal: true\nnested tuples equal: true\nrecord tuple/unit equal: true";

#[test]
fn interpreted_operators_produce_values_and_correct_branches() {
    let output = eval_source_with_prelude(SOURCE, false).expect("evaluate primitive operators");
    assert_eq!(output.trim(), EXPECTED);
}

#[test]
fn native_operators_compile_and_produce_the_same_expected_values() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/primitive_operator_results.runa");
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .expect("execute compiled primitive operators");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
}

#[test]
fn non_boolean_if_conditions_fail_instead_of_selecting_a_branch() {
    for condition in ["()", "0", "\"yes\"", "[]"] {
        let source = format!("@ print(show(if {condition} {{ 1 }} else {{ 2 }}))\n");
        let error = eval_source_with_prelude(&source, false)
            .expect_err("non-Boolean conditions must not select a branch");
        assert!(error.contains("if condition must return Bool"), "{error}");
    }
}

#[test]
fn single_equals_expressions_fail_at_the_operator_with_a_comparison_hint() {
    for marked in [
        "# P(status: String)\n| fee(p) -> 0 under p.status «=» \"student\"",
        "= x = 3\n= selected = if x «=» 5 { \"five\" } else { \"other\" }",
        "> chk(n: Int) -> Bool { n «=» 5 }",
        "filter([1, 2, 3], |n| n «=» 2)",
        "= løn = 3\r\n= result = (løn + 2 «=» 5)\r\n",
        "= result = 1 + (2 «=» 3)",
        "= result = \"\"\"answer {{ 1 «=» 2 }}\"\"\"",
    ] {
        let before = marked.split_once('«').unwrap().0;
        let line = before.chars().filter(|c| *c == '\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
        let source = marked.replace(['«', '»'], "");
        let error = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .expect_err("single-equals expressions cannot reach either execution backend");
        assert!(
            error.contains("use `==` for comparison"),
            "{source}: {error}"
        );
        assert!(
            error.contains(&format!("{line}:{column}:")),
            "{source}: {error}"
        );
    }
}

#[test]
fn binding_named_argument_and_opaque_equals_syntax_still_parses() {
    let source = r#"
# Record(value: Int)
# Choice = One | Two
= n: Int = 3
= record = Record(value = n)
= copy = n
> compare(n: Int) -> Bool { = same = n == 5; same }
= text = "an = sign"
= symbol = '='
---- prose = unchanged ----
@ rust { fn embedded() -> bool { let x = 3; x == 3 } }
"#;
    Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("equals remains valid in declarations, named arguments and opaque text");
}

#[test]
fn all_cli_execution_modes_reject_single_equals_before_any_effect() {
    let path = std::env::temp_dir().join(format!(
        "futuruna-single-equals-{}-{}.runa",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &path,
        "@ print(\"must not execute\")\n= n = 3\n= result = if n = 5 { 1 } else { 2 }\n",
    )
    .unwrap();
    let outputs = [
        vec![],
        vec!["run"],
        vec!["check", "--frontend"],
        vec!["check"],
    ]
    .into_iter()
    .map(|args| {
        Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(args)
            .arg(&path)
            .output()
            .unwrap()
    })
    .collect::<Vec<_>>();
    std::fs::remove_file(&path).unwrap();
    for output in outputs {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(stderr.contains("3:17:"), "{stderr}");
        assert!(stderr.contains("use `==` for comparison"), "{stderr}");
    }
}

const CORRECTED_EQUALITY: &str = include_str!("differential/corpus/bindings_and_equality.runa");
const CORRECTED_EQUALITY_OUTPUT: &str = "10\n0\nother\ntrue\nfalse\n[2]";

#[test]
fn explicit_equality_and_named_arguments_have_the_expected_interpreted_results() {
    let output = eval_source_with_prelude(CORRECTED_EQUALITY, false).unwrap();
    assert_eq!(output.trim(), CORRECTED_EQUALITY_OUTPUT);
}

#[test]
fn explicit_equality_and_named_arguments_have_the_expected_native_results() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/bindings_and_equality.runa");
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        CORRECTED_EQUALITY_OUTPUT
    );
}
