use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    files: Vec<PathBuf>,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "futuruna-import-bodies-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Self {
            root,
            files: Vec::new(),
        }
    }

    fn write(&mut self, name: &str, source: &str) {
        let path = self.root.join(name);
        std::fs::write(&path, source).unwrap();
        if !self.files.contains(&path) {
            self.files.push(path);
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        Command::new(runa)
            .current_dir(&self.root)
            .env("NO_COLOR", "1")
            .args(args)
            .arg("main.runa")
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in &self.files {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(&self.root).unwrap();
    }
}

const BAD_BODY: &str = "-- æøå: the error belongs to this file\n@ export\n> helper(a: Int) -> Int {\n    a + missing_var\n}\n";

#[test]
fn json_diagnostics_retain_the_imported_location_and_import_site() {
    let mut fixture = Fixture::new();
    fixture.write("badtype.runa", BAD_BODY);
    fixture.write("main.runa", "@ import ./badtype\n");
    let output = fixture.run(&["check", "--json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .unwrap()
                .contains("missing_var")
        })
        .unwrap();
    assert!(diagnostic["location"]["file"]
        .as_str()
        .unwrap()
        .ends_with("badtype.runa"));
    assert_eq!(diagnostic["location"]["range"]["start"]["line"], 4);
    assert_eq!(diagnostic["location"]["range"]["start"]["column"], 9);
    assert_eq!(diagnostic["import_site"]["range"]["start"]["line"], 1);
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn runtime_import_initialization_errors_keep_their_source() {
    let mut fixture = Fixture::new();
    fixture.write(
        "broken.runa",
        "-- imported initialization\n\n= broken = head([])\n",
    );
    for import in ["@ import ./broken", "@ import Broken from ./broken"] {
        fixture.write("main.runa", &format!("{import}\n@ print(\"after\")\n"));
        let output = fixture.run(&[]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("head: empty list") && stderr.contains("broken.runa:3:"),
            "{stderr}"
        );
        assert!(!stderr.contains("panicked at"), "{stderr}");
    }
}

#[test]
fn faults_in_imported_helpers_identify_the_call_site() {
    let mut fixture = Fixture::new();
    fixture.write(
        "helper.runa",
        "\n\n\n@ export\n> broken() -> Int { head([]) }\n",
    );
    for (import, call) in [
        ("@ import ./helper", "broken()"),
        ("@ import Helper from ./helper", "Helper.broken()"),
    ] {
        fixture.write(
            "main.runa",
            &format!("{import}\n@ print(show({call}))\n@ print(\"after\")\n"),
        );
        let output = fixture.run(&[]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("head: empty list") && stderr.contains("main.runa:2:"),
            "{stderr}"
        );
        assert!(!stderr.contains("main.runa:5:"), "{stderr}");
    }
}

#[test]
fn library_diagnostics_preserve_import_origin_and_caller_anchor() {
    use futuruna::{parse_prelude, prepend_prelude, Lexer, Parser, TypeChecker};
    let mut fixture = Fixture::new();
    fixture.write("badtype.runa", BAD_BODY);
    let source = "@ import ./badtype\n@ print(show(helper(1)))\n";
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    let statements = prepend_prelude(parse_prelude(), &statements);
    let directory = Some(fixture.root.to_string_lossy().into_owned());
    let diagnostics = TypeChecker::check_with_diagnostics(&statements, directory.clone(), source);
    let diagnostic = diagnostics
        .iter()
        .find(|diag| diag.message.contains("undefined variable `missing_var`"))
        .expect("imported function body must be checked");
    let origin = diagnostic
        .origin
        .as_ref()
        .expect("retain original source location");
    assert_eq!(
        origin.path,
        std::fs::canonicalize(fixture.root.join("badtype.runa")).unwrap()
    );
    assert_eq!(origin.source.as_ref(), BAD_BODY);
    assert_eq!(origin.span.unwrap().start_line_col(&origin.source), (4, 9));
    assert_eq!(diagnostic.span.unwrap().start_line_col(source), (1, 10));
    let legacy = TypeChecker::check_with_source(&statements, directory, source);
    assert!(
        legacy
            .iter()
            .any(|error| error.contains("badtype.runa:4:9: undefined variable `missing_var`")),
        "{legacy:?}"
    );
}

#[test]
fn cyclic_plain_imports_check_each_body_without_recursing_forever() {
    let mut fixture = Fixture::new();
    fixture.write("main.runa", "@ import ./left\n@ print(show(left(0)))\n");
    fixture.write("left.runa", "@ import ./right\n> left(value: Int) -> Int { if value > 0 { right(value - 1) } else { 0 } }\n");
    fixture.write(
        "right.runa",
        "@ import ./left\n> right(value: Int) -> Int { left(value) }\n",
    );
    let valid = fixture.run(&["check", "--frontend"]);
    assert!(valid.status.success(), "{valid:?}");
    fixture.write(
        "right.runa",
        "@ import ./left\n> right(value: Int) -> Int { missing_var }\n",
    );
    let invalid = fixture.run(&["check", "--frontend"]);
    assert_eq!(invalid.status.code(), Some(1), "{invalid:?}");
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert_eq!(
        stderr.matches("undefined variable `missing_var`").count(),
        1,
        "{stderr}"
    );
    assert!(stderr.contains("right.runa:2:"), "{stderr}");
}

#[test]
fn inline_module_imports_keep_their_local_constructor_scope() {
    let mut fixture = Fixture::new();
    fixture.write("maker.runa", "> make_item() -> Item { Item(value = 42) }\n");
    fixture.write("main.runa", "> module Box {\n    # Item(value: Int)\n    @ import ./maker\n    = item = make_item()\n}\n@ print(show(Box.item))\n");
    let checked = fixture.run(&["check", "--frontend"]);
    assert!(checked.status.success(), "{checked:?}");
    let interpreted = fixture.run(&[]);
    assert!(interpreted.status.success(), "{interpreted:?}");
    assert_eq!(
        String::from_utf8_lossy(&interpreted.stdout).trim(),
        "Item(value: 42)"
    );

    fixture.write(
        "maker.runa",
        "> make_item() -> Item { Item(value = missing_var) }\n",
    );
    let invalid = fixture.run(&["check", "--frontend"]);
    assert_eq!(invalid.status.code(), Some(1), "{invalid:?}");
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(
        stderr.contains("undefined variable `missing_var`"),
        "{stderr}"
    );
    assert!(stderr.contains("maker.runa:1:"), "{stderr}");
}

#[test]
fn shared_nullary_constructor_names_do_not_inherit_the_last_imported_parent() {
    let mut fixture = Fixture::new();
    fixture.write(
        "legacy.runa",
        "# Legacy = Shared | LegacyOnly\n# Entry(kind: Legacy)\n| accepted(kind: Legacy) -> True\n= entry = Entry(kind = Shared)\n= before = accepted(entry.kind)\n",
    );
    fixture.write("current.runa", "# Current = Shared | CurrentOnly\n");
    for imports in [
        "@ import ./legacy\n@ import ./current\n",
        "@ import ./current\n@ import ./legacy\n",
    ] {
        fixture.write(
            "main.runa",
            &format!("{imports}@ print(show(before))\n@ print(show(accepted(entry.kind)))\n"),
        );
        for args in [&["check", "--frontend"][..], &[][..], &["run"][..]] {
            let output = fixture.run(args);
            assert!(output.status.success(), "{imports}, {args:?}: {output:?}");
            if args.first() != Some(&"check") {
                assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "true\ntrue");
            }
        }
    }
}

#[test]
fn imported_annotation_names_use_prepared_types_and_original_source() {
    let mut fixture = Fixture::new();
    fixture.write("types.runa", "# Input(value: Int)\n");
    fixture.write("helper.runa", "> helper(value: Input) -> Int { 1 }\n");
    fixture.write("main.runa", "@ import ./helper\n@ import ./types\n");
    let valid = fixture.run(&["check", "--frontend"]);
    assert!(valid.status.success(), "{valid:?}");

    fixture.write(
        "helper.runa",
        "-- unused function with an unknown annotation\n> helper(value: Intt) -> Int { 1 }\n",
    );
    for imports in ["@ import ./helper\n", "@ import Helper from ./helper\n"] {
        fixture.write("main.runa", imports);
        let invalid = fixture.run(&["check", "--frontend"]);
        assert_eq!(invalid.status.code(), Some(1), "{invalid:?}");
        let stderr = String::from_utf8_lossy(&invalid.stderr);
        assert!(
            stderr.contains("unknown type `Intt`") && stderr.contains("helper.runa:2:17"),
            "{stderr}"
        );
    }
}

#[test]
fn imported_operator_errors_are_checked_before_root_effects() {
    let mut fixture = Fixture::new();
    fixture.write(
        "helper.runa",
        "-- unused function still has a checked body\n> invalid(a: String, b: Int) -> Bool { a >= b }\n",
    );
    for import in ["@ import ./helper", "@ import Helper from ./helper"] {
        fixture.write(
            "main.runa",
            &format!("@ print(\"must not run\")\n{import}\n"),
        );
        for args in [
            &[][..],
            &["run"][..],
            &["check", "--frontend"][..],
            &["check"][..],
        ] {
            let output = fixture.run(args);
            assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("helper.runa:2:"), "{stderr}");
            assert!(
                stderr.contains("unsupported operands for operator `>=`"),
                "{stderr}"
            );
            assert!(
                stderr.contains("String") && stderr.contains("Int"),
                "{stderr}"
            );
        }
    }
}

