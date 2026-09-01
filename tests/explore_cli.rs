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

fn dependent_fiber_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/relational-explore-dependent-fibers.runa")
}

fn run_explore(fixture: &Path, run_state: &Path, output: &Path) -> Output {
    run_explore_query(
        fixture,
        "relational_stream_nonempty_smoke",
        run_state,
        output,
    )
}

fn run_explore_query(fixture: &Path, query: &str, run_state: &Path, output: &Path) -> Output {
    Command::new(runa())
        .args([
            "explore",
            fixture.to_str().expect("UTF-8 Explore fixture path"),
            "--query",
            query,
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

fn answer_mechanism<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["answer"]["mechanism_requests"]
        .as_array()
        .expect("answer mechanism requests")
        .iter()
        .find(|request| request["name"] == name)
        .unwrap_or_else(|| panic!("missing answer mechanism request `{name}`"))
}

fn answer_result<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["answer"]["result_views"]
        .as_array()
        .expect("answer result views")
        .iter()
        .find(|result| result["name"] == name)
        .unwrap_or_else(|| panic!("missing answer result view `{name}`"))
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

fn named_artifact_path(manifest: &Value, output: &Path, kind: &str, name: &str) -> PathBuf {
    let relative = manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts")
        .iter()
        .find(|artifact| artifact["kind"] == kind && artifact["name"] == name)
        .unwrap_or_else(|| panic!("missing `{kind}` publication artifact `{name}`"))["path"]
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
fn relational_explore_cli_closes_an_empty_case_transition_graph_exactly() {
    let fixture = fixture();
    let temp = TestDirectory::new();
    let run_state = temp.path().join("empty-state");
    let output_directory = temp.path().join("empty-output");

    let output = run_explore_query(
        &fixture,
        "relational_stream_empty_smoke",
        &run_state,
        &output_directory,
    );
    assert_success(&output);
    let report = parse_stdout(&output);
    assert_eq!(report["run"]["lifecycle"], "complete");
    assert_exact_count(&report["counts"]["selected"], "0");
    assert_eq!(report["answer"]["find_frontier_closed"], true);
    assert_exact_count(&report["answer"]["counts"]["selected_cases"], "0");
    assert_eq!(report["publication"]["caught_up"], true);

    let manifest = read_json(&output_directory.join("manifest.json"));
    let records = read_ndjson(&artifact_path(
        &manifest,
        &output_directory,
        "selected_case_transition_graph",
    ));
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["record"]["kind"], "header");
    let closure = &records[1]["record"];
    assert_eq!(closure["kind"], "closure");
    assert_eq!(closure["frontier"], "exact");
    assert_exact_count(&closure["counts"]["selected_cases"], "0");
    assert_exact_count(&closure["counts"]["state_nodes"], "0");
    assert_exact_count(&closure["counts"]["semantic_transitions"], "0");
}

#[test]
fn relational_explore_cli_closes_dependent_source_and_successor_fibers_exactly() {
    let fixture = dependent_fiber_fixture();
    let temp = TestDirectory::new();
    let run_state = temp.path().join("dependent-fibers-state");
    let output_directory = temp.path().join("dependent-fibers-output");

    let output = run_explore_query(
        &fixture,
        "relational_stream_dependent_fibers_smoke",
        &run_state,
        &output_directory,
    );
    assert_success(&output);
    let first = parse_stdout(&output);

    assert_eq!(first["run"]["lifecycle"], "complete");
    assert_eq!(first["coverage"]["relation_closed"], true);
    assert_eq!(first["coverage"]["find_closed"], true);
    assert_eq!(first["coverage"]["analysis_closed"], true);
    assert_exact_count(&first["counts"]["sources"], "4");
    assert_exact_count(&first["counts"]["cases"], "4");
    assert_exact_count(&first["counts"]["admitted"], "4");
    assert_exact_count(&first["counts"]["selected"], "4");
    assert_exact_count(&first["counts"]["not_selected"], "0");

    let mechanisms = analysis_layer(&first, "mechanisms", "dependent_paths");
    assert_eq!(mechanisms["status"], "mechanism_closed");
    assert_exact_count(&mechanisms["counts"]["target_cases"], "4");
    assert_exact_count(&mechanisms["counts"]["incidence_cases"], "4");
    assert_exact_count(&mechanisms["counts"]["raw_signatures"], "1");
    assert_exact_count(&mechanisms["counts"]["structural_mechanisms"], "1");
    assert_eq!(mechanisms["support_closure_totals"]["target_cases"], "4");
    assert_eq!(mechanisms["support_closure_totals"]["target_starters"], "3");

    let manifest = read_json(&output_directory.join("manifest.json"));
    let case_rows = read_ndjson(&named_artifact_path(
        &manifest,
        &output_directory,
        "result_view",
        "dependent_cases",
    ));
    let mut cases_by_id = BTreeMap::new();
    let mut extensional_cases = BTreeSet::new();
    for row in case_rows {
        assert_eq!(row["record"]["kind"], "selected_case");
        let values = &row["record"]["values"];
        let case_id = values["case_id"]["value"]
            .as_str()
            .expect("dependent-fiber CaseId")
            .to_owned();
        let action = values["context"]["fields"]["action"]
            .as_i64()
            .expect("dependent-fiber Context action");
        let before = values["before"].as_i64().expect("dependent-fiber Before");
        let after = values["after"].as_i64().expect("dependent-fiber After");
        assert!(
            cases_by_id
                .insert(case_id, (action, before, after))
                .is_none(),
            "duplicate canonical CaseId"
        );
        extensional_cases.insert((action, before, after));
    }
    assert_eq!(
        extensional_cases,
        BTreeSet::from([(0, 1, 2), (0, 2, 3), (0, 2, 4), (1, 2, 3)])
    );

    let transition_records = read_ndjson(&artifact_path(
        &manifest,
        &output_directory,
        "selected_case_transition_graph",
    ));
    let transition_rows = transition_records
        .iter()
        .filter(|row| row["record"]["kind"] == "case_transition")
        .collect::<Vec<_>>();
    assert_eq!(transition_rows.len(), 4);
    let mut same_numeric_transition_ids = BTreeSet::new();
    let mut same_numeric_actions = BTreeSet::new();
    for row in transition_rows {
        let record = &row["record"];
        let case_id = record["case_id"].as_str().expect("transition CaseId");
        let expected = cases_by_id
            .get(case_id)
            .expect("transition must retain exact case support");
        let action = record["context"]["fields"]["action"]
            .as_i64()
            .expect("transition Context action");
        let before = record["before"].as_i64().expect("transition Before");
        let after = record["after"].as_i64().expect("transition After");
        assert_eq!((action, before, after), *expected);
        if before == 2 && after == 3 {
            same_numeric_actions.insert(action);
            same_numeric_transition_ids.insert(
                record["transition_id"]
                    .as_str()
                    .expect("TransitionId")
                    .to_owned(),
            );
        }
    }
    assert_eq!(same_numeric_actions, BTreeSet::from([0, 1]));
    assert_eq!(same_numeric_transition_ids.len(), 2);
    let transition_closure = transition_records
        .iter()
        .find(|row| row["record"]["kind"] == "closure")
        .expect("dependent transition closure");
    assert_exact_count(
        &transition_closure["record"]["counts"]["selected_cases"],
        "4",
    );
    assert_exact_count(&transition_closure["record"]["counts"]["state_nodes"], "4");
    assert_exact_count(
        &transition_closure["record"]["counts"]["semantic_transitions"],
        "4",
    );

    let structural_support = read_ndjson(&artifact_path(
        &manifest,
        &output_directory,
        "mechanism_structural_support",
    ));
    let mechanism_support = structural_support
        .iter()
        .find(|row| {
            row["record"]["kind"] == "structural_subject_support"
                && row["record"]["subject"]["kind"] == "mechanism"
        })
        .expect("dependent mechanism starter-support summary");
    assert_exact_count(&mechanism_support["record"]["case_count"], "4");
    assert_exact_count(
        &mechanism_support["record"]["origin_preimage_support"]["distinct_starter_count"],
        "3",
    );

    let checkpoint = first["run"]["checkpoint"].clone();
    let identity = first["query"]["identity"].clone();
    let closure_root = first["analysis"]["analysis_closure_set_root"].clone();
    let resumed_output = run_explore_query(
        &fixture,
        "relational_stream_dependent_fibers_smoke",
        &run_state,
        &output_directory,
    );
    assert_success(&resumed_output);
    let resumed = parse_stdout(&resumed_output);
    assert_eq!(resumed["run"]["lifecycle"], "complete");
    assert_eq!(resumed["run"]["appended"]["semantic_batches"], 0);
    assert_eq!(resumed["run"]["appended"]["semantic_events"], 0);
    assert_eq!(resumed["publication"]["lines_appended"], 0);
    assert_eq!(resumed["query"]["identity"], identity);
    assert_eq!(resumed["run"]["checkpoint"], checkpoint);
    assert_eq!(
        resumed["analysis"]["analysis_closure_set_root"],
        closure_root
    );
}

#[test]
fn relational_explore_cli_attaches_route_conditioned_node_starters_without_reexploration() {
    let fixture = fixture();
    let temp = TestDirectory::new();
    let run_state = temp.path().join("state");
    let output_directory = temp.path().join("output");

    let first_output = run_explore(&fixture, &run_state, &output_directory);
    assert_success(&first_output);
    let first = parse_stdout(&first_output);

    assert_eq!(first["schema"], "futuruna.explore.relational-stream.v5");
    assert_eq!(first["schema_version"], 5);
    assert_eq!(first["query"]["name"], "relational_stream_nonempty_smoke");
    assert_eq!(first["run"]["lifecycle"], "complete");
    assert!(first["run"].get("preparation_wall_milliseconds").is_none());
    assert!(first["run"].get("slice_budget_milliseconds").is_none());
    assert!(
        first["run"]["appended"]["semantic_events"]
            .as_u64()
            .expect("semantic event count")
            > 0
    );
    assert_exact_count(&first["counts"]["cases"], "4");
    assert_exact_count(&first["counts"]["selected"], "2");
    assert_exact_count(&first["counts"]["not_selected"], "2");

    assert_eq!(first["answer"]["population"], "selected_before_after_cases");
    assert_eq!(first["answer"]["find_frontier_closed"], true);
    assert_eq!(first["answer"]["analysis_frontier_closed"], true);
    assert_eq!(first["answer"]["source_coverage_has_gaps"], false);
    assert_exact_count(&first["answer"]["counts"]["selected_cases"], "2");
    assert_exact_count(&first["answer"]["counts"]["admitted_cases"], "4");
    let answer_paths = answer_mechanism(&first, "paths");
    assert_exact_count(&answer_paths["counts"]["structural_mechanisms"], "1");
    assert_exact_count(&answer_paths["counts"]["successful_cases"], "2");
    assert_exact_count(&answer_paths["counts"]["unavailable_cases"], "0");
    assert_eq!(
        answer_paths["sealed_target_support"]["distinct_target_starters"],
        "2"
    );
    assert!(answer_paths["evidence"]["structural_closure_root"]
        .as_str()
        .is_some_and(|root| root.len() == 64));
    assert!(answer_paths["evidence"]["starter_support_closure_root"]
        .as_str()
        .is_some_and(|root| root.len() == 64));

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
    let mechanism_preview = &mechanism_summary["grouped_preview"];
    assert_exact_count(&mechanism_preview["counts"]["raw_groups"], "1");
    assert_exact_count(&mechanism_preview["counts"]["output_groups"], "1");
    assert_eq!(mechanism_preview["preview"]["status"]["kind"], "complete");
    assert_eq!(
        mechanism_preview["preview"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mechanism_preview_row = &mechanism_preview["preview"]["rows"][0];
    assert_eq!(mechanism_preview_row["projection_ordinal"], "0");
    assert!(mechanism_preview_row.get("record_id").is_none());
    assert!(mechanism_preview_row.get("member_count").is_none());
    assert_eq!(mechanism_preview_row["values"]["mechanisms"], 1);
    assert_eq!(mechanism_preview_row["values"]["execution_profiles"], 1);
    assert_eq!(mechanism_preview_row["values"]["raw_signatures"], 1);
    assert_eq!(mechanism_preview_row["values"]["explained_cases"], 2);
    let answer_mechanism_summary = answer_result(&first, "mechanism_summary");
    assert_eq!(
        &answer_mechanism_summary["grouped_result"],
        mechanism_preview
    );

    assert_eq!(first["publication"]["caught_up"], true);
    assert_eq!(
        first["publication"]["artifacts_caught_up"],
        first["publication"]["artifact_count"]
    );

    let manifest = read_json(&output_directory.join("manifest.json"));
    assert_eq!(manifest["schema_version"], 9);
    assert_eq!(
        manifest["publication_cursor"]["file"],
        ".publication-cursor-v9.json"
    );
    let publication_cursor = read_json(&output_directory.join(".publication-cursor-v9.json"));
    assert_eq!(publication_cursor["schema_version"], 9);
    let report_artifacts = first["publication"]["artifacts"]
        .as_array()
        .expect("report publication artifacts");
    let manifest_artifacts = manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts");
    assert_eq!(report_artifacts.len(), manifest_artifacts.len());
    for (reported, manifested) in report_artifacts.iter().zip(manifest_artifacts) {
        for key in [
            "key",
            "name",
            "kind",
            "published_lines",
            "published_bytes",
            "caught_up_to_journal_prefix",
            "prefix_digest",
            "layer_roots",
        ] {
            assert_eq!(reported[key], manifested[key], "artifact `{key}` diverged");
        }
        assert_eq!(reported["relative_path"], manifested["path"]);
    }
    assert!(manifest["artifacts"]
        .as_array()
        .expect("manifest artifacts")
        .iter()
        .all(|artifact| artifact["kind"] != "subject_starter_support"));

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
        assert_eq!(selected["schema_version"], 9);
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
    assert_eq!(selected_transitions, vec![(1, 2), (2, 3)]);

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
    let mut mechanism_transition_by_case = BTreeMap::new();
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
        assert!(
            mechanism_transition_by_case
                .insert(
                    row["record"]["row_id"]["case_id"]
                        .as_str()
                        .expect("incidence case ID")
                        .to_owned(),
                    row["record"]["row_id"]["transition_id"]
                        .as_str()
                        .expect("incidence transition ID")
                        .to_owned(),
                )
                .is_none(),
            "smoke query has one mechanism incidence per selected case"
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
    let mechanism_id = structural_mechanisms
        .iter()
        .next()
        .expect("one structural mechanism ID")
        .to_string();

    let case_transitions = read_ndjson(&artifact_path(
        &manifest,
        &output_directory,
        "selected_case_transition_graph",
    ));
    assert_eq!(case_transitions.len(), 4);
    assert_eq!(case_transitions[0]["record"]["kind"], "header");
    assert_eq!(
        case_transitions[0]["record"]["source_order"],
        "journal_selected_discovery"
    );
    assert_eq!(
        case_transitions[0]["record"]["value_authorization"]["authorizing_view_name"],
        "selected_cases"
    );
    let transition_rows = case_transitions
        .iter()
        .filter(|record| record["record"]["kind"] == "case_transition")
        .collect::<Vec<_>>();
    assert_eq!(transition_rows.len(), 2);
    for row in transition_rows {
        let record = &row["record"];
        let case_id = record["case_id"].as_str().expect("case-transition CaseId");
        let &(before, after) = selected_values_by_case
            .get(case_id)
            .expect("case-transition row must name a selected case");
        assert_eq!(record["context"], serde_json::json!({ "kind": "unit" }));
        assert_eq!(record["before"], before);
        assert_eq!(record["after"], after);
        assert_ne!(record["before_state_id"], record["after_state_id"]);
        let expected_transition_id = mechanism_transition_by_case
            .get(case_id)
            .expect("mechanism incidence must join through TransitionId");
        assert_eq!(
            record["transition_id"].as_str(),
            Some(expected_transition_id.as_str())
        );
    }
    let case_transition_closure = &case_transitions[3]["record"];
    assert_eq!(case_transition_closure["kind"], "closure");
    assert_eq!(case_transition_closure["frontier"], "exact");
    assert_exact_count(&case_transition_closure["counts"]["selected_cases"], "2");
    assert_exact_count(&case_transition_closure["counts"]["state_nodes"], "3");
    assert_exact_count(
        &case_transition_closure["counts"]["semantic_transitions"],
        "2",
    );
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
        "not_materialized"
    );
    assert_eq!(
        mechanism_support["record"]["starter_projection"]["artifact"],
        Value::Null
    );

    let node_support = structural_support
        .iter()
        .find(|record| {
            record["record"]["kind"] == "structural_subject_support"
                && record["record"]["subject"]["kind"] == "node"
                && record["record"]["subject"]["facet"] == "activation"
                && record["record"]["case_count"]["status"] == "exact"
                && record["record"]["case_count"]["value"] == "2"
                && record["record"]["origin_preimage_support"]["distinct_starter_count"]["status"]
                    == "exact"
                && record["record"]["origin_preimage_support"]["distinct_starter_count"]["value"]
                    == "2"
        })
        .expect("activation node with exact two-starter origin preimage");
    assert_eq!(
        node_support["record"]["origin_preimage_support"]["correlated_origin_successor"]["status"],
        "not_materialized"
    );
    assert_eq!(
        node_support["record"]["starter_projection"]["status"],
        "not_materialized"
    );
    let node_id = node_support["record"]["subject"]["structural_node_id"]
        .as_str()
        .expect("structural node ID")
        .to_owned();
    let node_projection_plan_id = node_support["record"]["projection_plan_id"]
        .as_str()
        .expect("node projection plan ID")
        .to_owned();

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

    let first_next_sequence = first["run"]["checkpoint"]["next_sequence"].clone();
    let first_journal_head = first["run"]["checkpoint"]["journal_head"].clone();
    let first_artifact_count = first["publication"]["artifact_count"]
        .as_u64()
        .expect("first artifact count");

    // Model a completed publication-v9 cursor created before the automatic
    // case-transition consumer existed. The cursor remains authenticated by
    // the unchanged journal and prior artifact prefixes; removing the derived
    // graph entry/file and stale manifest exercises the additive-extension
    // path without altering semantic evidence.
    let cursor_path = output_directory.join(".publication-cursor-v9.json");
    let mut legacy_cursor = read_json(&cursor_path);
    assert!(legacy_cursor["artifacts"]
        .as_object_mut()
        .expect("publication cursor artifacts")
        .remove("graph:case-transitions")
        .is_some());
    std::fs::write(
        &cursor_path,
        serde_json::to_vec_pretty(&legacy_cursor).expect("serialize legacy publication cursor"),
    )
    .expect("write legacy publication cursor");
    std::fs::remove_file(artifact_path(
        &manifest,
        &output_directory,
        "selected_case_transition_graph",
    ))
    .expect("remove derived graph for additive attachment simulation");
    std::fs::remove_file(output_directory.join("manifest.json"))
        .expect("remove stale manifest for additive attachment simulation");
    let published_before_attachment = snapshot_files(&output_directory);

    let fixture_source = std::fs::read_to_string(&fixture).expect("read Explore fixture");
    let next_query = fixture_source
        .find("\n? explore relational_stream_empty_smoke")
        .expect("second Explore fixture query");
    let closing_brace = fixture_source[..next_query]
        .rfind('}')
        .expect("first Explore fixture query closing brace");
    let projected_source = format!(
        "{}    starters selected_activation_node_in_path from mechanisms paths for node activation \"{}\" within mechanism \"{}\" using values from selected_cases\n{}",
        &fixture_source[..closing_brace],
        node_id,
        mechanism_id,
        &fixture_source[closing_brace..],
    );
    let projected_fixture = temp
        .path()
        .join("relational-explore-with-node-starters.runa");
    std::fs::write(&projected_fixture, projected_source).expect("write projected Explore fixture");

    let attached_output = run_explore(&projected_fixture, &run_state, &output_directory);
    assert_success(&attached_output);
    let attached = parse_stdout(&attached_output);

    assert_eq!(attached["run"]["lifecycle"], "complete");
    assert_eq!(attached["run"]["appended"]["semantic_batches"], 0);
    assert_eq!(attached["run"]["appended"]["semantic_events"], 0);
    assert_eq!(
        attached["run"]["checkpoint"]["next_sequence"],
        first_next_sequence
    );
    assert_eq!(
        attached["run"]["checkpoint"]["journal_head"],
        first_journal_head
    );
    assert_eq!(attached["query"]["identity"], first["query"]["identity"]);
    assert_eq!(attached["answer"], first["answer"]);
    assert_eq!(attached["publication"]["lines_appended"], 7);
    assert_eq!(attached["publication"]["source_ordinals_advanced"], 7);
    assert_eq!(attached["publication"]["caught_up"], true);
    assert_eq!(
        attached["publication"]["artifact_count"],
        first_artifact_count + 1
    );

    let attached_manifest = read_json(&output_directory.join("manifest.json"));
    assert_eq!(attached_manifest["schema_version"], 9);
    for identity_key in [
        "checked_program",
        "relation_id",
        "admission_id",
        "question_id",
        "analysis_graph_digest",
        "journal_id",
    ] {
        assert_eq!(
            attached_manifest["identity"][identity_key], manifest["identity"][identity_key],
            "starter attachment changed core identity `{identity_key}`"
        );
    }
    assert_ne!(
        attached_manifest["identity"]["starter_consumer_set_id"],
        manifest["identity"]["starter_consumer_set_id"]
    );
    let attached_case_transition_path = artifact_path(
        &attached_manifest,
        &output_directory,
        "selected_case_transition_graph",
    );
    // Projection payload and order are stable. The envelope authorization is
    // deliberately the checkpoint at which this additive publication occurs,
    // so a completed attachment must name the final checkpoint on every line.
    let attached_case_transitions = read_ndjson(&attached_case_transition_path);
    assert_eq!(
        attached_case_transitions.len(),
        case_transitions.len(),
        "additive attachment changed the case-transition record count"
    );
    for (record_index, (attached_record, original_record)) in attached_case_transitions
        .iter()
        .zip(&case_transitions)
        .enumerate()
    {
        for stable_key in [
            "artifact",
            "name",
            "record",
            "schema_version",
            "source_ordinal",
        ] {
            assert_eq!(
                attached_record[stable_key], original_record[stable_key],
                "case-transition graph record {record_index} changed stable field `{stable_key}` during additive attachment"
            );
        }
        assert_eq!(
            attached_record["authorized_at"]["next_sequence"],
            first_next_sequence
        );
        assert_eq!(
            attached_record["authorized_at"]["journal_head"],
            first_journal_head
        );
    }

    let starter_artifact = attached_manifest["artifacts"]
        .as_array()
        .expect("attached manifest artifacts")
        .iter()
        .find(|artifact| {
            artifact["kind"] == "subject_starter_support"
                && artifact["name"] == "selected_activation_node_in_path"
        })
        .expect("explicit node starter artifact");
    assert_eq!(
        starter_artifact["path"],
        "starters/selected_activation_node_in_path.ndjson"
    );
    assert_eq!(starter_artifact["published_lines"], "3");
    assert_eq!(starter_artifact["scope"], "one_explicit_structural_subject");
    assert_eq!(
        starter_artifact["record_schema"],
        "futuruna.relational-subject-starters-v2"
    );
    assert_eq!(starter_artifact["record_schema_version"], 2);
    assert_eq!(
        starter_artifact["availability"]["status"],
        "exact_projection_available"
    );
    assert_eq!(starter_artifact["subject"]["kind"], "node");
    assert_eq!(starter_artifact["subject"]["facet"], "activation");
    assert_eq!(starter_artifact["subject"]["structural_node_id"], node_id);
    assert_eq!(
        starter_artifact["support_slice"]["kind"],
        "within_mechanism"
    );
    assert_eq!(
        starter_artifact["support_slice"]["structural_mechanism_id"],
        mechanism_id
    );
    assert_eq!(starter_artifact["contains_node_edge_projections"], true);
    assert_eq!(starter_artifact["contains_typed_values"], true);
    assert_eq!(
        starter_artifact["authorization"]["authorizing_view_name"],
        "selected_cases"
    );

    let starter_records = read_ndjson(
        &output_directory.join(
            starter_artifact["path"]
                .as_str()
                .expect("starter artifact path"),
        ),
    );
    assert_eq!(starter_records.len(), 3);
    assert!(starter_records
        .iter()
        .all(|record| record["schema_version"] == 9));
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
            "subject_starters_header",
            "subject_starters_page",
            "subject_starters_closure",
        ]
    );
    let starter_header = &starter_records[0]["record"];
    assert_ne!(
        starter_header["projection_plan_id"], node_projection_plan_id,
        "route-conditioned plan identity must differ from total-node support"
    );
    assert_eq!(
        starter_header["support_slice"],
        starter_artifact["support_slice"]
    );
    assert_eq!(starter_header["exact_case_count"], "2");
    assert_eq!(starter_header["exact_distinct_starter_count"], Value::Null);

    let starter_page = &starter_records[1]["record"];
    assert_eq!(
        starter_page["support_slice"],
        starter_artifact["support_slice"]
    );
    assert_eq!(starter_page["page_ordinal"], "0");
    assert_eq!(starter_page["start_after"], Value::Null);
    assert_eq!(starter_page["exhausted"], true);
    let starter_members = starter_page["members"]
        .as_array()
        .expect("starter page members");
    assert_eq!(starter_members.len(), 2);
    assert!(
        starter_members.windows(2).all(|window| {
            window[0]["source_key"]
                .as_str()
                .expect("prior starter source key")
                < window[1]["source_key"]
                    .as_str()
                    .expect("next starter source key")
        }),
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

    let starter_closure = &starter_records[2]["record"];
    assert_eq!(
        starter_closure["support_slice"],
        starter_artifact["support_slice"]
    );
    assert_eq!(starter_closure["exact_case_count"], "2");
    assert_eq!(starter_closure["exact_distinct_starter_count"], "2");
    assert_eq!(starter_closure["page_count"], "1");

    let published_after_attachment = snapshot_files(&output_directory);
    for (path, before) in &published_before_attachment {
        if path == Path::new("manifest.json") || path == Path::new(".publication-cursor-v9.json") {
            continue;
        }
        assert_eq!(
            published_after_attachment.get(path),
            Some(before),
            "starter attachment rewrote prior artifact {}",
            path.display()
        );
    }
    let added_paths = published_after_attachment
        .keys()
        .filter(|path| !published_before_attachment.contains_key(*path))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        added_paths,
        vec![
            PathBuf::from("graphs/case-transitions.ndjson"),
            PathBuf::from("manifest.json"),
            PathBuf::from("starters/selected_activation_node_in_path.ndjson"),
        ]
    );

    let resumed_output = run_explore(&projected_fixture, &run_state, &output_directory);
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
    assert_eq!(resumed["answer"], attached["answer"]);
    assert_eq!(resumed["publication"]["lines_appended"], 0);
    assert_eq!(resumed["publication"]["source_ordinals_advanced"], 0);
    assert_eq!(resumed["publication"]["caught_up"], true);
    assert_eq!(
        snapshot_files(&output_directory),
        published_after_attachment
    );
}
