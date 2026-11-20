//! `runa cases` on small laws: a ceiling threshold, an age threshold, a
//! date threshold, an `Option` head and a constructor head; rejected
//! results, technical rejections of uncited clauses, new list elements, the
//! minimal seed, the input domain, results that rest on decisions builtins
//! make, and the solver's resource limit (`docs/rfcs/conformance-cases.md`).
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cases")
        .join(name)
}

fn z3_available() -> bool {
    Command::new("z3").arg("--version").output().is_ok()
}

/// The output of one `runa cases` run: the case files by id, the coverage
/// report, stdout and stderr.
struct Generated {
    cases: BTreeMap<String, Value>,
    coverage: Value,
    stdout: String,
    stderr: String,
}

/// Run `runa cases` on the levy into a fresh directory.
fn generate(name: &str, stale: bool) -> (BTreeMap<String, Value>, Value, String) {
    let generated = generate_with(
        name,
        &[
            "levy.entry.runa",
            "--entry",
            "small_levy",
            "--scenario",
            "levy.scenario.runa",
        ],
        stale,
    );
    (generated.cases, generated.coverage, generated.stdout)
}

/// Run `runa cases` with `args` into a fresh directory.
fn generate_with(name: &str, args: &[&str], stale: bool) -> Generated {
    let out = std::env::temp_dir().join(format!("runa-cases-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("create output directory");
    if stale {
        std::fs::write(out.join("old.case.json"), "{}").expect("write stale case");
    }
    let output = Command::new(runa())
        .current_dir(fixture(""))
        .arg("cases")
        .args(args)
        .args(["--out", out.to_str().expect("output path")])
        .output()
        .expect("run runa cases");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    let mut cases = BTreeMap::new();
    let mut coverage = Value::Null;
    for entry in std::fs::read_dir(&out).expect("read output") {
        let path = entry.expect("entry").path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let value: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read file"))
                .expect("JSON");
        if name == "coverage.json" {
            coverage = value;
        } else {
            let id = name
                .strip_suffix(".case.json")
                .expect("case file name")
                .to_string();
            assert_eq!(value["id"], id.as_str(), "the file name is the id");
            cases.insert(id, value);
        }
    }
    let _ = std::fs::remove_dir_all(&out);
    Generated {
        cases,
        coverage,
        stdout,
        stderr,
    }
}

/// Run `runa cases` on the ledger with its seed, refusals as rejections.
fn generate_ledger(name: &str) -> Generated {
    generate_with(
        name,
        &[
            "ledger.entry.runa",
            "--entry",
            "ledger",
            "--seed",
            "ledger-seed.json",
            "--rejected",
            "Refused",
        ],
        false,
    )
}

/// The coverage entry of a clause.
fn clause<'a>(coverage: &'a Value, rule: &str, key: &str) -> &'a Value {
    coverage["clauses"]
        .as_array()
        .expect("clauses")
        .iter()
        .find(|clause| clause["rule"] == rule && clause["clause"] == key)
        .unwrap_or_else(|| panic!("{}.{} in coverage", rule, key))
}

fn covers(case: &Value, rule: &str, clause: &str) -> bool {
    case["covers"]
        .as_array()
        .expect("covers")
        .iter()
        .any(|cover| cover["rule"] == rule && cover["clause"] == clause)
}

fn ceiling(year: i64) -> i64 {
    if year >= 2025 {
        120000
    } else {
        100000
    }
}

