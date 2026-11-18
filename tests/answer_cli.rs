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

const DESCRIBED_LAW: &str = r#"# Info(id: String, statement: String, resolve_with: String)
# LawRole(a) = Assumption(value: a)
# impl MetaRole for LawRole {}
= rounding = Info(id = "ROUND", statement = "Amounts are whole kroner.", resolve_with = "the rate table")
# Case(@ field(label = "Income", question = "What did you earn?", help = "Before tax.") income: Int)

@ meta(Assumption(value = rounding))
@ section report {

@ entry
> tax(c: Case) -> Int { c.income }

}
"#;

/// `runa call --partial` on the law above, the input document carrying
/// `describe` when given, with `arguments` after the input.
fn described_call(name: &str, document_describe: Option<Value>, arguments: &[&str]) -> Value {
    let dir = std::env::temp_dir().join(format!(
        "futuruna-answer-cli-describe-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let law = dir.join("tax.entry.runa");
    std::fs::write(&law, DESCRIBED_LAW).unwrap();
    let law = law.to_str().unwrap();
    let schema: Value = serde_json::from_slice(&runa(&["schema", law]).stdout).unwrap();
    let mut envelope = json!({
        "$futuruna": {
            "schema": "futuruna.entry.input.v2",
            "schema_hash": schema["schema_hash"],
            "entry": "tax"
        },
        "cases": [
            { "case_id": "open", "input_status": "ready", "input": {} },
            { "case_id": "known", "input_status": "ready", "input": { "income": 5 } }
        ]
    });
    if let Some(describe) = document_describe {
        envelope["describe"] = describe;
    }
    let input = dir.join("input.json");
    std::fs::write(&input, envelope.to_string()).unwrap();
    let mut command = vec!["call", law, "--partial", "--input", input.to_str().unwrap()];
    command.extend_from_slice(arguments);
    let output = runa(&command);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(output.status.success(), "{}", text(&output));
    serde_json::from_slice(&output.stdout).unwrap()
}

fn need_and_assumption(document: &Value) -> (Value, Value) {
    (
        document["results"][0]["result"]["needs"][0].clone(),
        document["results"][1]["result"]["assumptions"][0]["value"].clone(),
    )
}

#[test]
fn the_default_output_carries_no_descriptive_text() {
    let (need, assumption) = need_and_assumption(&described_call("default", None, &[]));
    let mut members: Vec<&str> = need
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    members.sort();
    assert_eq!(members, ["kind", "path", "waiting"], "{need}");
    assert_eq!(assumption, json!({ "id": "ROUND" }));
}

#[test]
fn describe_adds_the_text_it_names() {
    let (need, assumption) = need_and_assumption(&described_call(
        "some",
        None,
        &["--describe=label,resolve_with"],
    ));
    assert_eq!(need["label"], "Income");
    assert!(need.get("question").is_none(), "{need}");
    assert_eq!(
        assumption,
        json!({ "id": "ROUND", "resolve_with": "the rate table" })
    );

    let (need, assumption) = need_and_assumption(&described_call("all", None, &["--describe"]));
    assert_eq!(need["question"], "What did you earn?");
    assert_eq!(need["help"], "Before tax.");
    assert_eq!(assumption["statement"], "Amounts are whole kroner.");

    // The input document asks the same; the flag replaces it.
    let (need, assumption) = need_and_assumption(&described_call(
        "document",
        Some(json!(["question", "statement"])),
        &[],
    ));
    assert_eq!(need["question"], "What did you earn?");
    assert!(need.get("label").is_none(), "{need}");
    assert_eq!(assumption["statement"], "Amounts are whole kroner.");
    let (need, _) = need_and_assumption(&described_call(
        "replaced",
        Some(json!(["question"])),
        &["--describe=label"],
    ));
    assert_eq!(need["label"], "Income");
    assert!(need.get("question").is_none(), "{need}");
}

#[test]
fn describe_names_only_descriptive_members() {
    let law = needs_dir().join(LAW);
    let output = runa(&[
        "call",
        law.to_str().unwrap(),
        "--input",
        "x.json",
        "--describe=path",
    ]);
    assert!(!output.status.success());
    assert!(
        text(&output).contains("--describe=path: not a described member"),
        "{}",
        text(&output)
    );
}
