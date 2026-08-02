use calamine::{open_workbook_auto, Data, Reader, SheetVisible};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_PATH_ID: AtomicU64 = AtomicU64::new(0);

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/calculation/tax.calculate.runa")
}

fn temp_path(extension: &str) -> PathBuf {
    let unique_id = NEXT_TEMP_PATH_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "futuruna-calculate-{}-{}-{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
        unique_id,
        extension
    ))
}

fn run(args: &[&str]) -> Output {
    Command::new(runa())
        .args(args)
        .output()
        .expect("run runa calculation command")
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn schema_exposes_reachable_types_metadata_and_fingerprint() {
    let fixture = fixture();
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    assert_eq!(schema["schema"], "futuruna.calculate.v1");
    assert_eq!(schema["entry"], "calculate_tax");
    assert_eq!(schema["input"]["name"], "TaxInput");
    assert_eq!(schema["output"]["name"], "TaxResult");
    assert_eq!(schema["schema_hash"].as_str().expect("hash").len(), 64);

    let definitions = schema["definitions"].as_array().expect("definitions");
    assert!(definitions
        .iter()
        .any(|definition| definition["name"] == "Child"));
    assert!(definitions
        .iter()
        .any(|definition| definition["name"] == "FilingStatus"));
    assert_eq!(schema["metadata"][0]["role"], "source");
    assert_eq!(schema["metadata"][0]["binding"], "tax_source");
    assert!(schema["metadata"][0]["symbols"]
        .as_array()
        .expect("metadata symbols")
        .iter()
        .any(|symbol| symbol == "calculate_tax"));
}

#[test]
fn schema_resolves_metadata_from_plain_imports() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/meta-imports/model.calculate.runa");
    let output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let schema = parse_stdout(&output);
    let metadata = schema["metadata"].as_array().expect("metadata");
    assert_eq!(metadata.len(), 2);
    for reference in metadata {
        assert_eq!(reference["type"], "SourceInfo");
        assert!(reference["value"]
            .as_str()
            .expect("imported metadata value")
            .starts_with("SourceInfo("));
        assert!(reference.get("definition_file").is_none());
    }
}

#[test]
fn json_batch_invokes_conditions_and_exceptions() {
    let fixture = fixture();
    let input_path = temp_path("json");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "json",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());

    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&input_path).expect("read generated JSON template"))
            .expect("template JSON");
    input["cases"][0]["input"] = serde_json::json!({
        "monthly_income": 50_000,
        "filing_status": { "$variant": "Married" },
        "deduction": 12_000,
        "children": [{ "name": "Ada", "age": 7 }]
    });
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&input).expect("encode input"),
    )
    .expect("write input");

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = parse_stdout(&output);
    assert_eq!(
        result["diagnostics"].as_array().expect("diagnostics").len(),
        0
    );
    assert_eq!(result["results"][0]["result"]["annual_income"], 600_000);
    assert_eq!(result["results"][0]["result"]["taxable_income"], 588_000);
    assert_eq!(result["results"][0]["result"]["annual_tax"], 117_600);
}

#[test]
fn stale_json_template_fails_closed_with_both_hashes() {
    let fixture = fixture();
    let input_path = temp_path("json");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());
    let mut input: Value = serde_json::from_slice(&std::fs::read(&input_path).expect("template"))
        .expect("template JSON");
    input["$futuruna"]["schema_hash"] = Value::String("stale".to_string());
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input).unwrap()).unwrap();

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    let message = result["diagnostics"][0]["message"]
        .as_str()
        .expect("stale diagnostic");
    assert!(message.contains("stale calculation template"));
    assert!(message.contains("stale"));
    assert!(message.contains(
        result["$futuruna"]["schema_hash"]
            .as_str()
            .expect("expected hash")
    ));
    assert!(result["results"].as_array().expect("results").is_empty());
}

#[test]
fn toml_template_round_trips_optional_and_nested_values() {
    let fixture = fixture();
    let input_path = temp_path("toml");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "toml",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );
    let source = std::fs::read_to_string(&input_path).expect("TOML template");
    assert!(source.contains("[cases.input.filing_status]"));
    assert!(!source.contains("deduction"));

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = parse_stdout(&output);
    assert_eq!(result["results"][0]["result"]["annual_tax"], 0);
}

