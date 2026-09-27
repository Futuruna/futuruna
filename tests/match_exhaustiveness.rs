use futuruna::{eval_source_with_prelude, Lexer, Parser, TypeChecker};

const COMPLETE: &str = include_str!("differential/corpus/match_nested_coverage.runa");

fn diagnostics(source: &str) -> Vec<String> {
    let tokens = Lexer::new(source).tokenize();
    let statements = Parser::new(tokens, source)
        .parse_program()
        .expect("parse match fixture");
    TypeChecker::check_with_diagnostics(&statements, None, source)
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn guarded_arms_do_not_establish_exhaustiveness() {
    for source in [
        "# Color = Red | Green | Blue\n> name(c: Color) -> String { match c { | Red -> \"red\" | Green -> \"green\" | Blue if False -> \"blue\" } }",
        "# Color = Red | Green | Blue\n> name(c: Color) -> Int { match c { | _ if False -> 0 } }",
        "> describe(flag: Bool) -> Int { match flag { | True -> 1 | False if False -> 0 } }",
    ] {
        let errors = diagnostics(source);
        assert!(errors.iter().any(|error| error.contains("non-exhaustive match")), "{source}: {errors:?}");
    }
}

#[test]
fn incomplete_match_diagnostic_points_to_the_match_expression() {
    let source =
        "# Color = Red | Blue\n> name(color: Color) -> Int {\n    match color { | Red -> 1 }\n}\n";
    let tokens = Lexer::new(source).tokenize();
    let statements = Parser::new(tokens, source).parse_program().unwrap();
    let errors = TypeChecker::check_with_diagnostics(&statements, None, source);
    let error = errors
        .iter()
        .find(|error| error.message.contains("non-exhaustive match"))
        .unwrap();
    let span = error.span.expect("match diagnostic has source span");
    assert_eq!(span.start, source.find("match color").unwrap());
}

#[test]
fn nested_patterns_must_cover_fields_and_their_combinations() {
    for source in [
        "# Option(a) = None | Some(a)\n> days(o: Option(Int)) -> Int { match o { | Some(0) -> 0 | None -> 0 } }",
        "# Option(a) = None | Some(a)\n> days(o: Option(Option(Int))) -> Int { match o { | Some(Some(n)) -> n | None -> 0 } }",
        "# Pairing = Together(Bool, Bool) | Apart\n> count(p: Pairing) -> Int { match p { | Together(True, True) -> 2 | Together(False, False) -> 0 | Apart -> 0 } }",
        "# Record = Entry(flag: Bool, count: Int)\n> amount(r: Record) -> Int { match r { | Entry(flag: True, count: n) -> n } }",
        "> zero(n: Int) -> Bool { match n { | 0 -> True } }",
    ] {
        let errors = diagnostics(source);
        assert!(errors.iter().any(|error| error.contains("non-exhaustive match")), "{source}: {errors:?}");
    }
}

#[test]
fn complete_nested_patterns_and_guard_fallbacks_keep_their_values() {
    let source = COMPLETE;
    assert!(diagnostics(source).is_empty());
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "2\n1\n0"
    );
}

#[test]
fn named_generic_and_recursive_patterns_keep_complete_coverage() {
    for source in [
        "# Boxed(a) = Boxed(a)\n# Inner = Inner(Int)\n> unwrap_nested(value: Boxed(Inner)) -> Int { match value { | Boxed(Inner(n)) -> n } }",
        "# Record = Entry(flag: Bool, count: Int)\n> amount(value: Record) -> Int { match value { | Entry(flag: True, count: n) -> n | Entry(flag: False, count: _) -> 0 } }",
        "# Chain(a) = End | Link(a, Chain(a))\n> first_or_zero(value: Chain(Int)) -> Int { match value { | End -> 0 | Link(n, End) -> n | Link(n, Link(_, _)) -> n } }",
        "# Tagged = Full(value: Int) | Empty\n> tag(value: Tagged) -> Int { match value { | Full -> 1 | Empty -> 0 } }",
    ] {
        let errors = diagnostics(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn native_complete_nested_matches_compile_and_keep_the_same_values() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/match_nested_coverage.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .expect("execute native match coverage fixture");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "2\n1\n0");
}

#[test]
fn implicit_method_receivers_use_their_owner_instead_of_outer_bindings() {
    for source in [
        "# Shape = Circle(Float) | Rectangle(Float, Float)\n= c = Circle(1.0)\n# Color = Red | Blue { > name(c) -> String { match c { | Red -> \"red\" | Blue -> \"blue\" } } }\n",
        "# Shape = Circle(Float) | Rectangle(Float, Float)\n= c = Circle(1.0)\n# Color = Red | Blue\n# trait Label { > name(c) -> String }\n# impl Label for Color { > name(c) -> String { match c { | Red -> \"red\" | Blue -> \"blue\" } } }\n",
        "= value = True\n# Boxed(a) = Empty | Full(a) { > is_empty(value) -> Bool { match value { | Empty -> True | Full(_) -> False } } }\n",
    ] {
        let errors = diagnostics(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn implicit_method_receivers_still_require_complete_owner_coverage() {
    for source in [
        "= c = True\n# Color = Red | Blue { > name(c) -> String { match c { | Red -> \"red\" } } }\n",
        "= c = True\n# Color = Red | Blue\n# trait Label { > name(c) -> String }\n# impl Label for Color { > name(c) -> String { match c { | Red -> \"red\" } } }\n",
    ] {
        let errors = diagnostics(source);
        assert!(
            errors.iter().any(|error| error.contains("non-exhaustive match on `Color`")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn typed_lambda_parameters_preserve_nested_match_subject_types() {
    let prefix = "# Option(a) = None | Some(a)\n# Choice = Left(Int) | Right(Int)\n# Record(value: Option(Choice))\n= item = True\n";
    let complete = format!("{prefix}= complete = |item: Record| match item.value {{ | Some(Left(_)) -> True | Some(Right(_)) -> True | None -> False }}\n");
    assert!(
        diagnostics(&complete).is_empty(),
        "{:?}",
        diagnostics(&complete)
    );
    let incomplete = format!("{prefix}= incomplete = |item: Record| match item.value {{ | Some(Left(_)) -> True | None -> False }}\n");
    assert!(diagnostics(&incomplete)
        .iter()
        .any(|error| error.contains("non-exhaustive match")));
    let missing_field = format!("{prefix}= missing = |item: Record| item.absent\n");
    assert!(diagnostics(&missing_field)
        .iter()
        .any(|error| error.contains("no field `absent`")));
}

#[test]
fn method_receiver_types_do_not_escape_or_override_explicit_parameters() {
    let source = "# Shape = Circle(Float) | Rectangle(Float, Float)\n= c = Circle(1.0)\n# Color = Red | Blue { > name(c) -> String { match c { | Red -> \"red\" | Blue -> \"blue\" } } > numeric(c: Int) -> Bool { match c { | 0 -> False | _ -> True } } }\n= shape = match c { | Circle(_) -> True | Rectangle(_, _) -> False }\n";
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
}
