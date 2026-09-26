use futuruna::eval_source_with_prelude;

fn assert_output(source: &str, expected: &str) {
    let output = eval_source_with_prelude(source, false).expect("evaluate logic query");
    assert_eq!(output.trim(), expected);
}

#[test]
fn anonymous_variables_do_not_leak_between_goals() {
    assert_output(
        r#"
| p(1)
| q(2)
| both() -> p(_), q(_)
| reversed() -> q(_), p(_)
@ print(show(both()))
@ print(show(reversed()))
"#,
        "true\ntrue",
    );
}

#[test]
fn anonymous_variables_do_not_turn_negated_existence_into_a_false_claim() {
    assert_output(
        r#"
| a(1)
| b(2)
| a_but_no_b() -> a(_), not(b(_))
| a_but_no_b_one() -> a(_), not(b(1))
@ print(show(a_but_no_b()))
@ print(show(a_but_no_b_one()))
"#,
        "false\ntrue",
    );
}

#[test]
fn anonymous_variables_leave_named_bindings_and_backtracking_intact() {
    assert_output(
        r#"
| edge(1, 10)
| edge(2, 20)
| wanted(20)
| distinct(3, 4)
| shared() -> edge(_, value), wanted(value)
| independent() -> edge(_, _), distinct(_, _)
| same_named() -> wanted(value), edge(1, value)
@ print(show(shared()))
@ print(show(independent()))
@ print(show(same_named()))
"#,
        "true\ntrue\nfalse",
    );
}

#[test]
fn anonymous_queries_work_in_single_goals_and_negation() {
    assert_output(
        r#"
| parent("alice", "bob")
| is_parent(x) -> parent(x, _)
| childless(x) -> not(parent(x, _))
@ print(show(is_parent("alice")))
@ print(show(childless("alice")))
@ print(show(is_parent("bob")))
@ print(show(childless("bob")))
@ print(show(parent(_, _)))
"#,
        "true\nfalse\nfalse\ntrue\ntrue",
    );
}

#[test]
fn findall_negation_agrees_with_direct_and_helper_rule_calls() {
    assert_output(
        r#"
| item(1)
| item(2)
| hidden(1)
| visible(x) -> item(x), not(hidden(x))
| shown(x) -> not(hidden(x))
| visible_via_helper(x) -> item(x), shown(x)
@ print(show(visible(1)))
@ print(show(visible(2)))
@ print(show(findall(x, visible(x))))
@ print(show(findall(x, visible_via_helper(x))))
"#,
        "false\ntrue\n[2]\n[2]",
    );
}

#[test]
fn findall_evaluates_builtin_and_function_predicates() {
    assert_output(
        r#"
| item(1)
| item(2)
> above_one(x: Int) -> Bool { x > 1 }
| selected(x) -> item(x), contains([2, 3], x), above_one(x)
| rejected(x) -> item(x), contains([], x)
@ print(show(findall(x, selected(x))))
@ print(show(findall(x, rejected(x))))
"#,
        "[2]\n[]",
    );
}

#[test]
fn findall_evaluates_nested_predicate_arguments_once_per_candidate() {
    assert_output(
        r#"
| item(1)
| item(2)
> logged_test(x: Int) -> Bool {
    @ print(show(x))
    x == 2
}
| selected(x) -> item(x), not(logged_test(x))
@ print(show(findall(x, selected(x))))
"#,
        "1\n2\n[1]",
    );
}

#[test]
fn findall_negation_respects_anonymous_existential_arguments() {
    assert_output(
        r#"
| item(1)
| item(2)
| blocked(1, "reason")
| visible(x) -> item(x), not(blocked(x, _))
@ print(show(visible(1)))
@ print(show(visible(2)))
@ print(show(findall(x, visible(x))))
"#,
        "false\ntrue\n[2]",
    );
}