#[test]
fn xlsx_template_round_trips_and_output_has_result_sheets() {
    let fixture = fixture();
    let input_path = temp_path("xlsx");
    let output_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        for expected in ["_futuruna", "_tables", "_columns", "cases", "children"] {
            assert!(
                workbook.sheet_names().iter().any(|name| name == expected),
                "missing generated sheet {expected}"
            );
        }
        assert_workbook_visibility(&workbook, &["_futuruna", "_tables", "_columns"], "cases");
        assert_eq!(
            workbook_headers(&mut workbook, "cases"),
            ["case_id", "monthly_income", "filing_status", "deduction"]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "children"),
            ["case_id", "item_id", "position", "name", "age"]
        );
    }

    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(sheets, "cases", 1, 1, Data::String("50000".to_string()));
        set_workbook_cell(sheets, "cases", 1, 2, Data::String("Married".to_string()));
        set_workbook_cell(sheets, "cases", 1, 3, Data::String("12000".to_string()));
        workbook_sheet_mut(sheets, "children").push(vec![
            Data::String("case-1".to_string()),
            Data::String("child-1".to_string()),
            Data::Int(1),
            Data::String("Ada".to_string()),
            Data::Int(7),
        ]);
    });

    let call = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
        "--output",
        output_path.to_str().expect("output path"),
    ]);
    assert!(
        call.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&call.stderr)
    );

    let mut workbook = open_workbook_auto(&output_path).expect("result workbook");
    assert_workbook_visibility(&workbook, &["_futuruna"], "results");
    assert!(workbook.sheet_names().iter().any(|name| name == "results"));
    assert!(workbook
        .sheet_names()
        .iter()
        .any(|name| name == "diagnostics"));
    let results = workbook.worksheet_range("results").expect("results sheet");
    assert_eq!(
        results.get((1, 0)).map(ToString::to_string).as_deref(),
        Some("case-1")
    );
    assert!(results
        .get((1, 1))
        .map(ToString::to_string)
        .expect("result JSON")
        .contains("\"annual_tax\":117600"));

    std::fs::remove_file(&input_path).ok();
    std::fs::remove_file(&output_path).ok();
}

#[test]
fn personskatteloven_xlsx_boundary_round_trips_wage_earner_case() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat.calculate.runa");
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    edit_workbook(&input_path, |sheets| {
        let values = [
            Data::String("personskat-2026".to_string()),
            Data::String("2026".to_string()),
            Data::String("København".to_string()),
            Data::String("600000".to_string()),
            Data::String("0".to_string()),
            Data::String("0".to_string()),
            Data::String("Ll9lMereEnd15ÅrFørFolkepension".to_string()),
            Data::String("0".to_string()),
            Data::String("0".to_string()),
            Data::String("0".to_string()),
            Data::String("0".to_string()),
            Data::String("Ll9lIngenPbl20Udbetaling".to_string()),
            Data::String("Fyldt18EllerGift".to_string()),
            Data::Bool(false),
        ];
        for (column, value) in values.into_iter().enumerate() {
            set_workbook_cell(sheets, "cases", 1, column, value);
        }
    });

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .is_empty());
    assert_eq!(
        result["results"][0]["result"]["samlet_inkl_am_efter_personfradrag_kroner"],
        208_726
    );
}

#[test]
fn investment_classification_xlsx_expands_payloads_and_round_trips_template() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/investeringsklassifikation.calculate.runa");
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab",
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        let case_headers = workbook_headers(&mut workbook, "cases");
        for expected in [
            "meddelelse.$variant",
            "meddelelse.AblPar19BOrdinærMeddelelse.virkningsår",
            "meddelelse.AblPar19BNyoprettetMeddelelse.oprettelsesdato.år",
            "oplysninger.$variant",
        ] {
            assert!(
                case_headers.iter().any(|header| header == expected),
                "missing typed investment input column {expected}"
            );
        }
        let owner_headers = workbook_headers(&mut workbook, "aktivmasse_ejerposter");
        for expected in [
            "$variant",
            "AblEjerpostIPar19B.ejerandel.ejede_kapitalenheder",
            "AblEjerpostIPar21.klassifikationsinput.oplysninger.$variant",
        ] {
            assert!(
                owner_headers.iter().any(|header| header == expected),
                "missing typed owner input column {expected}"
            );
        }

        let metadata = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let notification_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(2).map(ToString::to_string).as_deref()
                    == Some("meddelelse.AblPar19BOrdinærMeddelelse.virkningsår")
            })
            .expect("notification payload metadata");
        assert_eq!(
            notification_row.get(5).map(ToString::to_string).as_deref(),
            Some("integer")
        );
        assert!(notification_row
            .get(8)
            .map(ToString::to_string)
            .expect("variant guard")
            .contains("AblPar19BOrdinærMeddelelse"));
    }

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab",
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .is_empty());
    assert_eq!(
        result["results"][0]["result"]["effektiv_status"]["$variant"],
        "AblObligationsbaseretInvesteringsselskabEfterPar19C"
    );
}

