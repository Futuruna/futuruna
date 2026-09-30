use futuruna::{Lexer, Parser};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct SourceFile(std::path::PathBuf);

impl SourceFile {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "futuruna-lexical-{}-{}-{}.runa",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        Command::new(runa).args(args).arg(&self.0).output().unwrap()
    }
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn parse(source: &str) -> Result<Vec<futuruna::Stmt>, String> {
    Parser::new(Lexer::new(source).tokenize(), source).parse_program()
}

#[test]
fn unknown_characters_are_errors_at_their_unicode_source_position() {
    for (source, position, symbol) in [
        ("= x = 5 § 3\n@ print(show(x))\n", "1:9", "§"),
        ("= prís = 5\n= y = prís ¤ 2\n", "2:12", "¤"),
        ("= prís = 5\r\n= y = prís – 2\r\n", "2:12", "–"),
        ("# Record(value: Int, §)\n", "1:22", "§"),
        ("> actor Worker(state: Int) { § }\n", "1:30", "§"),
        ("= x = ”wrong”\n", "1:7", "”"),
    ] {
        let error = parse(source).expect_err("unknown characters cannot disappear");
        assert!(error.contains("unexpected character"), "{source}: {error}");
        assert!(error.contains(position), "{source}: {error}");
        assert!(error.contains(symbol), "{source}: {error}");
    }
    let error = parse("= x = 5\u{a0}3\n").expect_err("nonbreaking space must not disappear");
    assert!(error.contains("unexpected character"), "{error}");
}

#[test]
fn unterminated_strings_and_source_quotations_point_to_the_opening_delimiter() {
    for (source, message, position) in [
        (
            "= s = \"hello\n@ print(s)\n",
            "unterminated string literal",
            "1:7",
        ),
        ("= s = \"hello\\", "unterminated string literal", "1:7"),
        (
            "= s = \"\"\"hello\n",
            "unterminated triple-quoted string literal",
            "1:7",
        ),
        (
            "= x = 1\n----\nnever closed\n@ print(show(x))\n",
            "unterminated block comment",
            "2:1",
        ),
        (
            "= x = 1\r\n----\r\nnever closed\r\n",
            "unterminated block comment",
            "2:1",
        ),
        (
            "= s = \"hello\r\n@ print(s)\r\n",
            "unterminated string literal",
            "1:7",
        ),
    ] {
        let error = parse(source).expect_err("an incomplete lexical form must not swallow source");
        assert!(error.contains(message), "{source}: {error}");
        assert!(error.contains(position), "{source}: {error}");
    }
}

#[test]
fn incomplete_character_literals_are_rejected() {
    for source in ["= c = 'a\n", "= c = 'ab'\n", "= c = ''\n", "= c = '\\\n"] {
        let error =
            parse(source).expect_err("a character literal needs one character and a closing quote");
        assert!(error.contains("character literal"), "{error}");
        assert!(error.contains("1:7"), "{error}");
    }
}

#[test]
fn unknown_escapes_report_the_backslash_at_its_source_position() {
    for (source, position, kind) in [
        ("= s = \"a\\qb\"\n", "1:9", "string"),
        ("= prís = \"æ\\q\"\r\n", "1:12", "string"),
        ("= c = '\\q'\n", "1:8", "character"),
        ("= s = \"\"\"\n{{ \"a\\q\" }}\n\"\"\"\n", "2:6", "string"),
    ] {
        let error = parse(source)
            .expect_err("unknown escapes cannot silently change or preserve characters");
        assert!(
            error.contains(&format!("unknown {kind} escape")),
            "{source}: {error}"
        );
        assert!(error.contains(position), "{source}: {error}");
        assert!(error.contains("literal backslash"), "{error}");
    }
}

#[test]
fn punctuation_cannot_be_pattern_variables() {
    for source in ["= = 5\n", "= + = 5\n", "= x = match 1 { | -> 2 }\n"] {
        let error = parse(source).expect_err("a pattern needs a valid syntactic form");
        assert!(error.contains("expected a pattern"), "{source}: {error}");
    }
}

#[test]
fn carriage_return_escapes_have_their_character_value() {
    let tokens = Lexer::new("\"\\r\" '\\r' \"\\\\r\"").tokenize();
    assert_eq!(tokens[0].text, "\r");
    assert_eq!(tokens[1].text, "\r");
    assert_eq!(tokens[2].text, "\\r");
}

#[test]
fn invalid_escapes_and_binding_names_fail_before_effects_and_preserve_formatted_source() {
    for declaration in ["= s = \"a\\q\"", "= c = '\\q'", "= = 5"] {
        let source = format!("@ print(\"must not run\")\n{declaration}\n");
        let file = SourceFile::new(&source);
        for args in [
            &[][..],
            &["run"][..],
            &["check", "--frontend"][..],
            &["check"][..],
            &["emit"][..],
            &["fmt", "--check"][..],
            &["fmt"][..],
        ] {
            let output = file.run(args);
            assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("escape") || stderr.contains("expected a pattern"),
                "{stderr}"
            );
            assert!(!stderr.contains("error[E"), "{stderr}");
        }
        assert_eq!(std::fs::read_to_string(&file.0).unwrap(), source);
    }
}