#[test]
fn findall_binds_every_argument_of_a_derived_goal() {
    assert_output(
        r#"
| owns("ann", "car")
| owns("bob", "bike")
| owned(person, item) -> owns(person, item)
| owned_twice(person, item) -> owns(person, item), owns(person, item)
| owned_item(item) -> owns(_, item)
@ print(show(findall(item, owned(_, item))))
@ print(show(findall(item, owned_twice(_, item))))
@ print(show(findall(item, owned_item(item))))
"#,
        "[car, bike]\n[car, bike]\n[car, bike]",
    );
}

#[test]
fn findall_preserves_correlations_before_continuing_to_the_next_goal() {
    assert_output(
        r#"
| owns("ann", "car")
| owns("bob", "bike")
| permitted("ann", "bike")
| permitted("bob", "bike")
| available(person, item) -> owns(person, item), permitted(person, item)
@ print(show(findall(person, available(person, _))))
"#,
        "[bob]",
    );
}

#[test]
fn findall_captures_bound_argument_expressions_once_before_binding_free_arguments() {
    assert_output(
        r#"
| entry("fixed", "ann", "car")
| entry("fixed", "bob", "bike")
> label() -> String { @ print("label"); "fixed" }
| owned(person, item) -> entry(label(), person, item)
@ print(show(findall(item, owned(_, item))))
"#,
        "label\n[car, bike]",
    );
}

#[test]
fn findall_body_arguments_with_the_same_name_must_agree() {
    assert_output(
        r#"
| edge("a", "b")
| edge("b", "b")
| self_edge(node) -> edge(node, node)
@ print(show(findall(node, self_edge(node))))
"#,
        "[b]",
    );
}

#[test]
fn derived_existential_queries_agree_with_enumeration_and_backtrack() {
    assert_output(
        r#"
| parent("alice", "bob")
| parent("alice", "carol")
| wanted("carol")
| anc(x, y) -> parent(x, y)
| has_child(x) -> parent(x, y), parent(x, y)
| has_desc(x) -> anc(x, y), anc(x, y)
| has_wanted_desc(x) -> anc(x, y), wanted(y)
| childless(x) -> not(anc(x, _))
@ print(show(has_child("alice")))
@ print(show(has_desc("alice")))
@ print(show(has_wanted_desc("alice")))
@ print(show(has_desc("nobody")))
@ print(show(childless("alice")))
@ print(show(childless("bob")))
@ print(show(findall(y, anc("alice", y))))
"#,
        "true\ntrue\ntrue\nfalse\nfalse\ntrue\n[bob, carol]",
    );
}

#[test]
fn existential_queries_preserve_literal_heads_with_bodies() {
    assert_output(
        r#"
| ready("ann") -> True
| owns("ann", "car")
| available("ann", item) -> owns("ann", item)
| has_ready() -> ready(person), True
| has_available(person) -> available(person, item), True
@ print(show(has_ready()))
@ print(show(has_available("ann")))
@ print(show(has_available("bob")))
@ print(show(findall(person, ready(person))))
@ print(show(findall(item, available("bob", item))))
"#,
        "true\ntrue\nfalse\n[ann]\n[]",
    );
}

#[test]
fn existential_queries_match_constructor_heads_and_project_constructor_results() {
    assert_output(
        r#"
# Status = Active | Retired
# Id = Id(n: String)
| status("ann", Active)
| owns(Id("ann"), "car")
| qualified(Id("ann"), "license") -> True
| registered(Id(name)) -> status(name, Active)
| anyone_retired() -> status(person, Retired), True
| has_status(person) -> status(person, state), True
| has_item(person) -> owns(person, item), True
| has_license(person) -> qualified(person, item), True
@ print(show(anyone_retired()))
@ print(show(has_status("ann")))
@ print(show(has_item(Id("ann"))))
@ print(show(has_item(Id("bob"))))
@ print(show(has_license(Id("ann"))))
@ print(show(has_license(Id("bob"))))
@ print(show(registered(_)))
@ print(show(findall(person, registered(person))))
"#,
        "false\ntrue\ntrue\nfalse\ntrue\nfalse\ntrue\n[Id(n: ann)]",
    );
}