#[test]
fn xlsx_relational_tables_round_trip_nested_collections_and_isolate_bad_cases() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    std::fs::write(
        &source_path,
        "# Toy(label: String)\n\
# Child(name: String, toys: List(Toy))\n\
# Input(children: List(Child), aliases: List(String), totals: Map(String, Int), flags: Set(Int))\n\
@ calculate\n\
> echo(input: Input) -> Input { input }\n",
    )
    .expect("write relational calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(sheets, "cases", 1, 0, Data::String("valid".to_string()));
        workbook_sheet_mut(sheets, "cases").push(vec![Data::String("invalid".to_string())]);

        workbook_sheet_mut(sheets, "children").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("child-valid".to_string()),
                Data::Int(1),
                Data::String("Ada".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("child-invalid".to_string()),
                Data::Int(1),
                Data::String("Bad".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "children_toys").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("child-valid".to_string()),
                Data::String("toy-valid".to_string()),
                Data::Int(1),
                Data::String("Ball".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("missing-parent".to_string()),
                Data::String("toy-orphan".to_string()),
                Data::Int(1),
                Data::String("Orphan".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "aliases").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("alias-valid-2".to_string()),
                Data::Int(2),
                Data::String("B".to_string()),
            ],
            vec![
                Data::String("valid".to_string()),
                Data::String("alias-valid-1".to_string()),
                Data::Int(1),
                Data::String("A".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("alias-bad-1".to_string()),
                Data::Int(1),
                Data::String("B".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("alias-bad-2".to_string()),
                Data::Int(1),
                Data::String("C".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "totals").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("total-valid".to_string()),
                Data::String("salary".to_string()),
                Data::Int(10),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("total-bad-1".to_string()),
                Data::String("same".to_string()),
                Data::String("not-an-int".to_string()),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("total-bad-2".to_string()),
                Data::String("same".to_string()),
                Data::Int(2),
            ],
        ]);
        workbook_sheet_mut(sheets, "flags").extend([
            vec![
                Data::String("valid".to_string()),
                Data::String("flag-valid".to_string()),
                Data::Int(7),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("flag-duplicate".to_string()),
                Data::Int(1),
            ],
            vec![
                Data::String("invalid".to_string()),
                Data::String("flag-duplicate".to_string()),
                Data::Int(2),
            ],
        ]);
    });

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert_eq!(result["results"].as_array().expect("results").len(), 1);
    assert_eq!(result["results"][0]["case_id"], "valid");
    assert_eq!(result["results"][0]["result"]["children"][0]["name"], "Ada");
    assert_eq!(
        result["results"][0]["result"]["children"][0]["toys"][0]["label"],
        "Ball"
    );
    assert_eq!(result["results"][0]["result"]["aliases"][0], "A");
    assert_eq!(result["results"][0]["result"]["aliases"][1], "B");
    assert_eq!(result["results"][0]["result"]["totals"]["salary"], 10);
    assert_eq!(result["results"][0]["result"]["flags"][0], 7);
    let diagnostics = result["diagnostics"].as_array().expect("diagnostics");
    for expected in [
        "orphan parent_id",
        "duplicate list position",
        "duplicate map key",
        "duplicate item_id",
        "not an exact signed integer",
    ] {
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["case_id"] == "invalid"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(expected))
        }));
    }
}

