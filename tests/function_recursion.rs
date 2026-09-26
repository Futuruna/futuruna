use futuruna::eval_source_with_prelude;

fn interpret(source: &str) -> Result<String, String> {
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || eval_source_with_prelude(&source, false))
        .expect("spawn CLI-sized interpreter stack")
        .join()
        .expect("join interpreter")
}

#[test]
fn excessive_direct_mutual_and_callback_recursion_reports_incomplete_evaluation() {
    for source in [
        "> depth_count(n: Int) -> Int { if n == 0 { 0 } else { 1 + depth_count(n - 1) } }\n@ print(show(depth_count(3000)))",
        "> left_call(n: Int) -> Int { right_call(n + 1) }\n> right_call(n: Int) -> Int { left_call(n + 1) }\n@ print(show(left_call(0)))",
        "> through_map(n: Int) -> Int { head(map([n], |next| through_map(next + 1))) }\n@ print(show(through_map(0)))",
        "> through_poll() -> Int { poll(through_poll, 0) }\n@ print(show(through_poll()))",
    ] {
        let error = interpret(source).expect_err("recursive execution must fail explicitly");
        assert!(error.contains("function call") && error.contains("recursion limit")
            && error.contains("incomplete"), "{source}: {error}");
    }
}

#[test]
fn bounded_recursion_and_long_flat_callback_iteration_keep_their_values() {
    let source = "\
> depth_count(n: Int) -> Int { if n == 0 { 0 } else { 1 + depth_count(n - 1) } }
> polled_value() -> Int { poll_helper(41) }
> poll_helper(n: Int) -> Int { n + 1 }
@ print(show(depth_count(127)))
@ print(show(depth_count(127)))
@ print(show(sum_list(map(range(0, 5000), |value| 1))))
@ print(show(poll(polled_value, 0)))
";
    assert_eq!(interpret(source).unwrap().trim(), "127\n127\n5000\n42");
}