#[test]
fn derived_existential_rule_locals_do_not_capture_globals() {
    assert_output(
        r#"
= person = "unrelated"
= child = "unrelated"
| parent("alice", "bob")
| anc(person, child) -> parent(person, child)
| any_desc() -> anc(_, _), True
@ print(show(any_desc()))
@ print(show(findall(result, anc("alice", result))))
"#,
        "true\n[bob]",
    );
}

#[test]
fn derived_existential_queries_keep_declaring_rule_and_constructor_namespaces() {
    assert_output(
        r#"
# Id = Id(name: String)
| owns(Id("root"), "root item")
> module Ledger {
    # Id = Id(name: String)
    | owns(Id("ann"), "car")
    | entry(person, item) -> owns(person, item)
    | has_entry(person) -> entry(person, item), True
    | any_entry() -> entry(_, _), True
    > check() -> Bool { has_entry(Id("ann")) }
}
@ print(show(Ledger.any_entry()))
@ print(show(Ledger.check()))
@ print(show(Ledger.has_entry(Id("ann"))))
@ print(show(Ledger.has_entry(Id("root"))))
"#,
        "true\ntrue\nfalse\nfalse",
    );
}

#[test]
fn derived_existential_queries_capture_bound_expressions_once() {
    assert_output(
        r#"
| row("fixed", "ann", "car")
| row("fixed", "bob", "bike")
| entry(label, person, item) -> row(label, person, item)
| wanted("bike")
> label() -> String { @ print("label"); "fixed" }
| wanted_entry() -> entry(label(), person, item), wanted(item)
@ print(show(wanted_entry()))
"#,
        "label\ntrue",
    );
}

fn recursive_chain_source(edges: usize, query: &str) -> String {
    let mut source = String::new();
    for node in 0..edges {
        source.push_str(&format!("| edge({node}, {})\n", node + 1));
    }
    source.push_str("| reach(a, b) -> edge(a, b)\n");
    source.push_str("| reach(a, b) -> edge(a, m), reach(m, b)\n");
    source.push_str(query);
    source
}

fn eval_recursive_query(source: String) -> Result<String, String> {
    // Match the CLI runtime stack for a deliberately deep recursive fixture.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || eval_source_with_prelude(&source, false))
        .expect("spawn query runtime")
        .join()
        .expect("join query runtime")
}

#[test]
fn recursive_queries_do_not_return_partial_answers_at_the_depth_limit() {
    for query in [
        "@ print(show(length(findall(b, reach(0, b)))))",
        "@ print(show(not(reach(0, _))))",
    ] {
        let error = eval_recursive_query(recursive_chain_source(71, query))
            .expect_err("depth exhaustion must fail instead of publishing a partial answer");
        assert!(error.contains("logic query"), "{error}");
        assert!(error.contains("recursion limit"), "{error}");
        assert!(error.contains("incomplete"), "{error}");
    }
}

#[test]
fn recursive_queries_within_the_depth_limit_keep_complete_answers() {
    let output = eval_recursive_query(recursive_chain_source(
        6,
        "@ print(show(length(findall(b, reach(0, b)))))\n@ print(show(reach(0, 6)))",
    ))
    .expect("evaluate finite recursive query");
    assert_eq!(output.trim(), "6\ntrue");
}

#[test]
fn findall_keeps_distinct_values_that_share_display_text() {
    assert_output(
        r#"
| tag(1)
| tag("1")
| tag(1)
| derived(value) -> tag(value)
@ print(show(length(findall(value, tag(value)))))
@ print(show(length(findall(value, derived(value)))))
"#,
        "2\n2",
    );
}

#[test]
fn findall_deduplicates_structured_answers_without_merging_distinct_fields() {
    assert_output(
        r#"
| pair((1, "a"))
| pair(("1", "a"))
| pair((1, "a"))
| derived(value) -> pair(value)
@ print(show(length(findall(value, pair(value)))))
@ print(show(length(findall(value, derived(value)))))
"#,
        "2\n2",
    );
}

