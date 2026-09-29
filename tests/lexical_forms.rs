use futuruna::{eval_source_with_prelude, Lexer, Parser};
use std::process::Command;

fn parse(source: &str) -> Result<Vec<futuruna::Stmt>, String> {
    Parser::new(Lexer::new(source).tokenize(), source).parse_program()
}

fn output(source: &str) -> String {
    eval_source_with_prelude(source, false)
        .unwrap_or_else(|error| panic!("{source}\n{error}"))
        .trim()
        .to_string()
}

fn assert_rejected(cases: &[(&str, &str, &str)]) {
    let mut wrong = Vec::new();
    for (source, location, message) in cases {
        match parse(source) {
            Ok(_) => wrong.push(format!("{source}\nunexpectedly parsed")),
            Err(error) if !error.contains(location) || !error.contains(message) => wrong.push(
                format!("{source}\nexpected {location} {message:?}: {error}"),
            ),
            Err(_) => {}
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

#[test]
fn dash_banners_of_every_length_enclose_their_text() {
    for length in 3..=12 {
        let banner = "-".repeat(length);
        let english = format!("{banner}\n-- Tax law\n{banner}\n= x = 5\n@ print(show(x))\n");
        assert_eq!(output(&english), "5", "{length} dashes");
        let danish =
            format!("{banner}\n-- Skattelov\n{banner}\n@ sprog da\n= x = 5\n@ skriv(vis(x))\n");
        assert_eq!(output(&danish), "5", "{length} dashes in Danish mode");
    }
}

#[test]
fn a_dash_run_is_one_delimiter_whatever_its_length() {
    for length in 4..=12 {
        let banner = "-".repeat(length);
        let closing = "-".repeat(16 - length);
        let source = format!(
            "{banner}\n@ print(\"quoted\")\n{closing}\n@ print(\"code\")\n---- inline {banner} = after = 1\n"
        );
        assert_eq!(output(&source), "code", "{length} dashes");
        let unclosed = format!("= x = 1\n{banner}\n= y = 2\n");
        let error = parse(&unclosed).expect_err("an unclosed banner must be rejected");
        assert!(
            error.contains("2:1: unterminated block comment"),
            "{length} dashes: {error}"
        );
    }
}

#[test]
fn numbers_that_read_as_dates_or_grouped_digits_are_located_errors() {
    assert_rejected(&[
        ("= deadline = 2024-01-31\n", "1:14:", "not a date"),
        ("| late(d) -> d > 2024-12-15\n", "1:18:", "not a date"),
        ("@ print(show(late(2024/2/1)))\n", "1:19:", "not a date"),
        ("= deadline = 31-01-2024\n", "1:14:", "not a date"),
        ("= deadline = 31/12/2024\n", "1:14:", "not a date"),
        ("= c = [1,000, 2]\n", "1:8:", "write `1_000`"),
        ("= c = f(12,500)\n", "1:9:", "write `12_500`"),
        ("= c = 1,000,000\n", "1:7:", "write `1_000_000`"),
        ("= c = 1.000.000\n", "1:7:", "more than one `.`"),
        ("= c = 007\n", "1:7:", "write `7`"),
        ("= c = 00.5\n", "1:7:", "write `0.5`"),
        ("= c = 1__000\n", "1:7:", "between two digits"),
        ("= c = 1000_\n", "1:7:", "between two digits"),
    ]);
}

#[test]
fn digit_groups_and_similar_valid_spellings_keep_their_value() {
    let source = "\
> pair(a: Int, b: Int) -> Int { a * 1000 + b }
= grouped = 1_000_000
= fraction = 0.000_25
= spaced = 2024 - 1 - 31
= short = pair(1,50)
= listed = [1, 500, 2]
= _000 = 3
@ print(show(grouped + 1))
@ print(show(fraction * 4.0))
@ print(show(spaced))
@ print(show(short))
@ print(show(listed))
@ print(show(_000 + 0))
";
    assert_eq!(output(source), "1000001\n0.001\n1992\n1050\n[1, 500, 2]\n3");
}

#[test]
fn an_indented_line_starting_with_an_operator_is_rejected() {
    assert_rejected(&[
        (
            "= net = 100\n    - 30\n",
            "2:5:",
            "an indented line starting with `-` does not continue",
        ),
        ("= ok = True\n    || False\n", "2:5:", "starting with `||`"),
        (
            "> net() -> Int {\n    = gross = 100\n        - 30\n    gross\n}\n",
            "3:9:",
            "starting with `-`",
        ),
        (
            "> net() -> Int {\n    = gross = 100\n        - 30\n}\n",
            "3:9:",
            "starting with `-`",
        ),
        (
            "= total = max(\n    1, 2)\n    - 3\n",
            "3:5:",
            "starting with `-`",
        ),
    ]);
}

#[test]
fn continuation_operators_and_same_indentation_negation_keep_their_meaning() {
    let source = "\
= trailing = 100 -
    30
= added = 100
    + 30
= both = True
    && False
= grouped = (
    100
    - 30
)
> negate(x: Int) -> Int {
    = y = x
    -y
}
@ print(show(trailing))
@ print(show(added))
@ print(show(both))
@ print(show(grouped))
@ print(show(negate(3)))
";
    assert_eq!(output(source), "70\n130\nfalse\n70\n-3");
}

#[test]
fn compiled_programs_read_banners_and_digit_groups_like_the_interpreter() {
    let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let directory = std::env::temp_dir().join(format!(
        "futuruna-lexical-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let input = directory.join("banner.runa");
    let source = "\
-----
-- Tax law
---------
= base = 1_000_000
= rate = 0.000_5
@ print(show(base + 5))
@ print(show(rate * 2.0))
";
    std::fs::write(&input, source).unwrap();
    let compiled = Command::new(&runa)
        .arg("run")
        .arg(&input)
        .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
        .output()
        .unwrap();
    assert!(compiled.status.success(), "{compiled:?}");
    assert_eq!(
        String::from_utf8_lossy(&compiled.stdout).trim(),
        output(source)
    );
    assert_eq!(output(source), "1000005\n0.001");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn module_paths_and_hashes_keep_digit_names() {
    for (source, expected) in [
        (
            "@ import ./kapitel-04-omregning\n",
            "./kapitel-04-omregning",
        ),
        ("@ import ../love/2024-01-05\n", "../love/2024-01-05"),
    ] {
        let statements = parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(
            matches!(statements.as_slice(), [futuruna::Stmt::Import(path)] if path == expected),
            "{source}: {statements:?}"
        );
    }
    let hashed = parse("@ import #007f from ./rates\n").unwrap();
    assert!(
        matches!(hashed.as_slice(), [futuruna::Stmt::HashImport(hash, path)] if hash == "007f" && path == "./rates"),
        "{hashed:?}"
    );
}
