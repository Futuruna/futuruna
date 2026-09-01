#![cfg(target_os = "macos")]

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let unique_id = NEXT_TEMP_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "futuruna-explore-cli-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
            unique_id
        ));
        std::fs::create_dir_all(&path).expect("create Explore CLI test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/relational-explore-stream-smoke.runa")
}

fn run_explore(fixture: &Path, run_state: &Path, output: &Path) -> Output {
    Command::new(runa())
        .args([
            "explore",
            fixture.to_str().expect("UTF-8 Explore fixture path"),
            "--query",
            "relational_stream_nonempty_smoke",
            "--run-state",
            run_state.to_str().expect("UTF-8 run-state path"),
            "--output",
            output.to_str().expect("UTF-8 output path"),
            "--time-limit",
            "5m",
            "--json",
        ])
        .output()
        .expect("run public relational Explore command")
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid Explore JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_exact_count(count: &Value, expected: &str) {
    assert_eq!(count["status"], "exact", "count was not exact: {count}");
    assert_eq!(count["value"], expected, "unexpected exact count: {count}");
}

fn analysis_layer<'a>(report: &'a Value, kind: &str, name: &str) -> &'a Value {
    report["analysis"]["layers"]
        .as_array()
        .expect("analysis layers")
        .iter()
        .find(|layer| layer["kind"] == kind && layer["name"] == name)
        .unwrap_or_else(|| panic!("missing {kind} analysis layer `{name}`"))
}

fn read_json(path: &Path) -> Value {
    let bytes = std::fs::read(path).unwrap_or_else(|error| {
        panic!("read {}: {error}", path.display());
    });
    serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!("parse {} as JSON: {error}", path.display());
    })
}

fn read_ndjson(path: &Path) -> Vec<Value> {
    let source = std::fs::read_to_string(path).unwrap_or_else(|error| {
        panic!("read {}: {error}", path.display());
    });
    source
        .lines()
        .enumerate()
        .map(|(line_index, line)| {
            serde_json::from_str(line).unwrap_or_else(|error| {
                panic!(
                    "parse {} line {} as JSON: {error}",
                    path.display(),
                    line_index + 1
                );
            })
        })
        .collect()
}

fn artifact_path(manifest: &Value, output: &Path, kind: &str) -> PathBuf {
    let relative = manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts")
        .iter()
        .find(|artifact| artifact["kind"] == kind)
        .unwrap_or_else(|| panic!("missing `{kind}` publication artifact"))["path"]
        .as_str()
        .expect("artifact path");
    output.join(relative)
}

