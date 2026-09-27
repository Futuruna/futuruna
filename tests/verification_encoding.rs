use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn verify(source: &str) -> Output {
    let path = std::env::temp_dir().join(format!(
        "futuruna-verify-encoding-{}-{}-{}.runa",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, source).unwrap();
    let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(runa)
        .arg("verify")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    output
}

fn assert_proved(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("PROVED: |claim|"), "{stdout}");
    } else {
        assert_eq!(output.status.code(), Some(1));
        assert!(stdout.contains("Z3 not found"), "{stdout}\n{stderr}");
        assert!(!stdout.contains("PROVED"), "{stdout}");
    }
}

#[test]
fn unicode_function_names_do_not_collide_with_encoded_looking_names() {
    let source = "> øre(x: Int) -> Int { x * 100 }\n> runa_s_c3b87265(x: Int) -> Int { x + 1 }\n= k = 2\n| claim: k -> øre(k) == 200 && runa_s_c3b87265(k) == 3\n";
    assert_proved(&verify(source));
}

#[test]
fn quotes_literal_backslashes_and_unicode_remain_string_data() {
    for (source, expected_literal) in [
        (
            r#"= s = "a\"b"
| claim: s -> s == "a\"b"
"#
            .to_string(),
            r#""a""b""#,
        ),
        (
            r#"= s = "\\u{61}"
| claim: s -> s != "a"
"#
            .to_string(),
            r#""\u{5c}u{61}""#,
        ),
        (
            r#"= s = "\\u0061"
| claim: s -> s != "a"
"#
            .to_string(),
            r#""\u{5c}u0061""#,
        ),
        (
            "= s = \"æøå😀\"\n| claim: s -> s == \"æøå😀\"\n".to_string(),
            r#""\u{e6}\u{f8}\u{e5}\u{1f600}""#,
        ),
        (
            "= s = \"a\n\t\0b\"\n| claim: s -> s == \"a\n\t\0b\"\n".to_string(),
            r#""a\u{a}\u{9}\u{0}b""#,
        ),
    ] {
        let runtime = format!("{source}? claim else {{ @ print(\"incorrect\") }}\n");
        assert!(!futuruna::eval_source_with_prelude(&runtime, true)
            .unwrap()
            .contains("incorrect"));
        let output = verify(&source);
        assert_proved(&output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(expected_literal), "{stdout}");
    }
}

#[test]
fn unicode_nominal_types_constructors_fields_and_matches_share_the_encoding() {
    let source = "# Beløb(værdi: Int)\n# Indpakning = Æske(Beløb)\n> læs(p: Indpakning) -> Int { match p { | Æske(b) -> b.værdi } }\n> mønster(p: Indpakning) -> Int { match p { | Æske(Beløb(værdi: n)) -> n } }\n= p = Æske(Beløb(værdi = 200))\n| claim: p -> læs(p) == 200 && mønster(p) == 200\n";
    assert_proved(&verify(source));
}

#[test]
fn strings_outside_the_smt_alphabet_are_explicitly_unverified() {
    let source = format!("= s = \"{}\"\n| claim: s -> s == s\n", '\u{30000}');
    let output = verify(&source);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SMT string alphabet"), "{stdout}");
    assert!(!stdout.contains("PROVED"), "{stdout}");
}