#[test]
fn cases_cover_every_clause_and_both_sides_of_each_threshold() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let (cases, coverage, stdout) = generate("levy", true);
    assert!(
        stdout.starts_with("runa cases: cited rule clauses: 9 of 9 covered, of which 0 not in the result (reported, still counted);"),
        "{}",
        stdout
    );
    assert!(
        stdout.trim_end().ends_with(
            "; uncited rule clauses: 0 of 0 covered, of which 0 not in the result; technical rejections not in the suite: 0"
        ),
        "{}",
        stdout
    );
    assert_eq!(
        coverage["summary"],
        "cited rule clauses: 9 of 9 covered, of which 0 not in the result (reported, still counted)"
    );
    assert_eq!(
        coverage["cited"],
        serde_json::json!({ "covered": 9, "clauses": 9, "not_in_result": 0 })
    );
    assert_eq!(
        coverage["uncited"],
        serde_json::json!({ "covered": 0, "clauses": 0, "not_in_result": 0 })
    );
    assert_eq!(coverage["technical"], serde_json::json!([]));
    assert!(!cases.contains_key("old"), "a stale case file is removed");

    // Every case: entry, a complete input, the result, what it covers.
    for case in cases.values() {
        assert_eq!(case["entry"], "small_levy");
        assert!(case["input"]["account"]["deposits"].is_i64());
        assert!(case["expected"]["levy"].is_i64());
        assert!(!case["covers"].as_array().expect("covers").is_empty());
    }

    // The scenario's call, exported as it was written.
    let scenario = &cases["scenario.levy.1"];
    assert_eq!(scenario["kind"], "scenario");
    assert_eq!(scenario["input"]["account"]["deposits"], 150000);
    assert_eq!(scenario["expected"]["levy"], 300);
    assert!(covers(scenario, "rate", "company"));
    assert!(covers(scenario, "closed_in", "closed"));

    // Clause keys, locations and citations.
    let clauses = coverage["clauses"].as_array().expect("clauses");
    let young = clauses
        .iter()
        .find(|clause| clause["rule"] == "rate" && clause["clause"] == "young")
        .expect("rate.young");
    assert_eq!(young["cites"], serde_json::json!(["§ 10, stk. 3"]));
    assert!(young["location"]
        .as_str()
        .unwrap()
        .starts_with("levy.entry.runa:"));
    assert!(clauses
        .iter()
        .any(|clause| clause["rule"] == "levy" && clause["clause"] == "clause-1"));
    for clause in clauses {
        assert!(
            !clause["covered_by"].as_array().unwrap().is_empty(),
            "{} is covered",
            clause
        );
    }

    // The deposit ceiling: below and at do not levy, above does, and only
    // above covers the exception.
    let side = |name: &str| {
        let case = &cases[&format!("levy.over.t1.boundary-{}", name)];
        assert_eq!(case["kind"], format!("boundary-{}", name));
        let input = &case["input"];
        let year = input["year"].as_i64().unwrap();
        (
            input["account"]["deposits"].as_i64().unwrap() - ceiling(year),
            covers(case, "levy", "over"),
        )
    };
    assert_eq!(side("below"), (-1, false));
    assert_eq!(side("at"), (0, false));
    assert_eq!(side("above"), (1, true));

    // The age threshold of a person's rate.
    let age = |name: &str| {
        let case = &cases[&format!("rate.young.t1.boundary-{}", name)];
        assert_eq!(case["input"]["owner"]["kind"]["$variant"], "Person");
        case["input"]["owner"]["age"].as_i64().unwrap()
    };
    assert_eq!((age("below"), age("at"), age("above")), (17, 18, 19));

    // The date threshold: a day either side of 1 July 2024.
    let opened = |name: &str| {
        let case = &cases[&format!("late.after.t1.boundary-{}", name)];
        let day = &case["input"]["account"]["opened"];
        (
            day["year"].as_i64().unwrap(),
            day["month"].as_i64().unwrap(),
            day["day"].as_i64().unwrap(),
        )
    };
    let dates = [opened("below"), opened("at"), opened("above")];
    assert_eq!(
        dates,
        [(2024, 6, 30), (2024, 7, 1), (2024, 7, 2)],
        "{:?}",
        dates
    );

    // The thresholds the report lists, each side with a case.
    let thresholds = coverage["thresholds"].as_array().expect("thresholds");
    assert_eq!(thresholds.len(), 4, "{:#?}", thresholds);
    for threshold in thresholds {
        for side in ["below", "at", "above"] {
            assert!(threshold[side].is_string(), "{}", threshold);
        }
    }
}

#[test]
fn cases_are_the_same_on_every_run() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let (first, first_coverage, _) = generate("first", false);
    let (second, second_coverage, _) = generate("second", false);
    assert_eq!(first, second);
    assert_eq!(first_coverage, second_coverage);
}