fn assert_bad_import(fixture: &Fixture) {
    for args in [
        &["check", "--frontend"][..],
        &[][..],
        &["check"][..],
        &["emit"][..],
        &["run"][..],
    ] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(
            output.stdout.is_empty(),
            "invalid imported source executed: {output:?}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("undefined variable `missing_var`"),
            "{args:?}: {stderr}"
        );
        assert!(stderr.contains("badtype.runa:4:9"), "{args:?}: {stderr}");
    }
}

#[test]
fn plain_import_errors_report_the_imported_body_source() {
    let mut fixture = Fixture::new();
    fixture.write("badtype.runa", BAD_BODY);
    fixture.write(
        "main.runa",
        "@ import ./badtype\n@ print(\"must not run\")\n@ print(show(helper(1)))\n",
    );
    assert_bad_import(&fixture);
}

#[test]
fn qualified_import_errors_are_not_discarded_during_return_inference() {
    let mut fixture = Fixture::new();
    fixture.write("badtype.runa", BAD_BODY);
    fixture.write("main.runa", "@ import Model from ./badtype\n@ print(\"must not run\")\n@ print(show(Model.helper(1)))\n");
    assert_bad_import(&fixture);
}

#[test]
fn transitive_diamond_import_reports_the_original_error_once() {
    let mut fixture = Fixture::new();
    fixture.write("badtype.runa", BAD_BODY);
    fixture.write(
        "left.runa",
        "@ import ./badtype\n> left(a: Int) -> Int { helper(a) }\n",
    );
    fixture.write(
        "right.runa",
        "@ import ./badtype\n> right(a: Int) -> Int { helper(a) }\n",
    );
    fixture.write(
        "main.runa",
        "@ import ./left\n@ import ./right\n@ print(show(left(1) + right(2)))\n",
    );
    assert_bad_import(&fixture);
    let output = fixture.run(&["check", "--frontend"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("undefined variable `missing_var`").count(),
        1,
        "{stderr}"
    );
}

#[test]
fn valid_imports_keep_private_helpers_forward_references_and_builtins() {
    let mut fixture = Fixture::new();
    fixture.write("valid.runa", "@ export\n> helper(a: Int) -> Int { private_helper(a) }\n> private_helper(a: Int) -> Int { abs(-a) }\n");
    for source in [
        "@ import ./valid\n@ print(show(helper(42)))\n",
        "@ import Model from ./valid\n@ print(show(Model.helper(42)))\n",
    ] {
        fixture.write("main.runa", source);
        let checked = fixture.run(&["check", "--frontend"]);
        assert!(checked.status.success(), "{source}: {checked:?}");
        let interpreted = fixture.run(&[]);
        assert!(interpreted.status.success(), "{source}: {interpreted:?}");
        assert_eq!(String::from_utf8_lossy(&interpreted.stdout).trim(), "42");
    }
}

#[test]
fn conflicting_rule_results_across_imports_keep_the_imported_origin() {
    let mut fixture = Fixture::new();
    fixture.write(
        "main.runa",
        "| tax(income: Int) -> income / 4\n@ import ./exempt\n",
    );
    fixture.write(
        "exempt.runa",
        "| exception special tax(income: Int) -> \"exempt\" under income < 10\n",
    );
    let checked = fixture.run(&["check", "--frontend"]);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    let error = String::from_utf8_lossy(&checked.stderr);
    assert!(
        error.contains("conflicting return types")
            && error.contains("Int")
            && error.contains("String"),
        "{error}"
    );
    assert!(error.contains("exempt.runa:1:"), "{error}");
}

#[test]
fn inline_module_rule_contracts_include_their_plain_imports() {
    let mut fixture = Fixture::new();
    fixture.write(
        "main.runa",
        "> module Local {\n| tax(income: Int) -> income / 4\n@ import ./exempt\n}\n",
    );
    fixture.write(
        "exempt.runa",
        "| exception special tax(income: Int) -> \"exempt\" under income < 10\n",
    );
    let checked = fixture.run(&["check", "--frontend"]);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    let error = String::from_utf8_lossy(&checked.stderr);
    assert!(
        error.contains("conflicting return types")
            && error.contains("Int")
            && error.contains("String"),
        "{error}"
    );
    assert!(error.contains("exempt.runa:1:"), "{error}");
}

#[test]
fn alternate_import_paths_keep_each_inline_rule_namespace() {
    let mut fixture = Fixture::new();
    fixture.write("main.runa", "> module First {\n| tax(income: Int) -> \"ordinary\"\n@ import ./exempt\n}\n> module Second {\n| tax(income: Int) -> 7\n@ import ././exempt\n}\n");
    fixture.write(
        "exempt.runa",
        "| exception special tax(income: Int) -> \"exempt\" under income < 10\n",
    );
    let checked = fixture.run(&["check", "--frontend"]);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    let error = String::from_utf8_lossy(&checked.stderr);
    assert!(
        error.contains("conflicting return types")
            && error.contains("Int")
            && error.contains("String"),
        "{error}"
    );
    assert!(
        error.contains("in module `Second`") && error.contains("exempt.runa:1:"),
        "{error}"
    );
}

#[test]
fn import_anchor_ignores_the_same_path_in_earlier_quoted_prose() {
    use futuruna::{parse_prelude, prepend_prelude, Lexer, Parser, TypeChecker};
    let mut fixture = Fixture::new();
    fixture.write("badtype.runa", BAD_BODY);
    let source = "= note = \"./badtype\"\n@ import ./badtype\n";
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    let statements = prepend_prelude(parse_prelude(), &statements);
    let errors = TypeChecker::check_with_diagnostics(
        &statements,
        Some(fixture.root.to_string_lossy().into_owned()),
        source,
    );
    let error = errors
        .iter()
        .find(|error| error.message.contains("undefined variable `missing_var`"))
        .unwrap_or_else(|| panic!("missing imported-body error: {errors:?}"));
    assert_eq!(error.span.unwrap().start_line_col(source), (2, 10));
    assert_eq!(
        error
            .origin
            .as_ref()
            .unwrap()
            .span
            .unwrap()
            .start_line_col(BAD_BODY),
        (4, 9)
    );
}
