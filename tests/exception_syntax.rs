use futuruna::{eval_source_with_prelude, Lexer, Parser};
use std::path::Path;
use std::process::Command;

const SOURCE: &str = include_str!("differential/corpus/labeled_rule_exceptions.runa");
const EXPECTED: &str = "10\n25\n10\n25\n10\n25";

#[test]
fn missing_exception_labels_and_non_callable_heads_are_located_errors() {
    for marked in [
        "| exception rate«(»x) -> 10 under x < 10",
        "@ sprog da\n| undtagelse sats«(»x) -> 10 under x < 10",
        "| exception missing «7» -> 10",
        "| exception missing «[»1, 2] -> 10",
        "| exception missing «r»ate -> 10",
        "| exception missing «p»olicy.rate(x) -> 10",
        "| exception missing «r»ate(x) + 1 -> 10",
    ] {
        let before = marked.split_once('«').unwrap().0;
        let line = before.chars().filter(|c| *c == '\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
        let source = marked.replace(['«', '»'], "");
        let error = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .expect_err("a malformed exception cannot silently disappear from dispatch");
        assert!(error.contains("exception"), "{source}: {error}");
        assert!(
            error.contains("label") && error.contains("rate(x)"),
            "{source}: {error}"
        );
        assert!(
            error.contains(&format!("{line}:{column}:")),
            "{source}: {error}"
        );
    }
}

#[test]
fn labeled_english_danish_and_scoped_exceptions_apply() {
    assert_eq!(
        eval_source_with_prelude(SOURCE, false).unwrap().trim(),
        EXPECTED
    );
}

#[test]
fn native_labeled_exceptions_match_interpreted_dispatch() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/labeled_rule_exceptions.runa");
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
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
}

#[test]
fn malformed_exceptions_stop_every_cli_mode_before_effects() {
    let path = std::env::temp_dir().join(format!(
        "futuruna-exception-syntax-{}-{}.runa",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for rule in [
        "| exception rate(x) -> 10 under x < 10",
        "| exception discount 7 -> 10",
    ] {
        std::fs::write(
            &path,
            format!("@ print(\"must not run\")\n| rate(x) -> 25\n{rule}\n@ print(show(rate(5)))\n"),
        )
        .unwrap();
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
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                error.contains("exception") && error.contains("label"),
                "{args:?}: {output:?}"
            );
        }
    }
    std::fs::remove_file(path).unwrap();
}
