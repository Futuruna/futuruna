//! Units of measure (`# unit Ore "DKK øre"`): the checker's arithmetic rules
//! and messages, the interpreter, generated Rust, the solver encoding and the
//! calculation contract. See docs/rfcs/unit-types.md.

use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

const UNITS: &str = "# unit Ore \"DKK øre\"\n# unit Micro \"millionths of a share\"\n";

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "futuruna-unit-types-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn runa(arguments: &[&str], source: &str) -> Output {
    let dir = temp_dir();
    let path = dir.join("model.runa");
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(arguments)
        .arg(&path)
        .env("FUTURUNA_DISABLE_CALCULATION_CACHE", "1")
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(dir);
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn rejected(body: &str, message: &str) {
    let output = runa(&["check", "--frontend"], &format!("{UNITS}{body}"));
    assert!(!output.status.success(), "{body}: accepted");
    assert!(
        stderr(&output).contains(message),
        "{body}: expected `{message}` in\n{}",
        stderr(&output)
    );
}

fn accepted(body: &str) {
    let output = runa(&["check", "--frontend"], &format!("{UNITS}{body}"));
    assert!(output.status.success(), "{body}:\n{}", stderr(&output));
}

#[test]
fn arithmetic_keeps_one_unit() {
    accepted(
        "> sum(a: Ore, b: Ore) -> Ore { a + b - a % b }\n\
         > scale(a: Ore, n: Int) -> Ore { a * n + n * a + a / n + a % n }\n\
         > ratio(a: Ore, b: Ore) -> Int { a / b }\n\
         > negate(a: Ore) -> Ore { -a }\n\
         > larger(a: Ore, b: Ore) -> Bool { a > b && a != b }\n",
    );
}

#[test]
fn numbers_written_in_the_source_take_the_unit_of_their_place() {
    accepted(
        "# Account(cash: Ore, holdings: List(Ore), limit: Ore?)\n\
         > positive(a: Ore) -> Ore { if a > 0 { a } else { 0 } }\n\
         > opened() -> Account { Account(cash = 0, holdings = [0, 100 * 100], limit = Some(-5)) }\n\
         > fee(a: Ore) -> Ore { a + 100 }\n\
         = start: Ore = 0\n\
         = amounts: List(Ore) = [-5, 0, 100]\n\
         = fees_hold = all(filter(amounts, |a| a >= 0), |a| fee(a) >= a)\n",
    );
    rejected(
        "> fee(a: Ore) -> Ore { a + 100 }\n\
         = counts = [0, 1]\n\
         = fees = map(counts, |n| fee(n))\n",
        "argument `a` to `fee` expects `Ore`, got `Int`",
    );
}

#[test]
fn rule_families_and_folds_take_the_unit_of_their_values() {
    accepted(
        "# Holding(amount: Ore)\n\
         | fee(x: Ore) -> 0 under x < 100\n\
         | fee(x: Ore) -> x / 10\n\
         > charged(x: Ore) -> Ore { fee(x) + x }\n\
         > total(holdings: List(Holding)) -> Ore { foldl(holdings, 0, |sum, h| sum + h.amount) }\n",
    );
    rejected(
        "> counted(xs: List(Ore), n: Int) -> Ore { foldl(xs, 0, |sum, x| sum + n) }\n",
        "return type mismatch: function `counted` expects `Ore` but expression has `Int`",
    );
}

#[test]
fn conversions_and_unit_keeping_builtins() {
    accepted(
        "> value(quantity: Micro, price: Ore) -> Ore {\n    price * (Int(quantity) / 1000000) + price * (Int(quantity) % 1000000) / 1000000\n}\n\
         > kroner(k: Int) -> Ore { Ore(k * 100) }\n\
         > bounded(a: Ore, b: Ore) -> Ore { clamp(min_int(a, b), 0, max_int(abs(a), b)) }\n\
         > flat(a: Ore, b: Ore) -> Ore { clamp(a, 0, max_int(abs(a), b)) + min_int(a, b) }\n\
         > total(xs: List(Ore)) -> Ore { sum_list(xs) }\n",
    );
    rejected(
        "> mixed(a: Ore, q: Micro) -> Ore { min_int(a, q) }\n",
        "`min_int` mixes units `Ore` and `Micro`; every argument has the same unit",
    );
}