/// Every Z3 query `runa cases` makes (and the clause audit it starts from)
/// is bounded by Z3's resource limit, never by time, so a query is decided
/// the same way however loaded the machine: a `z3` first on the PATH logs
/// each script before running the real one.
#[cfg(unix)]
#[test]
fn every_solver_query_has_a_resource_limit_and_no_time_limit() {
    use std::os::unix::fs::PermissionsExt;
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let dir = std::env::temp_dir().join(format!("runa-cases-{}-z3-log", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let log = dir.join("log");
    std::fs::create_dir_all(&log).expect("create log directory");
    let wrapper = dir.join("z3");
    std::fs::write(
        &wrapper,
        "#!/bin/sh\nf=$(mktemp \"$RUNA_Z3_LOG/q.XXXXXX\")\ncat > \"$f\"\nPATH=\"$RUNA_Z3_PATH\" exec z3 \"$@\" < \"$f\"\n",
    )
    .expect("write z3 wrapper");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
        .expect("make the wrapper executable");
    let path = std::env::var("PATH").unwrap_or_default();
    let out = dir.join("out");
    let output = Command::new(runa())
        .current_dir(fixture(""))
        .args([
            "cases",
            "levy.entry.runa",
            "--entry",
            "small_levy",
            "--scenario",
            "levy.scenario.runa",
            "--out",
            out.to_str().expect("output path"),
        ])
        .env("PATH", format!("{}:{}", dir.display(), path))
        .env("RUNA_Z3_LOG", &log)
        .env("RUNA_Z3_PATH", &path)
        .output()
        .expect("run runa cases");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let scripts: Vec<String> = std::fs::read_dir(&log)
        .expect("read log")
        .map(|entry| std::fs::read_to_string(entry.expect("entry").path()).expect("read script"))
        .collect();
    assert!(!scripts.is_empty(), "runa cases asked Z3 nothing");
    for script in &scripts {
        assert!(script.contains("(set-option :rlimit 20000000)\n"), "{}", script);
        assert!(!script.contains(":timeout"), "{}", script);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rejected_case_covers_only_the_clauses_its_rejection_rests_on() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_ledger("rejected");
    let (cases, coverage) = (&generated.cases, &generated.coverage);
    assert_eq!(coverage["rejections"], serde_json::json!(["Refused"]));
    let rejected = cases
        .values()
        .filter(|case| case["kind"] == "rejected")
        .count();
    assert!(rejected > 0, "{:#?}", cases.keys());
    assert_eq!(coverage["cases"]["rejected"], rejected);
    assert!(
        generated
            .stdout
            .contains(&format!(", {} rejected) in", rejected)),
        "{}",
        generated.stdout
    );
    // A refusal is a rejected case, and every rejected case is a refusal.
    for case in cases.values() {
        assert_eq!(
            case["kind"] == "rejected",
            case["expected"]["$variant"] == "Refused",
            "{}",
            case
        );
    }

    // Too many units: the validation clause is covered by a refusal, which
    // does not claim the value and bonus computed before it.
    let too_many = clause(coverage, "units_valid", "too_many");
    let by = too_many["covered_by"].as_array().expect("covered_by");
    assert!(!by.is_empty(), "{}", too_many);
    for id in by {
        let case = &cases[id.as_str().unwrap()];
        assert_eq!(case["kind"], "rejected", "{}", case);
        assert!(!covers(case, "bonus", "clause-1"), "{}", case);
        assert!(!covers(case, "fee", "clause-1"), "{}", case);
    }
    // A clause that fires only where the ledger is refused, off the
    // refusal's path, is not covered.
    let big = clause(coverage, "bonus", "big");
    assert!(big["covered_by"].as_array().unwrap().is_empty(), "{}", big);
    assert_eq!(big["reason"], "rejected", "{}", big);
    // The boundary of the units ceiling: below and at are booked, above is
    // refused because of it (the clause's own case).
    let side = |name: &str| {
        let case = threshold_case(&generated, "units_valid", "too_many", name);
        (
            case["input"]["units"].as_i64().unwrap(),
            case["kind"].as_str().unwrap().to_string(),
        )
    };
    assert_eq!(side("below"), (999, "boundary-below".to_string()));
    assert_eq!(side("at"), (1000, "boundary-at".to_string()));
    assert_eq!(side("above"), (1001, "rejected".to_string()));
    // The fee's boundary is not observable in a refusal: no side is refused.
    for name in ["below", "at", "above"] {
        let case = threshold_case(&generated, "fee", "large", name);
        assert_eq!(case["expected"]["$variant"], "Booked", "{}", case);
    }
}

#[test]
fn a_rejection_resting_only_on_uncited_clauses_is_technical_and_not_a_case() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_ledger("technical");
    let (cases, coverage) = (&generated.cases, &generated.coverage);
    // The fee, the units ceiling and the bonus cite § 12; the positive
    // amount cites nothing.
    assert_eq!(
        coverage["summary"],
        "cited rule clauses: 5 of 6 covered, of which 0 not in the result (reported, still counted)",
        "{}",
        coverage
    );
    assert_eq!(
        coverage["uncited"],
        serde_json::json!({ "covered": 1, "clauses": 2, "not_in_result": 0 })
    );
    assert!(
        generated
            .stdout
            .contains("; uncited rule clauses: 1 of 2 covered, of which 0 not in the result; "),
        "{}",
        generated.stdout
    );
    // Every rejected case rests on a cited clause.
    for case in cases.values().filter(|case| case["kind"] == "rejected") {
        assert!(
            case["covers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|cover| !cover["cites"].as_array().unwrap().is_empty()),
            "{}",
            case
        );
        assert!(!covers(case, "amount_valid", "not_positive"), "{}", case);
    }
    // An amount that is not positive is refused only by the uncited check:
    // listed as technical with the clause it rests on, and no case.
    let not_positive = clause(coverage, "amount_valid", "not_positive");
    assert_eq!(
        not_positive["covered_by"],
        serde_json::json!([]),
        "{}",
        not_positive
    );
    assert_eq!(not_positive["reason"], "technical", "{}", not_positive);
    let technical = coverage["technical"].as_array().expect("technical");
    let rejection = technical
        .iter()
        .find(|rejection| rejection["id"] == "amount_valid.not_positive.clause")
        .unwrap_or_else(|| panic!("{:#?}", technical));
    assert_eq!(rejection["searched_as"], "clause");
    let rests_on = rejection["rests_on"].as_array().expect("rests_on");
    assert_eq!(rests_on.len(), 1, "{}", rejection);
    assert_eq!(rests_on[0]["rule"], "amount_valid");
    assert_eq!(rests_on[0]["clause"], "not_positive");
    assert!(rests_on[0]["location"]
        .as_str()
        .unwrap()
        .starts_with("ledger.entry.runa:"));
    assert!(!cases.contains_key("amount_valid.not_positive.clause"));
    assert!(
        generated.stdout.contains(&format!(
            "; technical rejections not in the suite: {}",
            technical.len()
        )),
        "{}",
        generated.stdout
    );
    // Only the guards of cited clauses have boundaries.
    let thresholds = coverage["thresholds"].as_array().expect("thresholds");
    assert!(
        thresholds
            .iter()
            .all(|threshold| threshold["rule"] != "amount_valid"),
        "{:#?}",
        thresholds
    );
    assert!(
        !cases.keys().any(|id| id.starts_with("amount_valid.")),
        "{:#?}",
        cases.keys()
    );
}

/// The case of one side of a clause's first threshold, as coverage.json
/// names it.
fn threshold_case<'a>(generated: &'a Generated, rule: &str, key: &str, side: &str) -> &'a Value {
    let threshold = generated.coverage["thresholds"]
        .as_array()
        .expect("thresholds")
        .iter()
        .find(|threshold| {
            threshold["rule"] == rule && threshold["clause"] == key && threshold["threshold"] == 1
        })
        .unwrap_or_else(|| panic!("threshold of {}.{}", rule, key));
    let id = threshold[side]
        .as_str()
        .unwrap_or_else(|| panic!("{} side of {}", side, threshold));
    &generated.cases[id]
}

#[test]
fn a_rule_over_list_elements_gets_a_new_element_carrying_the_witness() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_ledger("elements");
    let cases = &generated.cases;
    // Turning the seed's only deposit into a large withdrawal is refused (no
    // deposit); a new withdrawal next to it is booked with a fee.
    let large = &cases["fee.large.clause"];
    assert_eq!(large["kind"], "clause", "{}", large);
    assert_eq!(large["expected"]["$variant"], "Booked", "{}", large);
    assert!(covers(large, "fee", "large"));
    let events = large["input"]["events"].as_array().expect("events");
    assert_eq!(events.len(), 2, "{}", large);
    assert_eq!(events[0]["id"], "e1");
    assert_eq!(events[0]["kind"]["$variant"], "Deposit");
    assert_eq!(
        events[1]["id"], "fee.large.clause",
        "the new id is the case's"
    );
    assert_eq!(events[1]["kind"]["$variant"], "Withdrawal");
    assert!(events[1]["amount"].as_i64().unwrap() > 5000, "{}", large);
    // The threshold of the fee on both sides, each with a new withdrawal.
    for (side, amount) in [("below", 4999), ("at", 5000), ("above", 5001)] {
        let case = threshold_case(&generated, "fee", "large", side);
        let events = case["input"]["events"].as_array().expect("events");
        assert_eq!(events.len(), 2, "{}", case);
        assert_eq!(events[0]["id"], "e1");
        let new = &events[1];
        assert!(
            new["id"].as_str().unwrap().starts_with("fee.large."),
            "{}",
            case
        );
        assert_eq!(new["kind"]["$variant"], "Withdrawal");
        assert_eq!(new["amount"], amount, "{}", case);
    }
}