#[test]
fn unsupported_findall_templates_have_a_located_frontend_error() {
    for query in [
        "findall((x, y), pair(x, y))",
        "search((x, y), pair(x, y))",
        "findall(42, pair(x, y))",
        "findall([x], pair(x, y))",
    ] {
        let source = format!("| pair(1, 2)\n@ print(show({query}))\n");
        let error = eval_source_with_prelude(&source, false)
            .expect_err("unsupported template must not become an empty result");
        assert!(error.starts_with("2:"), "expected source location: {error}");
        assert!(
            error.contains("template must be a single variable"),
            "{error}"
        );
    }
}

#[test]
fn unsupported_template_error_points_to_the_call_instead_of_earlier_text() {
    let source = "| pair(1, 2)\n@ print(\"findall\")\n@ print(show(findall((x, y), pair(x, y))))\n";
    let error = eval_source_with_prelude(source, false).expect_err("reject tuple template");
    assert!(error.starts_with("3:"), "wrong call location: {error}");
}

#[test]
fn custom_functions_named_findall_and_search_keep_ordinary_arguments() {
    assert_output(
        r#"
> findall(template: Int, goal: Int) -> Int { template + goal }
> search(template: Int, goal: Int) -> Int { template * goal }
@ print(show(findall(2, 3)))
@ print(show(search(2, 3)))
"#,
        "5\n6",
    );
}

#[test]
fn local_functions_named_findall_keep_ordinary_arguments() {
    assert_output(
        r#"
> calculate() -> Int {
    = findall = |left, right| left + right
    findall(2, 3)
}

@ print(show(calculate()))
"#,
        "5",
    );
}

#[test]
fn signed_numeric_rule_heads_compare_values_and_enumerate_their_sign() {
    assert_output(
        r#"
| balance("bob", -5)
| surcharge(-1) -> 100
| surcharge(n) -> 0
| fraction(-1.5)
@ print(show(balance("bob", -5)))
@ print(show(balance("bob", 7)))
@ print(show(surcharge(-1)))
@ print(show(surcharge(5)))
@ print(show(fraction(-1.5)))
@ print(show(fraction(1.5)))
@ print(show(findall(value, balance("bob", value))))
"#,
        "true\nfalse\n100\n0\ntrue\nfalse\n[-5]",
    );
}

#[test]
fn list_rule_heads_compare_length_and_fields_and_bind_local_variables() {
    assert_output(
        r#"
= left = 1000
| tags("ann", [1, 2])
| empty([])
| sum_pair([left, right]) -> left + right
@ print(show(tags("ann", [1, 2])))
@ print(show(tags("ann", [9, 9, 9])))
@ print(show(tags("ann", [1, 9])))
@ print(show(empty([])))
@ print(show(empty([1])))
@ print(show(sum_pair([3, 4])))
@ print(show(findall(value, tags("ann", value))))
"#,
        "true\nfalse\nfalse\ntrue\nfalse\n7\n[[1, 2]]",
    );
}

#[test]
fn list_and_numeric_heads_match_inside_constructors() {
    assert_output(
        r#"
# Basket(items: List(Int))
| accepted(Basket([-1, 2]))
@ print(show(accepted(Basket([-1, 2]))))
@ print(show(accepted(Basket([1, 2]))))
@ print(show(accepted(Basket([-1]))))
@ print(show(length(findall(value, accepted(value)))))
"#,
        "true\nfalse\nfalse\n1",
    );
}

#[test]
fn unit_rule_heads_match_only_unit() {
    assert_output(
        r#"
| unit_only(()) -> 1
| unit_only(value) -> 0
@ print(show(unit_only(())))
@ print(show(unit_only(42)))
"#,
        "1\n0",
    );
}