fn snapshot_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = std::fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("read {} entry: {error}", directory.display()));
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else if path.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .expect("published file remains under output root")
                    .to_path_buf();
                files.insert(
                    relative,
                    std::fs::read(&path)
                        .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn relational_explore_cli_publishes_structure_and_resumes_without_additions() {
    let fixture = fixture();
    let temp = TestDirectory::new();
    let run_state = temp.path().join("state");
    let output_directory = temp.path().join("output");

    let first_output = run_explore(&fixture, &run_state, &output_directory);
    assert_success(&first_output);
    let first = parse_stdout(&first_output);

    assert_eq!(first["schema"], "futuruna.explore.relational-stream.v3");
    assert_eq!(first["query"]["name"], "relational_stream_nonempty_smoke");
    assert_eq!(first["run"]["lifecycle"], "complete");
    assert!(
        first["run"]["appended"]["semantic_events"]
            .as_u64()
            .expect("semantic event count")
            > 0
    );
    assert_exact_count(&first["counts"]["cases"], "4");
    assert_exact_count(&first["counts"]["selected"], "2");
    assert_exact_count(&first["counts"]["not_selected"], "2");

    let selected_cases = analysis_layer(&first, "result", "selected_cases");
    assert_eq!(selected_cases["status"], "result_published");
    assert_exact_count(&selected_cases["counts"]["projection_records"], "2");

    let mechanisms = analysis_layer(&first, "mechanisms", "paths");
    assert_eq!(mechanisms["status"], "mechanism_closed");
    assert_exact_count(&mechanisms["counts"]["target_cases"], "2");
    assert_exact_count(&mechanisms["counts"]["incidence_cases"], "2");
    assert_exact_count(&mechanisms["counts"]["structural_mechanisms"], "1");
    assert_exact_count(&mechanisms["counts"]["execution_profiles"], "1");
    assert_eq!(mechanisms["support_closure_totals"]["target_cases"], "2");
    assert_eq!(
        mechanisms["support_closure_totals"]["successful_cases"],
        "2"
    );
    assert_eq!(
        mechanisms["support_closure_totals"]["signature_fibers"],
        "1"
    );
    assert_eq!(mechanisms["support_closure_totals"]["target_starters"], "2");

    let structural_incidences = analysis_layer(&first, "result", "structural_incidences");
    assert_eq!(structural_incidences["status"], "result_published");
    assert_exact_count(&structural_incidences["counts"]["projection_records"], "2");
    let mechanism_summary = analysis_layer(&first, "result", "mechanism_summary");
    assert_eq!(mechanism_summary["status"], "result_published");
    assert_exact_count(&mechanism_summary["counts"]["projection_records"], "1");

    assert_eq!(first["publication"]["caught_up"], true);
    assert_eq!(
        first["publication"]["artifacts_caught_up"],
        first["publication"]["artifact_count"]
    );

    let manifest = read_json(&output_directory.join("manifest.json"));
    assert_eq!(manifest["schema_version"], 8);
    assert_eq!(
        manifest["publication_cursor"]["file"],
        ".publication-cursor-v8.json"
    );
    let publication_cursor = read_json(&output_directory.join(".publication-cursor-v8.json"));
    assert_eq!(publication_cursor["schema_version"], 8);

    let starter_artifact = manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts")
        .iter()
        .find(|artifact| artifact["kind"] == "mechanism_starter_support")
        .expect("mechanism starter-support artifact");
    assert_eq!(
        starter_artifact["availability"]["status"],
        "exact_projection_available"
    );
    assert_eq!(
        starter_artifact["scope"],
        "whole_structural_mechanisms_only"
    );
    assert_eq!(
        starter_artifact["canonical_projection_order"],
        serde_json::json!([
            "structural_mechanism_ordinal",
            "source_key",
            "successor_key",
        ])
    );
    assert_eq!(starter_artifact["contains_typed_values"], true);
    assert_eq!(starter_artifact["published_lines"], "5");

    let selected_artifact = manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts")
        .iter()
        .find(|artifact| artifact["kind"] == "result_view" && artifact["name"] == "selected_cases")
        .expect("selected-cases result artifact");
    let selected_path = selected_artifact["path"]
        .as_str()
        .expect("selected-cases artifact path");
    let selected_records = read_ndjson(&output_directory.join(selected_path));
    assert_eq!(selected_records.len(), 2);
    let mut selected_values_by_case = BTreeMap::new();
    for selected in &selected_records {
        assert_eq!(selected["schema_version"], 8);
        assert_eq!(selected["record"]["kind"], "selected_case");
        assert_eq!(selected["record"]["row_id"]["kind"], "case");
        assert_eq!(selected["record"]["values"]["case_id"]["kind"], "case_id");
        assert_eq!(
            selected["record"]["row_id"]["case_id"],
            selected["record"]["values"]["case_id"]["value"]
        );
        assert_eq!(
            selected["record"]["values"]["context"],
            serde_json::json!({ "kind": "unit" })
        );
        let case_id = selected["record"]["row_id"]["case_id"]
            .as_str()
            .expect("selected case ID")
            .to_owned();
        let before = selected["record"]["values"]["before"]
            .as_i64()
            .expect("selected before value");
        let after = selected["record"]["values"]["after"]
            .as_i64()
            .expect("selected after value");
        assert!(
            selected_values_by_case
                .insert(case_id, (before, after))
                .is_none(),
            "duplicate selected case ID"
        );
    }
    let mut selected_transitions = selected_values_by_case
        .values()
        .copied()
        .collect::<Vec<_>>();
    selected_transitions.sort_unstable();
    assert_eq!(selected_transitions, vec![(1, 2), (3, 4)]);

    let structural_incidence_artifact = manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts")
        .iter()
        .find(|artifact| {
            artifact["kind"] == "result_view" && artifact["name"] == "structural_incidences"
        })
        .expect("structural-incidence result artifact");
    let structural_incidence_records = read_ndjson(
        &output_directory.join(
            structural_incidence_artifact["path"]
                .as_str()
                .expect("structural-incidence artifact path"),
        ),
    );
    let incidence_rows = structural_incidence_records
        .iter()
        .filter(|record| record["record"]["kind"] == "result_row")
        .collect::<Vec<_>>();
    assert_eq!(incidence_rows.len(), 2);
    let mut raw_signatures = BTreeSet::new();
    let mut structural_mechanisms = BTreeSet::new();
    let mut execution_profiles = BTreeSet::new();
    for row in incidence_rows {
        assert_eq!(row["record"]["row_id"]["kind"], "incidence");
        let values = &row["record"]["values"];
        assert_eq!(values["case_id"]["kind"], "case_id");
        assert_eq!(values["signature_id"]["kind"], "signature_id");
        assert_eq!(
            values["structural_mechanism_id"]["kind"],
            "structural_mechanism_id"
        );
        assert_eq!(
            values["execution_profile_id"]["kind"],
            "execution_profile_id"
        );
        assert_eq!(
            row["record"]["row_id"]["case_id"],
            values["case_id"]["value"]
        );
        assert_eq!(
            row["record"]["row_id"]["signature_id"],
            values["signature_id"]["value"]
        );
        raw_signatures.insert(
            values["signature_id"]["value"]
                .as_str()
                .expect("raw signature ID"),
        );
        structural_mechanisms.insert(
            values["structural_mechanism_id"]["value"]
                .as_str()
                .expect("structural mechanism ID"),
        );
        execution_profiles.insert(
            values["execution_profile_id"]["value"]
                .as_str()
                .expect("execution profile ID"),
        );
    }
    assert_eq!(raw_signatures.len(), 1);
    assert_eq!(structural_mechanisms.len(), 1);
    assert_eq!(execution_profiles.len(), 1);

    let mechanism_summary_artifact = manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts")
        .iter()
        .find(|artifact| {
            artifact["kind"] == "result_view" && artifact["name"] == "mechanism_summary"
        })
        .expect("mechanism-summary result artifact");
    let mechanism_summary_records = read_ndjson(
        &output_directory.join(
            mechanism_summary_artifact["path"]
                .as_str()
                .expect("mechanism-summary artifact path"),
        ),
    );
    let summary = mechanism_summary_records
        .iter()
        .find(|record| record["record"]["kind"] == "result_group")
        .expect("exact mechanism-summary group");
    assert_eq!(summary["record"]["disposition"], "exact");
    assert_eq!(summary["record"]["values"]["mechanisms"], 1);
    assert_eq!(summary["record"]["values"]["execution_profiles"], 1);
    assert_eq!(summary["record"]["values"]["raw_signatures"], 1);
    assert_eq!(summary["record"]["values"]["explained_cases"], 2);

    let starter_path = starter_artifact["path"]
        .as_str()
        .expect("starter artifact path");
    let starter_records = read_ndjson(&output_directory.join(starter_path));
    assert_eq!(starter_records.len(), 5);
    assert!(starter_records
        .iter()
        .all(|record| record["schema_version"] == 8));
    assert!(starter_records
        .iter()
        .all(|record| record["artifact"] == starter_artifact["key"]));
    assert_eq!(
        starter_records
            .iter()
            .map(|record| record["record"]["kind"]
                .as_str()
                .expect("starter record kind"))
            .collect::<Vec<_>>(),
        vec![
            "mechanism_starters_header",
            "mechanism_starter_projection_header",
            "mechanism_starter_projection_page",
            "mechanism_starter_projection_closure",
            "mechanism_starters_closure",
        ]
    );
    assert_eq!(starter_records[0]["record"]["mechanism_count"], "1");
    assert_eq!(starter_records[1]["record"]["mechanism_ordinal"], "0");
    assert_eq!(starter_records[1]["record"]["exact_case_count"], "2");

    let starter_page = &starter_records[2]["record"];
    assert_eq!(starter_page["mechanism_ordinal"], "0");
    assert_eq!(starter_page["page_ordinal"], "0");
    assert_eq!(starter_page["start_after"], Value::Null);
    assert_eq!(starter_page["exhausted"], true);
    let starter_members = starter_page["members"]
        .as_array()
        .expect("starter page members");
    assert_eq!(starter_members.len(), 2);
    assert!(
        starter_members[0]["source_key"]
            .as_str()
            .expect("first starter source key")
            < starter_members[1]["source_key"]
                .as_str()
                .expect("second starter source key"),
        "starter members must use canonical SourceKey order"
    );
    for member in starter_members {
        assert_eq!(member["context"], serde_json::json!({ "kind": "unit" }));
        let case_id = member["case_id"].as_str().expect("starter member case ID");
        let &(before, after) = selected_values_by_case
            .get(case_id)
            .expect("starter member must be a selected case");
        assert_eq!(member["before"], before);
        assert_eq!(member["after"], after);
    }
    let last_member = starter_members.last().expect("last starter member");
    assert_eq!(
        starter_page["end_cursor"]["source_key"],
        last_member["source_key"]
    );
    assert_eq!(
        starter_page["end_cursor"]["successor_key"],
        last_member["successor_key"]
    );

    assert_eq!(starter_records[3]["record"]["mechanism_ordinal"], "0");
    assert_eq!(starter_records[3]["record"]["exact_case_count"], "2");
    assert_eq!(
        starter_records[3]["record"]["exact_distinct_starter_count"],
        "2"
    );
    assert_eq!(starter_records[3]["record"]["page_count"], "1");
    assert_eq!(starter_records[4]["record"]["exact_mechanism_count"], "1");
    assert_eq!(starter_records[4]["record"]["exact_case_count"], "2");
    assert_eq!(
        starter_records[4]["record"]["exact_distinct_starter_count_sum"],
        "2"
    );

    let structural_support = read_ndjson(&artifact_path(
        &manifest,
        &output_directory,
        "mechanism_structural_support",
    ));
    for required_kind in [
        "structural_assignment",
        "structural_quotient_closure",
        "structural_subject_support",
        "mechanism_support_closure",
    ] {
        assert!(
            structural_support
                .iter()
                .any(|record| record["record"]["kind"] == required_kind),
            "missing `{required_kind}` structural-support record"
        );
    }
    let mechanism_support = structural_support
        .iter()
        .find(|record| {
            record["record"]["kind"] == "structural_subject_support"
                && record["record"]["subject"]["kind"] == "mechanism"
        })
        .expect("mechanism starter-support summary");
    assert_exact_count(
        &mechanism_support["record"]["origin_preimage_support"]["distinct_starter_count"],
        "2",
    );
    assert_eq!(
        mechanism_support["record"]["origin_preimage_support"]["correlated_origin_successor"]
            ["status"],
        "not_materialized"
    );
    assert_eq!(
        mechanism_support["record"]["starter_projection"]["status"],
        "authorized_separate_artifact"
    );
    assert_eq!(
        mechanism_support["record"]["starter_projection"]["authorization_id"],
        starter_artifact["authorization"]["authorization_id"]
    );
    assert_eq!(
        mechanism_support["record"]["starter_projection"]["authorizing_view_id"],
        starter_artifact["authorization"]["authorizing_view_id"]
    );
    assert_eq!(
        mechanism_support["record"]["starter_projection"]["artifact"]["key"],
        starter_artifact["key"]
    );
    assert_eq!(
        mechanism_support["record"]["starter_projection"]["artifact"]["path"],
        starter_artifact["path"]
    );

    let structural_definitions = read_ndjson(&artifact_path(
        &manifest,
        &output_directory,
        "mechanism_structural_definitions",
    ));
    for required_kind in [
        "structural_definition_catalog_header",
        "structural_node_definition",
        "structural_edge_definition",
        "structural_mechanism_definition",
        "structural_execution_profile_definition",
        "structural_definition_catalog_closure",
    ] {
        assert!(
            structural_definitions
                .iter()
                .any(|record| record["record"]["kind"] == required_kind),
            "missing `{required_kind}` structural-definition record"
        );
    }

    let case_support = read_ndjson(&artifact_path(
        &manifest,
        &output_directory,
        "case_support_graph",
    ));
    let case_support_closure = case_support
        .iter()
        .find(|record| record["record"]["kind"] == "closure")
        .expect("case-support closure");
    assert_eq!(
        case_support_closure["record"]["exact_logical_case_count"],
        "4"
    );
    assert_eq!(
        case_support_closure["record"]["exact_selected_case_count"],
        "2"
    );

    let published_before_resume = snapshot_files(&output_directory);
    let first_next_sequence = first["run"]["checkpoint"]["next_sequence"].clone();
    let first_journal_head = first["run"]["checkpoint"]["journal_head"].clone();

    let resumed_output = run_explore(&fixture, &run_state, &output_directory);
    assert_success(&resumed_output);
    let resumed = parse_stdout(&resumed_output);

    assert_eq!(resumed["run"]["lifecycle"], "complete");
    assert_eq!(resumed["run"]["appended"]["semantic_batches"], 0);
    assert_eq!(resumed["run"]["appended"]["semantic_events"], 0);
    assert_eq!(
        resumed["run"]["checkpoint"]["next_sequence"],
        first_next_sequence
    );
    assert_eq!(
        resumed["run"]["checkpoint"]["journal_head"],
        first_journal_head
    );
    assert_eq!(resumed["query"]["identity"], first["query"]["identity"]);
    assert_eq!(resumed["publication"]["lines_appended"], 0);
    assert_eq!(resumed["publication"]["source_ordinals_advanced"], 0);
    assert_eq!(resumed["publication"]["caught_up"], true);
    assert_eq!(snapshot_files(&output_directory), published_before_resume);
}
