//! Answering an entry from the command line: a partial call's needs carry the
//! law's facts and nothing about who answers them, and a file that answers
//! an entry passes a full `runa check`, interpreted only.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Output};

fn needs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/expect/run/needs")
}

fn runa(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(arguments)
        .env("FUTURUNA_DISABLE_CALCULATION_CACHE", "1")
        .output()
        .unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const LAW: &str = "answer_household_law.runa";

#[test]
fn a_partial_call_names_no_one_who_answers() {
    let law = needs_dir().join(LAW);
    let law = law.to_str().unwrap();
    let schema: Value = serde_json::from_slice(&runa(&["schema", law]).stdout).unwrap();
    let envelope = json!({
        "$futuruna": {
            "schema": "futuruna.entry.input.v2",
            "schema_hash": schema["schema_hash"],
            "entry": "household_tax"
        },
        "cases": [{ "case_id": "c", "input_status": "ready", "input": { "income": 500, "married": true, "children": [{ "name": "Ada" }] } }]
    });
    let path =
        std::env::temp_dir().join(format!("futuruna-answer-cli-{}.json", std::process::id()));
    std::fs::write(&path, envelope.to_string()).unwrap();
    let output = runa(&["call", law, "--partial", "--input", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(path);
    assert!(output.status.success(), "{}", text(&output));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let needs = document["results"][0]["result"]["needs"]
        .as_array()
        .unwrap();
    let paths: Vec<&str> = needs
        .iter()
        .map(|need| need["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"spouse"), "{paths:?}");
    assert!(paths.contains(&"children[0].age"), "{paths:?}");
    assert!(
        needs.iter().all(|need| need.get("by").is_none()),
        "{needs:?}"
    );
}

#[test]
fn a_file_that_answers_an_entry_passes_a_full_check_interpreted() {
    let file = needs_dir().join("answer_household.runa");
    let output = runa(&["check", file.to_str().unwrap()]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("interpreted (query)"),
        "{}",
        text(&output)
    );
}

#[test]
fn the_sdk_takes_no_guide_file() {
    let law = needs_dir().join(LAW);
    let output = runa(&[
        "sdk",
        "ts",
        law.to_str().unwrap(),
        "--out",
        std::env::temp_dir().to_str().unwrap(),
        "--guides",
        "x.runa",
    ]);
    assert!(!output.status.success());
    assert!(
        text(&output).contains("unknown option `--guides`"),
        "{}",
        text(&output)
    );
}