#[test]
fn unsupported_computed_rule_heads_are_rejected() {
    for head in ["1 + 2", "length([1])", "-value"] {
        let source = format!("| invalid({head}) -> True\n@ print(show(invalid(9)))\n");
        let error = eval_source_with_prelude(&source, false)
            .expect_err("unsupported head must not act as a wildcard");
        assert!(error.contains("unsupported rule-head pattern"), "{error}");
    }
}

#[test]
fn repeated_head_variables_compare_values_instead_of_overwriting() {
    assert_output(
        r#"
= x = 999
| same(x, x)
@ print(show(same(1, 1)))
@ print(show(same(1, 2)))
@ print(show(same(1, "1")))
@ print(show(same((1, 2), (1, 2))))
@ print(show(same([1, 2], [1, 3])))
@ print(show(same((), ())))
@ print(show(findall(value, same(value, 7))))
@ print(show(findall(value, same(7, value))))
"#,
        "true\nfalse\nfalse\ntrue\nfalse\ntrue\n[7]\n[7]",
    );
}

#[test]
fn repeated_variables_in_nested_head_patterns_share_bindings() {
    assert_output(
        r#"
# Pair(left: Int, right: Int)
| same_fields(Pair(x, x))
| same_list([x, x])
| same_tuple((x, x))
@ print(show(same_fields(Pair(1, 1))))
@ print(show(same_fields(Pair(1, 2))))
@ print(show(same_list([1, 1])))
@ print(show(same_list([1, 2])))
@ print(show(same_tuple((1, 1))))
@ print(show(same_tuple((1, 2))))
"#,
        "true\nfalse\ntrue\nfalse\ntrue\nfalse",
    );
}

#[test]
fn repeated_query_variables_filter_facts_and_derived_rows_before_later_effects() {
    assert_output(
        r#"
| edge("a", "b")
| edge("b", "b")
> checked() -> Bool { @ print("checked"); True }
| derived(left, right) -> edge(left, right), checked()
| has_self_loop() -> edge(n, n), True
@ print(show(has_self_loop()))
@ print(show(findall(n, edge(n, n))))
@ print(show(findall(n, derived(n, n))))
"#,
        "true\n[b]\nchecked\n[b]",
    );
}

#[test]
fn repeated_non_template_query_variables_keep_row_correlations() {
    assert_output(
        r#"
| row("wrong", "a", "b")
| row("right", "b", "b")
| derived(result, left, right) -> row(result, left, right)
@ print(show(findall(result, row(result, same_node, same_node))))
@ print(show(findall(result, derived(result, same_node, same_node))))
"#,
        "[right]\n[right]",
    );
}

#[test]
fn repeated_query_variables_constrain_finite_typed_domains() {
    assert_output(
        r#"
# Color = Red | Blue
| same_color(left: Color, right: Color) -> left == right
@ print(show(findall(color, same_color(color, color))))
"#,
        "[Red, Blue]",
    );
}

#[test]
fn repeated_query_positions_unify_complementary_constructor_head_fields() {
    assert_output(
        r#"
# Pair(left: Int, right: Int)
| pair_terms(Pair(x, 1), Pair(2, y))
@ print(show(findall(value, pair_terms(value, value))))
"#,
        "[Pair(left: 2, right: 1)]",
    );
}

#[test]
fn repeated_query_positions_propagate_chains_within_structured_head_terms() {
    assert_output(
        r#"
| terms((a, b, c, d), (b, c, d, 1))
@ print(show(findall(value, terms(value, value))))
"#,
        "[(1, 1, 1, 1)]",
    );
}

#[test]
fn repeated_rule_head_names_shadow_lexical_parent_bindings() {
    assert_output(
        r#"
= x = 999
> module Local {
    | same(x, x)
}
@ print(show(Local.same(1, 1)))
@ print(show(Local.same(1, 2)))
"#,
        "true\nfalse",
    );
}

