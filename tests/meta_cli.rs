use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn run_meta(args: &[&str], target: &Path) -> Output {
    Command::new(runa())
        .arg("meta")
        .args(args)
        .arg(target)
        .output()
        .expect("run runa meta")
}

fn parse_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn directory_meta_json_sweeps_typed_sources_recursively() {
    let target = fixture_dir("meta-corpus");
    let output = run_meta(&["--json", "--type", "Shape", "--role", "source"], &target);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = parse_json(&output);
    assert_eq!(document["schema"], "futuruna.meta.collection.v1");
    assert_eq!(document["counts"]["files_scanned"], 2);
    assert_eq!(document["counts"]["files_returned"], 2);
    assert_eq!(document["counts"]["references"], 2);
    assert_eq!(document["counts"]["diagnostics"], 0);

    let files = document["files"].as_array().expect("files array");
    assert_eq!(files[0]["schema"], "futuruna.meta.v1");
    assert!(files[0]["file"]
        .as_str()
        .expect("first file")
        .ends_with("meta-corpus/alpha.runa"));
    assert_eq!(files[0]["references"][0]["binding"], "circle_source");
    assert_eq!(files[0]["references"][0]["type"], "Shape");
    assert_eq!(files[0]["references"][0]["data"]["kind"], "constructor");
    assert_eq!(files[0]["references"][0]["data"]["name"], "Circle");
    assert_eq!(files[0]["references"][0]["data"]["applied"], false);
    assert_eq!(files[0]["anchors"][0]["text_begin_marker_line"], 6);
    assert_eq!(files[0]["anchors"][0]["text_end_marker_line"], 8);
    assert_eq!(files[0]["anchors"][0]["text_start_line"], 7);
    assert_eq!(files[0]["anchors"][0]["text_end_line"], 7);
    assert!(files[1]["file"]
        .as_str()
        .expect("second file")
        .ends_with("meta-corpus/nested/beta.runa"));
    assert_eq!(files[1]["references"][0]["binding"], "square_source");
}

#[test]
fn directory_meta_json_sweeps_warning_roles() {
    let output = run_meta(
        &["--json", "--role", "warning"],
        &fixture_dir("meta-corpus"),
    );
    assert!(output.status.success());

    let document = parse_json(&output);
    assert_eq!(document["counts"]["files_scanned"], 2);
    assert_eq!(document["counts"]["files_returned"], 1);
    assert_eq!(document["counts"]["references"], 1);
    assert_eq!(document["files"][0]["references"][0]["role"], "warning");
    assert_eq!(document["files"][0]["references"][0]["type"], "Warning");
    assert_eq!(
        document["files"][0]["references"][0]["binding"],
        "shape_warning"
    );
    let data = &document["files"][0]["references"][0]["data"];
    assert_eq!(data["kind"], "constructor");
    assert_eq!(data["name"], "Warning");
    assert_eq!(data["arguments"][0]["field"], "message");
    assert_eq!(data["arguments"][0]["value"]["kind"], "string");
    assert_eq!(data["arguments"][0]["value"]["value"], "Review this shape");
}

#[test]
fn directory_meta_human_output_reports_a_collection_summary() {
    let output = run_meta(&["--role", "warning"], &fixture_dir("meta-corpus"));
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("meta collection:"));
    assert!(
        stdout.contains("reference square_rule role warning binding shape_warning type Warning")
    );
    assert!(stdout.contains(
        "summary files scanned 2 returned 1 references 1 anchors 1 spans 1 diagnostics 0"
    ));
}

#[test]
fn imported_meta_bindings_retain_types_values_and_definition_locations() {
    let target = fixture_dir("meta-imports").join("model.calculate.runa");
    let output = run_meta(&["--json"], &target);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = parse_json(&output);
    assert_eq!(document["counts"]["diagnostics"], 0);
    let references = document["references"].as_array().expect("references");
    assert_eq!(references.len(), 2);

    let direct = references
        .iter()
        .find(|reference| reference["binding"] == "registry_source")
        .expect("direct imported reference");
    assert_eq!(direct["type"], "SourceInfo");
    assert_eq!(direct["data"]["name"], "SourceInfo");
    assert!(direct["definition_file"]
        .as_str()
        .expect("direct definition file")
        .ends_with("meta-imports/registry.runa"));
    assert_eq!(direct["definition_line"], 3);

    let nested = references
        .iter()
        .find(|reference| reference["binding"] == "imported_source")
        .expect("nested imported reference");
    assert_eq!(nested["type"], "SourceInfo");
    assert_eq!(nested["data"]["arguments"][0]["field"], "url");
    assert_eq!(
        nested["data"]["arguments"][0]["value"]["value"],
        "https://example.invalid/imported"
    );
    assert!(nested["definition_file"]
        .as_str()
        .expect("nested definition file")
        .ends_with("meta-imports/nested/source.runa"));
    assert_eq!(nested["definition_line"], 3);
}

#[test]
fn directory_meta_json_qualifies_diagnostics_with_their_file() {
    let output = run_meta(&["--json"], &fixture_dir("meta-corpus-invalid"));
    assert!(!output.status.success());

    let document = parse_json(&output);
    assert_eq!(document["schema"], "futuruna.meta.collection.v1");
    assert_eq!(document["counts"]["diagnostics"], 1);
    assert!(document["diagnostics"][0]["file"]
        .as_str()
        .expect("diagnostic file")
        .ends_with("meta-corpus-invalid/unresolved.runa"));
    assert!(document["diagnostics"][0]["message"]
        .as_str()
        .expect("diagnostic message")
        .contains("unresolved binding `missing_warning`"));
}
