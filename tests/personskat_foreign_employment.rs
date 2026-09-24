//! Source-allocation checks; all income and document references are fictional.
use serde_json::{json, Value};
use std::process::{Command, Output};

const MODEL: &str = "examples/danish-income-tax/beskaeftigelsesfradrag.calculate.runa";

fn execute(args: &[&str]) -> Output {
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .env("FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS", "1")
        .args(args)
        .output()
        .expect("execute foreign employment audit");
    assert!(
        output.status.success(),
        "{args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    output
}

fn run(args: &[&str]) -> Value {
    serde_json::from_slice(&execute(args).stdout).expect("calculation JSON")
}

fn employment(home: Option<bool>, work: Option<bool>, employer: Option<bool>) -> Value {
    json!({"$variant":"Ll9jAnsættelsesindkomst", "dbo_hjemmehørende_udland":home,
        "arbejde_udført_udland":work, "udenlandsk_arbejdsgiver":employer})
}

fn post(id: &str, year: i64, amount: i64, facts: Value) -> Value {
    json!({"identifikation":id, "indkomstkilde":id, "kildereference":"fictional-source",
        "indkomstår":year, "første_dag_i_året":1, "sidste_dag_i_året":365,
        "grundlag_kroner":amount, "forhold":facts})
}

fn input(year: i64, basis: Option<i64>, posts: Vec<Value>) -> Value {
    json!({"skatteår":year, "grundlag_før_udlandsafgrænsning_kroner":basis,
        "grundlag_kildereference":"fictional-independent-basis",
        "fordeling":{"alle_kilder_og_perioder_oplyst":true, "poster":posts}})
}

#[test]
fn allocation_edges_match_native_and_interpreted_execution() {
    let path = "tests/personskat_foreign_allocation_test.runa";
    let interpreted = execute(&[path]);
    let native = execute(&["run", path]);
    assert_eq!(interpreted.stdout, native.stdout);
    let text = String::from_utf8(interpreted.stdout).unwrap();
    assert!(text.contains("27 betingelseskombinationer"));
    assert!(text.contains("grundlag_efter_udlandsafgrænsning_kroner: -500000"));
}

#[test]
fn audit_interview_has_danish_questions_and_provenance() {
    let schema = run(&["schema", MODEL]);
    let fields = schema["field_metadata"].as_array().unwrap();
    assert_eq!(fields.len(), 16);
    for field in fields {
        for name in ["label", "question", "help"] {
            assert!(!field[name].as_str().unwrap().trim().is_empty(), "{field}");
        }
        let sources = field["sources"].as_array().unwrap();
        assert!(sources
            .iter()
            .any(|source| source["binding"] == "ll9j_udland_lovkilde"));
        assert!(sources
            .iter()
            .any(|source| source["binding"] == "ll9j_udland_bemærkninger"));
    }
    let residence = fields
        .iter()
        .find(|field| {
            field["path"]
                == "fordeling.poster.forhold.Ll9jAnsættelsesindkomst.dbo_hjemmehørende_udland"
        })
        .unwrap();
    assert!(residence["help"].as_str().unwrap().contains("null"));
    let path = "examples/danish-income-tax/beskaeftigelsesfradrag.scenario.runa";
    let example = execute(&[path]);
    let native = execute(&["run", path]);
    assert_eq!(example.stdout, native.stdout, "public audit native parity");
    let output = String::from_utf8(example.stdout).unwrap();
    assert!(output.contains("Uafklaret DBO-hjemsted: None"));
    assert!(output.contains("jobfradrag_kroner: 2916"));
}

#[test]
fn public_audit_preserves_mixed_income_and_withholds_unknown_results() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let placeholder = envelope["cases"][0]["input"].clone();
    assert!(placeholder["grundlag_før_udlandsafgrænsning_kroner"].is_null());
    assert_eq!(
        placeholder["fordeling"]["alle_kilder_og_perioder_oplyst"],
        false
    );
    let mut cases = Vec::new();
    let mut expectations = Vec::new();
    let mut add = |id: &str, facts: Value, expected: Option<(i64, i64, i64)>| {
        cases.push(json!({"case_id":id, "input":facts}));
        expectations.push((id.to_owned(), expected));
    };
    add("unfilled-template", placeholder, None);
    for (year, regular, job) in [
        (2023, 45600, 2700),
        (2024, 45100, 2800),
        (2025, 55600, 2900),
        (2026, 63300, 3100),
    ] {
        add(
            &format!("domestic-{year}"),
            input(
                year,
                Some(600000),
                vec![post("a", year, 600000, employment(Some(false), None, None))],
            ),
            Some((600000, regular, job)),
        );
        add(
            &format!("excluded-{year}"),
            input(
                year,
                Some(600000),
                vec![post(
                    "a",
                    year,
                    600000,
                    employment(Some(true), Some(true), Some(true)),
                )],
            ),
            Some((0, 0, 0)),
        );
    }
    let mixed_posts = vec![
        // Same person's simultaneous sources: retained income was earned in
        // Denmark; do not fabricate contradictory treaty-residence facts.
        post("a", 2026, 300000, employment(None, Some(false), Some(true))),
        post(
            "b",
            2026,
            300000,
            employment(Some(true), Some(true), Some(true)),
        ),
    ];
    add(
        "mixed-income",
        input(2026, Some(600000), mixed_posts.clone()),
        Some((300000, 38250, 2916)),
    );
    add(
        "negative-other-income",
        input(
            2026,
            Some(100000),
            vec![
                post(
                    "a",
                    2026,
                    600000,
                    employment(Some(true), Some(true), Some(true)),
                ),
                post(
                    "b",
                    2026,
                    -500000,
                    json!({"$variant":"Ll9jIkkeAnsættelsesindkomst"}),
                ),
            ],
        ),
        Some((-500000, 0, 0)),
    );
    add("known-zero", input(2026, Some(0), vec![]), Some((0, 0, 0)));
    add("unknown-zero", input(2026, None, vec![]), None);
    add("unsupported-year", input(2027, Some(0), vec![]), None);
    add(
        "wrong-total",
        input(2026, Some(599999), mixed_posts.clone()),
        None,
    );
    let mut incomplete = input(2026, Some(600000), mixed_posts);
    incomplete["fordeling"]["alle_kilder_og_perioder_oplyst"] = json!(false);
    add("incomplete", incomplete, None);
    // All 27 three-valued combinations exercise the public JSON boundary.
    for home in [None, Some(false), Some(true)] {
        for work in [None, Some(false), Some(true)] {
            for employer in [None, Some(false), Some(true)] {
                let facts = [home, work, employer];
                let expected = if facts.contains(&Some(false)) {
                    Some((600000, 63300, 3100))
                } else if facts.iter().all(|fact| *fact == Some(true)) {
                    Some((0, 0, 0))
                } else {
                    None
                };
                add(
                    &format!("conditions-{home:?}-{work:?}-{employer:?}"),
                    input(
                        2026,
                        Some(600000),
                        vec![post("a", 2026, 600000, employment(home, work, employer))],
                    ),
                    expected,
                );
            }
        }
    }
    envelope["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-foreign-employment-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let result = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(result["diagnostics"], json!([]));
    let results = result["results"].as_array().unwrap();
    assert_eq!(results.len(), expectations.len());
    for (row, (id, expected)) in results.iter().zip(expectations) {
        assert_eq!(row["case_id"], id);
        let result = &row["result"];
        let allocation = &result["afgrænsning"];
        let deductions = &result["fradrag_til_sammenligning"];
        assert_eq!(
            allocation["alle_input_gyldige"],
            expected.is_some(),
            "{id}: {result}"
        );
        if let Some((basis, regular, job)) = expected {
            assert_eq!(
                allocation["grundlag"]["grundlag_efter_udlandsafgrænsning_kroner"], basis,
                "{id}"
            );
            assert_eq!(
                deductions["almindeligt_beskæftigelsesfradrag_kroner"], regular,
                "{id}"
            );
            assert_eq!(deductions["jobfradrag_kroner"], job, "{id}");
        } else {
            assert!(allocation["grundlag"].is_null(), "{id}: {result}");
            assert!(deductions.is_null(), "{id}: {result}");
            assert!(allocation["kontroller"]
                .as_array()
                .unwrap()
                .iter()
                .any(|control| control["gyldig"] == false));
        }
    }
    println!("{} fictional public audit cases passed", results.len());
}