#[test]
fn repeated_query_positions_reject_indirectly_cyclic_head_terms() {
    assert_output(
        r#"
# Node = End | Next(Node)
| cyclic("bad", x, Next(y), y, Next(x))
@ print(show(findall(label, cyclic(label, left_node, left_node, right_node, right_node))))
"#,
        "[]",
    );
}

#[test]
fn query_exceptions_exclude_the_same_people_as_direct_calls() {
    assert_output(
        r#"
| sanctioned("ann")
| eligible("ann")
| eligible("bob")
| exception sanctions eligible(p) -> False under sanctioned(p)
| anyone_but_bob() -> eligible(p), p != "bob"
@ print(show(eligible("ann")))
@ print(show(eligible("bob")))
@ print(show(findall(p, eligible(p))))
@ print(show(anyone_but_bob()))
"#,
        "false\ntrue\n[bob]\nfalse",
    );
}

#[test]
fn query_guarded_defaults_filter_candidates_and_project_inputs() {
    assert_output(
        r#"
| sanctioned("ann")
| eligible("ann")
| eligible("bob")
| eligible(p) -> False under sanctioned(p)
| eligible(p) -> False
| eligible("carol") -> True under True
@ print(show(eligible("ann")))
@ print(show(eligible("zed")))
@ print(show(findall(p, eligible(p))))
"#,
        "false\nfalse\n[bob, carol]",
    );
}

#[test]
fn query_overrides_follow_priority_and_first_applicable_order() {
    assert_output(
        r#"
| selected("ann")
| selected("bob")
| selected("carol") -> True under True
| selected("bob") -> False under True
| exception deny selected("ann") -> False under True
| exception later_allow selected("ann") -> True under True
| exception allow selected("bob") -> True under True
| exception additional selected("dave") -> True under True
= choices = findall(person, selected(person))
@ print(show(length(choices)))
@ print(show(contains(choices, "ann")))
@ print(show(contains(choices, "bob")))
@ print(show(contains(choices, "carol")))
@ print(show(contains(choices, "dave")))
"#,
        "3\nfalse\ntrue\ntrue\ntrue",
    );
}

#[test]
fn query_overrides_run_once_before_remaining_clause_effects() {
    assert_output(
        r#"
> blocked(p: String) -> Bool { @ print("guard " + p); p == "ann" }
> checked(p: String) -> Bool { @ print("body " + p); True }
| person("ann")
| person("bob")
| selected(p) -> person(p), checked(p)
| exception deny selected(p) -> False under blocked(p)
@ print(show(findall(person, selected(person))))
"#,
        "guard ann\nguard bob\nbody bob\n[bob]",
    );
}

#[test]
fn query_positive_overrides_can_supply_their_own_finite_candidates() {
    assert_output(
        r#"
| person("ann")
| person("bob")
| exception available selected(p) -> True under person(p)
| has_selected() -> selected(_)
@ print(show(findall(person, selected(person))))
@ print(show(has_selected()))
"#,
        "[ann, bob]\ntrue",
    );
}

#[test]
fn query_priority_decisions_keep_full_argument_correlations() {
    assert_output(
        r#"
| eligible("ann", "read")
| eligible("ann", "write")
| eligible("bob", "write")
| exception deny eligible(p, "read") -> False under True
@ print(show(findall(person, eligible(person, _))))
@ print(show(findall(person, eligible(person, "read"))))
@ print(show(findall(person, eligible(person, "write"))))
"#,
        "[ann, bob]\n[]\n[ann, bob]",
    );
}

#[test]
fn query_duplicate_candidates_do_not_repeat_selected_override_effects() {
    assert_output(
        r#"
> flagged(p: String) -> Bool { @ print("guard " + p); True }
> granted(p: String) -> Bool { @ print("value " + p); True }
| selected("ann")
| selected("ann")
| exception grant selected("ann") -> granted("ann") under flagged("ann")
@ print(show(findall(person, selected(person))))
"#,
        "guard ann\nvalue ann\n[ann]",
    );
}