#[test]
fn mixing_units_is_rejected() {
    rejected(
        "> mixed(q: Micro, p: Ore) -> Ore { p + q }\n",
        "unsupported operands for operator `+`: `Ore` and `Micro` are different units; state the conversion in a function that uses `Int(...)` and `Ore(...)` or `Micro(...)`",
    );
    rejected(
        "> compared(q: Micro, p: Ore) -> Bool { p < q }\n",
        "unsupported operands for operator `<`: `Ore` and `Micro` are different units",
    );
    rejected(
        "> converted(q: Micro) -> Ore { Ore(q) }\n",
        "`Ore(...)` gives a plain `Int` the unit `Ore`, got `Micro`; state the conversion from `Micro` in a function that uses `Int(...)`",
    );
    rejected(
        "> passed(q: Micro) -> Ore { q }\n",
        "return type mismatch: function `passed` expects `Ore` but expression has `Micro`",
    );
    rejected(
        "> takes(p: Ore) -> Ore { p }\n> gives(q: Micro) -> Ore { takes(q) }\n",
        "argument `p` to `takes` expects `Ore`, got `Micro`; `Micro` and `Ore` are different units",
    );
}

#[test]
fn branches_take_the_unit_of_their_unit_arm() {
    accepted(
        "> value_or_zero(price: Ore?) -> Ore {\n    match price {\n        | Some(p) -> p\n        | None -> 0\n    }\n}\n\
         > zero_or_value(price: Ore?) -> Ore {\n    match price {\n        | None -> 0\n        | Some(p) -> p\n    }\n}\n\
         > doubled(price: Ore?) -> Ore { 2 * match price {\n    | Some(p) -> p\n    | None -> 0\n} }\n\
         > floor(a: Ore) -> Ore { if a < 0 { 0 } else { a } }\n\
         | kept_ore(closed: Bool, carried_ore: Ore) -> carried_ore\n\
         | exception lost kept_ore(closed: Bool, carried_ore: Ore) -> 0 under closed\n",
    );
    rejected(
        "> mixed(price: Ore?, q: Micro) -> Ore {\n    match price {\n        | Some(p) -> p\n        | None -> q\n    }\n}\n",
        "match arms must have the same type; first arm is `Ore` but arm 2 is `Micro`",
    );
}

#[test]
fn callbacks_see_the_units_of_their_elements() {
    accepted(
        "# Part(country: String, gross_ore: Ore)\n\
         > merged(groups: List(List(Part))) -> List(Part) {\n    map(groups, |state| Part(\n        country = head(state).country,\n        gross_ore = sum_list(map(state, |d| d.gross_ore))\n    ))\n}\n",
    );
    rejected(
        "# In(price_micro: Micro)\n# Out(price_ore: Ore)\n\
         > all_out(xs: List(In)) -> List(Out) { map(xs, |p| Out(price_ore = p.price_micro)) }\n",
        "constructor `Out` field `price_ore` expects `Ore` but expression has `Micro`; `Micro` and `Ore` are different units",
    );
    rejected(
        "# In(price_micro: Micro)\n\
         > big(xs: List(In), limit: Ore) -> List(In) { filter(xs, |p| p.price_micro > limit) }\n",
        "unsupported operands for operator `>`: `Micro` and `Ore` are different units",
    );
}

