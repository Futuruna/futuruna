//! `runa call --guides FILE --guide NAME` (docs/rfcs/answer-guide.md): each
//! need of a partial call names its group as `by`; a need at a held path is a
//! diagnostic of its case. And a file of guides passes a full `runa check`,
//! interpreted only.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_INPUT: AtomicU64 = AtomicU64::new(0);

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
const GUIDES: &str = "answer_household.runa";

/// A partial call of `household_tax` on `input`, with `extra` arguments.
fn call(input: Value, extra: &[&str]) -> Output {
    let law = needs_dir().join(LAW);
    let law = law.to_str().unwrap();
    let schema: Value = serde_json::from_slice(&runa(&["schema", law]).stdout).unwrap();
    let envelope = json!({
        "$futuruna": {
            "schema": "futuruna.entry.input.v2",
            "schema_hash": schema["schema_hash"],
            "entry": "household_tax"
        },
        "cases": [{ "case_id": "c", "input_status": "ready", "input": input }]
    });
    let path = std::env::temp_dir().join(format!(
        "futuruna-answer-guide-{}-{}.json",
        std::process::id(),
        NEXT_INPUT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, envelope.to_string()).unwrap();
    let mut arguments = vec!["call", law, "--partial", "--input", path.to_str().unwrap()];
    arguments.extend_from_slice(extra);
    let output = runa(&arguments);
    let _ = std::fs::remove_file(path);
    output
}

fn guides() -> String {
    needs_dir().join(GUIDES).to_str().unwrap().to_string()
}

#[test]
fn each_need_names_its_group() {
    let guides = guides();
    let output = call(
        json!({ "income": 500, "married": true, "children": [{ "name": "Ada" }] }),
        &["--guides", &guides, "--guide", "household_guide"],
    );
    assert!(output.status.success(), "{}", text(&output));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let needs = &document["results"][0]["result"]["needs"];
    let by: Vec<(String, String)> = needs
        .as_array()
        .unwrap()
        .iter()
        .map(|need| {
            (
                need["path"].as_str().unwrap().to_string(),
                need["by"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(
        by.contains(&("spouse".to_string(), "Customer".to_string())),
        "{by:?}"
    );
    assert!(
        by.contains(&("children[0].age".to_string(), "Registry".to_string())),
        "{by:?}"
    );
}

/// Two guides of one file, each with its own group type declaring
/// `Customer` (one bare, one written `AskOrigin.Customer`): each names the
/// group by its bare constructor name.
#[test]
fn a_group_shared_by_two_group_types_is_named_bare() {
    let shared = needs_dir().join("answer_household_shared_groups.runa");
    let shared = shared.to_str().unwrap();
    for (guide, other) in [("household_guide", "Registry"), ("ask_guide", "Schools")] {
        let output = call(
            json!({ "income": 500, "married": true, "children": [{ "name": "Ada" }] }),
            &["--guides", shared, "--guide", guide],
        );
        assert!(output.status.success(), "{guide}: {}", text(&output));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let by: Vec<(&str, &str)> = document["results"][0]["result"]["needs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|need| (need["path"].as_str().unwrap(), need["by"].as_str().unwrap()))
            .collect();
        assert!(by.contains(&("spouse", "Customer")), "{guide}: {by:?}");
        assert!(by.contains(&("children[0].age", other)), "{guide}: {by:?}");
    }
}

#[test]
fn without_a_guide_a_need_has_no_group() {
    let output = call(
        json!({ "income": 500, "married": true, "children": [] }),
        &[],
    );
    assert!(output.status.success(), "{}", text(&output));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("\"by\""));
}

#[test]
fn a_need_at_a_held_path_is_a_diagnostic() {
    let guides = guides();
    let output = call(
        json!({ "married": true, "spouse": null, "children": [] }),
        &["--guides", &guides, "--guide", "household_guide"],
    );
    assert!(!output.status.success(), "{}", text(&output));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = &document["diagnostics"][0];
    assert_eq!(diagnostic["path"], "income");
    assert_eq!(
        diagnostic["message"],
        "`income` is held (the AnswerGuide `household_guide` names no group for it), but the case leaves it unknown"
    );
}

#[test]
fn a_guide_the_file_does_not_bind_is_refused() {
    let guides = guides();
    let output = call(
        json!({ "income": 500 }),
        &["--guides", &guides, "--guide", "elsewhere"],
    );
    assert!(!output.status.success());
    assert!(
        text(&output).contains("binds no AnswerGuide `elsewhere`; it binds `household_guide`"),
        "{}",
        text(&output)
    );
}

#[test]
fn a_guide_needs_a_partial_call_and_both_flags() {
    let law = needs_dir().join(LAW);
    let law = law.to_str().unwrap();
    let guides = guides();
    let output = runa(&["call", law, "--input", "x.json", "--guides", &guides, "--guide", "g"]);
    assert!(text(&output).contains("--guide applies to a partial call"), "{}", text(&output));
    let output = runa(&["call", law, "--partial", "--input", "x.json", "--guides", &guides]);
    assert!(text(&output).contains("--guides FILE and --guide NAME go together"), "{}", text(&output));
}

#[test]
fn a_file_of_guides_passes_a_full_check_interpreted() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/expect/check/answer_guide_shadowed_item.runa");
    let output = runa(&["check", file.to_str().unwrap()]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("interpreted (guide)"), "{}", text(&output));
    assert!(text(&output).contains("resolves nothing"), "{}", text(&output));
}
