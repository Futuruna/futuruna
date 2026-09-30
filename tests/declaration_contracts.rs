use futuruna::{
    eval_source_with_prelude, parse_prelude, prepend_prelude, Lexer, Parser, TypeChecker,
};
use std::path::PathBuf;
use std::process::Command;

fn diagnostics(source: &str) -> Vec<futuruna::Diagnostic> {
    let statements = Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("fixture parses");
    TypeChecker::check_with_diagnostics(
        &prepend_prelude(parse_prelude(), &statements),
        None,
        source,
    )
}

/// The program is rejected with an error containing `fragment` located on
/// `line` (1-based).
fn assert_rejected_at(source: &str, fragment: &str, line: usize) {
    let errors = diagnostics(source);
    let error = errors
        .iter()
        .find(|error| error.message.contains(fragment))
        .unwrap_or_else(|| panic!("expected `{fragment}` for:\n{source}\ngot {errors:?}"));
    let span = error
        .span
        .unwrap_or_else(|| panic!("`{fragment}` has no location: {errors:?}"));
    assert_eq!(span.start_line_col(source).0, line, "{source}: {errors:?}");
}

fn temp_program(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "futuruna-declaration-contracts-{}-{name}.runa",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn runa(arguments: &[&str], path: &PathBuf) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(arguments)
        .arg(path)
        .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
        .output()
        .unwrap()
}

/// Interpreted and compiled execution both print `expected`.
fn assert_both_modes(name: &str, source: &str, expected: &str) {
    assert!(diagnostics(source).is_empty(), "{:?}", diagnostics(source));
    assert_eq!(
        eval_source_with_prelude(source, true).unwrap().trim(),
        expected
    );
    let path = temp_program(name, source);
    let output = runa(&["run"], &path);
    let _ = std::fs::remove_file(&path);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
}

