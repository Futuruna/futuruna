use futuruna::{eval_source_with_prelude, interpreter_stack_bytes};

/// Evaluate on a thread with the stack the `runa` command uses.
fn interpret(source: &str) -> Result<String, String> {
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(interpreter_stack_bytes())
        .spawn(move || eval_source_with_prelude(&source, false))
        .expect("spawn CLI-sized interpreter stack")
        .join()
        .expect("join interpreter")
}

/// Evaluate on a small stack so that unbounded recursion reaches the limit quickly.
fn interpret_on_small_stack(source: &str) -> Result<String, String> {
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || eval_source_with_prelude(&source, false))
        .expect("spawn small interpreter stack")
        .join()
        .expect("join interpreter")
}

#[test]
fn unbounded_direct_mutual_and_callback_recursion_reports_incomplete_evaluation() {
    for source in [
        "> depth_count(n: Int) -> Int { 1 + depth_count(n + 1) }\n@ print(show(depth_count(0)))",
        "> left_call(n: Int) -> Int { right_call(n + 1) }\n> right_call(n: Int) -> Int { left_call(n + 1) }\n@ print(show(left_call(0)))",
        "> through_map(n: Int) -> Int { head(map([n], |next| through_map(next + 1))) }\n@ print(show(through_map(0)))",
        "> through_poll() -> Int { poll(through_poll, 0) }\n@ print(show(through_poll()))",
    ] {
        let error =
            interpret_on_small_stack(source).expect_err("unbounded recursion must fail explicitly");
        assert!(
            error.contains("call") && error.contains("recursion limit") && error.contains("incomplete"),
            "{source}: {error}"
        );
    }
}

#[test]
fn ordinary_deep_recursion_keeps_its_values() {
    let source = "\
> depth_count(n: Int) -> Int { if n == 0 { 0 } else { 1 + depth_count(n - 1) } }
> sum_list_r(xs: List(Int)) -> Int {
    if length(xs) == 0 { 0 } else { head(xs) + sum_list_r(tail(xs)) }
}
> polled_value() -> Int { poll_helper(41) }
> poll_helper(n: Int) -> Int { n + 1 }
@ print(show(depth_count(100000)))
@ print(show(sum_list_r(range(0, 10000))))
@ print(show(sum_list(map(range(0, 5000), |value| 1))))
@ print(show(poll(polled_value, 0)))
";
    assert_eq!(
        interpret(source).unwrap().trim(),
        "100000\n49995000\n5000\n42"
    );
}