#[test]
fn the_minimal_seed_never_invents_open_keys() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_ledger("minimal");
    // The minimal ledger gives no price, and the law looks one up.
    assert!(
        generated.stderr.contains("seed `minimal` is not used"),
        "{}",
        generated.stderr
    );
    assert!(!generated.cases.contains_key("seed.minimal"));
    // No case gives a price under a key the seed did not.
    for case in generated.cases.values() {
        for price in case["input"]["prices"].as_array().expect("prices") {
            assert_eq!(price["isin"], "DK0000000001", "{}", case);
        }
    }
}

#[test]
fn a_rejection_must_be_a_variant_of_the_result() {
    let output = Command::new(runa())
        .current_dir(fixture(""))
        .args([
            "cases",
            "ledger.entry.runa",
            "--rejected",
            "Nope",
            "--out",
            std::env::temp_dir()
                .join(format!("runa-cases-{}-nope", std::process::id()))
                .to_str()
                .unwrap(),
        ])
        .output()
        .expect("run runa cases");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--rejected `Nope` is not a variant of the result of `ledger` (its variants: Booked, Refused)"),
        "{}",
        stderr
    );
}

#[test]
fn a_result_err_is_a_rejection_without_a_flag() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_with("result", &["charge.entry.runa"], false);
    assert_eq!(generated.coverage["rejections"], serde_json::json!(["Err"]));
    let negative = &generated.cases["valid.negative.clause"];
    assert_eq!(negative["kind"], "rejected", "{}", negative);
    assert_eq!(negative["expected"]["$variant"], "Err");
    // The fee was computed before the charge was refused: not observable.
    assert!(covers(negative, "valid", "negative"));
    assert!(!covers(negative, "fee", "clause-1"), "{}", negative);
}

