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
fn calculation_output_encodes_empty_list_literals() {
    let source_path = temp_path("runa");
    let input_path = temp_path("json");
    std::fs::write(
        &source_path,
        "# EmptyListInput(marker: Int)\n\
# EmptyListResult(marker: Int, items: List(Int))\n\
\n\
@ calculate\n\
| calculate_empty_list(input: EmptyListInput) -> EmptyListResult(marker = input.marker, items = [])\n",
    )
    .expect("write empty-list calculation");

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

    let output = run(&[
        "call",
        source_path.to_str().expect("source path"),
        "--input",
        input_path.to_str().expect("input path"),
    ]);
    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let result = parse_stdout(&output);
    assert_eq!(
        result["results"][0]["result"]["items"],
        serde_json::json!([])
    );
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
        for expected in [
            "_futuruna",
            "_tables",
            "_columns",
            "_choices",
            "cases",
            "children",
        ] {
            assert!(
                workbook.sheet_names().iter().any(|name| name == expected),
                "missing generated sheet {expected}"
            );
        }
        assert_workbook_visibility(
            &workbook,
            &["_futuruna", "_tables", "_columns", "_choices"],
            "cases",
        );
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
fn xlsx_long_choice_sets_use_hidden_validation_ranges() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    let choices: Vec<String> = (1..=8)
        .map(|index| format!("ValidationChoiceWithEnoughCharactersNumber{index:02}"))
        .collect();
    let source = format!(
        "# LongChoice = {}\n\
# LongChoiceInput(choice: LongChoice)\n\
# LongChoiceResult(choice: LongChoice)\n\
\n\
@ calculate\n\
| calculate_long_choice(input: LongChoiceInput) -> LongChoiceResult(choice = input.choice)\n",
        choices.join(" | ")
    );
    std::fs::write(&source_path, source).expect("write long-choice calculation");

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

    let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
    assert_workbook_visibility(
        &workbook,
        &["_futuruna", "_tables", "_columns", "_choices"],
        "cases",
    );
    let choice_cells: Vec<String> = workbook
        .worksheet_range("_choices")
        .expect("choice metadata")
        .rows()
        .skip(1)
        .filter_map(|row| row.get(1))
        .map(ToString::to_string)
        .collect();
    assert_eq!(choice_cells, choices);

    std::fs::remove_file(&source_path).ok();
    std::fs::remove_file(&input_path).ok();
}

