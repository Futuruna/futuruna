use futuruna::{parse_prelude, prepend_prelude, Diagnostic, Lexer, Parser, TypeChecker};

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("diagnostic fixture parses");
    let statements = prepend_prelude(parse_prelude(), &statements);
    TypeChecker::check_with_diagnostics(&statements, None, source)
}

fn position(source: &str, message: &str) -> (usize, usize) {
    let errors = diagnostics(source);
    let error = errors
        .iter()
        .find(|error| error.message.contains(message))
        .unwrap_or_else(|| panic!("missing {message}: {errors:?}"));
    error
        .span
        .expect("source diagnostic has a span")
        .start_line_col(source)
}

#[test]
fn non_exhaustive_match_points_at_match_despite_earlier_unicode_string() {
    let source = "= intro = \"ææææ øøøø: Color\"\n# Color = Red | Green | Blue\n> name(c: Color) -> String {\n    match c {\n        | Red -> \"r\"\n        | Green -> \"g\"\n    }\n}\n";
    assert_eq!(position(source, "non-exhaustive match"), (4, 5));
}

#[test]
fn interpolation_error_retains_its_line_and_unicode_column() {
    for newline in ["\n", "\r\n"] {
        let source = format!("= intro = \"\"\"{newline}first{newline}æøå{newline}🙂 {{{{unknown_value}}}}{newline}\"\"\"{newline}");
        assert_eq!(
            position(&source, "undefined variable `unknown_value`"),
            (4, 5)
        );
    }
}

#[test]
fn unary_operand_errors_point_at_the_operator_after_unicode_and_crlf() {
    for operator in ["!3", "-True"] {
        let source = format!("= intro = \"æøå ! - before\"\r\n= invalid = {operator}\r\n");
        assert_eq!(
            position(&source, "unsupported operands for operator"),
            (2, 13)
        );
    }
}

#[test]
fn constructor_arity_error_points_at_the_pattern() {
    let source = "# Pair(left: Int, right: Int)\n> value(p: Pair) -> Int {\n    match p {\n        | Pair(x) -> x\n    }\n}\n";
    assert_eq!(
        position(source, "constructor pattern `Pair` expects 2 fields"),
        (4, 11)
    );
}

#[test]
fn legacy_name_fallback_skips_strings_and_uses_character_columns() {
    let source = "= intro = \"ææææ øøøø: Color\"\n# Color = Red | Green\n";
    let mut checker = TypeChecker::new();
    checker.source_text = source.to_string();
    checker.error("problem with `Color`".to_string());
    assert_eq!(
        checker.diagnostics[0].span.unwrap().start_line_col(source),
        (2, 3)
    );
}

#[test]
fn malformed_char_arrow_and_eof_have_parse_diagnostics() {
    for (source, expected) in [
        ("= invalid = 'ab'\n", "character literal"),
        (
            "> value(x: Int) -> Int { match x { | 0 x } }\n",
            "expected `->`",
        ),
        ("> value(x: Int) -> Int {\n", "unexpected end of file"),
    ] {
        let errors = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect_err("invalid syntax must fail before name checking");
        assert!(
            format!("{errors:?}").contains(expected),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn declaration_hashes_remain_independent_of_diagnostic_positions() {
    for (expected, source) in [
        ("5cc9f6910663", "> classify(value: Bool) -> String { match value { | True -> \"yes\" | False -> \"no\" } }"),
        ("9dad2f3d0be8", "> describe(value: Int) -> String { \"\"\"value={{value}}\"\"\" }"),
    ] {
        let hash = |source: &str| {
            let statements = Parser::new(Lexer::new(source).tokenize(), source)
                .parse_program()
                .unwrap();
            let futuruna::Stmt::Defn(definition) = &statements[0] else {
                panic!("fixture is a function")
            };
            futuruna::content_hash_defn(definition)
        };
        let original = hash(source);
        assert_eq!(original, expected, "diagnostic metadata must preserve existing identities");
        assert_eq!(original, hash(&format!("-- æøå\n\n{source}\n")));
    }
}

#[test]
fn fallback_columns_count_unicode_characters_on_the_target_line() {
    let source = "= intro = \"mål\"\n= café = 1; = mål = café\n";
    let mut checker = TypeChecker::new();
    checker.source_text = source.to_string();
    checker.error("problem with `mål`".to_string());
    assert_eq!(
        checker.diagnostics[0].span.unwrap().start_line_col(source),
        (2, 15)
    );
}

#[test]
fn template_and_pattern_positions_preserve_interpreted_and_native_values() {
    let source = include_str!("differential/corpus/diagnostic_source_positions.runa");
    assert!(diagnostics(source).is_empty());
    assert_eq!(
        futuruna::eval_source_with_prelude(source, false)
            .unwrap()
            .trim(),
        "§ 2\nø 3\n5"
    );
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/diagnostic_source_positions.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "§ 2\nø 3\n5"
    );
}
