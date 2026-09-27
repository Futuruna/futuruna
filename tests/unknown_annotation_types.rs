use futuruna::{parse_prelude, prepend_prelude, Diagnostic, Lexer, Parser, TypeChecker};

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("annotation fixture parses");
    let statements = prepend_prelude(parse_prelude(), &statements);
    TypeChecker::check_with_diagnostics(&statements, None, source)
}

#[test]
fn unknown_parameter_type_is_reported_at_its_annotation_without_a_call() {
    let source = "= note = \"Intt\"\n> f(a: Intt) -> Int { 1 }\n";
    let errors = diagnostics(source);
    let error = errors
        .iter()
        .find(|error| error.message.contains("unknown type `Intt`"))
        .unwrap_or_else(|| panic!("missing annotation diagnostic: {errors:?}"));
    assert_eq!(error.span.unwrap().start_line_col(source), (2, 8));
}

#[test]
fn an_earlier_value_name_does_not_capture_an_annotation_error() {
    let source = "= Intt = 1\n> f(value: Intt) -> Int { 1 }\n";
    let errors = diagnostics(source);
    let error = errors
        .iter()
        .find(|error| error.message.contains("unknown type `Intt`"))
        .unwrap();
    assert_eq!(error.span.unwrap().start_line_col(source), (2, 12));
    let span = error.span.unwrap();
    assert_eq!(span.end - span.start, 4);
}

#[test]
fn unknown_types_in_nested_and_non_function_annotations_are_rejected() {
    for source in [
        "> f(value: List(Intt)) -> Int { 1 }\n",
        "> f(value: Int) -> Intt { value }\n",
        "# Record(value: Intt)\n",
        "| tax(value: Intt) -> 1\n",
        "# Case(value: Intt) { | tax() -> 1 }\n",
        "= value: Intt = 1\n",
        "> f(value: &Intt) -> Int { 1 }\n",
        "> f(value: shared Intt) -> Int { 1 }\n",
        "> f(value: Intt?) -> Int { 1 }\n",
        "> f(value: Int -> Intt) -> Int { 1 }\n",
        "= mapper = |value: Intt| 1\n",
        "# effect Store { > save(value: Intt) -> () }\n",
        "# trait Readable { > read(self) -> Intt }\n",
    ] {
        let errors = diagnostics(source);
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("unknown type `Intt`")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn generic_and_forward_declared_types_remain_valid() {
    let source = r#"
> identity(value: a) -> a { value }
> measure(value: Later) -> Int { value.amount }
# Box(a) = Wrap(a)
> unbox(value: Box(Int)) -> Int { match value { | Wrap(item) -> item } }
# Later(amount: Int)
# Intt(value: Int)
> authored_name(value: Intt) -> Int { value.value }
"#;
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn explicit_rust_type_declarations_remain_valid() {
    let source = "@ rust { struct External; }\n> consume(value: External) -> Int { 1 }\n";
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn posthoc_type_exports_name_declarations_without_constructing_values() {
    let source = "# Chain = Empty | Link(Int, Chain)\n@ export Chain\n";
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
    let errors = diagnostics(&format!("{source}@ export Missing\n"));
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("undefined constructor `Missing`")),
        "{errors:?}"
    );
}

#[test]
fn rust_uses_aliases_and_qualified_types_remain_backend_checked() {
    for source in [
        "@ use std::time::Duration\n> consume(value: Duration) -> Int { 1 }\n",
        "@ use std::collections::{HashMap, BTreeMap}\n> consume(value: HashMap(String, Int)) -> Int { 1 }\n",
        "@ rust { use std::time::Duration as Elapsed; }\n> consume(value: Elapsed) -> Int { 1 }\n",
        "@ rust { enum External { Empty } type Alias = External; }\n> consume(value: Alias) -> Int { 1 }\n",
        "> consume(value: std::time::Duration) -> Int { 1 }\n",
        "@ use std::time::*\n> consume(value: Duration) -> Int { 1 }\n",
    ] {
        let errors = diagnostics(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
    let errors = diagnostics("@ rust { struct External; }\n> consume(value: Intt) -> Int { 1 }\n");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("unknown type `Intt`")),
        "{errors:?}"
    );
}

#[test]
fn annotation_names_follow_inline_type_and_rust_scopes() {
    let valid = "> module Local {\n@ rust { struct External; }\n# Item(value: Int)\n> consume(value: External, item: Item) -> Int { 1 }\n}\n";
    assert!(diagnostics(valid).is_empty());
    for annotation in ["Item", "External"] {
        let source = format!("{valid}> outside(value: {annotation}) -> Int {{ 1 }}\n");
        let errors = diagnostics(&source);
        assert!(
            errors.iter().any(|error| error
                .message
                .contains(&format!("unknown type `{annotation}`"))),
            "{errors:?}"
        );
    }
}

#[test]
fn native_collection_annotations_and_explicit_type_parameters_remain_valid() {
    let source = "# Box(T) = Wrap(T)\n> ignore(value: Box(Int), count: Nat, lookup: Map(String, Int), items: Set(Int)) -> Int { 1 }\n";
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
    let errors = diagnostics(&format!("{source}> invalid(value: T) -> Int {{ 1 }}\n"));
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("unknown type `T`")),
        "{errors:?}"
    );
}

#[test]
fn cli_reports_unknown_annotations_before_execution_or_rust_generation() {
    let root = std::env::temp_dir().join(format!(
        "futuruna-annotation-types-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let main = root.join("main.runa");
    std::fs::write(&main, "> f(a: Intt) -> Int { 1 }\n@ print(show(f(1)))\n").unwrap();
    let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    for args in [
        &[][..],
        &["check", "--frontend"][..],
        &["check"][..],
        &["emit"][..],
        &["run"][..],
    ] {
        let output = std::process::Command::new(&runa)
            .args(args)
            .arg(&main)
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unknown type `Intt`") && stderr.contains("main.runa:1:8"),
            "{stderr}"
        );
        assert!(
            !stderr.contains("E0425") && !stderr.contains("check.rs"),
            "{stderr}"
        );
    }
    std::fs::remove_file(main).unwrap();
    std::fs::remove_dir(root).unwrap();
}
