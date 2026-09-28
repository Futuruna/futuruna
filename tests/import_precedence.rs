use futuruna::{parse_prelude, prepend_prelude, Interpreter, Lexer, Parser};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture {
    directory: PathBuf,
}

impl Fixture {
    fn new(dependency: &str, source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "futuruna-import-precedence-{}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("dependency.runa"), dependency).unwrap();
        std::fs::write(directory.join("main.runa"), source).unwrap();
        Self { directory }
    }

    fn run(&self, arguments: &[&str]) -> Output {
        let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        Command::new(runa)
            .args(arguments)
            .arg(self.directory.join("main.runa"))
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in ["dependency.runa", "main.runa"] {
            std::fs::remove_file(self.directory.join(name)).unwrap();
        }
        std::fs::remove_dir(&self.directory).unwrap();
    }
}

fn fixtures() -> Vec<(Fixture, &'static str, &'static str)> {
    vec![
        (Fixture::new(
            "> fee(x: Int) -> Int { x * 100 }\n> imported_total(x: Int) -> Int { fee(x) }\n",
            "@ import ./dependency\n> fee(x: Int) -> Int { x + 1 }\n@ print(show(fee(5)))\n@ print(show(imported_total(5)))\n| local_fee: 0 -> fee(5) == 6 && imported_total(5) == 6\n",
        ), "6\n6", "local_fee"),
        (Fixture::new(
            "| rate(i: Int) -> 1\n| rate(i: Int) -> 2 under i > 10\n",
            "@ import ./dependency\n| rate(i: Int) -> 3 under i > 5\n@ print(show(rate(20)))\n| earlier_imported_guard: 0 -> rate(20) == 2\n",
        ), "2", "earlier_imported_guard"),
    ]
}

#[test]
fn prefix_import_precedence_is_the_same_with_and_without_the_prelude() {
    for (fixture, expected, _) in fixtures() {
        for arguments in [&[][..], &["--no-prelude"][..]] {
            let output = fixture.run(arguments);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                expected,
                "{arguments:?}"
            );
        }
    }
}

#[test]
fn native_prefix_imports_preserve_local_functions_and_earlier_rule_guards() {
    for (fixture, expected, _) in fixtures() {
        let output = fixture.run(&["run"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn verification_uses_the_same_import_precedence() {
    for (fixture, _, invariant) in fixtures() {
        let output = fixture.run(&["verify"]);
        assert_eq!(
            output.status.success(),
            Command::new("z3")
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success()),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        if Command::new("z3").arg("--version").output().is_ok() {
            assert!(
                stdout.contains(&format!("PROVED: |{invariant}|")),
                "{stdout}"
            );
        } else {
            assert!(stdout.contains("Z3 not found"), "{stdout}");
        }
    }
}

#[test]
fn authored_declarations_before_imports_keep_static_precedence() {
    let fixture = Fixture::new(
        "> fee(x: Int) -> Int { x * 100 }\n",
        "> identity(x: a) -> a { x }\n@ import ./dependency\n> fee(x: Int) -> Int { x + 1 }\n@ print(show(fee(5)))\n",
    );
    for arguments in [&[][..], &["--no-prelude"][..]] {
        let output = fixture.run(arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "6",
            "{arguments:?}"
        );
    }
}

#[test]
fn cloned_and_repeated_prelude_injection_keeps_origin_distinct_from_authored_copies() {
    let source = "@ import ./dependency\n> fee(x: Int) -> Int { x + 1 }\n@ print(show(fee(5)))\n@ print(show(identity(5)))\n";
    let fixture = Fixture::new(
        "> fee(x: Int) -> Int { x * 100 }\n> identity(x: Int) -> Int { x + 1 }\n",
        source,
    );
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    let injected = prepend_prelude(parse_prelude(), &statements);
    let repeated = prepend_prelude(parse_prelude(), &injected);
    let mut authored = parse_prelude();
    authored.extend(statements);
    for (program, expected) in [
        (injected.clone(), "6\n6"),
        (repeated, "6\n6"),
        (authored, "6\n5"),
    ] {
        let mut interpreter = Interpreter::new();
        interpreter.source_dir = Some(fixture.directory.to_string_lossy().into_owned());
        let mut environment = interpreter.default_env();
        interpreter.run_program(&program, &mut environment);
        assert_eq!(interpreter.output.join("\n"), expected);
    }
}

#[test]
fn verification_and_execution_share_static_import_precedence() {
    for prefix in ["> anchor() -> Int { 0 }\n", "> identity(x: a) -> a { x }\n"] {
        let fixture = Fixture::new(
            "> fee(x: Int) -> Int { x * 100 }\n",
            &format!("{prefix}@ import ./dependency\n> fee(x: Int) -> Int {{ x + 1 }}\n| correct: 0 -> fee(5) == 6\n| wrong: 0 -> fee(5) == 500\n? correct else {{ @ print(\"wrong precedence\") }}\n? wrong else {{ @ print(\"counterexample\") }}\n"),
        );
        for args in [&[][..], &["run"][..]] {
            let output = fixture.run(args);
            assert!(output.status.success(), "{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "counterexample"
            );
        }
        let verified = fixture.run(&["verify"]);
        assert!(!verified.status.success());
        let stdout = String::from_utf8_lossy(&verified.stdout);
        assert!(!stdout.contains("PROVED: |wrong|"), "{stdout}");
        if Command::new("z3")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            assert!(stdout.contains("PROVED: |correct|"), "{stdout}");
            assert!(stdout.contains("COUNTEREXAMPLE"), "{stdout}");
        } else {
            assert!(stdout.contains("Z3 not found"), "{stdout}");
        }
    }
}