/// Whether a date on the wire is a day on the calendar, years 0000 to 9999:
/// what the SDK's `IsoDateSchema` accepts.
fn on_calendar(date: &Value) -> bool {
    let part = |name: &str| date[name].as_i64().expect("date part");
    let (year, month, day) = (part("year"), part("month"), part("day"));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (0..=9999).contains(&year) && (1..=12).contains(&month) && (1..=days).contains(&day)
}

/// The threshold of a clause in the coverage report.
fn threshold<'a>(coverage: &'a Value, rule: &str, key: &str) -> &'a Value {
    coverage["thresholds"]
        .as_array()
        .expect("thresholds")
        .iter()
        .find(|threshold| threshold["rule"] == rule && threshold["clause"] == key)
        .unwrap_or_else(|| panic!("threshold of {}.{}", rule, key))
}

#[test]
fn a_clause_that_needs_an_amount_beyond_2_53_is_outside_the_input_domain() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    const SAFE: i64 = (1 << 53) - 1;
    let generated = generate_with("transfer", &["transfer.entry.runa"], false);
    let coverage = &generated.coverage;
    assert_eq!(
        coverage["summary"],
        "cited rule clauses: 1 of 2 covered, of which 0 not in the result (reported, still counted)",
        "{}",
        coverage
    );
    let huge = clause(coverage, "fee", "huge");
    assert_eq!(huge["covered_by"], serde_json::json!([]), "{}", huge);
    assert_eq!(huge["reason"], "outside-input-domain", "{}", huge);
    assert!(
        huge["witness"]["amount"].as_i64().unwrap() > SAFE,
        "{}",
        huge
    );
    // Below and at the bound an integrator can send; above it not.
    let bound = threshold(coverage, "fee", "huge");
    let amount = |side: &str| {
        let id = bound[side]
            .as_str()
            .unwrap_or_else(|| panic!("{} of {}", side, bound));
        generated.cases[id]["input"]["amount"].as_i64().unwrap()
    };
    assert_eq!((amount("below"), amount("at")), (SAFE - 1, SAFE));
    assert!(bound["above"].is_null(), "{}", bound);
    assert!(
        bound["detail"]
            .as_str()
            .unwrap()
            .contains("above: outside the input domain"),
        "{}",
        bound
    );
    for case in generated.cases.values() {
        assert!(
            case["input"]["amount"].as_i64().unwrap().abs() <= SAFE,
            "{}",
            case
        );
        assert!(case["expected"].as_i64().unwrap().abs() <= SAFE, "{}", case);
    }
}

