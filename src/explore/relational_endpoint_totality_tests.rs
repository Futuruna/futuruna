use super::{
    RelationalAnalysisPlan, RelationalAnalysisPlanRoot, RelationalEndpointRole,
    RelationalEndpointTotalityCertificateId, RelationalEndpointTotalityIssue,
    RelationalEndpointTotalityIssueReason,
};
use crate::{
    parse_prelude, prepend_prelude, CheckedExploreAnalysisIdentity, CheckedExploreQueryAccessError,
    CheckedExploreQueryArtifactIssue, Lexer, Parser, TypeCheckArtifacts, TypeChecker,
};
use std::{fs, path::Path};

fn artifacts(source: &str) -> TypeCheckArtifacts {
    let mut lexer = Lexer::new(source);
    let statements = Parser::new(lexer.tokenize(), source)
        .parse_program()
        .expect("parse endpoint-totality fixture");
    TypeChecker::check_with_artifacts(&statements, None, source)
}

fn checked_plan_identity(
    source: &str,
) -> (
    RelationalEndpointTotalityCertificateId,
    RelationalAnalysisPlanRoot,
) {
    let artifacts = artifacts(source);
    assert!(
        artifacts.diagnostics.is_empty(),
        "unexpected endpoint-totality diagnostics: {:?}",
        artifacts.diagnostics
    );
    let checked = artifacts
        .checked_exploration_query(0)
        .expect("endpoint-total query must mint a checked artifact");
    let certificate_id = checked
        .analysis_nodes()
        .find_map(|(_, identity)| match identity {
            CheckedExploreAnalysisIdentity::Mechanisms {
                endpoint_totality, ..
            } => {
                endpoint_totality
                    .validate_identity()
                    .expect("endpoint-totality certificate identity");
                assert!(
                    endpoint_totality.obligation_count().get() > 0,
                    "a non-vacuous endpoint observer must retain proof obligations"
                );
                Some(endpoint_totality.certificate_id())
            }
            CheckedExploreAnalysisIdentity::View { .. } => None,
        })
        .expect("checked mechanism endpoint-totality certificate");
    let plan = RelationalAnalysisPlan::from_checked(&checked)
        .expect("endpoint-totality authorization must admit analysis planning");
    assert!(plan.validate_root(), "analysis-plan identity must validate");
    (certificate_id, plan.root())
}

fn endpoint_issue_before_plan(source: &str) -> RelationalEndpointTotalityIssue {
    let artifacts = artifacts(source);
    assert!(
        artifacts.diagnostics.is_empty(),
        "the source language must accept the fixture before endpoint proof: {:?}",
        artifacts.diagnostics
    );
    match artifacts.checked_exploration_query(0) {
        Err(CheckedExploreQueryAccessError::Producer(
            CheckedExploreQueryArtifactIssue::EndpointTotality(issue),
        )) => issue,
        Err(other) => panic!("unexpected checked-query rejection: {other:?}"),
        Ok(_) => panic!("an unproved endpoint observer must fail before plan construction"),
    }
}

