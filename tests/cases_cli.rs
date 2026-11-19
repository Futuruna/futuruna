//! `runa cases` on a small law: a ceiling threshold, an age threshold, a
//! date threshold, an `Option` head and a constructor head
//! (`docs/rfcs/conformance-cases.md`).
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

/// Run `runa cases` into a fresh directory; the case files by id, the
/// coverage report and stdout.
fn generate(name: &str, stale: bool) -> (BTreeMap<String, Value>, Value, String) {
    let out = std::env::temp_dir().join(format!("runa-cases-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("create output directory");
    if stale {
        std::fs::write(out.join("old.case.json"), "{}").expect("write stale case");
    }
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
        .output()
        .expect("run runa cases");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
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
    (cases, coverage, stdout)
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
        stdout.starts_with("runa cases: 9 of 9 rule clauses covered;"),
        "{}",
        stdout
    );
    assert_eq!(coverage["summary"], "9 of 9 rule clauses covered");
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
