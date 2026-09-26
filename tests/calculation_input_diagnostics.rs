use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const MODEL: &str = "# Details(id: Int, label: String)\n# Choice = Named(details: Details, flag: Bool) | Positional(Details, Int)\n# Input(profile: Details, rows: List(Details), entries: Map(String, Details), tags: Set(Int), choice: Choice, extra: Details?)\n@ calculate\n> report(input: Input) -> Int { 42 }\n";

struct Fixture {
    directory: PathBuf,
    envelope: Value,
}

fn runa() -> std::ffi::OsString {
    std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into())
}

impl Fixture {
    fn new(source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "futuruna-input-errors-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("model.runa"), source).unwrap();
        let output = Command::new(runa())
            .args(["template"])
            .arg(directory.join("model.runa"))
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        Self {
            directory,
            envelope: serde_json::from_slice(&output.stdout).unwrap(),
        }
    }

    fn input(&self, cases: Value) {
        let mut envelope = self.envelope.clone();
        envelope["cases"] = cases;
        std::fs::write(
            self.directory.join("input.json"),
            serde_json::to_vec(&envelope).unwrap(),
        )
        .unwrap();
    }

    fn call(&self, jobs: &str) -> Value {
        let output = Command::new(runa())
            .arg("call")
            .arg(self.directory.join("model.runa"))
            .arg("--input")
            .arg(self.directory.join("input.json"))
            .env("FUTURUNA_CALCULATION_JOBS", jobs)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{output:?}"))
    }

    fn template(&self) -> Output {
        Command::new(runa())
            .arg("template")
            .arg(self.directory.join("model.runa"))
            .arg("--input")
            .arg(self.directory.join("input.json"))
            .arg("--output")
            .arg(self.directory.join("output.xlsx"))
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in ["model.runa", "input.json", "output.xlsx"] {
            let path = self.directory.join(name);
            if path.exists() {
                std::fs::remove_file(path).unwrap();
            }
        }
        std::fs::remove_dir(&self.directory).unwrap();
    }
}

fn paths(output: &Value, case: &str) -> BTreeSet<String> {
    output["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|error| error["case_id"] == case)
        .map(|error| error["path"].as_str().unwrap().to_string())
        .collect()
}

fn expected(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|path| (*path).to_string()).collect()
}

#[test]
fn missing_and_unknown_record_fields_are_reported_together() {
    let fixture = Fixture::new("# Input(first: Int, second: String, optional: Bool?)\n@ calculate\n> report(input: Input) -> Int { input.first }\n");
    fixture.input(json!([
        {"case_id":"before", "input":{"first":1,"second":"good"}},
        {"case_id":"empty", "input":{}},
        {"case_id":"unknown", "input":{"typo":1,"another":true}},
        {"case_id":"after", "input":{"first":2,"second":"good"}}
    ]));
    let serial = fixture.call("1");
    assert_eq!(serial, fixture.call("3"));
    assert_eq!(
        serial["results"],
        json!([{"case_id":"before","result":1},{"case_id":"after","result":2}])
    );
    assert_eq!(paths(&serial, "empty"), expected(&["$.first", "$.second"]));
    assert_eq!(
        paths(&serial, "unknown"),
        expected(&["$.first", "$.second", "$.typo", "$.another"])
    );
    assert_eq!(serial["diagnostics"].as_array().unwrap().len(), 6);
}

#[test]
fn nested_records_collections_and_variants_collect_independent_errors() {
    let fixture = Fixture::new(MODEL);
    fixture.input(json!([{"case_id":"nested", "input":{
        "profile":{"unexpected":1},
        "rows":[{}, {"id":"bad","label":false}],
        "entries":{"north":{}},
        "tags":[1, 1, "bad", true],
        "choice":{"$variant":"Named", "details":{}, "flag":0, "typo":1},
        "extra":{}
    }}, {"case_id":"positional", "input":{
        "profile":{"id":1,"label":"valid"}, "rows":[], "entries":{}, "tags":[],
        "choice":{"$variant":"Positional", "$values":[{}, "bad"], "unknown":1}
    }}]));
    let result = fixture.call("2");
    assert!(result["results"].as_array().unwrap().is_empty());
    assert_eq!(
        paths(&result, "nested"),
        expected(&[
            "$.profile.unexpected",
            "$.profile.id",
            "$.profile.label",
            "$.rows[0].id",
            "$.rows[0].label",
            "$.rows[1].id",
            "$.rows[1].label",
            "$.entries.north.id",
            "$.entries.north.label",
            "$.tags[1]",
            "$.tags[2]",
            "$.tags[3]",
            "$.choice.typo",
            "$.choice.details.id",
            "$.choice.details.label",
            "$.choice.flag",
            "$.extra.id",
            "$.extra.label"
        ])
    );
    assert_eq!(
        paths(&result, "positional"),
        expected(&[
            "$.choice.unknown",
            "$.choice.$values[0].id",
            "$.choice.$values[0].label",
            "$.choice.$values[1]"
        ])
    );
    assert_eq!(result["diagnostics"].as_array().unwrap().len(), 22);
}

#[test]
fn invalid_parent_shapes_do_not_invent_child_errors_and_template_reports_all_cases() {
    let fixture = Fixture::new(MODEL);
    fixture.input(json!([
        {"case_id":"bad-shapes", "input":{"profile":null,"rows":42,"entries":[],"tags":{},"choice":{"$variant":"Missing"}}},
        {"case_id":"other", "input":{}}
    ]));
    let result = fixture.call("1");
    assert_eq!(
        paths(&result, "bad-shapes"),
        expected(&[
            "$.profile",
            "$.rows",
            "$.entries",
            "$.tags",
            "$.choice.$variant"
        ])
    );
    assert_eq!(
        paths(&result, "other"),
        expected(&["$.profile", "$.rows", "$.entries", "$.tags", "$.choice"])
    );
    let output = fixture.template();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    for case in ["bad-shapes", "other"] {
        for path in paths(&result, case) {
            assert!(
                stderr.contains(&format!("case `{case}` {path}:")),
                "{stderr}"
            );
        }
    }
    assert!(!fixture.directory.join("output.xlsx").exists());
}

#[test]
fn complete_diagnostics_and_legacy_single_error_api_share_the_same_decoder() {
    let fixture = Fixture::new("# Input(first: Int, second: String, optional: Bool?)\n@ calculate\n> report(input: Input) -> Input { input }\n");
    let output = Command::new(runa())
        .arg("schema")
        .arg(fixture.directory.join("model.runa"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let contract: futuruna::calculate::CalculationContract =
        serde_json::from_slice(&output.stdout).unwrap();
    let invalid = json!({"first":"bad", "unknown":true});
    let errors = contract.decode_input_diagnostics(&invalid).unwrap_err();
    assert_eq!(errors.len(), 3);
    assert_eq!(contract.decode_input(&invalid).unwrap_err(), errors[0]);
    let valid = json!({"first":1, "second":"valid", "optional":null});
    for decoded in [
        contract.decode_input(&valid).unwrap(),
        contract.decode_input_diagnostics(&valid).unwrap(),
    ] {
        assert_eq!(contract.encode_output(&decoded).unwrap(), valid);
    }
}