#[test]
fn a_wrong_unit_in_a_constructor_field_is_one_error() {
    let source = format!(
        "{UNITS}# In(price_micro: Micro)\n# Out(price_ore: Ore)\n\
         > one(x: In) -> Out {{ Out(price_ore = x.price_micro) }}\n"
    );
    let output = runa(&["check", "--frontend"], &source);
    let errors = stderr(&output);
    assert!(!output.status.success(), "accepted");
    assert!(
        errors.contains("constructor `Out` field `price_ore` expects `Ore` but expression has `Micro`; `Micro` and `Ore` are different units: state the conversion in a function that uses `Int(...)` and `Ore(...)`"),
        "{errors}"
    );
    assert!(errors.contains("1 type error"), "{errors}");
    assert!(!errors.contains("return type mismatch"), "{errors}");
}

#[test]
fn a_plain_int_value_does_not_take_a_unit() {
    rejected(
        "> plain(p: Ore, n: Int) -> Ore { p + n }\n",
        "unsupported operands for operator `+`: `Ore` and a plain `Int`; write `Ore(n)` when the number is an amount in `Ore`, or take the unit off with `Int(...)`",
    );
    rejected(
        "> compared(p: Ore, n: Int) -> Bool { p > n }\n",
        "unsupported operands for operator `>`: `Ore` and a plain `Int`",
    );
    rejected(
        "> takes(p: Ore) -> Ore { p }\n> gives(n: Int) -> Ore { takes(n) }\n",
        "argument `p` to `takes` expects `Ore`, got `Int`; write `Ore(n)` when the number is an amount in `Ore`",
    );
    rejected(
        "> kept(p: Ore) -> Int { p }\n",
        "return type mismatch: function `kept` expects `Int` but expression has `Ore`",
    );
}

#[test]
fn squared_and_inverse_units_are_rejected() {
    rejected(
        "> squared(p: Ore) -> Ore { p * p }\n",
        "unsupported operands for operator `*`: `Ore` times `Ore` has no unit; multiply by a plain `Int`, or take the unit off one side with `Int(...)`",
    );
    rejected(
        "> inverse(p: Ore, n: Int) -> Int { n / p }\n",
        "unsupported operands for operator `/`: a plain `Int` divided by `Ore` has no unit; divide by a plain `Int`, or take the unit off with `Int(...)`",
    );
    rejected(
        "> remainder(p: Ore) -> Int { 100 % p }\n",
        "unsupported operands for operator `%`: a plain `Int` remainder by `Ore` has no unit",
    );
}

#[test]
fn unit_declarations_are_checked() {
    let output = runa(&["check", "--frontend"], "# unit Ore\n");
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unit `Ore` needs a description of what it counts"),
        "{}",
        stderr(&output)
    );
    rejected(
        "# unit Int \"whole numbers\"\n",
        "unit `Int` would rename the built-in type `Int`; choose another name",
    );
    rejected(
        "# unit Ore \"Danish øre\"\n",
        "unit `Ore` is declared twice, as \"DKK øre\" and as \"Danish øre\"; declare it once",
    );
    // `unit` stays an ordinary name outside a declaration.
    accepted("# Field(unit: String?)\n= f = Field(unit = None)\n");
}

const RUNNABLE: &str = "# unit Ore \"DKK øre\"
# unit Micro \"millionths of a share\"
# Holding(quantity: Micro, price: Ore)
# Account(cash: Ore) {
    | charge() -> 0 under cash < 1000
    | charge() -> cash / 100
}

> value(h: Holding) -> Ore {
    h.price * (Int(h.quantity) / 1000000) + h.price * (Int(h.quantity) % 1000000) / 1000000
}

| fee(x: Ore) -> 0 under x < 1000
| fee(x: Ore) -> x / 100

> kept(x: Ore) -> Ore { x - x / 10 }

> total(holdings: List(Holding)) -> Ore { foldl(holdings, 0, |sum, h| sum + value(h)) }