#[test]
fn xlsx_payload_variants_expand_into_typed_columns_and_child_tables() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    std::fs::write(
        &source_path,
        "# Child(name: String, age: Int)\n\
# Selection = Empty | Fixed(amount: Int) | Pair(Int, String) | Family(label: String, children: List(Child))\n\
# Input(selection: Selection, history: List(Selection))\n\
@ calculate\n\
> echo(input: Input) -> Input { input }\n",
    )
    .expect("write payload-variant calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        assert_eq!(
            workbook_headers(&mut workbook, "cases"),
            [
                "case_id",
                "selection.$variant",
                "selection.Fixed.amount",
                "selection.Pair._0",
                "selection.Pair._1",
                "selection.Family.label",
            ]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "history"),
            [
                "case_id",
                "item_id",
                "position",
                "$variant",
                "Fixed.amount",
                "Pair._0",
                "Pair._1",
                "Family.label",
            ]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "selection_Family_children"),
            ["case_id", "item_id", "position", "name", "age"]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "history_Family_children"),
            ["case_id", "parent_id", "item_id", "position", "name", "age",]
        );
    }

    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(sheets, "cases", 1, 1, Data::String("Family".to_string()));
        set_workbook_cell(sheets, "cases", 1, 5, Data::String("primary".to_string()));
        workbook_sheet_mut(sheets, "cases").push(vec![
            Data::String("invalid".to_string()),
            Data::String("Empty".to_string()),
            Data::Int(9),
        ]);
        workbook_sheet_mut(sheets, "cases").push(vec![
            Data::String("inactive-table".to_string()),
            Data::String("Empty".to_string()),
        ]);
        workbook_sheet_mut(sheets, "selection_Family_children").extend([
            vec![
                Data::String("case-1".to_string()),
                Data::String("root-child".to_string()),
                Data::Int(1),
                Data::String("Ada".to_string()),
                Data::Int(7),
            ],
            vec![
                Data::String("inactive-table".to_string()),
                Data::String("inactive-child".to_string()),
                Data::Int(1),
                Data::String("No".to_string()),
                Data::Int(1),
            ],
        ]);
        workbook_sheet_mut(sheets, "history").extend([
            vec![
                Data::String("case-1".to_string()),
                Data::String("history-fixed".to_string()),
                Data::Int(1),
                Data::String("Fixed".to_string()),
                Data::Int(12),
            ],
            vec![
                Data::String("case-1".to_string()),
                Data::String("history-pair".to_string()),
                Data::Int(2),
                Data::String("Pair".to_string()),
                Data::Empty,
                Data::Int(7),
                Data::String("seven".to_string()),
            ],
            vec![
                Data::String("case-1".to_string()),
                Data::String("history-family".to_string()),
                Data::Int(3),
                Data::String("Family".to_string()),
                Data::Empty,
                Data::Empty,
                Data::Empty,
                Data::String("secondary".to_string()),
            ],
        ]);
        workbook_sheet_mut(sheets, "history_Family_children").push(vec![
            Data::String("case-1".to_string()),
            Data::String("history-family".to_string()),
            Data::String("nested-child".to_string()),
            Data::Int(1),
            Data::String("Bo".to_string()),
            Data::Int(10),
        ]);
    });

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert_eq!(result["results"].as_array().expect("results").len(), 1);
    assert_eq!(result["results"][0]["case_id"], "case-1");
    assert_eq!(
        result["results"][0]["result"]["selection"]["$variant"],
        "Family"
    );
    assert_eq!(
        result["results"][0]["result"]["selection"]["children"][0]["name"],
        "Ada"
    );
    assert_eq!(
        result["results"][0]["result"]["history"][0],
        serde_json::json!({ "$variant": "Fixed", "amount": 12 })
    );
    assert_eq!(
        result["results"][0]["result"]["history"][1],
        serde_json::json!({ "$variant": "Pair", "$values": [7, "seven"] })
    );
    assert_eq!(
        result["results"][0]["result"]["history"][2]["children"][0]["name"],
        "Bo"
    );
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| {
            diagnostic["case_id"] == "invalid"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("input column is inactive"))
        }));
    assert!(result["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| {
            diagnostic["case_id"] == "inactive-table"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("collection table is inactive"))
        }));
}

#[test]
fn xlsx_rejects_tampered_collection_topology() {
    let fixture = fixture();
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());
    edit_workbook(&input_path, |sheets| {
        set_workbook_cell(
            sheets,
            "_tables",
            1,
            0,
            Data::String("tampered.children".to_string()),
        );
    });

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("`_tables` metadata does not match the current contract"));
}

#[test]
fn calculate_with_prompt_argument_is_a_parse_error() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(value: Int)\n@ calculate(\"Value\")\n| calculate(input: Input) -> input.value\n",
    )
    .expect("write invalid source");
    let output = run(&["check", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not take arguments"));
}

#[test]
fn typed_function_is_a_calculation_boundary_too() {
    let source_path = temp_path("runa");
    let input_path = temp_path("json");
    std::fs::write(
        &source_path,
        "# Input(value: Int)\n# Output(value: Int)\n@ calculate\n> increment(input: Input) -> Output { Output(value = input.value + 1) }\n",
    )
    .expect("write function calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );
    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&input_path).expect("template")).unwrap();
    input["cases"][0]["input"]["value"] = 41.into();
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input).unwrap()).unwrap();
    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(output.status.success());
    assert_eq!(parse_stdout(&output)["results"][0]["result"]["value"], 42);
}