const MISSPELLED_EXCEPTION: &str = "# Person(name: String, income: Int)
| tax(p: Person) -> p.income / 4
| exception big taxx(p) -> 0 under p.income > 100
= p = Person(\"A\", 1000)
@ print(show(tax(p)))
";

#[test]
fn exception_without_a_base_rule_is_rejected_at_its_head() {
    assert_rejected_at(MISSPELLED_EXCEPTION, "overrides rule `taxx`", 3);
    assert_rejected_at(
        "# Case(income: Int) {\n    | rate() -> 25\n    | exception low ratee() -> 20 under income < 10\n}\n",
        "overrides rule `ratee`",
        3,
    );
    // Same name, different arity is a different family.
    assert_rejected_at(
        "| rate(x) -> 25\n| exception low rate(x, y) -> 10\n",
        "overrides rule `rate` with 2 arguments",
        2,
    );
    assert!(diagnostics(
        "| rate(x: Int) -> 25\n| exception youth rate(x: Int) -> 10 under x < 18\n"
    )
    .is_empty());
}

#[test]
fn int_is_not_accepted_for_a_float_field_or_parameter() {
    let source = "# Rates(percent: Float)
> tax(income: Float, r: Rates) -> Float { income * (r.percent / 100) }
@ print(show(tax(1000.0, Rates(25))))
";
    assert_rejected_at(source, "field `percent` expects `Float`, got `Int`", 3);
    assert_rejected_at(
        "> half(x: Float) -> Float { x / 2.0 }\n@ print(show(half(3)))\n",
        "argument `x` to `half` expects `Float`, got `Int`",
        2,
    );
    assert_rejected_at(
        "# Shape = Circle(radius: Float) | Square(side: Float)\n= c = Circle(\"big\")\n",
        "field `radius` expects `Float`, got `String`",
        2,
    );
    assert!(
        diagnostics("# Rates(percent: Float)\n= a = Rates(25.0)\n= b = Rates(to_float(25))\n")
            .is_empty()
    );
}

#[test]
fn interpreter_and_audit_refuse_programs_the_checker_rejects() {
    let path = temp_program(
        "float-field",
        "@ print(\"must not run\")\n# Rates(percent: Float)\n= r = Rates(25)\n| share() -> r.percent\n",
    );
    for arguments in [&[][..], &["audit"][..]] {
        let output = runa(arguments, &path);
        assert!(!output.status.success(), "{arguments:?}: {output:?}");
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("must not run"),
            "{arguments:?}: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("expects `Float`, got `Int`"),
            "{arguments:?}: {output:?}"
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn bind_requires_result_option_or_effect_operation() {
    assert_rejected_at(
        "> seven() -> Int { 7 }
> compute() -> Result(Int, String) {
    = a <- seven()
    Ok(a * 2)
}
@ print(show(compute()))
",
        "`<-` needs a `Result` or `Option` value, got `Int`",
        3,
    );
    let source = "> safe_div(a: Int, b: Int) -> Result(Int, String) {
    if b == 0 { Err(\"zero\") } else { Ok(a / b) }
}
> compute(b: Int) -> Result(Int, String) {
    = q <- safe_div(100, b)
    Ok(q + 1)
}
@ print(show(compute(4)))
@ print(show(compute(0)))
";
    assert_both_modes("bind-result", source, "Ok(26)\nErr(zero)");
}

#[test]
fn sum_type_fields_need_every_variant_or_a_refining_match() {
    assert_rejected_at(
        "# Income = Salary(amount: Int) | Pension(amount: Int, supplement: Int)
> total(i: Income) -> Int { i.amount + i.supplement }
",
        "field `supplement` exists only on variant `Pension` of `Income`",
        2,
    );
    assert_rejected_at(
        "# Expr = Lit(value: Int) | Add(left: Expr, right: Expr)
> ev(e: Expr) -> Int {
    match e {
        | Lit -> e.value
        | Add -> ev(e.left) + e.value
    }
}
",
        "variant `Add` of `Expr` has no field `value`",
        5,
    );
    assert_rejected_at(
        "# A = Fixed(v: Int) | Text(v: String)\n> g(a: A) -> Int { a.v + 1 }\n",
        "field `v` has different types in the variants of `A`",
        2,
    );
    // A name rebound inside the arm is no longer the refined subject.
    assert_rejected_at(
        "# Expr = Lit(value: Int) | Add(left: Expr, right: Expr)
> ev(e: Expr, other: Expr) -> Int {
    match e {
        | Lit -> {
            = e = other
            e.value
        }
        | Add -> 0
    }
}
",
        "exists only on variant `Lit`",
        6,
    );
}

#[test]
fn refined_and_shared_sum_type_fields_run_in_both_modes() {
    let source = "# Income = Salary(amount: Int) | Pension(amount: Int, supplement: Int)
> total(i: Income) -> Int {
    match i {
        | Salary -> i.amount
        | Pension -> i.amount + i.supplement
    }
}
> base(i: Income) -> Int { i.amount }
@ print(show(total(Salary(100))))
@ print(show(total(Pension(100, 5))))
@ print(show(base(Pension(7, 5))))
";
    assert_both_modes("sum-fields", source, "100\n105\n7");
}

#[test]
fn rule_scope_members_resolve_on_lambda_parameters_and_nested_receivers() {
    let source = "# Person(income: Int) {
    | rate() -> income / 100
}
# Household(members: List(Person), bonus: Int) {
    | total() -> foldl(map(members, |p| p.rate()), 0, |acc, x| acc + x) + bonus
    | first_rate() -> head(members).rate()
}
= h = Household([Person(500), Person(700), Person(900)], 0)
@ print(show(h.total()))
@ print(show(h.first_rate()))
> sum_rates(ps: List(Person)) -> Int { foldl(map(ps, |p| p.rate()), 0, |acc, x| acc + x) }
@ print(show(sum_rates([Person(100), Person(200)])))
# Box(p: Person) {
    | r() -> p.rate()
}
# Twin(x: Box, y: Box) {
    | s() -> x.r() + y.r()
}
@ print(show(Twin(Box(Person(100)), Box(Person(300))).s()))
";
    assert_both_modes("rule-scope-receivers", source, "21\n5\n3\n4");
    // A receiver of a known non-RuleScope type still has no scoped members.
    assert_rejected_at(
        "# Person(age: Int)\n# TaxCase(person: Person) {\n    | allowance() -> person.age\n}\n= person = Person(40)\n= bad = person.allowance()\n",
        "scoped member `allowance` must be called on a RuleScope value",
        6,
    );
}

