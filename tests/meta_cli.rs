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
fn meta_type_filter_finds_values_nested_in_typed_aggregate() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("meta-aggregate.runa");
    let output = run_meta(&["--json", "--type", "Shape"], &target);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = parse_json(&output);
    assert_eq!(document["counts"]["references"], 1);
    assert_eq!(document["counts"]["typed_values"], 2);
    let reference = &document["references"][0];
    assert_eq!(reference["binding"], "geometry_metadata");
    assert_eq!(reference["type"], "Metadata");
    let shape_values = reference["typed_values"]
        .as_array()
        .expect("typed values")
        .iter()
        .filter(|value| value["type"] == "Shape")
        .collect::<Vec<_>>();
    assert_eq!(shape_values.len(), 2);
    assert_eq!(shape_values[0]["path"], "$.shapes[0]");
    assert_eq!(shape_values[0]["data"]["name"], "Circle");
    assert_eq!(shape_values[1]["path"], "$.shapes[1]");
    assert_eq!(shape_values[1]["data"]["name"], "Triangle");
}

#[test]
fn meta_json_preserves_typed_program_references() {
    let target = fixture_dir("meta-refof.runa");
    let output = run_meta(&["--json", "--type", "ProgramReference"], &target);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = parse_json(&output);
    assert_eq!(document["counts"]["diagnostics"], 0);
    assert_eq!(document["counts"]["references"], 1);
    let reference = &document["references"][0];
    let program_reference = reference["typed_values"]
        .as_array()
        .expect("typed values")
        .iter()
        .find(|value| value["type"] == "ProgramReference")
        .expect("nested ProgramReference");
    assert_eq!(program_reference["path"], "$.target");
    assert_eq!(program_reference["data"]["name"], "ProgramSymbolReference");
    assert_eq!(program_reference["data"]["arguments"][0]["field"], "name");
    assert_eq!(
        program_reference["data"]["arguments"][0]["value"]["value"],
        "tax_due"
    );
}

#[test]
fn meta_role_filter_finds_typed_attachments_nested_in_an_aggregate() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("meta-role-aggregate.runa");
    let output = run_meta(&["--json", "--role", "dependency_source"], &target);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = parse_json(&output);
    assert_eq!(document["counts"]["references"], 1);
    assert_eq!(document["counts"]["attachments"], 1);
    assert_eq!(
        document["references"][0]["meta_role_types"][0],
        "SourceMetaRole"
    );
    let attachments = document["references"][0]["attachments"]
        .as_array()
        .expect("attachments");
    let dependency = attachments
        .iter()
        .find(|attachment| attachment["role"] == "dependency_source")
        .expect("dependency source attachment");
    assert_eq!(dependency["path"], "$.attachments[1]");
    assert_eq!(dependency["value_path"], "$.attachments[1].value");
    assert_eq!(dependency["binding"], "dependency_source");
    assert_eq!(dependency["type"], "SourceInfo");
}

#[test]
fn aggregate_meta_resolves_values_through_nested_plain_imports() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("meta-imports")
        .join("aggregate.runa");
    let output = run_meta(&["--json", "--role", "source"], &target);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = parse_json(&output);
    assert_eq!(document["counts"]["diagnostics"], 0);
    assert_eq!(document["counts"]["attachments"], 1);
    let attachment = &document["references"][0]["attachments"][0];
    assert_eq!(attachment["role"], "source");
    assert_eq!(attachment["binding"], "registry_source");
    assert_eq!(attachment["type"], "SourceInfo");
    assert_eq!(
        attachment["data"]["arguments"][1]["value"]["value"],
        "nested"
    );
}

#[test]
fn canonical_meta_accepts_marker_implementation_from_an_import() {
    let target = fixture_dir("meta-imported-marker").join("model.runa");
    let output = run_meta(&["--json"], &target);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = parse_json(&output);
    assert_eq!(document["counts"]["diagnostics"], 0);
    assert_eq!(document["counts"]["references"], 1);
    assert_eq!(document["references"][0]["type"], "ImportedSourceMeta");
    assert_eq!(
        document["references"][0]["typed_values"][1]["type"],
        "SourceInfo"
    );
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
