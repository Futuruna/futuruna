//! Declared rule result types (docs/rfcs/named-scalar-types-and-rule-result-types.md,
//! Part 2): one test per rule and per diagnostic.

use futuruna::migrate::rule_types::migrate_rule_declarations;
use futuruna::{
    eval_source_with_prelude, parse_prelude, prepend_prelude, Lexer, Parser, TypeChecker,
};

fn parse(source: &str) -> Result<Vec<futuruna::Stmt>, String> {
    Parser::new(Lexer::new(source).tokenize(), source).parse_program()
}

fn parse_error(source: &str) -> String {
    parse(source).expect_err("the source must not parse")
}

fn check_errors(source: &str) -> Vec<String> {
    let statements = parse(source).unwrap();
    let statements = prepend_prelude(parse_prelude(), &statements);
    TypeChecker::check_with_artifacts(&statements, None, source)
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

fn run(source: &str) -> String {
    eval_source_with_prelude(source, true)
        .unwrap()
        .trim()
        .to_string()
}

#[test]
fn a_declaring_clause_types_its_family_and_later_clauses_write_names() {
    let source = "| days(year: Int, month: Int): Int -> 31\n| days(year, month) -> 30 under month == 4\n@ print(days(2025, 4))\n@ print(days(2025, 5))\n";
    assert!(
        check_errors(source).is_empty(),
        "{:?}",
        check_errors(source)
    );
    assert_eq!(run(source), "30\n31");
}

#[test]
fn a_bodiless_declaration_types_a_family_whose_first_clause_is_a_pattern() {
    let source = "| fib(n: Int): Int\n| fib(0) -> 0\n| fib(1) -> 1\n| fib(n) -> fib(n - 1) + fib(n - 2)\n@ print(fib(10))\n";
    assert!(
        check_errors(source).is_empty(),
        "{:?}",
        check_errors(source)
    );
    assert_eq!(run(source), "55");
}

#[test]
fn a_number_takes_the_declared_unit() {
    let source = "# unit Ore \"DKK øre\"\n| fee(amount: Ore): Ore -> 0\n| fee(amount) -> amount / 10 under amount > Ore(1000)\n= total: Ore = fee(Ore(5000)) + fee(Ore(10))\n@ print(total)\n";
    assert!(
        check_errors(source).is_empty(),
        "{:?}",
        check_errors(source)
    );
    assert_eq!(run(source), "500");
}

#[test]
fn none_takes_its_type_from_the_declaration() {
    let source = "| half(x: Int): Int? -> None\n| half(x) -> Some(x / 2) under x % 2 == 0\n= h: Int = unwrap_or(half(8), 0)\n@ print(h)\n";
    assert!(
        check_errors(source).is_empty(),
        "{:?}",
        check_errors(source)
    );
    assert_eq!(run(source), "4");
}

#[test]
fn a_family_declared_bool_answers_false_when_no_clause_applies() {
    let source = "| adult(age: Int): Bool -> True under age >= 18\n@ print(adult(3))\n";
    assert!(
        check_errors(source).is_empty(),
        "{:?}",
        check_errors(source)
    );
    assert_eq!(run(source), "false");
}

#[test]
fn a_family_of_another_declared_type_stops_when_no_clause_applies() {
    let source = "| pick(x: Int): Int\n| pick(0) -> g(1)\n| pick(1) -> g(2)\n> g(x: Int) -> Int { x }\n@ print(pick(5))\n";
    let error = eval_source_with_prelude(source, true).unwrap_err();
    assert!(error.contains("no value rule matched `pick/1`"), "{error}");
}

#[test]
fn a_clause_of_another_type_is_an_error_naming_the_declaration() {
    let errors = check_errors("| k(x: Int): Int -> \"no\"\n| k(x) -> 2 under x > 1\n");
    assert!(
        errors.iter().any(|error| error
            .contains("rule `k` (declared at line 1) expects `Int` but expression has `String`")),
        "{errors:?}"
    );
}

#[test]
fn a_fact_or_goal_body_requires_bool() {
    let errors = check_errors("| p(x: Int): Bool -> x > 0\n| r(x: Int): Int -> p(x), p(x)\n");
    assert!(
        errors.iter().any(|error| error
            .contains("rule `r` is declared `: Int` at line 2, but this clause is a goal body")),
        "{errors:?}"
    );
}

#[test]
fn a_type_on_a_later_clause_is_an_error_pointing_to_the_declaring_line() {
    let error =
        parse_error("| fee(amount: Int): Int -> 0\n| fee(amount: Int) -> 1 under amount > 3\n");
    assert!(error.contains("2:7: the first parameter of `fee` writes the type `Int`; types are written once, on the family's declaring line (line 1)"), "{error}");
}

#[test]
fn a_type_on_an_exception_is_an_error() {
    let error = parse_error(
        "| fee(amount: Int): Int -> 0\n| exception big fee(amount: Int) -> 1 under amount > 3\n",
    );
    assert!(error.contains("writes the type `Int`"), "{error}");
}

#[test]
fn an_exception_cannot_declare_a_result_type() {
    let error = parse_error(
        "| fee(amount: Int) -> 0\n| exception big fee(amount): Int -> 1 under amount > 3\n",
    );
    assert!(
        error.contains("exception `big` cannot declare a result type"),
        "{error}"
    );
}

#[test]
fn a_different_name_on_a_later_clause_is_an_error() {
    let error = parse_error("| g(x: Int): Int -> 1\n| g(y) -> 2 under y > 3\n");
    assert!(error.contains("2:5: the first parameter of `g` is `x` on its declaring line (line 1); call it `x` here too"), "{error}");
}

#[test]
fn an_undeclared_relation_keeps_its_logic_variables_free() {
    let source = "| parent(\"a\", \"b\")\n| parent(\"b\", \"c\")\n| ancestor(x, y) -> parent(x, y)\n| ancestor(x, z) -> parent(x, y), ancestor(y, z)\n";
    parse(source).unwrap();
}

#[test]
fn a_second_declaration_and_a_declaration_without_a_clause_are_errors() {
    let error = parse_error("| f(x: Int): Int -> 1\n| f(x: Int): Int -> 2\n| h(n: Int): Int\n");
    assert!(
        error.contains("rule `f` with 1 argument is declared twice; it is declared at line 1"),
        "{error}"
    );
    assert!(
        error.contains("rule `h` is declared but has no clause"),
        "{error}"
    );
}

#[test]
fn a_declaration_after_a_clause_is_an_error() {
    let error = parse_error("| f(x: Int) -> 1\n| f(x): Int -> 2 under x > 1\n");
    assert!(error.contains("the declaration of `f` must be the family's first line, but its clause at line 1 comes before it"), "{error}");
}

#[test]
fn a_bodiless_declaration_names_its_parameters() {
    let error = parse_error("| fib(0: Int): Int\n| fib(n) -> n\n");
    assert!(
        error.contains("a declaration names each parameter"),
        "{error}"
    );
}

#[test]
fn a_rule_scope_member_declares_on_its_own_first_line() {
    let source = "# unit Ore \"DKK øre\"\n# Case(gross: Ore) {\n    | loss(): Ore -> 0\n    | exception big loss() -> gross under gross > Ore(10)\n}\n= c = Case(gross = Ore(50))\n= l: Ore = c.loss()\n@ print(l)\n";
    assert!(
        check_errors(source).is_empty(),
        "{:?}",
        check_errors(source)
    );
    assert_eq!(run(source), "50");
}

#[test]
fn the_migration_writes_types_once_and_declares_the_inferred_result() {
    let directory = std::env::temp_dir().join(format!("rule-result-types-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("m.runa");
    let source = "# unit Ore \"DKK øre\"\n| zero() -> Ore(0)\n| days(year: Int, month: Int) -> 31\n| days(year: Int, month: Int) -> 30 under month == 4\n| exception feb days(year: Int, month: Int) -> 28 under month == 2\n| g(x: Int) -> 1\n| g(y: Int) -> 2 under y > 3\n";
    std::fs::write(&file, source).unwrap();
    let migration = migrate_rule_declarations(source, file.to_str().unwrap()).unwrap();
    assert_eq!(
        migration.source,
        "# unit Ore \"DKK øre\"\n| zero(): Ore -> 0\n| days(year: Int, month: Int): Int -> 31\n| days(year, month) -> 30 under month == 4\n| exception feb days(year, month) -> 28 under month == 2\n| g(x: Int) -> 1\n| g(y: Int) -> 2 under y > 3\n"
    );
    assert_eq!(migration.declared.len(), 2, "{:?}", migration.declared);
    assert!(
        migration
            .skipped
            .iter()
            .any(|skip| skip.contains("`g`") && skip.contains("call parameter 1 `x` and `y`")),
        "{:?}",
        migration.skipped
    );
    // Idempotent: nothing left to rewrite but the reported family.
    let again = migrate_rule_declarations(&migration.source, file.to_str().unwrap()).unwrap();
    assert_eq!(again.source, migration.source);
    std::fs::remove_dir_all(&directory).ok();
}
