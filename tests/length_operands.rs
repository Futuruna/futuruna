use futuruna::{eval_source_with_prelude, Lexer, Parser, TypeChecker};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
const VALID: &str = include_str!("differential/corpus/length_operands.runa");

fn diagnostics(source: &str) -> Vec<String> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    TypeChecker::check_with_diagnostics(&statements, None, source)
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

fn fixture(source: &str, extension: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "futuruna-length-{}-{}-{}.{}",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        extension
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn runa() -> std::ffi::OsString {
    std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into())
}

#[test]
fn length_rejects_known_non_list_and_non_string_types_in_the_frontend() {
    for expression in [
        "12345",
        "True",
        "2.5",
        "Some(1)",
        "map_new()",
        "set_new()",
        "()",
    ] {
        let source = format!("= count = length({expression})\n");
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("length expects List or String")),
            "{source}: {errors:?}"
        );
    }
    for ty in [
        "Int",
        "Bool",
        "Map(String, Int)",
        "Set(String)",
        "Int?",
        "List(Int)?",
    ] {
        let source = format!("> count(values: {ty}) -> Int {{ length(values) }}\n");
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("length expects List or String")),
            "{source}: {errors:?}"
        );
    }
    for source in [
        "= values = map_new()\n= count = length(values)\n",
        "= values = set_insert(set_new(), 1)\n= count = length(values)\n",
    ] {
        assert!(
            diagnostics(source)
                .iter()
                .any(|error| error.contains("length expects List or String")),
            "{source}"
        );
    }
    for source in [
        "> count(values: &List(a)) -> Int { length(values) }\n",
        "> count(value: &String) -> Int { length(value) }\n",
    ] {
        assert!(
            diagnostics(source).is_empty(),
            "{source}: {:?}",
            diagnostics(source)
        );
    }
}

#[test]
fn dynamically_called_length_rejects_invalid_values_instead_of_returning_zero() {
    for expression in ["12345", "True", "Some(1)", "map_new()", "set_new()", "()"] {
        let source = format!("= size = length\n@ print(show(size({expression})))\n");
        let error = eval_source_with_prelude(&source, true)
            .expect_err("invalid length must not yield a numeric result");
        assert!(error.contains("length expects List or String"), "{error}");
    }
}

#[test]
fn valid_lengths_preserve_unicode_counts_and_native_parity() {
    let expected = "3\n0\n3\n0\n2\n1\n1";
    assert_eq!(
        eval_source_with_prelude(VALID, true).unwrap().trim(),
        expected
    );
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/length_operands.runa");
    let output = Command::new(runa())
        .arg("run")
        .arg(source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
}

#[test]
fn authored_length_functions_and_local_callbacks_keep_their_own_types() {
    for source in [
        "> length(value: Int) -> Int { value + 1 }\n@ print(show(length(41)))\n",
        "> apply(length: Int -> Int) -> Int { length(41) }\n@ print(show(apply(|value| value + 1)))\n",
        "= length = |value: Int| value + 1\n@ print(show(length(41)))\n",
    ] {
        assert!(diagnostics(source).is_empty(), "{:?}", diagnostics(source));
        assert_eq!(eval_source_with_prelude(source, true).unwrap().trim(), "42");
        let path = fixture(source, "runa");
        let output = Command::new(runa()).arg("run").arg(&path).output().unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    }
}

#[test]
fn calculation_commands_reject_invalid_length_models_before_execution() {
    let source = fixture("# Input(values: Map(String, Int))\n# CountOutput(count: Int)\n@ calculate\n> calculate_count(input: Input) -> CountOutput { CountOutput(count = length(input.values)) }\n", "runa");
    let input = fixture(
        r#"{"cases":[{"case_id":"one","input":{"values":{"key":1}}}]}"#,
        "json",
    );
    for command in ["schema", "template", "call", "check"] {
        let mut child = Command::new(runa());
        child.arg(command).arg(&source);
        if command == "call" {
            child.arg("--input").arg(&input);
        }
        let output = child.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "{command}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("length expects List or String"),
            "{command}: {stderr}"
        );
        assert!(!stderr.contains("E0277"), "{stderr}");
    }
    std::fs::remove_file(source).unwrap();
    std::fs::remove_file(input).unwrap();
}
