use std::path::Path;
use std::process::Command;

#[test]
fn anonymous_logic_queries_compile_and_execute_with_independent_wildcards() {
    assert_both_backends(
        "logic_wildcard_independence",
        "anonymous logic queries passed",
    );
}

#[test]
fn derived_rule_parameters_follow_argument_positions_through_calls() {
    assert_both_backends(
        "logic_derived_parameter_types",
        "derived rule parameter types passed",
    );
}

#[test]
fn signed_and_list_rule_heads_compile_and_match_exact_values() {
    assert_both_backends(
        "logic_signed_list_heads",
        "signed and list rule heads passed",
    );
}

#[test]
fn ground_rule_alternatives_preserve_boolean_operators_and_short_circuiting() {
    assert_both_backends(
        "logic_ground_alternatives",
        "ground rule alternatives passed",
    );
}

#[test]
fn repeated_logic_variables_compare_head_and_query_positions() {
    assert_both_backends(
        "logic_repeated_variables",
        "repeated logic variables passed",
    );
}

#[test]
fn rust_keyword_rule_names_keep_valid_fact_table_names() {
    assert_both_backends(
        "logic_rust_keyword_names",
        "keyword-named logic rules passed",
    );
}

#[test]
fn native_existential_queries_reject_unproven_outer_value_capture() {
    let path = std::env::temp_dir().join(format!(
        "futuruna-native-existential-capture-{}-{}.runa",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::write(
        &path,
        "| edge(1, 2)\n= selected = 9\n| selected_edge() -> edge(selected, child), True\n@ print(show(selected_edge()))\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("check captured existential query");
    std::fs::remove_file(&path).unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "existential query capture of an outer value has no checked native binding contract"
        ),
        "{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_both_backends(name: &str, expected: &str) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/differential/corpus/{name}.runa"));
    for native in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        if native {
            command.arg("run");
        }
        let output = command
            .arg(&fixture)
            .output()
            .expect("execute rule regression");
        assert!(
            output.status.success(),
            "{name} (native={native}) failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}