#[test]
fn personskatteloven_xlsx_boundary_round_trips_source_fact_cases() {
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

    {
        let mut workbook = open_workbook_auto(&input_path).expect("input workbook");
        assert_workbook_visibility(
            &workbook,
            &["_futuruna", "_tables", "_columns", "_choices"],
            "cases",
        );
        let case_headers = workbook_headers(&mut workbook, "cases");
        assert_eq!(case_headers.len(), 118);
        for expected in [
            "aktieavance.ordinært_aktieår.$variant",
            "lønmodtager.ligningsfradrag.befordring.$variant",
            "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsdage",
            "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.ligningslov9d.$variant",
            "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.ligningslov9d.MedLigningslov9D.input.befordringsudgifter.dokumenteret_faktisk_udgift_kroner",
            "kapitalindkomst.renter.renteindtægter_kroner",
            "kapitalindkomst.renter.renteudgifter_kroner",
            "kapitalindkomst.renter.næringsstatus",
            "kapitalindkomst.renter.ligningslov6.$variant",
            "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kurstab_kroner",
            "kapitalindkomst.renter.ligningslov6a.$variant",
            "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.arbejderboliger_beløb_kroner",
            "kapitalindkomst.ejendomsavance.$variant",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.MedFremførtEjendomstab.tab_kroner",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
            "skatteforhold.$variant",
            "skatteforhold.SærligeSkatteforhold.forhold.øvrig_aktieindkomst_kroner",
            "underskudsforhold.$variant",
            "underskudsforhold.SærligeUnderskudsforhold.forhold.ægtefælle_skattepligtig_indkomst_kroner",
            "årsopgørelse.$variant",
            "årsopgørelse.MedÅrsopgørelse.kreditter.a_skat_og_am_indeholdt_kroner",
        ] {
            assert!(
                case_headers.iter().any(|header| header == expected),
                "missing typed Personskatteloven input column {expected}"
            );
        }
        assert!(!case_headers
            .iter()
            .any(|header| header == "lønmodtager.ligningsmæssige_fradrag_kroner"));
        assert!(!case_headers
            .iter()
            .any(|header| header.contains("aftrapningsindkomst_kroner")));
        assert!(!case_headers
            .iter()
            .any(|header| header.contains("ligningslov9d_resultat")));
        assert_eq!(
            workbook_headers(&mut workbook, "kapitalindkomst_omkostninger"),
            [
                "case_id",
                "item_id",
                "position",
                "identifikation",
                "beløb_kroner"
            ]
        );
        let own_property_sheet = workbook_collection_sheet_name(
            &mut workbook,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser",
        );
        let spouse_property_sheet = workbook_collection_sheet_name(
            &mut workbook,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_afståelser",
        );
        for sheet in [&own_property_sheet, &spouse_property_sheet] {
            let property_headers = workbook_headers(&mut workbook, sheet);
            for expected in [
                "identifikation",
                "afståelse",
                "erhvervet_som_led_i_næring",
                "kontant_anskaffelsessum_kroner",
                "regulering_efter_par5_og_5a_kroner",
                "kontant_afståelsessum_kroner",
                "par11_stk2_genanbragt_erhvervsejendom",
                "ejendomsafgrænsning.$variant",
                "ejendomsafgrænsning.EblPar9Ejendom.tab_vedrørende_stuehus_ejerbolig_kroner",
                "anskaffelsesgrundlag.$variant",
                "anskaffelsesgrundlag.EblPar4Stk3TredjePktAnskaffelsesgrundlag.tab_efter_ejendomsværdi_par4_stk3_nr1_eller_2_kroner",
            ] {
                assert!(
                    property_headers.iter().any(|header| header == expected),
                    "missing property source-fact column {expected} on {sheet}"
                );
            }
        }
        let special_asset_headers = workbook_headers(&mut workbook, "aktieavance_særlige_aktiver");
        for expected in [
            "aktiv",
            "par17_modprøve.næringsstatus",
            "par17_modprøve.erhvervelsesstatus",
            "investeringsklassifikation.$variant",
        ] {
            assert!(
                special_asset_headers
                    .iter()
                    .any(|header| header == expected),
                "missing source-level ABL input column {expected}"
            );
        }

        let choices = workbook
            .worksheet_range("_choices")
            .expect("choice metadata");
        assert!(choices
            .rows()
            .flatten()
            .any(|cell| cell.to_string() == "AblNæringsaktiePar17"));

        let metadata = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let annual_assessment_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(2).map(ToString::to_string).as_deref()
                    == Some("årsopgørelse.MedÅrsopgørelse.overført_restskat_mv_kroner")
            })
            .expect("annual assessment payload metadata");
        assert!(annual_assessment_row
            .get(8)
            .map(ToString::to_string)
            .expect("variant guard")
            .contains("MedÅrsopgørelse"));
        let commuting_row = metadata
            .rows()
            .skip(1)
            .find(|row| {
                row.get(2).map(ToString::to_string).as_deref()
                    == Some("lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsdage")
            })
            .expect("commuting payload metadata");
        assert!(commuting_row
            .get(8)
            .map(ToString::to_string)
            .expect("commuting variant guard")
            .contains("MedBefordringsfradrag"));
    }

    edit_workbook(&input_path, |sheets| {
        let fill_wage_case = |sheets: &mut [(String, Vec<Vec<Data>>)],
                              row: usize,
                              case_id: &str| {
            for (header, value) in [
                    ("case_id", Data::String(case_id.to_string())),
                    ("lønmodtager.skatteår", Data::String("2026".to_string())),
                    (
                        "lønmodtager.kommune",
                        Data::String("København".to_string()),
                    ),
                    (
                        "lønmodtager.bruttoløn_kroner",
                        Data::String("600000".to_string()),
                    ),
                    (
                        "lønmodtager.ligningsfradrag.befordring.$variant",
                        Data::String("UdenBefordringsfradrag".to_string()),
                    ),
                    (
                        "lønmodtager.pensionsfradrag.pensionsalder_status",
                        Data::String("Ll9lMereEnd15ÅrFørFolkepension".to_string()),
                    ),
                    (
                        "lønmodtager.pensionsfradrag.pbl18_fradragsberettiget_indbetaling_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pensionsfradrag.pbl19_rate_ophørende_bortseelsesret_efter_am_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pensionsfradrag.pbl19_øvrige_indbetalinger_efter_am_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pensionsfradrag.pbl20_indkomstskattepligtig_udbetaling_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "lønmodtager.pensionsfradrag.pbl20_udbetaling_status",
                        Data::String("Ll9lIngenPbl20Udbetaling".to_string()),
                    ),
                    (
                        "lønmodtager.personfradrag_alder_status",
                        Data::String("Fyldt18EllerGift".to_string()),
                    ),
                    ("lønmodtager.betaler_kirkeskat", Data::Bool(false)),
                    (
                        "kapitalindkomst.renter.renteindtægter_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.renteudgifter_kroner",
                        Data::String("0".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.næringsstatus",
                        Data::String("IkkeNæring".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.ligningslov6.$variant",
                        Data::String("UdenLigningslov6Kurstab".to_string()),
                    ),
                    (
                        "kapitalindkomst.renter.ligningslov6a.$variant",
                        Data::String("UdenLigningslov6AFradrag".to_string()),
                    ),
                    (
                        "kapitalindkomst.ejendomsavance.$variant",
                        Data::String("UdenEjendomsavance".to_string()),
                    ),
                    (
                        "skatteforhold.$variant",
                        Data::String("StandardSkatteforhold".to_string()),
                    ),
                    (
                        "aktieavance.ordinært_aktieår.$variant",
                        Data::String("UdenOrdinærtAktieår".to_string()),
                    ),
                    (
                        "underskudsforhold.$variant",
                        Data::String("StandardUnderskudsforhold".to_string()),
                    ),
                    (
                        "årsopgørelse.$variant",
                        Data::String("UdenÅrsopgørelse".to_string()),
                    ),
                ] {
                    set_workbook_cell_by_header(sheets, "cases", row, header, value);
                }
        };

        fill_wage_case(sheets, 1, "personskat-årsopgørelse-2026");
        fill_wage_case(sheets, 2, "personskat-abl-personlig-2026");
        fill_wage_case(sheets, 3, "personskat-renter-befordring-2026");
        for (header, value) in [
            (
                "lønmodtager.ligningsfradrag.befordring.$variant",
                Data::String("MedBefordringsfradrag".to_string()),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.arbejdsdage",
                Data::Int(203),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.daglige_befordringskilometer",
                Data::Int(60),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.bopæl_i_yderkommune_eller_lille_ø",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.befordringsformål",
                Data::String("IndtægtsgivendeArbejdsplads".to_string()),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.modtaget_skattefri_befordringsgodtgørelse_for_strækning",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.ligningslov9d.$variant",
                Data::String("UdenLigningslov9D".to_string()),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.fradrag_udelukket_folketingshverv_m_v",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.fri_befordring_betalt_af_arbejdsgiver_for_hele_strækningen",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.storebælt_bil_motorcykel_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.storebælt_kollektiv_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.øresund_bil_motorcykel_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.øresund_kollektiv_passager",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.broer.dokumenteret_og_afholdt_af_skattepligtige",
                Data::Bool(false),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.særlig_transport.faktisk_dokumenteret_udgift_kroner",
                Data::Int(0),
            ),
            (
                "lønmodtager.ligningsfradrag.befordring.MedBefordringsfradrag.fakta.særlig_transport.geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("MedFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.MedFremførtEjendomstab.fra_indkomstår",
                Data::Int(2025),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.MedFremførtEjendomstab.tab_kroner",
                Data::Int(25_000),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(true),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        let own_property_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser",
        );
        let spouse_property_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_afståelser",
        );
        for (row, item_id, identification, acquisition, disposal) in [
            (
                1,
                "egen-fortjeneste-1",
                "egen-fortjeneste",
                1_000_000,
                1_200_000,
            ),
            (2, "eget-tab-1", "eget-tab", 500_000, 450_000),
        ] {
            for (header, value) in [
                (
                    "case_id",
                    Data::String("personskat-renter-befordring-2026".to_string()),
                ),
                ("item_id", Data::String(item_id.to_string())),
                ("position", Data::Int(row as i64)),
                ("identifikation", Data::String(identification.to_string())),
                (
                    "afståelse",
                    Data::String("EblAlmindeligAfståelse".to_string()),
                ),
                ("erhvervet_som_led_i_næring", Data::Bool(false)),
                ("kontant_anskaffelsessum_kroner", Data::Int(acquisition)),
                ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
                ("regulering_efter_par5_og_5a_kroner", Data::Int(0)),
                ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
                ("kontant_afståelsessum_kroner", Data::Int(disposal)),
                ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
                ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
                ("par11_stk2_genanbragt_erhvervsejendom", Data::Bool(false)),
                (
                    "ejendomsafgrænsning.$variant",
                    Data::String("EblPar6AlmindeligEjendom".to_string()),
                ),
                (
                    "anskaffelsesgrundlag.$variant",
                    Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &own_property_sheet, row, header, value);
            }
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            ("item_id", Data::String("ægtefælles-tab-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("ægtefælles-tab".to_string())),
            (
                "afståelse",
                Data::String("EblAlmindeligAfståelse".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(300_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("regulering_efter_par5_og_5a_kroner", Data::Int(0)),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(270_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            ("par11_stk2_genanbragt_erhvervsejendom", Data::Bool(false)),
            (
                "ejendomsafgrænsning.$variant",
                Data::String("EblPar6AlmindeligEjendom".to_string()),
            ),
            (
                "anskaffelsesgrundlag.$variant",
                Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &spouse_property_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.renter.renteindtægter_kroner",
                Data::Int(20_000),
            ),
            (
                "kapitalindkomst.renter.renteudgifter_kroner",
                Data::Int(5_000),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.$variant",
                Data::String("MedLigningslov6Kurstab".to_string()),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.indkomstår",
                Data::Int(2026),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kontantlån_optaget_i_realkreditinstitut_før_19_maj_1993",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.indfrielse_sker_ved_realkreditlån",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.nyt_lån_optaget_før_1_januar_1996",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.nyt_lån_mindst_samme_løbetid",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.transaktioner_inden_for_1_år",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.lånetilbud_før_indfrielse_hvis_indfrielse_før_optagelse",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kurstab_kroner",
                Data::Int(24_000),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.samlet_antal_terminer",
                Data::Int(12),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.forfaldne_terminer_i_indkomståret",
                Data::Int(1),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.ekstraordinær_indfrielse_af_nyt_lån",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.stk4_omlægning_undtager_stk3",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.stk3_nedsættelse_basispoint",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.debitorskifte_i_året",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.debitordage_for_skattepligtig",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.dage_i_overdragelsesår",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6.MedLigningslov6Kurstab.input.kurstab_medregnes_efter_kursgevinstloven",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.$variant",
                Data::String("MedLigningslov6AFradrag".to_string()),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.indkomstår",
                Data::Int(2026),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.skattepligtig_person",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.arbejderboliger_beløb_kroner",
                Data::Int(1_000),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.arbejderboliger_betalt",
                Data::Bool(true),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.statshusmandsbrug_jordrente_beløb_kroner",
                Data::Int(0),
            ),
            (
                "kapitalindkomst.renter.ligningslov6a.MedLigningslov6AFradrag.input.statshusmandsbrug_jordrente_betalt",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            ("item_id", Data::String("kapitalomkostning-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("bankgebyr".to_string())),
            ("beløb_kroner", Data::Int(2_000)),
        ] {
            set_workbook_cell_by_header(sheets, "kapitalindkomst_omkostninger", 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-abl-personlig-2026".to_string()),
            ),
            ("item_id", Data::String("abl-par17-1".to_string())),
            ("position", Data::Int(1)),
            ("indkomstår", Data::Int(2026)),
            ("aktiv", Data::String("AblNæringsaktiePar17".to_string())),
            ("afståelsessum_kroner", Data::Int(37_000)),
            ("anskaffelsessum_kroner", Data::Int(30_000)),
            (
                "par17_modprøve.næringsstatus",
                Data::String("AblPar17UdøverNæringVedKøbOgSalgAfAktier".to_string()),
            ),
            (
                "par17_modprøve.erhvervelsesstatus",
                Data::String("AblPar17ErhvervetSomLedINæringsvej".to_string()),
            ),
            (
                "koncernintern_konvertibel_eller_tegningsret",
                Data::Bool(false),
            ),
            ("andelsforening_stiftet_før_22_maj_1987", Data::Bool(false)),
            (
                "afståelse_sker_for_at_undgå_likvidationsbeskatning",
                Data::Bool(false),
            ),
            (
                "investeringsklassifikation.$variant",
                Data::String("AblIngenInvesteringsklassifikation".to_string()),
            ),
            ("årets_netto_med_kgl_par14_23_kroner", Data::Int(7_000)),
        ] {
            set_workbook_cell_by_header(sheets, "aktieavance_særlige_aktiver", 1, header, value);
        }
        set_workbook_cell_by_header(
            sheets,
            "cases",
            1,
            "årsopgørelse.$variant",
            Data::String("MedÅrsopgørelse".to_string()),
        );
        for (header, value) in [
            (
                "årsopgørelse.MedÅrsopgørelse.overført_restskat_mv_kroner",
                "1000",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.pensionsbeskatningsafgift_kroner",
                "500",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.a_skat_og_am_indeholdt_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.par68_indbetalt_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.b_skat_betalt_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.udbytteskat_modregningsberettiget_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.frivillig_indbetaling_par59_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.virksomhedsordning_beløb_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.personskattelov_par8a_stk5_beløb_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.afskrivningslov_acontoskat_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.am_lov_par6_beløb_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.seniornedslag_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.energiafgiftskompensation_kroner",
                "0",
            ),
            (
                "årsopgørelse.MedÅrsopgørelse.kreditter.tilbagebetalt_par55_kroner",
                "0",
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                "cases",
                1,
                header,
                Data::String(value.to_string()),
            );
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

    let json_input_path = temp_path("json");
    let json_template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--format",
        "json",
        "--output",
        json_input_path.to_str().expect("JSON input path"),
    ]);
    assert!(
        json_template.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&json_template.stderr)
    );
    let mut json_input: Value = serde_json::from_slice(
        &std::fs::read(&json_input_path).expect("read generated Personskat JSON template"),
    )
    .expect("Personskat JSON template");
    json_input["cases"][0]["case_id"] = Value::String("personskat-abl-personlig-2026".into());
    json_input["cases"][0]["input"] = serde_json::json!({
        "lønmodtager": {
            "skatteår": 2026,
            "kommune": { "$variant": "København" },
            "bruttoløn_kroner": 600_000,
            "ligningsfradrag": {
                "befordring": { "$variant": "UdenBefordringsfradrag" }
            },
            "pensionsfradrag": {
                "pensionsalder_status": {
                    "$variant": "Ll9lMereEnd15ÅrFørFolkepension"
                },
                "pbl18_fradragsberettiget_indbetaling_kroner": 0,
                "pbl19_rate_ophørende_bortseelsesret_efter_am_kroner": 0,
                "pbl19_øvrige_indbetalinger_efter_am_kroner": 0,
                "pbl20_indkomstskattepligtig_udbetaling_kroner": 0,
                "pbl20_udbetaling_status": {
                    "$variant": "Ll9lIngenPbl20Udbetaling"
                }
            },
            "personfradrag_alder_status": { "$variant": "Fyldt18EllerGift" },
            "betaler_kirkeskat": false
        },
        "kapitalindkomst": {
            "renter": {
                "renteindtægter_kroner": 0,
                "renteudgifter_kroner": 0,
                "næringsstatus": { "$variant": "IkkeNæring" },
                "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
                "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
            },
            "ejendomsavance": { "$variant": "UdenEjendomsavance" },
            "omkostninger": []
        },
        "aktieavance": {
            "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
            "særlige_aktiver": [{
                "indkomstår": 2026,
                "aktiv": { "$variant": "AblNæringsaktiePar17" },
                "afståelsessum_kroner": 37_000,
                "anskaffelsessum_kroner": 30_000,
                "par17_modprøve": {
                    "næringsstatus": {
                        "$variant": "AblPar17UdøverNæringVedKøbOgSalgAfAktier"
                    },
                    "erhvervelsesstatus": {
                        "$variant": "AblPar17ErhvervetSomLedINæringsvej"
                    }
                },
                "koncernintern_konvertibel_eller_tegningsret": false,
                "andelsforening_stiftet_før_22_maj_1987": false,
                "afståelse_sker_for_at_undgå_likvidationsbeskatning": false,
                "investeringsklassifikation": {
                    "$variant": "AblIngenInvesteringsklassifikation"
                },
                "årets_netto_med_kgl_par14_23_kroner": 7_000
            }]
        },
        "skatteforhold": { "$variant": "StandardSkatteforhold" },
        "underskudsforhold": { "$variant": "StandardUnderskudsforhold" },
        "årsopgørelse": { "$variant": "UdenÅrsopgørelse" }
    });
    let mut interest_case = json_input["cases"][0].clone();
    interest_case["case_id"] = Value::String("personskat-renter-befordring-2026".into());
    interest_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 20_000,
            "renteudgifter_kroner": 5_000,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": {
                "$variant": "MedLigningslov6Kurstab",
                "input": {
                    "indkomstår": 2026,
                    "kontantlån_optaget_i_realkreditinstitut_før_19_maj_1993": true,
                    "indfrielse_sker_ved_realkreditlån": true,
                    "nyt_lån_optaget_før_1_januar_1996": true,
                    "nyt_lån_mindst_samme_løbetid": true,
                    "transaktioner_inden_for_1_år": true,
                    "lånetilbud_før_indfrielse_hvis_indfrielse_før_optagelse": true,
                    "kurstab_kroner": 24_000,
                    "samlet_antal_terminer": 12,
                    "forfaldne_terminer_i_indkomståret": 1,
                    "ekstraordinær_indfrielse_af_nyt_lån": false,
                    "stk4_omlægning_undtager_stk3": false,
                    "stk3_nedsættelse_basispoint": 0,
                    "debitorskifte_i_året": false,
                    "debitordage_for_skattepligtig": 0,
                    "dage_i_overdragelsesår": 0,
                    "kurstab_medregnes_efter_kursgevinstloven": false
                }
            },
            "ligningslov6a": {
                "$variant": "MedLigningslov6AFradrag",
                "input": {
                    "indkomstår": 2026,
                    "skattepligtig_person": true,
                    "arbejderboliger_beløb_kroner": 1_000,
                    "arbejderboliger_betalt": true,
                    "statshusmandsbrug_jordrente_beløb_kroner": 0,
                    "statshusmandsbrug_jordrente_betalt": false
                }
            }
        },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "egen-fortjeneste",
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 1_000_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "regulering_efter_par5_og_5a_kroner": 0,
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 1_200_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_genanbragt_erhvervsejendom": false,
                    "ejendomsafgrænsning": { "$variant": "EblPar6AlmindeligEjendom" },
                    "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" }
                }, {
                    "identifikation": "eget-tab",
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 500_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "regulering_efter_par5_og_5a_kroner": 0,
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 450_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_genanbragt_erhvervsejendom": false,
                    "ejendomsafgrænsning": { "$variant": "EblPar6AlmindeligEjendom" },
                    "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" }
                }],
                "eget_fremført_tab": {
                    "$variant": "MedFremførtEjendomstab",
                    "fra_indkomstår": 2025,
                    "tab_kroner": 25_000
                },
                "ægtefælles_afståelser": [{
                    "identifikation": "ægtefælles-tab",
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 300_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "regulering_efter_par5_og_5a_kroner": 0,
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 270_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_genanbragt_erhvervsejendom": false,
                    "ejendomsafgrænsning": { "$variant": "EblPar6AlmindeligEjendom" },
                    "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" }
                }],
                "ægtefælles_fremførte_tab": { "$variant": "UdenFremførtEjendomstab" },
                "gift_samlevende_ved_indkomstårets_udgang": true
            }
        },
        "omkostninger": [{
            "identifikation": "bankgebyr",
            "beløb_kroner": 2_000
        }]
    });
    interest_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": []
    });
    interest_case["input"]["lønmodtager"]["ligningsfradrag"] = serde_json::json!({
        "befordring": {
            "$variant": "MedBefordringsfradrag",
            "fakta": {
                "arbejdsdage": 203,
                "daglige_befordringskilometer": 60,
                "bopæl_i_yderkommune_eller_lille_ø": false,
                "befordringsformål": { "$variant": "IndtægtsgivendeArbejdsplads" },
                "modtaget_skattefri_befordringsgodtgørelse_for_strækning": false,
                "modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning": false,
                "ligningslov9d": { "$variant": "UdenLigningslov9D" },
                "fradrag_udelukket_folketingshverv_m_v": false,
                "fri_befordring_betalt_af_arbejdsgiver_for_hele_strækningen": false,
                "broer": {
                    "storebælt_bil_motorcykel_passager": 0,
                    "storebælt_kollektiv_passager": 0,
                    "øresund_bil_motorcykel_passager": 0,
                    "øresund_kollektiv_passager": 0,
                    "dokumenteret_og_afholdt_af_skattepligtige": false
                },
                "særlig_transport": {
                    "faktisk_dokumenteret_udgift_kroner": 0,
                    "geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten": false
                }
            }
        }
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(interest_case);
    std::fs::write(
        &json_input_path,
        serde_json::to_vec_pretty(&json_input).expect("encode Personskat JSON input"),
    )
    .expect("write Personskat JSON input");
    let json_output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--input",
        json_input_path.to_str().expect("JSON input path"),
    ]);
    std::fs::remove_file(&json_input_path).ok();
    assert!(
        json_output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&json_output.stderr),
        String::from_utf8_lossy(&json_output.stdout)
    );
    let json_result = parse_stdout(&json_output);
    assert_eq!(
        result["results"][1]["result"],
        json_result["results"][0]["result"]
    );
    assert_eq!(
        result["results"][2]["result"],
        json_result["results"][1]["result"]
    );

    assert_eq!(
        result["results"][0]["result"]["skat"]["samlet_inkl_am_efter_personfradrag_kroner"],
        208_726
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["$variant"],
        "BeregnetÅrsopgørelse"
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["resultat"]["slutskat_med_tillæg_kroner"],
        210_226
    );
    assert_eq!(
        result["results"][0]["result"]["årsopgørelse"]["resultat"]["restskat_kroner"],
        210_226
    );
    assert_eq!(
        result["results"][1]["result"]["aktieavance"]["personlig_indkomst_kroner"],
        7_000
    );
    assert_eq!(
        result["results"][1]["result"]["skat"]["øvrig_personlig_indkomst_kroner"],
        7_000
    );
    assert_eq!(
        result["results"][1]["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        48_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ligningslov6_resultat"]
            ["fradrag_kroner"],
        2_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ligningslov6a_resultat"]
            ["fradrag_kroner"],
        1_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        105_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]["$variant"],
        "BeregnetEjendomsavance"
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["eget_tabsårsresultat"]["gyldigt_fremført_tab_fra_tidligere_år_kroner"],
        25_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["ægtefælles_tabsårsresultat"]["tab_overført_til_ægtefælle_kroner"],
        30_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["par4_resultat"]["kapitalindkomst_kroner"],
        95_000
    );
    assert_eq!(
        result["results"][2]["result"]["skat"]["arbejdsmarkedsbidrag_kroner"],
        48_000
    );
    assert_eq!(
        result["results"][2]["result"]["ligningsfradrag"]["samlet_ligningsfradrag_kroner"],
        23_166
    );
    assert_eq!(
        result["results"][2]["result"]["ligningsfradrag"]["befordring"]["$variant"],
        "BeregnetBefordringsfradrag"
    );
    assert_eq!(
        result["results"][2]["result"]["ligningsfradrag"]["befordring"]["ligningslov9c_input"]
            ["aftrapningsindkomst_kroner"],
        600_000
    );
    assert_eq!(
        result["results"][2]["result"]["skat"]["øvrige_ligningsmæssige_fradrag_kroner"],
        23_166
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
fn par19_xlsx_derives_nested_fact_tables_and_round_trips_template() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/investeringsklassifikation.calculate.runa");
    let input_path = temp_path("xlsx");
    let template = run(&[
        "template",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab_efter_par19",
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
        let tables = workbook.worksheet_range("_tables").expect("table metadata");
        let table_paths = tables
            .rows()
            .skip(1)
            .filter_map(|row| row.first().map(ToString::to_string))
            .collect::<Vec<_>>();
        assert_eq!(table_paths.len(), 4);
        for expected_suffix in [
            ".deltagere_ved_indkomstårets_udgang",
            ".aktivmasse.direkte_aktiver",
            ".aktivmasse.ejerposter",
            ".underliggende_aktiver_efter_direkte_og_indirekte_gennemlysning",
        ] {
            assert!(
                table_paths
                    .iter()
                    .any(|path| path.ends_with(expected_suffix)),
                "missing derived § 19 fact table ending in {expected_suffix}: {table_paths:?}"
            );
        }
    }

    let output = run(&[
        "call",
        fixture.to_str().expect("fixture path"),
        "--entry",
        "klassificer_investeringsselskab_efter_par19",
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
        result["results"][0]["result"]["status"]["$variant"],
        "AblPar19IkkeInvesteringsselskab"
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

fn workbook_collection_sheet_name(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    path: &str,
) -> String {
    let rows: Vec<Vec<Data>> = workbook
        .worksheet_range("_tables")
        .expect("table metadata")
        .rows()
        .map(|row| row.to_vec())
        .collect();
    collection_sheet_name_from_metadata_rows(&rows, path)
}

fn workbook_collection_sheet_name_from_rows(
    sheets: &[(String, Vec<Vec<Data>>)],
    path: &str,
) -> String {
    let rows = &sheets
        .iter()
        .find(|(sheet, _)| sheet == "_tables")
        .expect("table metadata sheet")
        .1;
    collection_sheet_name_from_metadata_rows(rows, path)
}

fn collection_sheet_name_from_metadata_rows(rows: &[Vec<Data>], path: &str) -> String {
    let headers = rows.first().expect("table metadata headers");
    let path_column = headers
        .iter()
        .position(|cell| cell.to_string() == "path")
        .expect("table path column");
    let sheet_column = headers
        .iter()
        .position(|cell| cell.to_string() == "sheet")
        .expect("table sheet column");
    rows.iter()
        .skip(1)
        .find(|row| {
            row.get(path_column)
                .is_some_and(|cell| cell.to_string() == path)
        })
        .and_then(|row| row.get(sheet_column))
        .map(ToString::to_string)
        .unwrap_or_else(|| panic!("missing collection metadata for {path}"))
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

fn set_workbook_cell_by_header(
    sheets: &mut [(String, Vec<Vec<Data>>)],
    sheet: &str,
    row: usize,
    header: &str,
    value: Data,
) {
    let rows = workbook_sheet_mut(sheets, sheet);
    let column = rows
        .first()
        .expect("header row")
        .iter()
        .position(|cell| cell.to_string() == header)
        .unwrap_or_else(|| panic!("missing column {header} on sheet {sheet}"));
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
