use futuruna::{eval_source_with_prelude, Interpreter, Lexer, Parser, TypeChecker};
use std::path::Path;
use std::process::Command;

fn errors(source: &str) -> Vec<String> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    TypeChecker::check_with_source(&statements, None, source)
}

#[test]
fn known_receivers_reject_nonexistent_members_and_list_method_syntax() {
    for expression in ["xs.length", "xs.len()", "xs.map(|x| x * 2)"] {
        let source = format!("= xs = [1, 2]\n@ print(show({expression}))\n");
        let errors = errors(&source);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("no field") || e.contains("no member")),
            "{source}: {errors:?}"
        );
    }
    for source in [
        "# Entry(a) = Entry(value: a)\n> value(entry: Entry(Int)) -> Int { entry.missing }\n",
        "= pair = (1, 2)\n@ print(show(pair.missing))\n",
        "= pair = (1, 2)\n@ print(show(pair.2))\n",
        "# Policy(value: Int) { | result() -> value }\n= policy = Policy(3)\n@ print(show(policy.0))\n",
    ] {
        let errors = errors(source);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("no field") || e.contains("no member")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn untyped_missing_fields_fail_at_access_instead_of_becoming_unit() {
    let source = "# Person(status: String)\n| fee(p) -> 0 under p.stauts == \"student\"\n| fee(p) -> 10\n@ print(show(fee(Person(\"student\"))))\n";
    let error = eval_source_with_prelude(source, false).unwrap_err();
    assert!(
        error.contains("stauts") && error.contains("field"),
        "{error}"
    );
}

#[test]
fn unchecked_missing_fields_stop_execution() {
    for receiver in [
        "[1, 2]",
        "(1, 2)",
        "\"hello\"",
        "Record(1)",
        "Record(value = 1)",
    ] {
        let source = format!("# Record(value: Int)\n= object = {receiver}\nobject.missing\n@ print(\"must not run\")\n");
        let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap();
        let mut interpreter = Interpreter::new();
        interpreter.suppress_output = true;
        let mut env = interpreter.default_env();
        let error = interpreter
            .run_program_with_diagnostics(&statements, &mut env, Path::new("fields.runa"), &source)
            .expect_err("a missing field cannot return unit");
        assert!(
            error.message.contains("field") && error.message.contains("missing"),
            "{error:?}"
        );
        assert!(interpreter.output.is_empty());
    }
}

#[test]
fn valid_fields_tuple_projections_and_bound_methods_are_preserved() {
    let source = r#"
# Record(value: Int) { > doubled(self) -> Int { self.value * 2 } }
# PlainRecord(value: Int)
# UnitField(value: ())
= record = Record(value = 3)
= method = record.doubled
= pair = (4, 5)
@ print(show(record.value))
@ print(show(PlainRecord(value = 3).0))
@ print(show(record.doubled()))
@ print(show(method()))
@ print(show(pair.fst))
@ print(show(pair.1))
@ print(show(UnitField(value = ()).value))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "3\n3\n6\n6\n4\n5\n()"
    );
}

#[test]
fn inherent_method_values_remain_callable() {
    let source = r#"
# Counter = Counter(value: Int) {
    > incremented(self) -> Int { self.value + 1 }
}
= counter = Counter(4)
= bound = counter.incremented
@ print(show(counter.incremented()))
@ print(show(bound()))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "5\n5"
    );
}

#[test]
fn unknown_lexical_receivers_do_not_borrow_an_outer_schema() {
    let source = r#"
# Outer(other: Int)
# Inner(value: Int)
= item = Outer(1)
> read(item) { item.value }
@ print(show(read(Inner(7))))
"#;
    assert_eq!(eval_source_with_prelude(source, false).unwrap().trim(), "7");
}

fn assert_native_field_output(path: &Path, expected: &str) {
    for native in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        if native {
            command.arg("run");
        }
        let output = command.arg(path).output().expect("execute field fixture");
        assert!(
            output.status.success(),
            "{} (native={native}): {}\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn native_loop_fields_preserve_nested_and_shadowed_owners() {
    assert_native_field_output(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/differential/corpus/loop_record_field_owners.runa"),
        "Alice\nChild\n25\nBob\nChild\n45\nCarol\nborrowed string\n6",
    );
}

#[test]
fn website_rule_example_compiles_and_matches_interpretation() {
    let website = include_str!("../website/src/main.rs");
    let source = website
        .split_once("const EXAMPLE_RULES: &str = r#\"")
        .expect("website rules example start")
        .1
        .split_once("\"#;")
        .expect("website rules example end")
        .0;
    let path = std::env::temp_dir().join(format!(
        "futuruna-website-rule-fields-{}-{}.runa",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::write(&path, source).unwrap();
    assert_native_field_output(
        &path,
        "Alice (age 25, Student): 10% tax\nBob (age 45, Employee): 25% tax\nCarol (age 72, Retired): 0% tax\nDan (age 30, Unemployed): 25% tax",
    );
    std::fs::remove_file(path).unwrap();
}