#[test]
fn explicit_backslashes_raw_quotations_and_character_patterns_keep_their_values() {
    let source = include_str!("differential/corpus/quoted_literal_controls.runa");
    let file = SourceFile::new(source);
    for args in [&[][..], &["run"][..]] {
        let output = file.run(args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "true\ntrue\ntrue\n2\n3\n4\n12345\ntrue\ntrue\n13"
        );
    }
}

#[test]
fn adjacent_expression_tokens_require_a_statement_separator() {
    for source in [
        "= x = 5 5\n@ print(show(x))\n",
        "> wrong() -> Int { 5 5 }\n",
        "> module Wrong { = x = 5 ignored }\n",
        "= x = 5 ignored\n",
        "? assert(True)\n",
    ] {
        let error = parse(source)
            .expect_err("trailing expression tokens must not become another statement");
        assert!(error.contains("statement separator"), "{source}: {error}");
    }
    for source in [
        "= x = 5; 5\n",
        "= x = 5\n5\n",
        "> okay() -> Int { = x = 5; x }\n",
        "> effect_then_value() -> Int { @ print(\"hello\") 5 }\n",
    ] {
        parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn embedded_rust_keeps_its_boundary_and_following_futuruna_is_validated() {
    let body = r###"
macro_rules! identity { ($value:expr) => { $value }; }
fn borrowed<'a>(value: &'a str) -> &'a str { value }
fn raw() -> &'static str { r##"} " § ---- {{"## }
fn character() -> char { '\u{007d}' }
fn rust_escapes() -> &'static str { "\r\0\u{007d}" }
/* outer } /* nested } */ still outer } */
// } § ¤ – ” are Rust comment text.
"###;
    let source = format!("@ rust {{{body}}}\n= answer = 42\n@ print(show(answer))\n");
    let statements = parse(&source).expect("Rust literals and comments keep their own grammar");
    assert_eq!(
        statements.len(),
        3,
        "Rust must not consume following Futuruna"
    );
    let futuruna::Stmt::RustBlock(code) = &statements[0] else {
        panic!("expected an embedded Rust block");
    };
    assert_eq!(code, body.trim());
    let file = SourceFile::new(&source);
    let formatted = file.run(&["fmt"]);
    assert!(formatted.status.success(), "{formatted:?}");
    assert!(std::fs::read_to_string(&file.0)
        .unwrap()
        .contains(body.trim()));
    let native = file.run(&["run"]);
    assert!(native.status.success(), "{native:?}");
    assert_eq!(String::from_utf8_lossy(&native.stdout).trim(), "42");
    let error = parse(&format!("{source}= invalid = 1 § 2\n")).unwrap_err();
    assert!(error.contains("unexpected character"), "{error}");
    assert!(error.contains('§'), "{error}");
}

#[test]
fn unclosed_embedded_rust_is_rejected_at_its_opening_brace() {
    for source in [
        "@ rust { fn unfinished() {}\n",
        "@ rust { /* never closed }",
        "@ rust { r#\"missing close }\n",
    ] {
        let error = parse(source).expect_err("the Rust block needs a matching brace");
        assert!(
            error.contains("unterminated embedded Rust block"),
            "{error}"
        );
        assert!(error.contains("1:8"), "{error}");
    }
}

#[test]
fn closed_literals_quotations_and_embedded_rust_keep_their_own_contents() {
    for source in [
        "= s = \"§ ¤ – ”\"\n@ print(s)\n",
        "= s = \"hello\nworld\"\n",
        "= s = \"\"\"\n§ 1\n{{ 2 + 3 }}\n\"\"\"\n",
        "----\n§ 1. Quoted law: ”text”.\n----\n= x = 5\n",
        "= quote = '\"'\n= slash = '\\\\'\n",
        "@ rust {\nmacro_rules! identity { ($value:expr) => { $value }; }\nfn borrowed<'a>(value: &'a str) -> &'a str { value }\n// § ¤ – ” are Rust comment text.\n}\n@ print(\"okay\")\n",
    ] {
        parse(source).unwrap_or_else(|error| panic!("valid source was rejected: {source}: {error}"));
    }
}

#[test]
fn cli_commands_reject_lexical_errors_and_formatting_leaves_source_untouched() {
    for source in [
        "@ print(\"must not run\")\n= x = 5 § 3\n@ print(show(x))\n",
        "@ print(\"must not run\")\n= s = \"hello\n",
        "@ print(\"must not run\")\n= x = 1\n----\nmissing close\n",
    ] {
        let file = SourceFile::new(source);
        let outputs = [
            &[][..],
            &["check", "--frontend"][..],
            &["check"][..],
            &["emit"][..],
            &["fmt", "--check"][..],
            &["fmt"][..],
        ]
        .into_iter()
        .map(|args| {
            let output = file.run(args);
            (args, output)
        })
        .collect::<Vec<_>>();
        let after = std::fs::read_to_string(&file.0).unwrap();
        assert_eq!(after, source, "formatting changed malformed source");
        for (args, output) in outputs {
            assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("unexpected character") || stderr.contains("unterminated"),
                "{args:?}: {stderr}"
            );
        }
    }
}