#[test]
fn untyped_rule_boundary_is_rejected_by_check() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(value: Int)\n@ calculate\n| calculate(input) -> input\n",
    )
    .expect("write invalid calculation");
    let output = run(&["check", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("input needs an explicit type"));
}

#[test]
fn multiple_entries_require_explicit_selection() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(value: Int)\n# Output(value: Int)\n@ calculate\n> first(input: Input) -> Output { Output(value = input.value) }\n@ calculate\n> second(input: Input) -> Output { Output(value = input.value + 1) }\n",
    )
    .expect("write multiple calculations");
    let ambiguous = run(&["schema", path.to_str().expect("source path")]);
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("multiple calculations"));

    let selected = run(&[
        "schema",
        path.to_str().expect("source path"),
        "--entry",
        "second",
    ]);
    std::fs::remove_file(&path).ok();
    assert!(selected.status.success());
    assert_eq!(parse_stdout(&selected)["entry"], "second");
}

#[test]
fn xlsx_formula_is_rejected_before_evaluation() {
    let fixture = fixture();
    let schema_output = run(&["schema", fixture.to_str().expect("fixture path")]);
    assert!(schema_output.status.success());
    let schema = parse_stdout(&schema_output);
    let hash = schema["schema_hash"].as_str().expect("schema hash");
    let input_path = temp_path("xlsx");
    write_test_workbook(&input_path, hash, true);

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("formulas are rejected"));
}

#[test]
fn stale_xlsx_template_reports_structural_hash_diagnostic() {
    let fixture = fixture();
    let input_path = temp_path("xlsx");
    write_test_workbook(&input_path, "stale", false);
    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert!(result["diagnostics"][0]["message"]
        .as_str()
        .expect("diagnostic")
        .contains("stale calculation template"));
}

#[test]
fn canonical_values_cover_generics_sums_maps_sets_and_case_isolation() {
    let source_path = temp_path("runa");
    let input_path = temp_path("json");
    std::fs::write(
        &source_path,
        "# Choice = Fixed(amount: Int) | Pair(Int, String) | Empty\n\
# Box(a) = Box(value: a)\n\
# Input(choice: Choice, boxed: Box(Int), totals: Map(String, Int), flags: Set(Int))\n\
# Output(choice: Choice, boxed: Box(Int), totals: Map(String, Int), flags: Set(Int))\n\
@ calculate\n\
> echo(input: Input) -> Output {\n\
    Output(choice = input.choice, boxed = input.boxed, totals = input.totals, flags = input.flags)\n\
}\n",
    )
    .expect("write canonical-value calculation");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(
        template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&template.stderr)
    );
    let mut input: Value =
        serde_json::from_slice(&std::fs::read(&input_path).expect("template")).unwrap();
    input["cases"] = serde_json::json!([
        {
            "case_id": "valid",
            "input": {
                "choice": { "$variant": "Pair", "$values": [7, "seven"] },
                "boxed": { "value": 9 },
                "totals": { "a": 1, "b": 2 },
                "flags": [2, 1]
            }
        },
        {
            "case_id": "duplicate-set",
            "input": {
                "choice": { "$variant": "Empty" },
                "boxed": { "value": 0 },
                "totals": {},
                "flags": [1, 1]
            }
        },
        {
            "case_id": "unknown-field",
            "input": {
                "choice": { "$variant": "Fixed", "amount": 3 },
                "boxed": { "value": 0 },
                "totals": {},
                "flags": [],
                "undeclared": true
            }
        }
    ]);
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input).unwrap()).unwrap();

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert_eq!(result["results"].as_array().expect("results").len(), 1);
    assert_eq!(result["results"][0]["case_id"], "valid");
    assert_eq!(
        result["results"][0]["result"]["choice"],
        serde_json::json!({ "$variant": "Pair", "$values": [7, "seven"] })
    );
    assert_eq!(result["results"][0]["result"]["boxed"]["value"], 9);
    let diagnostics = result["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["case_id"] == "duplicate-set"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("duplicate set value"))
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["case_id"] == "unknown-field"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("unknown field"))
    }));
}