= holdings = [Holding(quantity = 2500000, price = 10000), Holding(quantity = 1, price = 3000000)]
@ print(show(total(holdings)))
@ print(show(fee(total(holdings))))
@ print(show(fee(Ore(500))))
@ print(show(Account(cash = 250000).charge()))
@ print(show(Account(cash = 999).charge()))
@ print(show(kept(total(holdings))))
@ print(show(min_int(total(holdings), 100) / Ore(10)))
";

#[test]
fn values_are_plain_numbers_when_interpreted() {
    let output = runa(&[], RUNNABLE);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).ends_with("25003\n250\n0\n2500\n0\n22503\n10\n"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn generated_rust_compiles_with_units_erased() {
    let output = runa(&["check"], RUNNABLE);
    assert!(output.status.success(), "{}", stderr(&output));
    let emitted = runa(&["emit"], RUNNABLE);
    assert!(emitted.status.success(), "{}", stderr(&emitted));
    let rust = stdout(&emitted);
    assert!(rust.contains("pub type Ore = i64;"), "{rust}");
    assert!(!rust.contains("Ore("), "{rust}");
    // The rule family and the RuleScope member over `Ore` are emitted with
    // their `Int` representation, not skipped.
    assert!(rust.contains("fn fee(x: i64) -> i64"), "{rust}");
    assert!(rust.contains("fn charge(&self) -> i64"), "{rust}");
}

#[test]
fn compiled_and_interpreted_runs_agree() {
    let dir = temp_dir();
    std::fs::write(dir.join("model.runa"), RUNNABLE).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(["test", "--roundtrip"])
        .arg(&dir)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let errors = stderr(&output);
    assert!(output.status.success(), "{errors}");
    assert!(errors.contains("Roundtrip: 1 matched"), "{errors}");
}

/// `Trade` is declared by two types, so every use below is resolved through
/// its owner: from a declared binding, written `AskEvent.Trade`, or from a
/// builder's return type. A literal in a unit field takes the field's unit
/// in each form, as it does for an unshared constructor.
const SHARED_CONSTRUCTORS: &str = "# unit Ore \"DKK øre\"
# unit Micro \"millionths of a share\"
# Event = Trade(quantity: Micro, amount: Ore) | Closed
# AskEvent = Trade(quantity: Micro, amount: Ore, fee: Ore) | Closed

> ask_trade(quantity: Micro) -> AskEvent { Trade(quantity = quantity, amount = 990, fee = 0) }
> event_trade() -> Event { Trade(quantity = 3000000, amount = 0) }

> paid(e: AskEvent) -> Ore {
    match e {
        | Trade -> e.amount + e.fee
        | Closed -> 0
    }
}

> traded(e: Event) -> Micro {
    match e {
        | Trade -> e.quantity
        | Closed -> Micro(0)
    }
}

= declared: AskEvent = Trade(quantity = 10000000, amount = 9900, fee = 0)
= qualified = AskEvent.Trade(quantity = 5, amount = 0, fee = 25)
= built = ask_trade(Micro(7))
= positional: Event = Trade(2000000, 100)
@ print(show(paid(declared) + paid(qualified) + paid(built)))
@ print(show(traded(positional) + traded(event_trade())))
";

#[test]
fn literals_take_the_unit_of_a_shared_constructor_field() {
    let output = runa(&["check", "--frontend"], SHARED_CONSTRUCTORS);
    assert!(output.status.success(), "{}", stderr(&output));
    let output = runa(&[], SHARED_CONSTRUCTORS);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "10915\n5000000\n");
    let dir = temp_dir();
    std::fs::write(dir.join("model.runa"), SHARED_CONSTRUCTORS).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(["test", "--roundtrip"])
        .arg(&dir)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let errors = stderr(&output);
    assert!(output.status.success(), "{errors}");
    assert!(errors.contains("Roundtrip: 1 matched"), "{errors}");
}

