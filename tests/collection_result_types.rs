use futuruna::{eval_source_with_prelude, Lexer, Parser, TypeChecker};

const VALID: &str = include_str!("differential/corpus/collection_result_types.runa");
const OUTPUT: &str = "42\n7\n2\n0\n9\n2\ntwo\n7\ntwo\nmissing\ntrue\nfalse\nyes\nno\ntrue\n42\ntrue\nknown\nkey once\ntwo\n42\n42";

fn diagnostics(source: &str) -> Vec<String> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap_or_else(|error| panic!("parse {source}: {error}"));
    TypeChecker::check_with_diagnostics(&statements, None, source)
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn constructed_maps_and_find_results_support_elvis_defaults() {
    assert_eq!(
        eval_source_with_prelude(VALID, false).unwrap().trim(),
        OUTPUT
    );
}

#[test]
fn native_collection_defaults_compile_and_keep_the_same_values() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/collection_result_types.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .expect("native collection result fixture");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), OUTPUT);
}

#[test]
fn collection_elements_preserve_generics_and_nested_type_wrappers() {
    for source in [
        "> lookup_or(values: Map(k, v), key: k, fallback: v) -> v { map_get(values, key) ?: fallback }",
        "> lookup_created(key: k, value: v) -> v { map_get(map_insert(map_new(), key, value), key) ?: value }",
        "> first_present(values: List(Int?)) -> Int? { find(values, |x| True) ?: None }",
        "> first_borrowed(values: List(&Int)) -> &Int { head(values) }",
        "> first_callback(values: List(Int -> Int)) -> (Int -> Int) { head(values) }",
        "> first_unit(values: List(())) -> () { head(values) }",
        "> first_generic(values: List(a), fallback: a) -> a { find(values, |x| True) ?: fallback }",
    ] {
        let errors = diagnostics(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn incompatible_defaults_are_rejected_with_concrete_element_types() {
    for source in [
        "= entries = map_insert(map_new(), \"known\", 1)\n= wrong = map_get(entries, \"missing\") ?: \"wrong\"",
        "= wrong = find([1, 2], |x| x > 5) ?: \"wrong\"",
    ] {
        let errors = diagnostics(source);
        assert!(errors.iter().any(|error| error.contains("match arms must have the same type") && error.contains("Int") && error.contains("String")), "{source}: {errors:?}");
    }
}

#[test]
fn authored_functions_keep_priority_over_collection_intrinsic_typing() {
    let source = r#"
> map_insert(value: Int, key: Int, entry: Int) -> String { "authored" }
> authored() -> String { map_insert(1, 2, 3) }
@ print(authored())
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "authored"
    );
}