fn write_test_workbook(path: &Path, schema_hash: &str, formula: bool) {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let metadata = workbook.add_worksheet();
    metadata.set_name("_futuruna").unwrap();
    metadata.write_string(0, 0, "key").unwrap();
    metadata.write_string(0, 1, "value").unwrap();
    for (row, (key, value)) in [
        ("schema", "futuruna.calculate.xlsx.input.v3"),
        ("contract_schema", "futuruna.calculate.v1"),
        ("schema_hash", schema_hash),
        ("entry", "calculate_tax"),
        ("encoding", "futuruna-canonical-json-v1"),
    ]
    .into_iter()
    .enumerate()
    {
        metadata.write_string(row as u32 + 1, 0, key).unwrap();
        metadata.write_string(row as u32 + 1, 1, value).unwrap();
    }

    let cases = workbook.add_worksheet();
    cases.set_name("cases").unwrap();
    for (column, header) in ["case_id", "monthly_income", "filing_status", "deduction"]
        .into_iter()
        .enumerate()
    {
        cases.write_string(0, column as u16, header).unwrap();
    }
    cases.write_string(1, 0, "case-1").unwrap();
    if formula {
        cases.write_formula(1, 1, "=1+1").unwrap();
    } else {
        cases.write_string(1, 1, "0").unwrap();
    }
    cases.write_string(1, 2, "Single").unwrap();
    workbook.save(path).unwrap();
}

fn workbook_headers(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    sheet: &str,
) -> Vec<String> {
    workbook
        .worksheet_range(sheet)
        .expect("worksheet")
        .rows()
        .next()
        .expect("header row")
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn assert_workbook_visibility(
    workbook: &calamine::Sheets<std::io::BufReader<std::fs::File>>,
    hidden_sheets: &[&str],
    first_visible_sheet: &str,
) {
    let metadata = workbook.sheets_metadata();
    for expected in hidden_sheets {
        let sheet = metadata
            .iter()
            .find(|sheet| sheet.name == *expected)
            .unwrap_or_else(|| panic!("missing generated sheet {expected}"));
        assert_eq!(
            sheet.visible,
            SheetVisible::Hidden,
            "machine sheet {expected} should be hidden"
        );
    }
    assert_eq!(
        metadata
            .iter()
            .find(|sheet| sheet.visible == SheetVisible::Visible)
            .map(|sheet| sheet.name.as_str()),
        Some(first_visible_sheet)
    );
}

fn edit_workbook(path: &Path, edit: impl FnOnce(&mut Vec<(String, Vec<Vec<Data>>)>)) {
    let mut workbook = open_workbook_auto(path).expect("open workbook for editing");
    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets: Vec<(String, Vec<Vec<Data>>)> = sheet_names
        .into_iter()
        .map(|name| {
            let rows = workbook
                .worksheet_range(&name)
                .expect("worksheet")
                .rows()
                .map(|row| row.to_vec())
                .collect();
            (name, rows)
        })
        .collect();
    drop(workbook);
    edit(&mut sheets);

    let mut workbook = rust_xlsxwriter::Workbook::new();
    for (name, rows) in sheets {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&name).expect("sheet name");
        for (row, cells) in rows.iter().enumerate() {
            for (column, cell) in cells.iter().enumerate() {
                write_workbook_data(worksheet, row as u32, column as u16, cell);
            }
        }
    }
    workbook.save(path).expect("save edited workbook");
}

fn workbook_sheet_mut<'a>(
    sheets: &'a mut [(String, Vec<Vec<Data>>)],
    name: &str,
) -> &'a mut Vec<Vec<Data>> {
    &mut sheets
        .iter_mut()
        .find(|(sheet, _)| sheet == name)
        .unwrap_or_else(|| panic!("missing sheet {name}"))
        .1
}

fn set_workbook_cell(
    sheets: &mut [(String, Vec<Vec<Data>>)],
    sheet: &str,
    row: usize,
    column: usize,
    value: Data,
) {
    let rows = workbook_sheet_mut(sheets, sheet);
    while rows.len() <= row {
        rows.push(Vec::new());
    }
    if rows[row].len() <= column {
        rows[row].resize(column + 1, Data::Empty);
    }
    rows[row][column] = value;
}

fn write_workbook_data(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: &Data,
) {
    match value {
        Data::Empty => {}
        Data::String(value) => {
            worksheet.write_string(row, column, value).unwrap();
        }
        Data::Float(value) => {
            worksheet.write_number(row, column, *value).unwrap();
        }
        Data::Int(value) => {
            worksheet.write_number(row, column, *value as f64).unwrap();
        }
        Data::Bool(value) => {
            worksheet.write_boolean(row, column, *value).unwrap();
        }
        value => {
            worksheet
                .write_string(row, column, value.to_string())
                .unwrap();
        }
    }
}