#[test]
fn a_date_guard_near_month_ends_gives_only_days_on_the_calendar() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_with(
        "payment",
        &["payment.entry.runa", "--seed", "payment-seed.json"],
        false,
    );
    let coverage = &generated.coverage;
    assert_eq!(
        coverage["summary"],
        "cited rule clauses: 2 of 2 covered, of which 0 not in the result (reported, still counted)",
        "{}",
        coverage
    );
    for case in generated.cases.values() {
        assert!(on_calendar(&case["input"]["paid"]), "{}", case);
    }
    // The seed pays on 14 February: the 29th, 30th and 31st are of other
    // months, or of a leap year.
    let bound = threshold(coverage, "surcharge", "month_end");
    for (side, day) in [("below", 29), ("at", 30), ("above", 31)] {
        let id = bound[side]
            .as_str()
            .unwrap_or_else(|| panic!("{} of {}", side, bound));
        let case = &generated.cases[id];
        assert_eq!(case["input"]["paid"]["day"], day, "{}", case);
        assert_eq!(
            covers(case, "surcharge", "month_end"),
            day >= 30,
            "{}",
            case
        );
    }
}

#[test]
fn a_clause_no_result_rests_on_is_reported_not_in_result_and_still_counted() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_with("step", &["step.entry.runa"], false);
    let coverage = &generated.coverage;
    // The step computes the fee and the report; the entry gives only the
    // fee. The report's clauses fire in every run, but no result rests on
    // them: they are not covered, reported `not-in-result`, and still
    // counted, so coverage understates rather than overstates.
    assert_eq!(
        coverage["summary"],
        "cited rule clauses: 2 of 4 covered, of which 2 not in the result (reported, still counted)",
        "{}",
        coverage
    );
    assert_eq!(
        coverage["cited"],
        serde_json::json!({ "covered": 2, "clauses": 4, "not_in_result": 2 })
    );
    assert!(
        generated.stdout.starts_with(
            "runa cases: cited rule clauses: 2 of 4 covered, of which 2 not in the result (reported, still counted);"
        ),
        "{}",
        generated.stdout
    );
    for key in ["clause-1", "large"] {
        let report = clause(coverage, "report", key);
        assert_eq!(report["covered_by"], serde_json::json!([]), "{}", report);
        assert_eq!(report["reason"], "not-in-result", "{}", report);
    }
    // No case claims the report, and its guard has no boundary cases.
    for case in generated.cases.values() {
        assert!(!covers(case, "report", "clause-1"), "{}", case);
        assert!(!covers(case, "report", "large"), "{}", case);
    }
    let thresholds = coverage["thresholds"].as_array().expect("thresholds");
    assert!(
        thresholds
            .iter()
            .all(|threshold| threshold["rule"] != "report"),
        "{:#?}",
        thresholds
    );
    // The fee is in the result: covered, with both sides of its threshold.
    let big = threshold(coverage, "fee", "big");
    for side in ["below", "at", "above"] {
        assert!(big[side].is_string(), "{}", big);
    }
}

#[test]
fn a_result_rests_on_what_filter_flat_map_unwrap_or_and_a_boolean_false_decided() {
    if !z3_available() {
        eprintln!("skipping: z3 is not installed");
        return;
    }
    let generated = generate_with(
        "decide",
        &["decide.entry.runa", "--seed", "decide-seed.json"],
        false,
    );
    let coverage = &generated.coverage;
    // `filter` keeps the items `counted` holds for, `flat_map` drops the
    // `[]` `relief` gives for an item of 50 or more, `unwrap_or` replaces
    // the `None` `bonus` gives, and `refused` answers `false` because
    // `liable` holds: each result rests on those decisions, so every clause
    // is in some result and covered by a case that rests on it (before,
    // `counted`, `relief`'s `[]` and `liable` were `not-in-result`).
    assert_eq!(
        coverage["summary"],
        "cited rule clauses: 7 of 7 covered, of which 0 not in the result (reported, still counted)",
        "{}",
        coverage
    );
    // The seed's result rests on all of them but `refused`, which fires
    // only for an owner who is not resident.
    let seed = &generated.cases["seed.three_items"];
    assert_eq!(seed["expected"], 170, "{}", seed);
    for (rule, key) in [
        ("counted", "clause-1"),
        ("relief", "clause-1"),
        ("relief", "small"),
        ("liable", "clause-1"),
        ("bonus", "clause-1"),
    ] {
        assert!(covers(seed, rule, key), "{} {} {}", rule, key, seed);
    }
}
