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
fn derived_queries_preserve_negation_and_helper_equivalence() {
    assert_both_backends(
        "logic_negated_derived_search",
        "negated derived searches passed",
    );
}

#[test]
fn derived_queries_bind_every_free_position_without_cross_row_joins() {
    assert_both_backends("logic_derived_row_bindings", "derived row bindings passed");
}

#[test]
fn derived_queries_keep_local_bindings_separate_from_generated_names() {
    assert_both_backends("logic_derived_query_hygiene", "query hygiene passed");
}

#[test]
fn derived_queries_filter_repeated_arguments_before_effects() {
    assert_both_backends(
        "logic_derived_query_effects",
        "checked ann\nchecked bob\n[car, bike]\nchecked ann\n[ann]\n[]",
    );
}

#[test]
fn existential_queries_follow_derived_relations() {
    assert_both_backends(
        "logic_derived_existential",
        "derived existential queries passed",
    );
}

#[test]
fn every_conjunct_can_bind_values_with_independent_disjunction_branches() {
    assert_both_backends("logic_goal_separators", "logic goal separators passed");
}

#[test]
fn recursive_rule_queries_match_the_introductory_tutorial() {
    assert_both_backends("logic_recursive_search", "recursive query passed");
}

#[test]
fn typed_primitive_queries_enumerate_reachable_rule_values() {
    assert_both_backends("logic_typed_derived_search", "typed derived queries passed");
}

#[test]
fn ground_and_anonymous_goals_stop_after_the_first_witness() {
    assert_both_backends(
        "logic_ground_query_witnesses",
        "checked ann\n[ann]\nchecked ann\nchecked bob\n[ann, bob]",
    );
}

#[test]
fn nested_derived_queries_visit_each_distinct_row_once_in_order() {
    assert_both_backends(
        "logic_streamed_query_rows",
        "checked ann\n[ann]\nchecked ann\nchecked car\nchecked bob\nchecked bike\n[car, bike]",
    );
}

#[test]
fn rule_predicates_use_their_declared_functions_despite_caller_shadowing() {
    assert_both_backends("logic_query_function_scope", "query function scope passed");
}

#[test]
fn native_queries_reject_unchecked_declaration_value_captures() {
    let path = std::env::temp_dir().join(format!(
        "futuruna-query-declaration-capture-{}.runa",
        std::process::id()
    ));
    std::fs::write(&path, "= threshold = 0\n| item(1)\n| selected(value) -> item(value), value > threshold\n> query(threshold: Int) -> List(Int) { findall(value, selected(value)) }\n@ print(show(query(9)))\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("checked declaration environment"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn existential_continuations_stop_before_unneeded_recursive_candidates() {
    assert_both_backends(
        "logic_early_query_witness",
        "accepted 2\nearly witness passed",
    );
}

#[test]
fn incomplete_queries_fail_without_printing_partial_results() {
    for (name, source) in [
        (
            "recursive",
            "| edge(1, 2)\n| edge(2, 1)\n| reach(x, y) -> edge(x, y)\n| reach(x, y) -> edge(x, z), reach(z, y)\n@ print(show(findall(y, reach(1, y))))\n",
        ),
        (
            "typed_recursive",
            "| edge(1, 2)\n| edge(2, 1)\n| reach(x: Int, y: Int) -> edge(x, y)\n| reach(x: Int, y: Int) -> edge(x, z), reach(z, y)\n@ print(show(findall(y, reach(1, y))))\n",
        ),
        (
            "unbound",
            "| any_value(x: Int) -> True\n@ print(show(findall(x, any_value(x))))\n",
        ),
    ] {
        let path = std::env::temp_dir().join(format!(
            "futuruna-query-incomplete-{name}-{}.runa",
            std::process::id()
        ));
        std::fs::write(&path, source).unwrap();
        for native in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
            if native {
                command.arg("run");
            }
            let output = command.arg(&path).output().unwrap();
            assert!(
                !output.status.success(),
                "{name}, native={native}: query must fail"
            );
            assert!(
                output.stdout.is_empty(),
                "{name}, native={native}: no partial output"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("evaluation is incomplete"),
                "{name}, native={native}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::fs::remove_file(path).unwrap();
    }
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
