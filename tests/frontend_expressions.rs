use futuruna::{eval_source_with_prelude, Lexer, Parser, TypeChecker};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn thousand_term_frontend_check_finishes_within_a_generous_budget() {
    let root =
        std::env::temp_dir().join(format!("futuruna-expression-scale-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("sum.runa");
    let source = format!("= total = {}\n", vec!["1"; 1000].join(" + "));
    std::fs::write(&path, source).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(["check", "--frontend"])
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // This is a hang/growth guard, not a microbenchmark. The reproduced debug
    // compiler exceeded 60 seconds for this input. Keep ample CI headroom.
    let started = Instant::now();
    let mut timed_out = false;
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert!(!timed_out, "1000-term frontend check exceeded 30 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn binary_walk_keeps_nested_list_addition_diagnostics() {
    for (expression, count) in [
        ("[1] + [2]", 1),
        ("([1] + [2]) + [3]", 2),
        ("[1] + ([2] + [3])", 2),
        ("([] + []) + [3]", 1),
        ("([1] - [2]) + [3]", 1),
        ("(if True { [1] } else { [2] }) + [3]", 1),
    ] {
        let source = format!("= result = {expression}\n");
        let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap();
        let errors = TypeChecker::check_with_source(&statements, None, &source);
        assert_eq!(
            errors
                .iter()
                .filter(|e| e.contains("does not concatenate lists"))
                .count(),
            count,
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn binary_walk_preserves_numeric_string_and_boolean_result_types() {
    let source = r#"
> amount(value: Int) -> Float { (value + 2) * 0.5 }
> label(value: Int) -> String { "sum " + (value + 2) }
> accepted(value: Int) -> Bool { value + 2 > 3 && value - 1 < 5 }
@ print(show(amount(3)))
@ print(label(3))
@ print(show(accepted(3)))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "2.5\nsum 5\ntrue"
    );
}