#[test]
fn guarded_transitive_integer_division_certifies_and_plans_deterministically() {
    let source = r#"
> endpoint_test_divide(numerator: Int, denominator: Int) -> Int {
    numerator / denominator
}

> endpoint_test_guarded_observer(state: Int, context: Unit) -> Int {
    if state <= 0 {
        0
    } else {
        endpoint_test_divide(12, state)
    }
}

? explore guarded_transitive_division {
    from {
        vary before in [0, 1, 2]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_guarded_observer
}
"#;

    let first = checked_plan_identity(source);
    let independently_checked = checked_plan_identity(source);
    assert_eq!(
        first, independently_checked,
        "identical checked source must mint identical certificate and plan identities"
    );
}

#[test]
fn deep_acyclic_endpoint_helper_chain_certifies_on_the_default_stack() {
    const DEPTH: usize = 192;

    let mut source = String::from("> endpoint_test_deep_0(value: Int) -> Int { value }\n");
    for index in 1..=DEPTH {
        let previous = index - 1;
        source.push_str(&format!(
            "> endpoint_test_deep_{index}(value: Int) -> Int {{ endpoint_test_deep_{previous}(value) }}\n"
        ));
    }
    source.push_str(&format!(
        "> endpoint_test_deep_observer(state: Int, context: Unit) -> Int {{ endpoint_test_deep_{DEPTH}(state) }}\n"
    ));
    source.push_str(
        r#"
? explore deep_acyclic_endpoint_helpers {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_deep_observer
}
"#,
    );

    checked_plan_identity(&source);
}

#[test]
fn acyclic_endpoint_helper_depth_exhaustion_is_a_typed_capacity_refusal() {
    const DEPTH: usize = 300;

    let mut source = String::from("> endpoint_test_too_deep_0(value: Int) -> Int { value }\n");
    for index in 1..=DEPTH {
        let previous = index - 1;
        source.push_str(&format!(
            "> endpoint_test_too_deep_{index}(value: Int) -> Int {{ endpoint_test_too_deep_{previous}(value) }}\n"
        ));
    }
    source.push_str(&format!(
        "> endpoint_test_too_deep_observer(state: Int, context: Unit) -> Int {{ endpoint_test_too_deep_{DEPTH}(state) }}\n"
    ));
    source.push_str(
        r#"
? explore too_deep_acyclic_endpoint_helpers {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_too_deep_observer
}
"#,
    );

    let issue = endpoint_issue_before_plan(&source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded
    );
}

#[test]
fn endpoint_equality_honors_the_canonical_runtime_node_limit_before_planning() {
    fn source_with_list_length(length: usize) -> String {
        let values = std::iter::repeat_n("0", length)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"
> endpoint_test_equality_limit_observer(state: Int, context: Unit) -> Bool {{
    [{values}] == [{values}]
}}

? explore equality_runtime_node_limit {{
    from {{
        vary before in [1]
        given context = ()
    }}
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_equality_limit_observer
}}
"#
        )
    }

    // Canonical replay lowers a List to Nil plus one Cons and one scalar value
    // per item: 255 items need 511 runtime nodes.
    checked_plan_identity(&source_with_list_length(255));

    // The next item needs 513 nodes and would trip the evaluator's 512-node
    // structural-equality guard.
    let issue = endpoint_issue_before_plan(&source_with_list_length(256));
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::ProofCapacityExceeded
    );
    assert!(issue.detail().contains("canonical replay limit is 512"));
}