#[test]
fn query_priority_uses_declaration_namespaces_and_constructor_patterns() {
    assert_output(
        r#"
# Id = Id(name: String)
| denied(Id("bob"))
> module Ledger {
    # Id = Id(name: String)
    | denied(Id("ann"))
    | selected(Id("ann"))
    | selected(Id("bob"))
    | exception denial selected(Id(name)) -> False under denied(Id(name))
    > choices() -> String { show(findall(person, selected(person))) }
    | any_selected() -> selected(_)
}
@ print(Ledger.choices())
@ print(show(Ledger.any_selected()))
"#,
        "[Id(name: bob)]\ntrue",
    );
}

#[test]
fn query_unbounded_positive_overrides_report_incomplete_evaluation() {
    let source = r#"
| exception everyone selected(person) -> True
@ print(show(findall(person, selected(person))))
"#;
    let error = eval_source_with_prelude(source, false).unwrap_err();
    assert!(
        error.contains("cannot resolve exception/default priority for an unbound head"),
        "{error}"
    );
    assert!(error.contains("evaluation is incomplete"), "{error}");
}

#[test]
fn rule_and_separators_keep_bindings_through_every_goal() {
    for language in ["", "@ sprog da\n"] {
        assert_output(
            &format!(
                r#"{language}
| person(1)
| parent(1, 2)
| adult(2)
| comma_form(x) -> person(x), parent(x, y), adult(y)
| and_form(x) -> person(x) and parent(x, y) and adult(y)
@ print(show(comma_form(1)))
@ print(show(and_form(1)))
@ print(show(and_form(2)))
"#
            ),
            "true\ntrue\nfalse",
        );
    }
}

#[test]
fn rule_or_branches_bind_independently_and_and_binds_more_tightly() {
    assert_output(
        r#"
| person(1)
| edge(1, 2)
| wanted(2)
| alternate(1, 3)
| other(3)
| selected(x) -> person(x) and edge(x, y) and wanted(y) or alternate(x, y) and other(y)
| filtered(x) -> person(x) and edge(x, y) and False or alternate(x, y) and other(y)
| either(x) -> edge(x, y) || other(y)
@ print(show(selected(1)))
@ print(show(filtered(1)))
@ print(show(either(1)))
@ print(show(findall(person, selected(person))))
"#,
        "true\ntrue\ntrue\n[1]",
    );
}

#[test]
fn rule_goal_parsing_retains_nested_and_ordinary_boolean_expressions() {
    assert_output(
        r#"
> identity(value: Bool) -> Bool { value }
| nested() -> identity(True and False) or identity(False or True)
| grouped() -> (True and False) or (False or True)
| guarded(x) -> x == 1 or x == 2 under x > 0
| guarded(x) -> False
= ordinary = False or True and True
@ print(show(nested()))
@ print(show(grouped()))
@ print(show(guarded(1)))
@ print(show(guarded(2)))
@ print(show(guarded(3)))
@ print(show(ordinary))
"#,
        "true\ntrue\ntrue\ntrue\nfalse\ntrue",
    );
}

#[test]
fn rule_alternatives_do_not_export_bindings_to_other_branches() {
    let source = r#"
| edge(1, 2)
| invalid(x) -> edge(x, y) and False or y == 2
@ print(show(invalid(1)))
"#;
    let error = eval_source_with_prelude(source, false).unwrap_err();
    assert!(error.contains("undefined variable `y`"), "{error}");
}

#[test]
fn cyclic_direct_queries_fail_explicitly_including_under_negation() {
    for query in ["reach(\"a\", \"z\")", "not(reach(\"a\", \"z\"))"] {
        let source = format!(
            r#"
| edge("a", "b")
| edge("b", "a")
| reach(x, y) -> edge(x, y)
| reach(x, y) -> edge(x, middle), reach(middle, y)
@ print(show({query}))
"#
        );
        let error = eval_recursive_query(source).expect_err("cyclic search must fail explicitly");
        assert!(error.contains("rule call"), "{error}");
        assert!(error.contains("recursion limit"), "{error}");
        assert!(error.contains("evaluation is incomplete"), "{error}");
    }
}