#[test]
fn a_wrong_unit_in_a_shared_constructor_field_is_rejected() {
    rejected(
        "# Event = Trade(quantity: Micro, amount: Ore) | Closed\n\
         # AskEvent = Trade(quantity: Micro, amount: Ore, fee: Ore) | Closed\n\
         = cost: Ore = 5\n\
         = trade = AskEvent.Trade(quantity = cost, amount = 0, fee = 0)\n",
        "constructor `AskEvent.Trade` field `quantity` expects `Micro`, got `Ore`",
    );
}

fn z3_available() -> bool {
    Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn the_solver_sees_units_as_int() {
    let source = "# unit Ore \"DKK øre\"
> positive_part(a: Ore) -> Ore { if a > 0 { a } else { 0 } }
| never_negative: x -> positive_part(x) >= 0
| always_positive: x -> positive_part(x) > 0
";
    let output = runa(&["verify"], source);
    let report = stdout(&output);
    if !z3_available() {
        assert!(report.contains("Z3 not found"), "{report}");
        return;
    }
    assert!(report.contains("PROVED: |never_negative|"), "{report}");
    assert!(
        report.contains("COUNTEREXAMPLE found for |always_positive|"),
        "{report}"
    );
    assert!(!output.status.success(), "{report}");
}

const CALCULATION: &str = "# unit Ore \"DKK øre\"
# CalculationField(path: String, label: String, unit: String?)
# CalculationGuide(fields: List(CalculationField))
# impl Meta for CalculationGuide {}

# Account(cash_ore: Ore, holdings: List(Ore), shares: Int)
# Result(total_ore: Ore)

= guide = CalculationGuide(fields = [
    CalculationField(path = pathof(Account::cash_ore), label = \"Cash\", unit = UNIT),
    CalculationField(path = pathof(Account::shares), label = \"Shares\", unit = None)
])

--@label:account_total::meta:guide--
--@begin:account_total--
@ calculate(\"Total of an account\")
> account_total(a: Account) -> Result { Result(total_ore = a.cash_ore + sum_list(a.holdings)) }
--@end:account_total--
";

#[test]
fn the_contract_names_units_and_takes_their_description() {
    let output = runa(&["schema"], &CALCULATION.replace("UNIT", "None"));
    assert!(output.status.success(), "{}", stderr(&output));
    let contract: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(
        contract["units"],
        serde_json::json!([{"name": "Ore", "description": "DKK øre"}])
    );
    let account = contract["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|definition| definition["name"] == "Account")
        .unwrap();
    let fields = &account["variants"][0]["fields"];
    assert_eq!(
        fields[0]["type"],
        serde_json::json!({"kind": "primitive", "name": "Int", "unit": "Ore"})
    );
    assert_eq!(
        fields[1]["type"]["item"],
        serde_json::json!({"kind": "primitive", "name": "Int", "unit": "Ore"})
    );
    assert_eq!(
        fields[2]["type"],
        serde_json::json!({"kind": "primitive", "name": "Int"})
    );
    let metadata = contract["field_metadata"].as_array().unwrap();
    let cash = metadata
        .iter()
        .find(|field| field["path"] == "cash_ore")
        .unwrap();
    assert_eq!(cash["unit"], "DKK øre");

    let said_twice = runa(
        &["schema"],
        &CALCULATION.replace("UNIT", "Some(\"DKK øre\")"),
    );
    assert!(!said_twice.status.success());
    assert!(
        stderr(&said_twice).contains(
            "calculation field `cash_ore` has the unit `Ore` (\"DKK øre\") from its type; remove `unit = Some(\"DKK øre\")` from its field metadata"
        ),
        "{}",
        stderr(&said_twice)
    );
}

#[test]
fn a_fold_accumulator_has_the_type_of_its_start() {
    rejected(
        "# Holding(value_ore: Ore, quantity_micro: Micro)\n\
         > total(xs: List(Holding)) -> Int { foldl(xs, 0, |t, x| t + x.value_ore + x.quantity_micro) }\n",
        "`Ore` and `Micro` are different units",
    );
    rejected(
        "> summed(xs: List(Int)) -> Ore { foldl(xs, Ore(0), |t, n| t + n) }\n",
        "unsupported operands for operator `+`: `Ore` and a plain `Int`",
    );
    accepted(
        "# Holding(value_ore: Ore, quantity_micro: Micro)\n\
         > counted(xs: List(Holding)) -> Int { foldl(xs, 0, |n, x| n + 1) }\n\
         > positive(xs: List(Holding)) -> Ore { foldl(xs, 0, |t, x| if x.value_ore > 0 { t + x.value_ore } else { t }) }\n\
         > summed(xs: List(Int)) -> Ore { foldl(xs, Ore(0), |t, n| t + Ore(n)) }\n",
    );
}

#[test]
fn equality_compares_the_units_inside_values() {
    rejected(
        "> same(a: Ore, m: Micro) -> Bool { [a] == [m] }\n",
        "unsupported operands for operator `==`: `List(Ore)` and `List(Micro)`: `Ore` and `Micro` are different units",
    );
    rejected(
        "> same(a: Ore, n: Int) -> Bool { Some(a) != Some(n) }\n",
        "unsupported operands for operator `!=`: `Option(Ore)` and `Option(Int)`: `Ore` and a plain `Int`",
    );
    accepted(
        "> same(a: Ore, b: Ore) -> Bool { [a] == [b] && [a] != [0, 5] && Some(a) == Some(0) }\n",
    );
}

#[test]
fn a_named_closure_is_checked_with_the_types_of_its_arguments() {
    let source =
        format!("{UNITS}> add(a: Ore, m: Micro) -> Ore {{\n    = f = |x| x + a\n    f(m)\n}}\n");
    let output = runa(&["check", "--frontend"], &source);
    let errors = stderr(&output);
    assert!(!output.status.success(), "accepted");
    assert!(
        errors.contains("`Micro` and `Ore` are different units"),
        "{errors}"
    );
    assert!(
        errors.contains("in closure `f` called with (Micro)"),
        "{errors}"
    );
    accepted("> add(a: Ore, b: Ore) -> Ore {\n    = f = |x| x + a\n    f(b) + f(5)\n}\n");
}

const SCALED: &str = "# unit Ore \"DKK øre\" = DKK / 100
# unit MicroDkk \"millionths of a krone\" = DKK / 1000000
# unit Micro \"millionths of a unit\" = 1 / 1000000
";

fn scaled_rejected(body: &str, message: &str) {
    let output = runa(&["check", "--frontend"], &format!("{SCALED}{body}"));
    assert!(!output.status.success(), "{body}: accepted");
    assert!(
        stderr(&output).contains(message),
        "{body}: expected `{message}` in\n{}",
        stderr(&output)
    );
}

const CONVERSIONS: &str = "# unit Ore \"DKK øre\" = DKK / 100
# unit MicroDkk \"millionths of a krone\" = DKK / 1000000
# Price(amount: MicroDkk)

> nearest(m: MicroDkk) -> Ore { to_nearest(Ore, m) }
> down(m: MicroDkk) -> Ore { to_floor(Ore, m) }
> up(m: MicroDkk) -> Ore { to_ceiling(Ore, m) }
> truncated(m: MicroDkk) -> Ore { to_toward_zero(Ore, m) }
> finer(o: Ore) -> MicroDkk { to(MicroDkk, o) }
> of_field(p: Price) -> Ore { to_nearest(Ore, p.amount) }
> of_sum(a: MicroDkk, b: MicroDkk) -> Ore { to_nearest(Ore, a + b) }

= amounts: List(MicroDkk) = [15000, 14999, -15000, -14999, 5000, -5000, 1, -1]
@ print(show(map(amounts, |m| nearest(m))))
@ print(show(map(amounts, |m| down(m))))
@ print(show(map(amounts, |m| up(m))))
@ print(show(map(amounts, |m| truncated(m))))
@ print(show(finer(-123)))
@ print(show(of_field(Price(amount = 125000)) + of_sum(10000, 5000)))
@ print(show(map(amounts, |m| to_nearest(Ore, m))))
";

#[test]
fn units_of_one_base_convert_with_a_stated_rounding() {
    let output = runa(&["check", "--frontend"], CONVERSIONS);
    assert!(output.status.success(), "{}", stderr(&output));
    let output = runa(&[], CONVERSIONS);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "[2, 1, -2, -1, 1, -1, 0, 0]\n\
         [1, 1, -2, -2, 0, -1, 0, -1]\n\
         [2, 2, -1, -1, 1, 0, 1, 0]\n\
         [1, 1, -1, -1, 0, 0, 0, 0]\n\
         -1230000\n\
         15\n\
         [2, 1, -2, -1, 1, -1, 0, 0]\n"
    );
    // Generated Rust runs the elaborated arithmetic and agrees.
    let dir = temp_dir();
    std::fs::write(dir.join("model.runa"), CONVERSIONS).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(["test", "--roundtrip"])
        .arg(&dir)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let errors = stderr(&output);
    assert!(output.status.success(), "{errors}");
    assert!(errors.contains("Roundtrip: 1 matched"), "{errors}");
}

#[test]
fn a_conversion_states_exactly_the_rounding_it_does() {
    scaled_rejected(
        "> f(m: MicroDkk) -> Ore { to(Ore, m) }\n",
        "`MicroDkk` to `Ore` divides by 10000; state how it rounds with `to_nearest` (halves away from zero), `to_floor`, `to_ceiling` or `to_toward_zero`",
    );
    scaled_rejected(
        "> f(o: Ore) -> MicroDkk { to_nearest(MicroDkk, o) }\n",
        "`Ore` to `MicroDkk` multiplies by 10000 and is exact, so it does not round; write `to(MicroDkk, ...)`",
    );
    scaled_rejected(
        "> f(q: Micro) -> Ore { to(Ore, q) }\n",
        "`Micro` is `1 / 1000000` and `Ore` is `DKK / 100`; units of different bases do not convert",
    );
    scaled_rejected(
        "# unit Third \"thirds of a krone\" = DKK / 3\n> f(o: Ore) -> Third { to(Third, o) }\n",
        "`Ore` is `DKK / 100` and `Third` is `DKK / 3`; neither is a whole multiple of the other",
    );
    scaled_rejected(
        "> f(o: Ore) -> Ore { to(Ore, o) }\n",
        "the amount is already `Ore`; remove the conversion",
    );
    scaled_rejected(
        "> f(n: Int) -> Ore { to(Ore, n) }\n",
        "`to` converts an amount of another unit, got `Int`; a plain `Int` becomes an amount with `Ore(n)`",
    );
    scaled_rejected(
        "> f() -> Ore { to(Ore, 5) }\n",
        "`to` converts an amount of another unit; a number written in the source has none, so write `Ore(n)`",
    );
    scaled_rejected(
        "# unit Bp \"basis points\"\n> f(b: Bp) -> Ore { to(Ore, b) }\n",
        "unit `Bp` declares no scale, so `to` cannot convert it",
    );
    scaled_rejected(
        "> f(m: MicroDkk) -> Ore { to(m, Ore) }\n",
        "the first argument of `to` is the unit to convert to",
    );
    // Mixing units of one base names the conversion that never rounds.
    scaled_rejected(
        "> f(o: Ore, m: MicroDkk) -> MicroDkk { m + o }\n",
        "`MicroDkk` and `Ore` are different units; convert `Ore` with `to(MicroDkk, ...)`",
    );
    scaled_rejected(
        "> f(m: MicroDkk) -> Ore { Ore(m) }\n",
        "`Ore(...)` gives a plain `Int` the unit `Ore`, got `MicroDkk`; convert it with `to_nearest(Ore, ...)`",
    );
    // A program's own `to` keeps its meaning.
    let output = runa(
        &[],
        &format!("{SCALED}> to(a: Int, b: Int) -> Int {{ a + b }}\n@ print(show(to(2, 3)))\n"),
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "5\n");
}

#[test]
fn unit_scales_are_checked() {
    scaled_rejected(
        "# unit Cent \"cents\" = Ore / 100\n",
        "the base of unit `Cent` is a name of its own, such as `DKK`, or `1`: `Ore` is a unit",
    );
    scaled_rejected(
        "# Money(amount: Int)\n# unit Cent \"cents\" = Money / 100\n",
        "`Money` is a declared type or constructor",
    );
    scaled_rejected(
        "# unit Cent \"cents\" = Int / 100\n",
        "`Int` is a built-in type; write `1` for a plain number",
    );
    scaled_rejected(
        "# DKK(amount: Int)\n",
        "`DKK` is the base of unit `MicroDkk`; choose another name",
    );
    scaled_rejected(
        "# unit DKK \"kroner\"\n",
        "`DKK` is the base of unit `MicroDkk`",
    );
    scaled_rejected(
        "# unit Ore \"DKK øre\" = DKK / 1000\n",
        "unit `Ore` is declared twice, as \"DKK øre\" = DKK / 100 and as \"DKK øre\" = DKK / 1000; declare it once",
    );
    let output = runa(
        &["check", "--frontend"],
        "# unit Ore \"DKK øre\" = DKK / 0\n",
    );
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains(
            "the scale of unit `Ore` divides its base by a whole number of at least 1, got `0`"
        ),
        "{}",
        stderr(&output)
    );
}

#[test]
fn conversions_in_an_imported_file_run_and_prove() {
    let dir = temp_dir();
    std::fs::write(
        dir.join("money.runa"),
        "-- library-hygiene: importable\n\
         # unit Ore \"DKK øre\" = DKK / 100\n\
         # unit MicroDkk \"millionths of a krone\" = DKK / 1000000\n\
         > nearest_ore(m: MicroDkk) -> Ore { to_nearest(Ore, m) }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.runa"),
        "@ import ./money\n\
         > in_micro(o: Ore) -> MicroDkk { to(MicroDkk, o) }\n\
         @ print(show(nearest_ore(15000)))\n\
         @ print(show(in_micro(nearest_ore(-15000))))\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("proofs.runa"),
        "@ import ./money\n\
         | nearest_mirror: m -> m < -9223372036854775807 || nearest_ore(0 - m) == 0 - nearest_ore(m)\n\
         | nearest_exact: m -> Int(nearest_ore(m)) * 10000 == Int(m)\n",
    )
    .unwrap();
    let run = |arguments: &[&str], file: &str| {
        Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(arguments)
            .arg(dir.join(file))
            .output()
            .unwrap()
    };
    let output = run(&[], "main.runa");
    let check = run(&["check"], "main.runa");
    let verify = run(&["verify"], "proofs.runa");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "2\n-20000\n");
    assert!(check.status.success(), "{}", stderr(&check));
    let report = stdout(&verify);
    if !z3_available() {
        assert!(report.contains("Z3 not found"), "{report}");
        return;
    }
    assert!(report.contains("PROVED: |nearest_mirror|"), "{report}");
    assert!(
        report.contains("COUNTEREXAMPLE found for |nearest_exact|"),
        "{report}"
    );
}

#[test]
fn the_contract_carries_unit_scales() {
    let source = CALCULATION.replace("UNIT", "None").replace(
        "# unit Ore \"DKK øre\"",
        "# unit Ore \"DKK øre\" = DKK / 100",
    );
    let output = runa(&["schema"], &source);
    assert!(output.status.success(), "{}", stderr(&output));
    let contract: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(
        contract["units"],
        serde_json::json!([{"name": "Ore", "description": "DKK øre", "scale": {"of": "DKK", "per": 100}}])
    );
}