#[test]
fn endpoint_totality_sort_by_skips_the_callback_for_an_exact_empty_list() {
    let source = r#"
= endpoint_test_empty_ints: List(Int) = []

> endpoint_test_unreachable_sort_key(value: Int) -> Int {
    1 / (value - value)
}

> endpoint_test_empty_sort_observer(state: Int, context: Unit) -> Int {
    length(sort_by(endpoint_test_empty_ints, endpoint_test_unreachable_sort_key))
}

? explore empty_sort_callback_is_unreachable {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_empty_sort_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn endpoint_totality_sort_by_proves_the_callback_on_each_reachable_exact_item() {
    let source = r#"
> endpoint_test_nonempty_sort_observer(state: Int, context: Unit) -> Int {
    length(sort_by([state, state + 1], |value: Int| 1 / (value - value)))
}

? explore nonempty_sort_callback_is_reachable {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_nonempty_sort_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded
    );
}

#[test]
fn endpoint_totality_sort_by_preserves_exact_runtime_key_order() {
    let source = r#"
> endpoint_test_exact_sort_observer(state: Int, context: Unit) -> Int {
    1 / head(sort_by([1, 0], |value: Int| value))
}

? explore exact_sort_order_reaches_zero {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_exact_sort_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded
    );
}

#[test]
fn endpoint_totality_find_skips_the_callback_for_an_exact_empty_list() {
    let source = r#"
= endpoint_test_empty_find_ints: List(Int) = []

> endpoint_test_unreachable_find_predicate(value: Int) -> Bool {
    1 / (value - value) > 0
}

> endpoint_test_empty_find_observer(state: Int, context: Unit) -> Bool {
    match find(endpoint_test_empty_find_ints, endpoint_test_unreachable_find_predicate) {
        | None -> True
        | Some(_) -> False
    }
}

? explore empty_find_callback_is_unreachable {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_empty_find_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn endpoint_totality_find_proves_the_callback_on_reachable_items() {
    let source = r#"
> endpoint_test_reachable_find_observer(state: Int, context: Unit) -> Bool {
    match find([state], |value: Int| 1 / (value - value) > 0) {
        | None -> False
        | Some(_) -> True
    }
}

? explore nonempty_find_callback_is_reachable {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_reachable_find_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded
    );
}

#[test]
fn distinct_nullary_variants_keep_filter_length_and_nested_guards_exact() {
    let source = r#"
# EndpointTestCommune = EndpointTestCopenhagen | EndpointTestAarhus | EndpointTestOdense
# EndpointTestMunicipalRow = EndpointTestMunicipalRow(kommune: EndpointTestCommune, rate: Int)

= endpoint_test_municipal_rows: List(EndpointTestMunicipalRow) = [
    EndpointTestMunicipalRow(kommune = EndpointTestCopenhagen, rate = 1),
    EndpointTestMunicipalRow(kommune = EndpointTestAarhus, rate = 2),
    EndpointTestMunicipalRow(kommune = EndpointTestOdense, rate = 3)
]

| endpoint_test_municipal_candidates(kommune: EndpointTestCommune) -> filter(endpoint_test_municipal_rows, |row: EndpointTestMunicipalRow| row.kommune == kommune)
| endpoint_test_municipality_supported(kommune: EndpointTestCommune) -> length(endpoint_test_municipal_candidates(kommune)) == 1
| endpoint_test_municipal_source(kommune: EndpointTestCommune) -> 7 under endpoint_test_municipality_supported(kommune)
| endpoint_test_municipal_result(kommune: EndpointTestCommune) -> endpoint_test_municipal_source(kommune) under endpoint_test_municipality_supported(kommune)

> endpoint_test_municipal_observer(state: Int, context: Unit) -> Int {
    endpoint_test_municipal_result(EndpointTestCopenhagen)
}

? explore exact_municipal_filter_supports_nested_guard {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_municipal_observer
}
"#;

    checked_plan_identity(source);
}

fn flat_map_observer_source(expression: &str) -> String {
    format!(
        r#"
> endpoint_test_flat_map_observer(state: Int, context: Unit) -> Int {{
    {expression}
}}
? explore flat_map_endpoint {{
    from {{
        vary before in range(0, 3)
        given context = ()
    }}
    transition after = before + 1
    find cases = all
    mechanisms paths from find cases using endpoint_test_flat_map_observer
}}
"#
    )
}

#[test]
fn endpoint_totality_flat_map_accepts_bounded_callback_and_source_summaries() {
    for expression in [
        // Exact source, optional callback output (the canonical tax-model gap).
        "length(flat_map([state, state + 1], |value: Int| if value > 0 { [value] } else { [] }))",
        // Both source and callback vary in length.
        "length(flat_map(filter([state, state + 1], |value: Int| value > 0), |value: Int| if value > 1 { [value, value + 1] } else { [] }))",
        // Preserve a nonzero lower length and the output element domain.
        "10 / head(flat_map([state, state + 1], |value: Int| if value > 1 { [1, 2] } else { [3] }))",
        // A summary with guaranteed members can also preserve nonemptiness.
        "10 / head(flat_map(if state > 0 { [1, 2] } else { [1] }, |value: Int| [value]))",
        // Existing exact ordering must not be widened unnecessarily.
        "10 / head(flat_map([1, 0], |value: Int| [value]))",
        // A definitely empty source must not execute its partial callback.
        "length(flat_map(range(0, 0), |value: Int| [1 / 0]))",
        // A nonempty summary whose callback always returns [] is empty.
        "length(flat_map(if state > 0 { [0, 1] } else { [0] }, |value: Int| filter([value], |item: Int| False)))",
    ] {
        let source = flat_map_observer_source(expression);
        assert_eq!(checked_plan_identity(&source), checked_plan_identity(&source));
    }
}

#[test]
fn endpoint_totality_flat_map_does_not_hide_possible_errors_or_invent_nonemptiness() {
    for (expression, reason) in [
        (
            "head(flat_map([state], |value: Int| if value > 0 { [value] } else { [] }))",
            RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
        ),
        (
            "head(flat_map(filter([state], |value: Int| value > 0), |value: Int| [1]))",
            RelationalEndpointTotalityIssueReason::NonExhaustivePattern,
        ),
        (
            "length(flat_map(if state > 0 { [0, 1] } else { [] }, |value: Int| [10 / value]))",
            RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded,
        ),
        (
            "10 / head(flat_map([0, 1], |value: Int| [value]))",
            RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded,
        ),
        (
            "length(flat_map(range(0, 4097), |value: Int| [value]))",
            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
        ),
        (
            "length(flat_map(range(0, 4097), |value: Int| filter([value], |item: Int| False)))",
            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
        ),
        (
            "length(flat_map([state, state + 1], |value: Int| if value > 0 { range(0, 3000) } else { [] }))",
            RelationalEndpointTotalityIssueReason::ProofCapacityExceeded,
        ),
    ] {
        let source = flat_map_observer_source(expression);
        let issue = endpoint_issue_before_plan(&source);
        assert_eq!(issue.reason(), reason, "{expression}: {issue:?}");
    }
}

#[test]
fn endpoint_totality_attributes_callback_failure_to_the_callback_argument() {
    let source = r#"
# effect EndpointTestCallbackEffect {
    > endpoint_test_callback_emit(value: Int) -> Bool
}

> endpoint_test_effectful_callback(value: Int) -> Bool with EndpointTestCallbackEffect {
    endpoint_test_callback_emit(value)
}

> endpoint_test_callback_site_observer(state: Int, context: Unit) -> Bool {
    any([state], endpoint_test_effectful_callback)
}

? explore callback_failure_has_exact_site {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_callback_site_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::EffectfulCall
    );
    assert!(
        issue.site().ast_path.ends_with(&[2]),
        "callback failure must point at the second application argument: {:?}",
        issue.site()
    );
}

#[test]
fn endpoint_totality_cached_base_call_does_not_hide_reachable_recursion() {
    let source = r#"
> endpoint_test_warm_then_recurse(value: Int) -> Int {
    if value == 0 {
        1
    } else {
        endpoint_test_warm_then_recurse(0)
    }
}

> endpoint_test_warm_recursive_observer(state: Int, context: Unit) -> Int {
    endpoint_test_warm_then_recurse(0) + endpoint_test_warm_then_recurse(state)
}

? explore cached_base_does_not_authorize_recursion {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_warm_recursive_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::RecursiveCall
    );
}

#[test]
fn endpoint_totality_false_clause_does_not_close_an_unrelated_dispatch_residual() {
    let source = r#"
> endpoint_test_false() -> Bool {
    False
}

| endpoint_test_partial_false(0) -> endpoint_test_false()

> endpoint_test_partial_false_observer(state: Int, context: Unit) -> Bool {
    endpoint_test_partial_false(state)
}

? explore false_clause_does_not_close_unrelated_residual {
    from {
        vary before in [0, 1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_partial_false_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::PartialRuleDispatch
    );
}

#[test]
fn endpoint_totality_reached_false_clause_closes_its_dispatch_domain() {
    let source = r#"
> endpoint_test_reached_false() -> Bool {
    False
}

| endpoint_test_false_at_zero(0) -> endpoint_test_reached_false()

> endpoint_test_reached_false_observer(state: Int, context: Unit) -> Bool {
    endpoint_test_false_at_zero(0)
}

? explore reached_false_clause_closes_its_domain {
    from {
        vary before in [0, 1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_reached_false_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn unguarded_transitive_integer_division_is_rejected_before_planning() {
    let source = r#"
> endpoint_test_divide(numerator: Int, denominator: Int) -> Int {
    numerator / denominator
}

> endpoint_test_unguarded_observer(state: Int, context: Unit) -> Int {
    endpoint_test_divide(12, state)
}

? explore unguarded_transitive_division {
    from {
        vary before in [0, 1, 2]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_unguarded_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded
    );
}

#[test]
fn integer_overflow_is_rejected_before_planning() {
    let source = r#"
> endpoint_test_overflowing_observer(state: Int, context: Unit) -> Int {
    state + 1
}

? explore overflowing_endpoint_observer {
    from {
        vary before in [9223372036854775807]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_overflowing_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded
    );
}

#[test]
fn bounded_sum_list_certifies_exact_integer_inputs() {
    let source = r#"
> endpoint_test_sum_list_observer(state: Int, context: Unit) -> Int {
    sum_list([state, 2, -1])
}

? explore bounded_sum_list_endpoint {
    from {
        vary before in [1, 2]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_sum_list_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn overflowing_sum_list_is_rejected_before_planning() {
    let source = r#"
> endpoint_test_overflowing_sum_list_observer(state: Int, context: Unit) -> Int {
    sum_list([state, 1])
}

? explore overflowing_sum_list_endpoint {
    from {
        vary before in [9223372036854775807]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_overflowing_sum_list_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::ArithmeticOverflowNotExcluded
    );
}

#[test]
fn structural_unit_equality_cannot_prune_a_runtime_partial_branch() {
    let source = r#"
> endpoint_test_unit_contains_observer(state: Int, context: Unit) -> Int {
    if contains([()], ()) {
        0
    } else {
        1 / (state - state)
    }
}

? explore structural_unit_equality_runtime_parity {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_unit_contains_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::DivisionByZeroNotExcluded
    );
}

#[test]
fn partial_value_rule_rejects_while_an_irrefutable_fallback_certifies() {
    let partial = r#"
| endpoint_test_partial_value(value: Int) -> 9 under value > 0

> endpoint_test_partial_rule_observer(state: Int, context: Unit) -> Int {
    endpoint_test_partial_value(state)
}

? explore partial_value_rule {
    from {
        vary before in [0, 1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_partial_rule_observer
}
"#;
    let issue = endpoint_issue_before_plan(partial);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::PartialRuleDispatch
    );

    let total = r#"
| endpoint_test_total_value(value: Int) -> 9 under value > 0
| endpoint_test_total_value(value: Int) -> 0

> endpoint_test_total_rule_observer(state: Int, context: Unit) -> Int {
    endpoint_test_total_value(state)
}

? explore total_value_rule {
    from {
        vary before in [0, 1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_total_rule_observer
}
"#;
    checked_plan_identity(total);
}

#[test]
fn finite_nullary_constructor_guards_close_exact_dispatch_domain() {
    let source = r#"
# EndpointTestAge = EndpointTestAdult | EndpointTestChild
# EndpointTestProfile(age: EndpointTestAge)

| endpoint_test_age_amount(status: EndpointTestAge) -> 1 under status == EndpointTestAdult
| endpoint_test_age_amount(status: EndpointTestAge) -> 2 under status == EndpointTestChild

> endpoint_test_age_observer(state: EndpointTestProfile, context: Unit) -> Int {
    endpoint_test_age_amount(state.age)
}

? explore finite_constructor_dispatch_domain {
    from {
        vary before in values(EndpointTestProfile)
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_age_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn missing_nullary_constructor_guard_retains_partial_dispatch() {
    let source = r#"
# EndpointTestAge = EndpointTestAdult | EndpointTestChild
# EndpointTestProfile(age: EndpointTestAge)

| endpoint_test_partial_age_amount(status: EndpointTestAge) -> 1 under status == EndpointTestAdult

> endpoint_test_partial_age_observer(state: EndpointTestProfile, context: Unit) -> Int {
    endpoint_test_partial_age_amount(state.age)
}

? explore partial_constructor_dispatch_domain {
    from {
        vary before in values(EndpointTestProfile)
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_partial_age_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::PartialRuleDispatch
    );
}

#[test]
fn complementary_modulo_guards_close_the_ordered_dispatch_residual() {
    let source = r#"
| endpoint_test_modulo_partition(value: Int) -> 0 under value % 100 == 0
| endpoint_test_modulo_partition(value: Int) -> 1 under value % 100 != 0

> endpoint_test_modulo_partition_observer(state: Int, context: Unit) -> Int {
    endpoint_test_modulo_partition(state)
}

? explore complementary_modulo_partition {
    from {
        vary before in range(0, 201)
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_modulo_partition_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn reversed_complementary_inequality_guards_close_the_ordered_dispatch_residual() {
    let source = r#"
| endpoint_test_order_partition(value: Int) -> 0 under value <= 0
| endpoint_test_order_partition(value: Int) -> 1 under 0 < value

> endpoint_test_order_partition_observer(state: Int, context: Unit) -> Int {
    endpoint_test_order_partition(state)
}

? explore complementary_order_partition {
    from {
        vary before in [-1, 0, 1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_order_partition_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn non_covering_modulo_guards_leave_a_partial_dispatch_residual() {
    let source = r#"
| endpoint_test_incomplete_modulo(value: Int) -> 0 under value % 100 == 0
| endpoint_test_incomplete_modulo(value: Int) -> 1 under value % 100 == 1

> endpoint_test_incomplete_modulo_observer(state: Int, context: Unit) -> Int {
    endpoint_test_incomplete_modulo(state)
}

? explore incomplete_modulo_partition {
    from {
        vary before in range(0, 201)
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_incomplete_modulo_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::PartialRuleDispatch
    );
}

#[test]
fn reversed_string_concatenation_guards_do_not_form_a_complementary_partition() {
    let source = r#"
# EndpointConcatTriple(x: String, y: String, z: String)

| endpoint_test_concat_partition(value: EndpointConcatTriple) -> 1 under value.x + value.y == value.z
| endpoint_test_concat_partition(value: EndpointConcatTriple) -> 2 under value.y + value.x != value.z

> endpoint_test_concat_partition_observer(state: EndpointConcatTriple, context: Unit) -> Int {
    endpoint_test_concat_partition(state)
}

? explore string_concat_order_is_not_total {
    from {
        vary before in [EndpointConcatTriple("a", "b", "ba"), EndpointConcatTriple("c", "b", "ba")]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_concat_partition_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::PartialRuleDispatch
    );
}

#[test]
fn nan_order_guards_do_not_masquerade_as_a_complementary_partition() {
    let source = r#"
| endpoint_test_nan_split(x: Float, y: Float) -> 1 under x <= y
| endpoint_test_nan_split(x: Float, y: Float) -> 2 under x > y

> endpoint_test_nan_split_observer(state: Int, context: Unit) -> Int {
    endpoint_test_nan_split(0.0 / 0.0, 0.0)
}

? explore nan_order_guards_are_not_total {
    from {
        vary before in [0]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_nan_split_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::PartialRuleDispatch
    );
}

#[test]
fn nan_self_equality_does_not_masquerade_as_reflexive_dispatch() {
    let source = r#"
| endpoint_test_nan_reflexive(value: Float) -> 1 under value == value

> endpoint_test_nan_reflexive_observer(state: Int, context: Unit) -> Int {
    endpoint_test_nan_reflexive(0.0 / 0.0)
}

? explore nan_self_equality_is_not_total {
    from {
        vary before in [0]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_nan_reflexive_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::PartialRuleDispatch
    );
}

#[test]
fn bare_scoped_rule_call_retains_its_exact_receiver_captures() {
    let source = r#"
# EndpointSiblingScope(base: Int) {
    | leaf() -> base
    | sibling() -> leaf() + 1
}

> endpoint_test_scoped_sibling_observer(state: Int, context: Unit) -> Int {
    EndpointSiblingScope(state).sibling()
}

? explore scoped_sibling_capture {
    from {
        vary before in [1, 2]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_scoped_sibling_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn closed_higher_order_rule_contract_certifies_without_legacy_totality() {
    let source = r#"
| endpoint_test_apply(value: Int, projection: (Int) -> Int) -> projection(value)
| endpoint_test_increment(value: Int) -> value + 1

> endpoint_test_higher_order_observer(state: Int, context: Unit) -> Int {
    endpoint_test_apply(state, endpoint_test_increment)
}

? explore higher_order_rule_contract {
    from {
        vary before in [1, 2]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_higher_order_observer
}
"#;

    checked_plan_identity(source);
}

#[test]
fn exhaustive_match_valued_rule_retains_a_type_contract_for_endpoint_proof() {
    let source = r#"
# EndpointForeignResult = EndpointForeignNone | EndpointForeignInvalid(code: Int, valid: Bool, compatible: Bool) | EndpointForeignCalculated(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, sixth: Int)

| endpoint_test_foreign_valid(result: EndpointForeignResult) -> match result {
    | EndpointForeignNone -> True
    | EndpointForeignInvalid(_, _, _) -> False
    | EndpointForeignCalculated(_, _, _, _, _, _) -> True
}

> endpoint_test_match_rule_observer(state: Int, context: Unit) -> Int {
    if endpoint_test_foreign_valid(EndpointForeignNone) { state } else { 0 }
}

? explore exhaustive_match_rule {
    from {
        vary before in [1, 2]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_match_rule_observer
}
"#;

    let artifacts = artifacts(source);
    assert!(
        artifacts.diagnostics.is_empty(),
        "unexpected match-rule diagnostics: {:?}",
        artifacts.diagnostics
    );
    let key = crate::RuleDispatchKey {
        scope: None,
        name: "endpoint_test_foreign_valid".to_string(),
        arity: 1,
    };
    assert!(
        artifacts
            .checked_resolutions
            .rule_dispatch_type_contracts
            .contains_key(&key),
        "match-valued rule lost its type contract: backend_type={:?}, backend_issue={:?}, parameters={:?}",
        artifacts.rule_dispatch_backend_return_types.get(&key),
        artifacts.rule_dispatch_backend_return_issues.get(&key),
        artifacts.rule_dispatch_parameter_types.get(&key),
    );
    let checked = artifacts
        .checked_exploration_query(0)
        .expect("exhaustive match-valued rule must certify");
    RelationalAnalysisPlan::from_checked(&checked)
        .expect("exhaustive match-valued rule must admit planning");
}

#[test]
fn transitive_declared_effect_is_rejected_by_endpoint_totality() {
    let source = r#"
# effect EndpointTestEffect {
    > endpoint_test_emit(value: Int) -> Int
}

> endpoint_test_effect_leaf(value: Int) -> Int with EndpointTestEffect {
    endpoint_test_emit(value)
}

> endpoint_test_effect_observer(state: Int, context: Unit) -> Int {
    endpoint_test_effect_leaf(state)
}

? explore effectful_endpoint_observer {
    from {
        vary before in [1]
        given context = ()
    }
    transition after = before
    find cases = all
    mechanisms paths from find cases using endpoint_test_effect_observer
}
"#;

    let issue = endpoint_issue_before_plan(source);
    assert_eq!(issue.endpoint(), RelationalEndpointRole::Before);
    assert_eq!(
        issue.reason(),
        RelationalEndpointTotalityIssueReason::EffectfulCall,
        "a declared effect must be rejected by the query-scoped endpoint proof"
    );
}

#[test]
fn personskat_200k_landscape_endpoint_totality_certifies_without_execution() {
    personskat_endpoint_totality_certifies(
        "personskat-mechanism-landscape-200k.explore.runa",
        "personskat_mechanism_landscape_conditioned_100_dkk_grid_200k_2026",
    );
}

#[test]
fn personskat_unit_income_distance_endpoint_totality_certifies_without_execution() {
    use super::relational_classification_capsule::{
        ClassificationLaneStatus, ClassificationSemanticLane,
    };
    let program = personskat_endpoint_totality_certifies(
        "personskat-income-distance-unit.explore.runa",
        "personskat_income_distance_unit_2026",
    );
    assert!(
        program
            .lane_manifest()
            .iter()
            .any(|lane| lane.lane == ClassificationSemanticLane::Successor
                && lane.status == ClassificationLaneStatus::Lowered),
        "the unchanged canonical unit transition must enter the proof graph"
    );
}

fn personskat_endpoint_totality_certifies(
    filename: &str,
    query_name: &str,
) -> std::sync::Arc<super::relational_classification_capsule::FrozenClassificationProgram> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax")
        .join(filename);
    let source = fs::read_to_string(&fixture).expect("read Personskat fixture");
    let mut lexer = Lexer::new(&source);
    let user_statements = Parser::new(lexer.tokenize(), &source)
        .parse_program()
        .expect("parse Personskat fixture");
    let statements = prepend_prelude(parse_prelude(), &user_statements);
    let source_dir = fixture
        .parent()
        .expect("Personskat fixture directory")
        .to_string_lossy()
        .into_owned();
    let artifacts =
        TypeChecker::check_with_explore_artifacts(&statements, Some(source_dir), &source);
    assert!(
        artifacts.diagnostics.is_empty(),
        "unexpected Personskat diagnostics for {filename}: {:?}",
        artifacts.diagnostics
    );
    let query_index = artifacts
        .exploration_universes
        .iter()
        .position(|query| query.name == query_name)
        .expect("Personskat query");
    let checked = artifacts
        .checked_exploration_query(query_index)
        .expect("Personskat endpoint-totality certificate");
    let certificate = checked
        .analysis_nodes()
        .find_map(|(_, identity)| match identity {
            CheckedExploreAnalysisIdentity::Mechanisms {
                endpoint_totality, ..
            } => Some(endpoint_totality),
            CheckedExploreAnalysisIdentity::View { .. } => None,
        })
        .expect("Personskat mechanism certificate");
    certificate
        .validate_identity()
        .expect("Personskat certificate identity");
    RelationalAnalysisPlan::from_checked(&checked)
        .expect("Personskat certificate must authorize plan construction");
    checked.classification_program()
}
