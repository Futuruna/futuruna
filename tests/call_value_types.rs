use futuruna::{eval_source_with_prelude, Interpreter, Lexer, Parser, TypeChecker};
use std::path::Path;

fn errors(source: &str) -> Vec<String> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    TypeChecker::check_with_source(&statements, None, source)
}

#[test]
fn scalar_and_collection_bindings_cannot_be_called() {
    for value in [
        "5",
        "True",
        "\"hello\"",
        "1.5",
        "'a'",
        "[1, 2]",
        "()",
        "(1, 2)",
    ] {
        let source = format!("= value = {value}\nvalue(1)\n");
        let errors = errors(&source);
        assert!(
            errors.iter().any(|e| e.contains("not callable")),
            "{source}: {errors:?}"
        );
    }
    for source in [
        "= length = 5\nlength([1, 2])\n",
        "> invalid(value: Int) { value(1) }\n",
    ] {
        let errors = errors(source);
        assert!(
            errors.iter().any(|e| e.contains("not callable")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn function_calls_reject_known_argument_type_contradictions() {
    for source in [
        "> length(value: Int) -> Int { value }\nlength([1, 2])\n",
        "> length(value: Int) -> Int { value }\nlength([])\n",
        "> accept(value: List(Int)) -> Int { 1 }\naccept(\"no\")\n",
        "> accept(value: List(Int)) -> Int { 1 }\naccept([\"no\"])\n",
        "> accept(left: Int, right: String) -> Int { left }\naccept(right = 5, left = 1)\n",
        "> accept(left: a, right: a) -> a { left }\naccept(1, \"no\")\n",
    ] {
        let errors = errors(source);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("argument") && e.contains("expects")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn inferred_functions_and_lexical_unknown_callbacks_remain_callable() {
    let source = r#"
= length = 5
> through(length) { length([1, 2]) }
> increment(value: Int) -> Int { value + 1 }
> choose(left: a, right: a) -> a { left }
= callback = increment
@ print(show(through(|items| sum_list(items))))
@ print(show(callback(2)))
@ print(show(choose("yes", "no")))
@ print(show(choose([1], [2])))
"#;
    assert_eq!(
        eval_source_with_prelude(source, false).unwrap().trim(),
        "3\n3\nyes\n[1]"
    );
}

#[test]
fn primitive_named_variants_do_not_alias_the_primitive_type() {
    let source = "# Money = Int\n> amount(value: Money) -> Int { 7 }\namount(42)\n";
    let diagnostics = errors(source);
    assert!(
        diagnostics
            .iter()
            .any(|error| error.contains("expects `Money`, got `Int`")),
        "{diagnostics:?}"
    );
    assert!(eval_source_with_prelude(source, false).is_err());

    let wrapped = "# Money(amount: Int)\n> amount(value: Money) -> Int { value.amount }\n@ print(show(amount(Money(42))))\n";
    assert_eq!(
        eval_source_with_prelude(wrapped, false).unwrap().trim(),
        "42"
    );
}

#[test]
fn unchecked_calls_to_non_functions_stop_at_the_call() {
    let source = "= value = 5\nvalue(1)\n@ print(\"must not run\")\n";
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .unwrap();
    let mut runtime = Interpreter::new();
    runtime.suppress_output = true;
    let mut env = runtime.default_env();
    let error = runtime
        .run_program_with_diagnostics(&statements, &mut env, Path::new("call.runa"), source)
        .expect_err("calling an integer cannot return unit and continue");
    assert!(error.message.contains("not callable"), "{error:?}");
    assert!(runtime.output.is_empty(), "{:?}", runtime.output);
}
