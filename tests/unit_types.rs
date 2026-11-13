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
         = start: Ore = 0\n",
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
         > bounded(a: Ore, b: Ore) -> Ore { clamp(a, 0, max_int(abs(a), b)) + min_int(a, b) }\n\
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

> value(h: Holding) -> Ore {
    h.price * (Int(h.quantity) / 1000000) + h.price * (Int(h.quantity) % 1000000) / 1000000
}

> fee(x: Ore) -> Ore { x / 100 }

> total(holdings: List(Holding)) -> Ore { foldl(holdings, 0, |sum, h| sum + value(h)) }

= holdings = [Holding(quantity = 2500000, price = 10000), Holding(quantity = 1, price = 3000000)]
@ print(show(total(holdings)))
@ print(show(fee(total(holdings))))
@ print(show(min_int(total(holdings), 100) / Ore(10)))
";

#[test]
fn values_are_plain_numbers_when_interpreted() {
    let output = runa(&[], RUNNABLE);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).ends_with("25003\n250\n10\n"),
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