#[test]
fn direct_query_depth_limit_also_bounds_changing_arguments() {
    let source = r#"
| never_done(n) -> never_done(n + 1)
@ print(show(never_done(0)))
"#;
    let error = eval_recursive_query(source.to_string()).unwrap_err();
    assert!(error.contains("rule call `never_done`"), "{error}");
    assert!(error.contains("recursion limit"), "{error}");
}

#[test]
fn terminating_direct_queries_retain_answers_and_restore_depth() {
    let source = recursive_chain_source(
        20,
        "@ print(show(reach(0, 20)))\n@ print(show(reach(0, 21)))\n@ print(show(reach(0, 20)))",
    );
    let output = eval_recursive_query(source).expect("finite direct queries complete");
    assert_eq!(output.trim(), "true\nfalse\ntrue");
}

#[test]
fn rejecting_overrides_only_evaluate_variable_guards_for_known_candidates() {
    assert_output(
        r#"
| eligible("ann")
| eligible("bob")
| exception denied eligible(person) -> False under person == "ann"
@ print(show(findall(person, eligible(person))))
"#,
        "[bob]",
    );
}

#[test]
fn positive_function_guards_need_an_enumerable_candidate_domain() {
    let source = r#"
> always(person: String) -> Bool { True }
| selected("ann")
| exception everyone selected(person) -> True under always(person)
@ print(show(findall(person, selected(person))))
"#;
    let error = eval_source_with_prelude(source, false).unwrap_err();
    assert!(
        error.contains("cannot enumerate an unbound override predicate"),
        "{error}"
    );
    assert!(error.contains("evaluation is incomplete"), "{error}");
}

#[test]
fn differential_logic_fixtures_execute_real_assertions() {
    for (name, source) in [
        (
            "logic_derived_existential",
            include_str!("differential/corpus/logic_derived_existential.runa"),
        ),
        (
            "logic_exception_priority",
            include_str!("differential/corpus/logic_exception_priority.runa"),
        ),
        (
            "logic_goal_separators",
            include_str!("differential/corpus/logic_goal_separators.runa"),
        ),
        (
            "logic_repeated_variables",
            include_str!("differential/corpus/logic_repeated_variables.runa"),
        ),
        (
            "logic_signed_list_heads",
            include_str!("differential/corpus/logic_signed_list_heads.runa"),
        ),
        (
            "logic_wildcard_independence",
            include_str!("differential/corpus/logic_wildcard_independence.runa"),
        ),
    ] {
        eval_source_with_prelude(source, false).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[test]
fn undeclared_uppercase_logic_variables_are_rejected_before_execution() {
    let source = r#"
| parent("alice", "bob")
| parent("bob", "carl")
| grand(X, Z) -> parent(X, Y), parent(Y, Z)
@ print(show(grand("alice", "carl")))
@ print(show(findall(Z, grand("alice", Z))))
"#;
    let error = eval_source_with_prelude(source, false)
        .expect_err("uppercase names cannot silently become logic variables");
    for name in ["X", "Y", "Z"] {
        assert!(
            error.contains(&format!("undefined constructor `{name}`")),
            "{error}"
        );
    }
}

#[test]
fn lowercase_logic_variables_and_declared_uppercase_constructors_agree() {
    for (declaration, alice, bob, carl, expected) in [
        ("", "\"alice\"", "\"bob\"", "\"carl\"", "true\n[carl]"),
        (
            "# Person = Alice | Bob | Carl\n",
            "Alice",
            "Bob",
            "Carl",
            "true\n[Carl]",
        ),
    ] {
        let source = format!("{declaration}| parent({alice}, {bob})\n| parent({bob}, {carl})\n| grand(x, z) -> parent(x, y), parent(y, z)\n@ print(show(grand({alice}, {carl})))\n@ print(show(findall(z, grand({alice}, z))))\n");
        assert_output(&source, expected);
    }
}