#[test]
fn variants_may_not_reuse_builtin_type_names() {
    assert_rejected_at(
        "# Money = Int\n",
        "variant `Int` of `Money` reuses the built-in type name `Int`",
        1,
    );
    assert_rejected_at(
        "= x = 1\n# Label = Text | String\n",
        "variant `String` of `Label` reuses the built-in type name",
        2,
    );
    assert!(diagnostics("# Money(amount: Int)\n= m = Money(42)\n").is_empty());
}

#[test]
fn prelude_and_list_constructor_names_are_reserved() {
    assert_rejected_at(
        "# Verdict = Ok | Err(reason: String)\n",
        "constructor `Err` belongs to the built-in `Result` type",
        1,
    );
    assert_rejected_at(
        "\n# Benefit = None | Partial(pct: Int) | Full\n",
        "constructor `None` belongs to the built-in `Option` type",
        2,
    );
    assert_rejected_at(
        "# L = Nil | Cons(h: Int, t: L)\n",
        "constructor `Nil` is reserved for built-in lists",
        1,
    );
    // Redefining a prelude type itself keeps its constructors.
    assert!(
        diagnostics("# Option(a) = None | Some(a)\n# Pair(left: Int, right: Int)\n").is_empty()
    );
}

#[test]
fn trait_impls_match_their_trait() {
    let unknown = "# Color = Red | Green
# impl Nope for Color {
    > display(self) -> String { \"x\" }
}
";
    assert_rejected_at(unknown, "unknown trait `Nope`", 2);
    assert_rejected_at(
        "# trait Show2 {\n    > display(self) -> String\n}\n# impl Show2 for Ghost {\n    > display(self) -> String { \"y\" }\n}\n",
        "unknown type `Ghost`",
        4,
    );
    let signature = "# trait Show2 {
    > display(self) -> String
    > weight(self, factor: Int) -> Int
}
# Color = Red | Green
# impl Show2 for Color {
    > display(self) -> Int { 3 }
    > weight(self, factor: Float) -> Int { 1 }
    > extra(self) -> Int { 1 }
}
";
    assert_rejected_at(signature, "method `display` must return `String`", 7);
    assert_rejected_at(signature, "parameter `factor` must have type `Int`", 8);
    assert_rejected_at(signature, "defines `extra`, which is not a method", 9);
    assert_rejected_at(
        "# trait Show2 {\n    > display(self) -> String\n    > size(self) -> Int\n}\n# Color = Red\n# impl Show2 for Color {\n    > display(self) -> String { \"r\" }\n}\n",
        "is missing method: size",
        6,
    );
    assert_rejected_at(
        "# trait Show2 {\n    > display(self, width: Int) -> String\n}\n# Color = Red\n# impl Show2 for Color {\n    > display(self) -> String { \"r\" }\n}\n",
        "takes 1 parameter but trait `Show2` declares 2",
        6,
    );
    let valid = "# trait Printable {
    > display(self) -> String
    > greet(self) -> String { \"hello\" }
}
# Color = Red | Green
# impl Printable for Color {
    > display(self) -> String {
        match self {
            | Red -> \"red\"
            | Green -> \"green\"
        }
    }
}
@ print(Red.display())
";
    assert!(diagnostics(valid).is_empty(), "{:?}", diagnostics(valid));
}
