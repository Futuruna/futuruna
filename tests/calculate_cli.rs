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
    assert_eq!(schema["label"], "Household tax calculation");
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
    assert_eq!(schema["metadata"][0]["data"]["name"], "SourceInfo");
    assert!(schema["metadata"][0]["symbols"]
        .as_array()
        .expect("metadata symbols")
        .iter()
        .any(|symbol| symbol == "calculate_tax"));

    let field_metadata = schema["field_metadata"].as_array().expect("field metadata");
    assert_eq!(field_metadata.len(), 5);
    let monthly_income = field_metadata
        .iter()
        .find(|metadata| metadata["path"] == "monthly_income")
        .expect("monthly income metadata");
    assert_eq!(monthly_income["label"], "Monthly income");
    assert_eq!(
        monthly_income["question"],
        "What is your income before tax each month?"
    );
    assert_eq!(monthly_income["unit"], "currency/month");
    assert_eq!(monthly_income["sources"][0]["binding"], "tax_source");
    assert_eq!(
        monthly_income["sources"][0]["data"]["arguments"][0]["value"]["value"],
        "https://example.invalid/tax"
    );
    assert!(field_metadata
        .iter()
        .any(|metadata| metadata["path"] == "children.age"));
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
            ["case_id", "Monthly income", "Filing status", "Deduction"]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "children"),
            ["case_id", "item_id", "position", "Child name", "Child age"]
        );
        let metadata = workbook
            .worksheet_range("_futuruna")
            .expect("workbook metadata");
        let label = metadata
            .rows()
            .skip(1)
            .find(|row| row.first().map(ToString::to_string).as_deref() == Some("label"))
            .and_then(|row| row.get(1))
            .map(ToString::to_string);
        assert_eq!(label.as_deref(), Some("Household tax calculation"));
        let columns = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let monthly_income = columns
            .rows()
            .skip(1)
            .find(|row| row.get(9).map(ToString::to_string).as_deref() == Some("monthly_income"))
            .expect("monthly income column metadata");
        assert_eq!(
            monthly_income.get(2).map(ToString::to_string).as_deref(),
            Some("monthly_income")
        );
        assert_eq!(
            monthly_income.get(10).map(ToString::to_string).as_deref(),
            Some("Monthly income")
        );
        assert_eq!(
            monthly_income.get(11).map(ToString::to_string).as_deref(),
            Some("What is your income before tax each month?")
        );
        assert!(monthly_income
            .get(14)
            .map(ToString::to_string)
            .expect("source metadata")
            .contains("tax_source"));
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
        assert_eq!(case_headers.len(), 121);
        for expected in [
            "Skatteår",
            "Bopælskommune",
            "Årlig bruttoløn",
            "Befordringsfradrag",
            "Aldersstatus for personfradrag",
            "Kirkeskat",
            "Årets renteindtægter",
            "Årets renteudgifter",
            "Kursgevinster og kurstab",
            "Personen, som beregningen vedrører",
            "Årsopgørelse",
            "Ordinært aktieår",
        ] {
            assert!(
                case_headers.iter().any(|header| header == expected),
                "missing human Personskatteloven input label {expected}"
            );
        }
        let column_metadata = workbook
            .worksheet_range("_columns")
            .expect("column metadata");
        let metadata_headers = column_metadata.rows().next().expect("metadata headers");
        let input_path_column = metadata_headers
            .iter()
            .position(|cell| cell.to_string() == "input_path")
            .expect("input_path metadata column");
        let canonical_input_paths = column_metadata
            .rows()
            .skip(1)
            .filter_map(|row| row.get(input_path_column))
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for expected in [
            "aktieavance.ordinært_aktieår.$variant",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.$variant",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.lejlighed_har_tjent_til_bolig_mens_skattefrihedsbetingelser_var_opfyldt_i_ejertiden",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.grundforhold.$variant",
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb.hændelser.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.afståelsesform.$variant",
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
            "kapitalindkomst.kursgevinst.$variant",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.skatteyder_identifikation",
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrigt_netto_fordringer_valutagæld_og_obligationsbaserede_investeringsbeviser_kroner",
            "skatteforhold.$variant",
            "skatteforhold.SærligeSkatteforhold.forhold.øvrig_aktieindkomst_kroner",
            "underskudsforhold.$variant",
            "underskudsforhold.SærligeUnderskudsforhold.forhold.ægtefælle_skattepligtig_indkomst_kroner",
            "årsopgørelse.$variant",
            "årsopgørelse.MedÅrsopgørelse.kreditter.a_skat_og_am_indeholdt_kroner",
        ] {
            assert!(
                canonical_input_paths.iter().any(|path| path == expected),
                "missing typed Personskatteloven input column {expected}"
            );
        }
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path == "lønmodtager.ligningsmæssige_fradrag_kroner"));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("aftrapningsindkomst_kroner")));
        assert!(!canonical_input_paths
            .iter()
            .any(|path| path.contains("ligningslov9d_resultat")));
        assert_eq!(
            workbook_headers(&mut workbook, "kapitalindkomst_omkostninger"),
            [
                "case_id",
                "item_id",
                "position",
                "Identifikation",
                "Beløb (DKK)"
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
            let property_paths = workbook_column_paths(&mut workbook, sheet);
            for expected in [
                "identifikation",
                "afståelsesdato.år",
                "afståelsesdato.måned",
                "afståelsesdato.dag",
                "afståelse",
                "erhvervet_som_led_i_næring",
                "kontant_anskaffelsessum_kroner",
                "par5_fakta.anskaffelsesdato.år",
                "par5_fakta.anskaffelsesdato.måned",
                "par5_fakta.anskaffelsesdato.dag",
                "par5_fakta.anskaffelsesgrundlag.$variant",
                "par5_fakta.anskaffelsesgrundlag.EblPar4Stk3TredjePktAnskaffelsesgrundlag.tab_efter_ejendomsværdi_par4_stk3_nr1_eller_2_kroner",
                "par5_fakta.fordeling.ejerandel_promille",
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.afstået_del_anskaffelsessum_før_par5_kroner",
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.hele_ejendommens_anskaffelsessum_før_par5_kroner",
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.ikke_boligdelens_anskaffelsessum_før_par5_kroner",
                "par5_fakta.stk6_overførsel.$variant",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.ejendomskategori",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.tillægsparcelværdi_kroner",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.teknisk_værdi_kroner",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.afstået_jord_anskaffelsessum_før_overførsel_kroner",
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.samlet_jord_anskaffelsessum_før_overførsel_kroner",
                "par5_fakta.reguleringsvalg.$variant",
                "par5_fakta.reguleringsvalg.EblPar5AMedIndeksering.kategori",
                "kontant_afståelsessum_kroner",
                "par11_stk2_valg.$variant",
                "par11_stk2_valg.EblPar11Stk2MedNytGenanbringelsesvalg.valg.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                "par11_stk2_valg.EblPar11Stk2MedNytGenanbringelsesvalg.valg.fakta.begæring.$variant",
                "par11_stk2_valg.EblPar11Stk2MedNytGenanbringelsesvalg.valg.hjemmel.$variant",
                "ejendomstype.$variant",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsanvendelse.EblPar6AUdlejetTilKontrolleretSelskab.bestemmende_indflydelse_består",
                "ejendomstype.EblBoligejendom.fakta.ejendomsart.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.placering.EblPar6AEjendomIUdlandet.begæringsforhold.$variant",
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.ejendommen_ændrede_anvendelse_til_par8_før_15_december_2005",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.kategori.$variant",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_anskaffelsessum_kroner",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.$variant",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.genanbringelse.$variant",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.boligandel_forøget",
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.EblPar9GenanbringelseEfterStk4.boligandel_forøget_før_15_december_2005",
                "par6d_valg.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.valgt_udskudt_fortjeneste_kroner",
                "par6d_valg.EblMedPar6DValg.fakta.årligt_beløb_kroner",
                "par6d_valg.EblMedPar6DValg.fakta.fordelingsår",
                "par6d_valg.EblMedPar6DValg.fakta.erhververen_erhvervede_ejendommen_som_led_i_næring",
                "par6d_valg.EblMedPar6DValg.fakta.overdragerens_anvendelse.EblPar6DUdlejetTilKontrolleretSelskab.fakta.ejeren_er_fysisk_person",
                "par6d_valg.EblMedPar6DValg.fakta.erhververens_anvendelse_ved_overdragelsen.EblPar6DUdlejetTilKontrolleretSelskab.fakta.ejeren_er_fysisk_person",
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.identifikation",
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.ejendomsplacering.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.afståelsesårets_hændelse.$variant",
                "par6d_valg.EblMedPar6DValg.fakta.afståelsesårets_hændelse.EblPar6DPantebrevAfståetEllerIndfriet.afståelses_eller_indfrielsesprovenu_kroner",
            ] {
                assert!(
                    property_paths.iter().any(|path| path == expected),
                    "missing canonical property source-fact path {expected} on {sheet}"
                );
            }
            let property_headers = workbook_headers(&mut workbook, sheet);
            let mut unique_property_headers = property_headers.clone();
            unique_property_headers.sort();
            unique_property_headers.dedup();
            assert_eq!(
                unique_property_headers.len(),
                property_headers.len(),
                "duplicate human property headers on {sheet}"
            );
            for expected in [
                "Ejendom",
                "Afståelsesår",
                "Anskaffelsesår",
                "Kontant anskaffelsessum",
                "Kontant afståelsessum",
                "Anskaffelsesgrundlag",
                "Ejerandel",
                "Hele eller en del af ejendommen",
                "Anskaffelsessum for den afståede del",
                "Anskaffelsessum for hele ejendommen",
                "Anskaffelsessum uden boligdel",
                "Overførsel efter EBL § 5, stk. 6",
                "Ejendomskategori for § 5, stk. 6",
                "Tillægsparcelværdi",
                "Teknisk værdi",
                "Ejendomskategori for § 5 A",
                "Indeksering efter § 5 A",
                "Ejendomstype",
                "Ny genanbringelse efter ekspropriation",
                "Erhvervsmæssigt grundlag for ny genanbringelse",
                "Begæring om ny genanbringelse",
                "Lovgrundlag for den nye genanbringelse",
                "Tidligere genanbringelse på ejendommen",
                "Bestemmende indflydelse består",
                "Boligejendommens art",
                "Genanbringelse ved boligejendom",
                "Lovgrundlag for genanbringelse (§ 8)",
                "Oprindelig erhvervsfortjeneste",
                "Geninvesteringsår",
                "Udenlandsk geninvestering i tilladt område",
                "Begæringsforløb for udenlandsk geninvestering",
                "Oplysninger og driftsbudget ved fraflytning",
                "Genanbringelse ved blandet ejendom",
                "Lovgrundlag for genanbringelse (§ 9)",
                "Boligandel forøget efter genanbringelsen",
                "Fordeling af ejendomsfortjeneste",
                "Fortjeneste fordelt til senere år",
                "Årligt beskattet fortjeneste",
                "Antal fordelingsår",
                "Køberens erhvervelse af ejendommen",
                "Sælgerens brug af ejendommen",
                "Køberens brug af ejendommen",
                "Udlejer er en fysisk person (sælger)",
                "Udlejer er en fysisk person (køber)",
                "Sælgerpantebrev",
                "Sælgerpantebrevets kontantværdi",
                "Meddelelse om fordelingsvalget",
                "Ejendommens beliggenhed",
                "Hændelse i afståelsesåret",
                "Provenu ved afståelse eller indfrielse",
            ] {
                assert!(
                    property_headers.iter().any(|header| header == expected),
                    "missing human property input label {expected} on {sheet}"
                );
            }
        }
        for property_path in [
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser",
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_afståelser",
        ] {
            let expense_path =
                format!("{property_path}.par5_fakta.vedligeholdelses_og_forbedringsudgifter");
            let expense_sheet = workbook_collection_sheet_name(&mut workbook, &expense_path);
            let expense_paths = workbook_column_paths(&mut workbook, &expense_sheet);
            for expected in [
                "afholdelsesdato.år",
                "afholdelsesdato.måned",
                "afholdelsesdato.dag",
                "fuldførelsesår",
                "beløb_for_afstået_del_kroner",
                "status",
            ] {
                assert!(
                    expense_paths.iter().any(|path| path == expected),
                    "missing canonical § 5 expense path {expected} on {expense_sheet}"
                );
            }

            let reduction_path = format!("{property_path}.par5_fakta.nedsættelser");
            let reduction_sheet = workbook_collection_sheet_name(&mut workbook, &reduction_path);
            let reduction_paths = workbook_column_paths(&mut workbook, &reduction_sheet);
            for expected in ["indkomstår", "beløb_for_afstået_del_kroner", "grund"] {
                assert!(
                    reduction_paths.iter().any(|path| path == expected),
                    "missing canonical § 5 reduction path {expected} on {reduction_sheet}"
                );
            }

            let milk_quota_path = format!("{property_path}.par5_fakta.mælkekvoter");
            let milk_quota_sheet = workbook_collection_sheet_name(&mut workbook, &milk_quota_path);
            let milk_quota_paths = workbook_column_paths(&mut workbook, &milk_quota_sheet);
            for expected in [
                "identifikation",
                "anskaffelsesdato.år",
                "anskaffelsesdato.måned",
                "anskaffelsesdato.dag",
                "oprindelige_enheder",
                "disponerede_enheder",
                "anskaffelsesgrundlag.$variant",
                "anskaffelsesgrundlag.EblPar5MælkekvoteKøbt.vederlag_kroner",
                "anskaffelsesgrundlag.EblPar5MælkekvoteVederlagsfritTildelt.beskattet_værdi_ved_tildeling_kroner",
                "disposition.$variant",
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.år",
                "disposition.EblPar5MælkekvoteAfstået.vederlag_kroner",
                "disposition.EblPar5MælkekvoteUdløbet.udløbsdato.år",
                "disposition.EblPar5MælkekvoteToldetUdenErstatning.toldningsdato.år",
            ] {
                assert!(
                    milk_quota_paths.iter().any(|path| path == expected),
                    "missing canonical milk-quota path {expected} on {milk_quota_sheet}"
                );
            }
            let milk_quota_headers = workbook_headers(&mut workbook, &milk_quota_sheet);
            for expected in [
                "Mælkekvote",
                "Anskaffelsesår",
                "Oprindelige kvoteenheder",
                "Disponerede kvoteenheder",
                "Køb eller vederlagsfri tildeling",
                "Købsvederlag",
                "Disposition over mælkekvoten",
                "Afståelsesår",
                "Afståelsesvederlag",
                "Udløbsår",
                "Toldningsår",
            ] {
                assert!(
                    milk_quota_headers.iter().any(|header| header == expected),
                    "missing human milk-quota label {expected} on {milk_quota_sheet}"
                );
            }

            let par6d_schedule_path = format!(
                "{property_path}.par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.afdragsplan"
            );
            let par6d_schedule_sheet =
                workbook_collection_sheet_name(&mut workbook, &par6d_schedule_path);
            assert_eq!(
                workbook_headers(&mut workbook, &par6d_schedule_sheet),
                [
                    "case_id",
                    "parent_id",
                    "item_id",
                    "position",
                    "År efter afståelsen",
                    "Forfaldende hovedstol"
                ]
            );

            let par6d_years_path = format!(
                "{property_path}.par6d_valg.EblMedPar6DValg.fakta.efterfølgende_årsforhold"
            );
            let par6d_years_sheet =
                workbook_collection_sheet_name(&mut workbook, &par6d_years_path);
            let par6d_year_paths = workbook_column_paths(&mut workbook, &par6d_years_sheet);
            for expected in [
                "indkomstår",
                "betalt_ordinært_hovedstolsafdrag_kroner",
                "hændelse.$variant",
                "hændelse.EblPar6DPantebrevAfståetEllerIndfriet.afståelses_eller_indfrielsesprovenu_kroner",
            ] {
                assert!(
                    par6d_year_paths.iter().any(|path| path == expected),
                    "missing canonical EBL § 6 D annual path {expected} on {par6d_years_sheet}"
                );
            }
            let par6d_year_headers = workbook_headers(&mut workbook, &par6d_years_sheet);
            for expected in [
                "Indkomstår",
                "Faktisk ordinært hovedstolsafdrag",
                "Hændelse i indkomståret",
                "Provenu ved afståelse eller indfrielse",
            ] {
                assert!(
                    par6d_year_headers.iter().any(|header| header == expected),
                    "missing human EBL § 6 D annual label {expected} on {par6d_years_sheet}"
                );
            }
        }
        let kgl_seller_note_path =
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.sælgerpantebreve";
        let kgl_seller_note_sheet =
            workbook_collection_sheet_name(&mut workbook, kgl_seller_note_path);
        let kgl_seller_note_paths = workbook_column_paths(&mut workbook, &kgl_seller_note_sheet);
        for expected in [
            "sælgerpantebrev_identifikation",
            "oprindelig_skatteyder_identifikation",
            "skatteyderfakta.udøver_næring_ved_køb_og_salg_af_fordringer",
            "skatteyderfakta.fordringen_erhvervet_uden_for_fordringsnæring",
            "skatteyderfakta.fordringen_erhvervet_som_vederlag_for_leverede_varer_eller_tjenesteydelser",
            "skatteyderfakta.fordringen_erhvervet_i_direkte_tilknytning_til_erhvervsmæssig_drift",
            "skatteyderfakta.debitor_omfattet_af_tabsbegrænsningen_i_kgl_par14_stk2",
            "skatteyderfakta.renter_eller_gevinster_fritaget_efter_dobbeltbeskatningsoverenskomst",
        ] {
            assert!(
                kgl_seller_note_paths.iter().any(|path| path == expected),
                "missing canonical KGL seller-note path {expected} on {kgl_seller_note_sheet}"
            );
        }
        let kgl_seller_note_headers = workbook_headers(&mut workbook, &kgl_seller_note_sheet);
        for expected in [
            "Sælgerpantebrev",
            "Sælgerpantebrevets oprindelige skatteyder",
            "Næring med fordringer",
            "Fordring uden for næring",
            "Vederlag for varer eller ydelser",
            "Direkte tilknytning til virksomheden",
            "Kontrolleret eller nærtstående debitor",
            "Fritagelse efter skatteaftale",
        ] {
            assert!(
                kgl_seller_note_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human KGL seller-note label {expected} on {kgl_seller_note_sheet}"
            );
        }
        let kgl_disposition_path =
            format!("{kgl_seller_note_path}.dispositioner_efter_ebl_forløbet");
        let kgl_disposition_sheet =
            workbook_collection_sheet_name(&mut workbook, &kgl_disposition_path);
        let kgl_disposition_paths = workbook_column_paths(&mut workbook, &kgl_disposition_sheet);
        for expected in [
            "indkomstår",
            "art.$variant",
            "berørt_hovedstol_kroner",
            "afståelses_eller_indfrielsessum_kroner",
        ] {
            assert!(
                kgl_disposition_paths.iter().any(|path| path == expected),
                "missing canonical post-EBL KGL disposition path {expected} on {kgl_disposition_sheet}"
            );
        }
        let kgl_disposition_headers = workbook_headers(&mut workbook, &kgl_disposition_sheet);
        for expected in [
            "Dispositionens indkomstår",
            "Disposition med restfordringen",
            "Berørt hovedstol",
            "Modtaget beløb",
        ] {
            assert!(
                kgl_disposition_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human post-EBL KGL disposition label {expected} on {kgl_disposition_sheet}"
            );
        }
        let special_asset_paths =
            workbook_column_paths(&mut workbook, "aktieavance_særlige_aktiver");
        for expected in [
            "aktiv",
            "par17_modprøve.næringsstatus",
            "par17_modprøve.erhvervelsesstatus",
            "investeringsklassifikation.$variant",
        ] {
            assert!(
                special_asset_paths.iter().any(|path| path == expected),
                "missing canonical source-level ABL input path {expected}"
            );
        }
        let ordinary_holdings_path =
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb";
        let ordinary_events_path = format!("{ordinary_holdings_path}.hændelser");
        let ordinary_events_sheet =
            workbook_collection_sheet_name(&mut workbook, &ordinary_events_path);
        let ordinary_event_headers = workbook_headers(&mut workbook, &ordinary_events_sheet);
        for expected in [
            "Boligret efter ABL § 15",
            "Værdipapir med boligret",
            "Udstederens ejendom med flere boliger",
            "Boligbrug i kvalificerende periode",
            "Grundareal knyttet til lejligheden",
            "Grundbetingelse efter EBL § 8",
            "Afståelsesform for boligret",
            "Udloddet i endeligt opløsningsår",
        ] {
            assert!(
                ordinary_event_headers
                    .iter()
                    .any(|header| header == expected),
                "missing human ABL § 15 input label {expected} on {ordinary_events_sheet}"
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
                        "kapitalindkomst.kursgevinst.$variant",
                        Data::String("UdenKursgevinst".to_string()),
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
        fill_wage_case(sheets, 4, "personskat-ebl5-kildefakta-2026");
        fill_wage_case(sheets, 5, "personskat-ebl6d-historisk-2026");
        fill_wage_case(sheets, 6, "personskat-ebl11-genanbringelse-2026");
        for (header, value) in [
            (
                "aktieavance.ordinært_aktieår.$variant",
                Data::String("MedOrdinærtAktieår".to_string()),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.indkomstår",
                Data::Int(2026),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.fremført_tab_efter_par13a_kroner",
                Data::Int(0),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.modregningsgrundlag.udbytter_for_aktier_med_gevinst_efter_par12_kroner",
                Data::Int(0),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.modregningsgrundlag.udbytter_og_nettogevinster_for_aktier_med_gevinst_efter_par19b_kroner",
                Data::Int(0),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.modregningsgrundlag.udbytter_for_aktier_omfattet_af_par44_kroner",
                Data::Int(0),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.modregningsgrundlag.ægtefælles_positive_nettobeløb_efter_par13a_kroner",
                Data::Int(0),
            ),
            (
                "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.gift_og_samlevende_ved_årets_udgang",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 3, header, value);
        }
        let ordinary_holdings_path =
            "aktieavance.ordinært_aktieår.MedOrdinærtAktieår.input.hændelsesforløb";
        let ordinary_events_path = format!("{ordinary_holdings_path}.hændelser");
        let ordinary_holdings_sheet =
            workbook_collection_sheet_name_from_rows(sheets, ordinary_holdings_path);
        let ordinary_events_sheet =
            workbook_collection_sheet_name_from_rows(sheets, &ordinary_events_path);
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("boligret-beholdning-1".to_string()),
            ),
            ("position".to_string(), Data::Int(1)),
            (
                format!("{ordinary_holdings_path}.position_primo.selskabsidentifikation"),
                Data::String("DK-BOLIGRET-1".to_string()),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.kapitalmængde.$variant"),
                Data::String("AblAktiekapitalUdenPålydendeVærdi".to_string()),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.kapitalmængde.AblAktiekapitalUdenPålydendeVærdi.antal_aktier"),
                Data::Int(10),
            ),
            (
                format!("{ordinary_holdings_path}.position_primo.anskaffelsessum_kroner"),
                Data::Int(10_000),
            ),
        ] {
            set_workbook_cell_by_header(
                sheets,
                &ordinary_holdings_sheet,
                1,
                &header,
                value,
            );
        }
        for (header, value) in [
            (
                "case_id".to_string(),
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            (
                "parent_id".to_string(),
                Data::String("boligret-beholdning-1".to_string()),
            ),
            (
                "item_id".to_string(),
                Data::String("boligret-afståelse-1".to_string()),
            ),
            ("position".to_string(), Data::Int(1)),
            (
                format!("{ordinary_events_path}.$variant"),
                Data::String("AblOrdinærAfståelse".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.kapitalmængde.$variant"),
                Data::String("AblAktiekapitalUdenPålydendeVærdi".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.kapitalmængde.AblAktiekapitalUdenPålydendeVærdi.antal_aktier"),
                Data::Int(10),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.afståelsessum_kroner"),
                Data::Int(15_000),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.markedsstatus"),
                Data::String("AblIkkeOptagetTilHandel".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.har_tidligere_været_optaget_til_handel"),
                Data::Bool(false),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.hovedaktionæraktier"),
                Data::Bool(false),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.afståede_aktiers_handelsværdi_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.beholdte_aktiers_handelsværdi_kroner"),
                Data::Int(0),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.oplysningsstatus"),
                Data::String("AblOplystRettidigt".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.$variant"),
                Data::String("AblBoligretEfterPar15".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.værdipapir_forbundet_med_brugsret_til_beboelseslejlighed"),
                Data::Bool(true),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.udsteder_ejer_direkte_ejendom_med_flere_beboelseslejligheder"),
                Data::Bool(true),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.lejlighed_har_tjent_til_bolig_mens_skattefrihedsbetingelser_var_opfyldt_i_ejertiden"),
                Data::Bool(true),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.fakta.grundforhold.$variant"),
                Data::String("EblPar8Stk4UdenBestemtGrundareal".to_string()),
            ),
            (
                format!("{ordinary_events_path}.AblOrdinærAfståelse.vilkår.boligret.AblBoligretEfterPar15.afståelsesform.$variant"),
                Data::String("AblPar15AlmindeligAfståelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &ordinary_events_sheet, 1, &header, value);
        }
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
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 4, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(false),
            ),
            (
                "kapitalindkomst.kursgevinst.$variant",
                Data::String("MedKursgevinst".to_string()),
            ),
            (
                "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.skatteyder_identifikation",
                Data::String("Sælger".to_string()),
            ),
            (
                "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.øvrigt_netto_fordringer_valutagæld_og_obligationsbaserede_investeringsbeviser_kroner",
                Data::Int(0),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 5, header, value);
        }
        for (header, value) in [
            (
                "kapitalindkomst.ejendomsavance.$variant",
                Data::String("MedEjendomsavance".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.eget_fremført_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.ægtefælles_fremførte_tab.$variant",
                Data::String("UdenFremførtEjendomstab".to_string()),
            ),
            (
                "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.gift_samlevende_ved_indkomstårets_udgang",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, "cases", 6, header, value);
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
                ("afståelsesdato.år", Data::Int(2026)),
                ("afståelsesdato.måned", Data::Int(12)),
                ("afståelsesdato.dag", Data::Int(31)),
                (
                    "afståelse",
                    Data::String("EblAlmindeligAfståelse".to_string()),
                ),
                ("erhvervet_som_led_i_næring", Data::Bool(false)),
                ("kontant_anskaffelsessum_kroner", Data::Int(acquisition)),
                ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
                ("par5_fakta.anskaffelsesdato.år", Data::Int(2026)),
                ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
                ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
                (
                    "par5_fakta.anskaffelsesgrundlag.$variant",
                    Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
                ),
                ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
                (
                    "par5_fakta.fordeling.afståelsesomfang.$variant",
                    Data::String("EblPar5HeleEjendommen".to_string()),
                ),
                (
                    "par5_fakta.stk6_overførsel.$variant",
                    Data::String("EblPar5UdenStk6Overførsel".to_string()),
                ),
                (
                    "par5_fakta.reguleringsvalg.$variant",
                    Data::String("EblPar5UdenIndeksering".to_string()),
                ),
                ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
                ("kontant_afståelsessum_kroner", Data::Int(disposal)),
                ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
                ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
                (
                    "par11_stk2_valg.$variant",
                    Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
                ),
                (
                    "ejendomstype.$variant",
                    Data::String("EblAndenFastEjendom".to_string()),
                ),
                (
                    "par6d_valg.$variant",
                    Data::String("EblUdenPar6DValg".to_string()),
                ),
            ] {
                set_workbook_cell_by_header(sheets, &own_property_sheet, row, header, value);
            }
        }
        set_workbook_cell_by_header(
            sheets,
            &own_property_sheet,
            2,
            "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
            Data::String("EblUdenAktivGenanbringelse".to_string()),
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl5-kildefakta-2026".to_string()),
            ),
            ("item_id", Data::String("ebl5-landbrug-1".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("landbrug-med-mælk-og-stk6".to_string()),
            ),
            ("afståelsesdato.år", Data::Int(2026)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblAlmindeligAfståelse".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(200_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(1985)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4Stk3Nr1Anskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(500)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5DelAfEjendommen".to_string()),
            ),
            (
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.afstået_del_anskaffelsessum_før_par5_kroner",
                Data::Int(200_000),
            ),
            (
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.hele_ejendommens_anskaffelsessum_før_par5_kroner",
                Data::Int(800_000),
            ),
            (
                "par5_fakta.fordeling.afståelsesomfang.EblPar5DelAfEjendommen.ikke_boligdelens_anskaffelsessum_før_par5_kroner",
                Data::Int(800_000),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5MedStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.ejendomskategori",
                Data::String("EblPar5Stk6Landbrugsejendom".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.tillægsparcelværdi_kroner",
                Data::Int(500_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.teknisk_værdi_kroner",
                Data::Int(600_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.afskrivninger_på_vurderingsomfattede_bygninger_kroner",
                Data::Int(100_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.stk4_nr2_afskrivninger_på_vurderingsomfattede_nedrevne_bygninger_kroner",
                Data::Int(50_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.nedskrevet_værdi_af_vurderingsomfattede_bygninger_kroner",
                Data::Int(150_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.afstået_jord_anskaffelsessum_før_overførsel_kroner",
                Data::Int(200_000),
            ),
            (
                "par5_fakta.stk6_overførsel.EblPar5MedStk6Overførsel.fakta.samlet_jord_anskaffelsessum_før_overførsel_kroner",
                Data::Int(800_000),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(500_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblLandbrugSkovNaturEllerBlandetEjendom".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.kategori.$variant",
                Data::String("EblPar9Landbrugsejendom".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_har_tjent_til_bolig_for_ejer_eller_husstand",
                Data::Bool(false),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.boligbetingelser_opfyldt_i_ejerperioden",
                Data::Bool(false),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.grundbetingelse",
                Data::String("EblPar8GrundbetingelseIkkeOpfyldt".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_anskaffelsessum_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.bolig_afståelsessum_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.skadeforhold.$variant",
                Data::String("EblPar9IngenVæsentligSkade".to_string()),
            ),
            (
                "ejendomstype.EblLandbrugSkovNaturEllerBlandetEjendom.fakta.genanbringelsesforhold.$variant",
                Data::String("EblPar9IngenGenanbringelseEfterStk4".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblUdenPar6DValg".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 3, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl6d-historisk-2026".to_string()),
            ),
            ("item_id", Data::String("ebl6d-salg-2025".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("erhvervsejendom-solgt-2025".to_string()),
            ),
            ("afståelsesdato.år", Data::Int(2025)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblAlmindeligAfståelse".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(25_000_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(2025)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5HeleEjendommen".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5UdenStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(30_000_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblAndenFastEjendom".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                Data::String("EblUdenAktivGenanbringelse".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblMedPar6DValg".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.valgt_udskudt_fortjeneste_kroner",
                Data::Int(3_000_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.årligt_beløb_kroner",
                Data::Int(300_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.fordelingsår",
                Data::Int(10),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.erhververen_erhvervede_ejendommen_som_led_i_næring",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.overdragerens_kapitalandel_i_erhververen_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.overdragerens_stemmeandel_i_erhververen_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.erhververens_kapitalandel_i_overdrageren_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.erhververens_stemmeandel_i_overdrageren_promille",
                Data::Int(0),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.parterne_er_nærtstående_efter_ligningslovens_par2_stk2",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.parterne_har_aftale_om_fælles_bestemmende_indflydelse",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.partrelation.parterne_er_under_fælles_bestemmende_indflydelse",
                Data::Bool(false),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.overdragerens_anvendelse.$variant",
                Data::String(
                    "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.erhververens_anvendelse_ved_overdragelsen.$variant",
                Data::String(
                    "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.identifikation",
                Data::String("sælgerpantebrev-2025".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.fordring_mod_erhververen",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.pant_i_den_overdragne_ejendom",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.kontantværdi_kroner",
                Data::Int(3_000_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.hovedstol_kroner",
                Data::Int(3_750_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.løbetid_år",
                Data::Int(10),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.$variant",
                Data::String("EblPar6DRettidigMeddelelse".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.kopi_af_sælgerpantebrev_vedlagt",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_skattepligtig_fortjeneste_efter_par6_kroner",
                Data::Int(4_990_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_valgt_udskudt_fortjeneste_kroner",
                Data::Int(3_000_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_årligt_beløb_kroner",
                Data::Int(300_000),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.meddelelse.EblPar6DRettidigMeddelelse.indhold.oplyst_antal_år",
                Data::Int(10),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgers_skatteforhold.skattepligtsstatus",
                Data::String("EblPar6DFuldtSkattepligtigTilDanmark".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgers_skatteforhold.hjemland_omfattet_af_nordisk_overenskomst_eller_eu_inddrivelsesdirektiv",
                Data::Bool(true),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.sælgers_skatteforhold.sikkerhed.$variant",
                Data::String("EblPar6DIngenSikkerhed".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.ejendomsplacering.$variant",
                Data::String("EblPar6DEjendomIDanmark".to_string()),
            ),
            (
                "par6d_valg.EblMedPar6DValg.fakta.afståelsesårets_hændelse.$variant",
                Data::String("EblPar6DIngenFremrykningshændelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 4, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl11-genanbringelse-2026".to_string()),
            ),
            ("item_id", Data::String("ebl11-salg-2026".to_string())),
            ("position", Data::Int(1)),
            (
                "identifikation",
                Data::String("eksproprieret-genanbringelsesejendom".to_string()),
            ),
            ("afståelsesdato.år", Data::Int(2026)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblEkspropriationserstatning".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(1_000_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(2026)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5HeleEjendommen".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5UdenStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(1_500_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblAndenFastEjendom".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                Data::String("EblMedGenanbringelseEfterPar6A".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                Data::Int(2025),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                Data::Int(200_000),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.tillæg_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.nedslag_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.afstået_ejendoms_erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.indkomstår",
                Data::Int(2026),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                Data::Int(1_000_000),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.ejendomsstatus",
                Data::String("EblPar6AEjendomOmfattetAfLovenIkkePar8".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.placering.$variant",
                Data::String("EblPar6AEjendomIDanmark".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.begæring.$variant",
                Data::String(
                    "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                        .to_string(),
                ),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.ejerskab.$variant",
                Data::String("EblPar6ASammeSkattepligtige".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblUdenPar6DValg".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 5, header, value);
        }
        let own_par6d_schedule_path = "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser.par6d_valg.EblMedPar6DValg.fakta.sælgerpantebrev.afdragsplan";
        let own_par6d_schedule_sheet =
            workbook_collection_sheet_name_from_rows(sheets, own_par6d_schedule_path);
        for year in 1_i64..=10 {
            for (header, value) in [
                (
                    "case_id",
                    Data::String("personskat-ebl6d-historisk-2026".to_string()),
                ),
                ("parent_id", Data::String("ebl6d-salg-2025".to_string())),
                ("item_id", Data::String(format!("ebl6d-afdrag-{year}"))),
                ("position", Data::Int(year)),
                ("år_efter_afståelsen", Data::Int(year)),
                ("hovedstol_forfalder_kroner", Data::Int(375_000)),
            ] {
                set_workbook_cell_by_header(
                    sheets,
                    &own_par6d_schedule_sheet,
                    year as usize,
                    header,
                    value,
                );
            }
        }
        let own_par6d_years_path = "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser.par6d_valg.EblMedPar6DValg.fakta.efterfølgende_årsforhold";
        let own_par6d_years_sheet =
            workbook_collection_sheet_name_from_rows(sheets, own_par6d_years_path);
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl6d-historisk-2026".to_string()),
            ),
            ("parent_id", Data::String("ebl6d-salg-2025".to_string())),
            ("item_id", Data::String("ebl6d-år-2026".to_string())),
            ("position", Data::Int(1)),
            ("indkomstår", Data::Int(2026)),
            (
                "betalt_ordinært_hovedstolsafdrag_kroner",
                Data::Int(375_000),
            ),
            (
                "hændelse.$variant",
                Data::String("EblPar6DIngenFremrykningshændelse".to_string()),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_par6d_years_sheet, 1, header, value);
        }
        let kgl_seller_note_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.kursgevinst.MedKursgevinst.fakta.sælgerpantebreve",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl6d-historisk-2026".to_string()),
            ),
            (
                "item_id",
                Data::String("kgl-sælgerpantebrev-2025".to_string()),
            ),
            ("position", Data::Int(1)),
            (
                "sælgerpantebrev_identifikation",
                Data::String("sælgerpantebrev-2025".to_string()),
            ),
            (
                "oprindelig_skatteyder_identifikation",
                Data::String("Sælger".to_string()),
            ),
            (
                "skatteyderfakta.udøver_næring_ved_køb_og_salg_af_fordringer",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.fordringen_erhvervet_uden_for_fordringsnæring",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.fordringen_erhvervet_som_vederlag_for_leverede_varer_eller_tjenesteydelser",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.fordringen_erhvervet_i_direkte_tilknytning_til_erhvervsmæssig_drift",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.debitor_omfattet_af_tabsbegrænsningen_i_kgl_par14_stk2",
                Data::Bool(false),
            ),
            (
                "skatteyderfakta.renter_eller_gevinster_fritaget_efter_dobbeltbeskatningsoverenskomst",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &kgl_seller_note_sheet, 1, header, value);
        }
        let own_milk_quota_sheet = workbook_collection_sheet_name_from_rows(
            sheets,
            "kapitalindkomst.ejendomsavance.MedEjendomsavance.fakta.egne_afståelser.par5_fakta.mælkekvoter",
        );
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-ebl5-kildefakta-2026".to_string()),
            ),
            ("parent_id", Data::String("ebl5-landbrug-1".to_string())),
            ("item_id", Data::String("mælkekvote-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("mælk-2000".to_string())),
            ("anskaffelsesdato.år", Data::Int(2000)),
            ("anskaffelsesdato.måned", Data::Int(1)),
            ("anskaffelsesdato.dag", Data::Int(1)),
            ("oprindelige_enheder", Data::Int(100)),
            ("disponerede_enheder", Data::Int(100)),
            (
                "anskaffelsesgrundlag.$variant",
                Data::String("EblPar5MælkekvoteKøbt".to_string()),
            ),
            (
                "anskaffelsesgrundlag.EblPar5MælkekvoteKøbt.vederlag_kroner",
                Data::Int(80_000),
            ),
            (
                "disposition.$variant",
                Data::String("EblPar5MælkekvoteAfstået".to_string()),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.år",
                Data::Int(2010),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.måned",
                Data::Int(6),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.afståelsesdato.dag",
                Data::Int(1),
            ),
            (
                "disposition.EblPar5MælkekvoteAfstået.vederlag_kroner",
                Data::Int(120_000),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_milk_quota_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "ejendomstype.$variant",
                Data::String("EblBoligejendom".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.ejendomsart.$variant",
                Data::String("EblPar8Ejerlejlighed".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.tjent_til_bolig_eller_privat_formål",
                Data::Bool(true),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.betingelser_opfyldt_i_ejerperioden",
                Data::Bool(true),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.afståelsesforhold.$variant",
                Data::String("EblPar8AlmindeligAfståelse".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.$variant",
                Data::String("EblPar8GenanbringelseEfterStk5".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.$variant",
                Data::String("EblMedGenanbringelseEfterPar6A".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.afståelsesindkomstår",
                Data::Int(2025),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.erhvervsfortjeneste_før_par6_stk2_kroner",
                Data::Int(190_000),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.tillæg_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.oprindelig_fortjeneste.regulering.nedslag_for_genanbragt_del_kroner",
                Data::Int(0),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.afstået_ejendoms_erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.indkomstår",
                Data::Int(2026),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsmæssigt_anskaffelsesgrundlag_kroner",
                Data::Int(1_000_000),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.ejendomsstatus",
                Data::String("EblPar6AEjendomOmfattetAfLovenIkkePar8".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.erhvervsanvendelse.$variant",
                Data::String(
                    "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed".to_string(),
                ),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.investering.placering.$variant",
                Data::String("EblPar6AEjendomIDanmark".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.begæring.$variant",
                Data::String(
                    "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                        .to_string(),
                ),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.genanbringelse.EblMedGenanbringelseEfterPar6A.fakta.ejerskab.$variant",
                Data::String("EblPar6ASammeSkattepligtige".to_string()),
            ),
            (
                "ejendomstype.EblBoligejendom.fakta.genanbringelsesforhold.EblPar8GenanbringelseEfterStk5.ejendommen_ændrede_anvendelse_til_par8_før_15_december_2005",
                Data::Bool(false),
            ),
        ] {
            set_workbook_cell_by_header(sheets, &own_property_sheet, 1, header, value);
        }
        for (header, value) in [
            (
                "case_id",
                Data::String("personskat-renter-befordring-2026".to_string()),
            ),
            ("item_id", Data::String("ægtefælles-tab-1".to_string())),
            ("position", Data::Int(1)),
            ("identifikation", Data::String("ægtefælles-tab".to_string())),
            ("afståelsesdato.år", Data::Int(2026)),
            ("afståelsesdato.måned", Data::Int(12)),
            ("afståelsesdato.dag", Data::Int(31)),
            (
                "afståelse",
                Data::String("EblAlmindeligAfståelse".to_string()),
            ),
            ("erhvervet_som_led_i_næring", Data::Bool(false)),
            ("kontant_anskaffelsessum_kroner", Data::Int(300_000)),
            ("gæld_kursværdi_ved_anskaffelse_kroner", Data::Int(0)),
            ("par5_fakta.anskaffelsesdato.år", Data::Int(2026)),
            ("par5_fakta.anskaffelsesdato.måned", Data::Int(1)),
            ("par5_fakta.anskaffelsesdato.dag", Data::Int(1)),
            (
                "par5_fakta.anskaffelsesgrundlag.$variant",
                Data::String("EblPar4AlmindeligtAnskaffelsesgrundlag".to_string()),
            ),
            ("par5_fakta.fordeling.ejerandel_promille", Data::Int(1000)),
            (
                "par5_fakta.fordeling.afståelsesomfang.$variant",
                Data::String("EblPar5HeleEjendommen".to_string()),
            ),
            (
                "par5_fakta.stk6_overførsel.$variant",
                Data::String("EblPar5UdenStk6Overførsel".to_string()),
            ),
            (
                "par5_fakta.reguleringsvalg.$variant",
                Data::String("EblPar5UdenIndeksering".to_string()),
            ),
            ("par4_stk8_anskaffelse_udeladt_kroner", Data::Int(0)),
            ("kontant_afståelsessum_kroner", Data::Int(270_000)),
            ("overdragne_gældsposter_kursværdi_kroner", Data::Int(0)),
            ("par4_stk8_afståelsesværdi_udeladt_kroner", Data::Int(0)),
            (
                "par11_stk2_valg.$variant",
                Data::String("EblPar11Stk2IngenNyGenanbringelse".to_string()),
            ),
            (
                "ejendomstype.$variant",
                Data::String("EblAndenFastEjendom".to_string()),
            ),
            (
                "ejendomstype.EblAndenFastEjendom.genanbringelse.$variant",
                Data::String("EblUdenAktivGenanbringelse".to_string()),
            ),
            (
                "par6d_valg.$variant",
                Data::String("EblUdenPar6DValg".to_string()),
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
            "kursgevinst": { "$variant": "UdenKursgevinst" },
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
    let reinvested_home = serde_json::json!({
        "$variant": "EblBoligejendom",
        "fakta": {
            "ejendomsart": { "$variant": "EblPar8Ejerlejlighed" },
            "tjent_til_bolig_eller_privat_formål": true,
            "betingelser_opfyldt_i_ejerperioden": true,
            "afståelsesforhold": { "$variant": "EblPar8AlmindeligAfståelse" },
            "genanbringelsesforhold": {
                "$variant": "EblPar8GenanbringelseEfterStk5",
                "genanbringelse": {
                    "$variant": "EblMedGenanbringelseEfterPar6A",
                    "fakta": {
                        "oprindelig_fortjeneste": {
                            "afståelsesindkomstår": 2025,
                            "erhvervsfortjeneste_før_par6_stk2_kroner": 190_000,
                            "regulering": {
                                "tillæg_for_genanbragt_del_kroner": 0,
                                "nedslag_for_genanbragt_del_kroner": 0
                            }
                        },
                        "afstået_ejendoms_erhvervsanvendelse": {
                            "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                        },
                        "investering": {
                            "indkomstår": 2026,
                            "erhvervsmæssigt_anskaffelsesgrundlag_kroner": 1_000_000,
                            "ejendomsstatus": {
                                "$variant": "EblPar6AEjendomOmfattetAfLovenIkkePar8"
                            },
                            "erhvervsanvendelse": {
                                "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                            },
                            "placering": {
                                "$variant": "EblPar6AEjendomIDanmark"
                            }
                        },
                        "begæring": {
                            "$variant": "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                        },
                        "ejerskab": {
                            "$variant": "EblPar6ASammeSkattepligtige"
                        }
                    }
                },
                "ejendommen_ændrede_anvendelse_til_par8_før_15_december_2005": false
            }
        }
    });
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
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 1_000_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 1_200_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": reinvested_home
                }, {
                    "identifikation": "eget-tab",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 500_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 450_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": { "$variant": "EblUdenAktivGenanbringelse" }
                    }
                }],
                "eget_fremført_tab": {
                    "$variant": "MedFremførtEjendomstab",
                    "fra_indkomstår": 2025,
                    "tab_kroner": 25_000
                },
                "ægtefælles_afståelser": [{
                    "identifikation": "ægtefælles-tab",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 300_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": { "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag" },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 270_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": { "$variant": "EblUdenAktivGenanbringelse" }
                    }
                }],
                "ægtefælles_fremførte_tab": { "$variant": "UdenFremførtEjendomstab" },
                "gift_samlevende_ved_indkomstårets_udgang": true
            }
        },
        "kursgevinst": { "$variant": "UdenKursgevinst" },
        "omkostninger": [{
            "identifikation": "bankgebyr",
            "beløb_kroner": 2_000
        }]
    });
    interest_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": {
            "$variant": "MedOrdinærtAktieår",
            "input": {
                "indkomstår": 2026,
                "hændelsesforløb": [{
                    "position_primo": {
                        "selskabsidentifikation": "DK-BOLIGRET-1",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 10
                        },
                        "anskaffelsessum_kroner": 10_000
                    },
                    "hændelser": [{
                        "$variant": "AblOrdinærAfståelse",
                        "kapitalmængde": {
                            "$variant": "AblAktiekapitalUdenPålydendeVærdi",
                            "antal_aktier": 10
                        },
                        "afståelsessum_kroner": 15_000,
                        "vilkår": {
                            "markedsstatus": { "$variant": "AblIkkeOptagetTilHandel" },
                            "har_tidligere_været_optaget_til_handel": false,
                            "hovedaktionæraktier": false,
                            "afståede_aktiers_handelsværdi_kroner": 0,
                            "beholdte_aktiers_handelsværdi_kroner": 0,
                            "oplysningsstatus": { "$variant": "AblOplystRettidigt" },
                            "boligret": {
                                "$variant": "AblBoligretEfterPar15",
                                "fakta": {
                                    "værdipapir_forbundet_med_brugsret_til_beboelseslejlighed": true,
                                    "udsteder_ejer_direkte_ejendom_med_flere_beboelseslejligheder": true,
                                    "lejlighed_har_tjent_til_bolig_mens_skattefrihedsbetingelser_var_opfyldt_i_ejertiden": true,
                                    "grundforhold": {
                                        "$variant": "EblPar8Stk4UdenBestemtGrundareal"
                                    }
                                },
                                "afståelsesform": {
                                    "$variant": "AblPar15AlmindeligAfståelse"
                                }
                            }
                        }
                    }]
                }],
                "investeringsbeviser": [],
                "fremført_tab_efter_par13a_kroner": 0,
                "modregningsgrundlag": {
                    "udbytter_for_aktier_med_gevinst_efter_par12_kroner": 0,
                    "udbytter_og_nettogevinster_for_aktier_med_gevinst_efter_par19b_kroner": 0,
                    "udbytter_for_aktier_omfattet_af_par44_kroner": 0,
                    "ægtefælles_positive_nettobeløb_efter_par13a_kroner": 0
                },
                "gift_og_samlevende_ved_årets_udgang": false
            }
        },
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
    let mut ebl5_case = json_input["cases"][0].clone();
    ebl5_case["case_id"] = Value::String("personskat-ebl5-kildefakta-2026".into());
    ebl5_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 0,
            "renteudgifter_kroner": 0,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
            "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
        },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "landbrug-med-mælk-og-stk6",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 200_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 1985, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": {
                            "$variant": "EblPar4Stk3Nr1Anskaffelsesgrundlag"
                        },
                        "fordeling": {
                            "ejerandel_promille": 500,
                            "afståelsesomfang": {
                                "$variant": "EblPar5DelAfEjendommen",
                                "afstået_del_anskaffelsessum_før_par5_kroner": 200_000,
                                "hele_ejendommens_anskaffelsessum_før_par5_kroner": 800_000,
                                "ikke_boligdelens_anskaffelsessum_før_par5_kroner": 800_000
                            }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [{
                            "identifikation": "mælk-2000",
                            "anskaffelsesdato": { "år": 2000, "måned": 1, "dag": 1 },
                            "oprindelige_enheder": 100,
                            "disponerede_enheder": 100,
                            "anskaffelsesgrundlag": {
                                "$variant": "EblPar5MælkekvoteKøbt",
                                "vederlag_kroner": 80_000
                            },
                            "disposition": {
                                "$variant": "EblPar5MælkekvoteAfstået",
                                "afståelsesdato": { "år": 2010, "måned": 6, "dag": 1 },
                                "vederlag_kroner": 120_000
                            }
                        }],
                        "stk6_overførsel": {
                            "$variant": "EblPar5MedStk6Overførsel",
                            "fakta": {
                                "ejendomskategori": {
                                    "$variant": "EblPar5Stk6Landbrugsejendom"
                                },
                                "tillægsparcelværdi_kroner": 500_000,
                                "teknisk_værdi_kroner": 600_000,
                                "afskrivninger_på_vurderingsomfattede_bygninger_kroner": 100_000,
                                "stk4_nr2_afskrivninger_på_vurderingsomfattede_nedrevne_bygninger_kroner": 50_000,
                                "nedskrevet_værdi_af_vurderingsomfattede_bygninger_kroner": 150_000,
                                "afstået_jord_anskaffelsessum_før_overførsel_kroner": 200_000,
                                "samlet_jord_anskaffelsessum_før_overførsel_kroner": 800_000
                            }
                        },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 500_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblLandbrugSkovNaturEllerBlandetEjendom",
                        "fakta": {
                            "kategori": { "$variant": "EblPar9Landbrugsejendom" },
                            "bolig_har_tjent_til_bolig_for_ejer_eller_husstand": false,
                            "boligbetingelser_opfyldt_i_ejerperioden": false,
                            "grundbetingelse": {
                                "$variant": "EblPar8GrundbetingelseIkkeOpfyldt"
                            },
                            "bolig_anskaffelsessum_kroner": 0,
                            "bolig_afståelsessum_kroner": 0,
                            "skadeforhold": {
                                "$variant": "EblPar9IngenVæsentligSkade"
                            },
                            "genanbringelsesforhold": {
                                "$variant": "EblPar9IngenGenanbringelseEfterStk4"
                            }
                        }
                    }
                }],
                "eget_fremført_tab": {
                    "$variant": "UdenFremførtEjendomstab"
                },
                "ægtefælles_afståelser": [],
                "ægtefælles_fremførte_tab": {
                    "$variant": "UdenFremførtEjendomstab"
                },
                "gift_samlevende_ved_indkomstårets_udgang": false
            }
        },
        "kursgevinst": { "$variant": "UdenKursgevinst" },
        "omkostninger": []
    });
    ebl5_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(ebl5_case);
    let par6d_schedule = (1..=10)
        .map(|year| {
            serde_json::json!({
                "år_efter_afståelsen": year,
                "hovedstol_forfalder_kroner": 375_000
            })
        })
        .collect::<Vec<_>>();
    let mut ebl6d_case = json_input["cases"][0].clone();
    ebl6d_case["case_id"] = Value::String("personskat-ebl6d-historisk-2026".into());
    let par6d_election = serde_json::json!({
        "$variant": "EblMedPar6DValg",
        "fakta": {
            "valgt_udskudt_fortjeneste_kroner": 3_000_000,
            "årligt_beløb_kroner": 300_000,
            "fordelingsår": 10,
            "erhververen_erhvervede_ejendommen_som_led_i_næring": false,
            "partrelation": {
                "overdragerens_kapitalandel_i_erhververen_promille": 0,
                "overdragerens_stemmeandel_i_erhververen_promille": 0,
                "erhververens_kapitalandel_i_overdrageren_promille": 0,
                "erhververens_stemmeandel_i_overdrageren_promille": 0,
                "parterne_er_nærtstående_efter_ligningslovens_par2_stk2": false,
                "parterne_har_aftale_om_fælles_bestemmende_indflydelse": false,
                "parterne_er_under_fælles_bestemmende_indflydelse": false
            },
            "overdragerens_anvendelse": {
                "$variant": "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
            },
            "erhververens_anvendelse_ved_overdragelsen": {
                "$variant": "EblPar6DEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
            },
            "sælgerpantebrev": {
                "identifikation": "sælgerpantebrev-2025",
                "fordring_mod_erhververen": true,
                "pant_i_den_overdragne_ejendom": true,
                "kontantværdi_kroner": 3_000_000,
                "hovedstol_kroner": 3_750_000,
                "løbetid_år": 10,
                "afdragsplan": par6d_schedule
            },
            "meddelelse": {
                "$variant": "EblPar6DRettidigMeddelelse",
                "indhold": {
                    "kopi_af_sælgerpantebrev_vedlagt": true,
                    "oplyst_skattepligtig_fortjeneste_efter_par6_kroner": 4_990_000,
                    "oplyst_valgt_udskudt_fortjeneste_kroner": 3_000_000,
                    "oplyst_årligt_beløb_kroner": 300_000,
                    "oplyst_antal_år": 10
                }
            },
            "sælgers_skatteforhold": {
                "skattepligtsstatus": {
                    "$variant": "EblPar6DFuldtSkattepligtigTilDanmark"
                },
                "hjemland_omfattet_af_nordisk_overenskomst_eller_eu_inddrivelsesdirektiv": true,
                "sikkerhed": { "$variant": "EblPar6DIngenSikkerhed" }
            },
            "ejendomsplacering": {
                "$variant": "EblPar6DEjendomIDanmark"
            },
            "afståelsesårets_hændelse": {
                "$variant": "EblPar6DIngenFremrykningshændelse"
            },
            "efterfølgende_årsforhold": [{
                "indkomstår": 2026,
                "betalt_ordinært_hovedstolsafdrag_kroner": 375_000,
                "hændelse": {
                    "$variant": "EblPar6DIngenFremrykningshændelse"
                }
            }]
        }
    });
    ebl6d_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 0,
            "renteudgifter_kroner": 0,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
            "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
        },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "erhvervsejendom-solgt-2025",
                    "afståelsesdato": { "år": 2025, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblAlmindeligAfståelse" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 25_000_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2025, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": {
                            "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag"
                        },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 30_000_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": { "$variant": "EblPar11Stk2IngenNyGenanbringelse" },
                    "par6d_valg": par6d_election,
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": { "$variant": "EblUdenAktivGenanbringelse" }
                    }
                }],
                "eget_fremført_tab": { "$variant": "UdenFremførtEjendomstab" },
                "ægtefælles_afståelser": [],
                "ægtefælles_fremførte_tab": { "$variant": "UdenFremførtEjendomstab" },
                "gift_samlevende_ved_indkomstårets_udgang": false
            }
        },
        "kursgevinst": {
            "$variant": "MedKursgevinst",
            "fakta": {
                "skatteyder_identifikation": "Sælger",
                "sælgerpantebreve": [{
                    "sælgerpantebrev_identifikation": "sælgerpantebrev-2025",
                    "oprindelig_skatteyder_identifikation": "Sælger",
                    "skatteyderfakta": {
                        "udøver_næring_ved_køb_og_salg_af_fordringer": false,
                        "fordringen_erhvervet_uden_for_fordringsnæring": false,
                        "fordringen_erhvervet_som_vederlag_for_leverede_varer_eller_tjenesteydelser": false,
                        "fordringen_erhvervet_i_direkte_tilknytning_til_erhvervsmæssig_drift": false,
                        "debitor_omfattet_af_tabsbegrænsningen_i_kgl_par14_stk2": false,
                        "renter_eller_gevinster_fritaget_efter_dobbeltbeskatningsoverenskomst": false
                    },
                    "dispositioner_efter_ebl_forløbet": []
                }],
                "øvrigt_netto_fordringer_valutagæld_og_obligationsbaserede_investeringsbeviser_kroner": 0
            }
        },
        "omkostninger": []
    });
    ebl6d_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(ebl6d_case);
    let mut ebl11_case = json_input["cases"][0].clone();
    ebl11_case["case_id"] = Value::String("personskat-ebl11-genanbringelse-2026".into());
    ebl11_case["input"]["kapitalindkomst"] = serde_json::json!({
        "renter": {
            "renteindtægter_kroner": 0,
            "renteudgifter_kroner": 0,
            "næringsstatus": { "$variant": "IkkeNæring" },
            "ligningslov6": { "$variant": "UdenLigningslov6Kurstab" },
            "ligningslov6a": { "$variant": "UdenLigningslov6AFradrag" }
        },
        "ejendomsavance": {
            "$variant": "MedEjendomsavance",
            "fakta": {
                "egne_afståelser": [{
                    "identifikation": "eksproprieret-genanbringelsesejendom",
                    "afståelsesdato": { "år": 2026, "måned": 12, "dag": 31 },
                    "afståelse": { "$variant": "EblEkspropriationserstatning" },
                    "erhvervet_som_led_i_næring": false,
                    "kontant_anskaffelsessum_kroner": 1_000_000,
                    "gæld_kursværdi_ved_anskaffelse_kroner": 0,
                    "par5_fakta": {
                        "anskaffelsesdato": { "år": 2026, "måned": 1, "dag": 1 },
                        "anskaffelsesgrundlag": {
                            "$variant": "EblPar4AlmindeligtAnskaffelsesgrundlag"
                        },
                        "fordeling": {
                            "ejerandel_promille": 1000,
                            "afståelsesomfang": { "$variant": "EblPar5HeleEjendommen" }
                        },
                        "vedligeholdelses_og_forbedringsudgifter": [],
                        "nedsættelser": [],
                        "mælkekvoter": [],
                        "stk6_overførsel": { "$variant": "EblPar5UdenStk6Overførsel" },
                        "reguleringsvalg": { "$variant": "EblPar5UdenIndeksering" }
                    },
                    "par4_stk8_anskaffelse_udeladt_kroner": 0,
                    "kontant_afståelsessum_kroner": 1_500_000,
                    "overdragne_gældsposter_kursværdi_kroner": 0,
                    "par4_stk8_afståelsesværdi_udeladt_kroner": 0,
                    "par11_stk2_valg": {
                        "$variant": "EblPar11Stk2IngenNyGenanbringelse"
                    },
                    "par6d_valg": { "$variant": "EblUdenPar6DValg" },
                    "ejendomstype": {
                        "$variant": "EblAndenFastEjendom",
                        "genanbringelse": {
                            "$variant": "EblMedGenanbringelseEfterPar6A",
                            "fakta": {
                                "oprindelig_fortjeneste": {
                                    "afståelsesindkomstår": 2025,
                                    "erhvervsfortjeneste_før_par6_stk2_kroner": 200_000,
                                    "regulering": {
                                        "tillæg_for_genanbragt_del_kroner": 0,
                                        "nedslag_for_genanbragt_del_kroner": 0
                                    }
                                },
                                "afstået_ejendoms_erhvervsanvendelse": {
                                    "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                                },
                                "investering": {
                                    "indkomstår": 2026,
                                    "erhvervsmæssigt_anskaffelsesgrundlag_kroner": 1_000_000,
                                    "ejendomsstatus": {
                                        "$variant": "EblPar6AEjendomOmfattetAfLovenIkkePar8"
                                    },
                                    "erhvervsanvendelse": {
                                        "$variant": "EblPar6AEgenEllerSamlevendeÆgtefællesErhvervsvirksomhed"
                                    },
                                    "placering": { "$variant": "EblPar6AEjendomIDanmark" }
                                },
                                "begæring": {
                                    "$variant": "EblPar6ABegæringVedRettidigAfgivelseEfterSkattekontrollovensPar2"
                                },
                                "ejerskab": { "$variant": "EblPar6ASammeSkattepligtige" }
                            }
                        }
                    }
                }],
                "eget_fremført_tab": { "$variant": "UdenFremførtEjendomstab" },
                "ægtefælles_afståelser": [],
                "ægtefælles_fremførte_tab": { "$variant": "UdenFremførtEjendomstab" },
                "gift_samlevende_ved_indkomstårets_udgang": false
            }
        },
        "kursgevinst": { "$variant": "UdenKursgevinst" },
        "omkostninger": []
    });
    ebl11_case["input"]["aktieavance"] = serde_json::json!({
        "ordinært_aktieår": { "$variant": "UdenOrdinærtAktieår" },
        "særlige_aktiver": []
    });
    json_input["cases"]
        .as_array_mut()
        .expect("Personskat JSON cases")
        .push(ebl11_case);
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
        result["results"][3]["result"],
        json_result["results"][2]["result"]
    );
    assert_eq!(
        result["results"][4]["result"],
        json_result["results"][3]["result"]
    );
    assert_eq!(
        result["results"][5]["result"],
        json_result["results"][4]["result"]
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
        75_000
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["aktieindkomst_kroner"],
        0
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["skattefri_efter_par15"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["par15_resultat"]
            ["$variant"],
        "AblPar15Vurderet"
    );
    assert_eq!(
        result["results"][2]["result"]["aktieavance"]["ordinært_aktieår"]["resultat"]
            ["hændelsesforløbsresultater"][0]["hændelsesresultater"][0]["par15_resultat"]
            ["ebl_par8_stk4_resultat"]["betingelser_for_skattefrihed_opfyldt"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]["$variant"],
        "BeregnetEjendomsavance"
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["$variant"],
        "EblPar8Behandling"
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["resultat"]["stk5_anvendt"],
        true
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["resultat"]
            ["skattefri_fortjeneste_kroner"],
        190_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par8_par9_behandling"]["resultat"]
            ["genanbragt_fortjeneste_til_beskatning_kroner"],
        190_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_kroner"],
        190_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["eget_tabsårsresultat"]["gyldigt_fremført_tab_fra_tidligere_år_kroner"],
        25_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["ægtefælles_tabsårsresultat"]["tab_overført_til_ægtefælle_kroner"],
        40_000
    );
    assert_eq!(
        result["results"][2]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["par4_resultat"]["kapitalindkomst_kroner"],
        65_000
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
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]["fast_tillæg_efter_stk1_kroner"],
        41_250
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]
            ["mælkekvoteforhøjelse_efter_stk3_kroner"],
        20_000
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]
            ["nedsættelser_efter_stk4_til_8_kroner"],
        30_000
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par5_resultat"]
            ["stk6_overført_til_afstået_jord_kroner"],
        37_500
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["reguleret_anskaffelsessum_kroner"],
        268_750
    );
    assert_eq!(
        result["results"][3]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_kroner"],
        231_250
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_før_par6d_kroner"],
        4_990_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par6d_resultat"]["valgt_udskudt_fortjeneste_kroner"],
        3_000_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["eget_tabsårsresultat"]["egne_skattepligtige_fortjenester_kroner"],
        300_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        375_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_input_gyldigt"],
        true
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_resultat"]
            ["årets_samlede_netto_efter_par14_kroner"],
        75_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_resultat"]
            ["sælgerpantebrevsresultater"][0]["kursgevinstresultat"]["dispositionsresultater"][0]
            ["frigivet_anskaffelsessum_kroner"],
        300_000
    );
    assert_eq!(
        result["results"][4]["result"]["kapitalindkomst"]["kursgevinst_resultat"]
            ["kursgevinstlov_resultater"][0]["netto_efter_kursgevinstloven_kroner"],
        75_000
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par11_stk2_resultat"]["stk2_anvendt"],
        true
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["par11_stk2_resultat"]
            ["anskaffelsessumsnedslag_bortfalder_kroner"],
        200_000
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["ejendomsavance_resultat"]
            ["egne_afståelsesresultater"][0]["skattepligtig_fortjeneste_kroner"],
        200_000
    );
    assert_eq!(
        result["results"][5]["result"]["kapitalindkomst"]["kapitalindkomst_resultat"]
            ["nettokapitalindkomst_kroner"],
        200_000
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
        let case_paths = workbook_column_paths(&mut workbook, "cases");
        for expected in [
            "meddelelse.$variant",
            "meddelelse.AblPar19BOrdinærMeddelelse.virkningsår",
            "meddelelse.AblPar19BNyoprettetMeddelelse.oprettelsesdato.år",
            "oplysninger.$variant",
        ] {
            assert!(
                case_paths.iter().any(|path| path == expected),
                "missing canonical investment input path {expected}"
            );
        }
        let owner_paths = workbook_column_paths(&mut workbook, "aktivmasse_ejerposter");
        for expected in [
            "$variant",
            "AblEjerpostIPar19B.ejerandel.ejede_kapitalenheder",
            "AblEjerpostIPar21.klassifikationsinput.oplysninger.$variant",
        ] {
            assert!(
                owner_paths.iter().any(|path| path == expected),
                "missing canonical owner input path {expected}"
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
                "Selection / Variant",
                "Selection / Fixed / Amount",
                "Selection / Pair / 0",
                "Selection / Pair / 1",
                "Selection / Family / Label",
            ]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "history"),
            [
                "case_id",
                "item_id",
                "position",
                "Variant",
                "Fixed / Amount",
                "Pair / 0",
                "Pair / 1",
                "Family / Label",
            ]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "selection_Family_children"),
            ["case_id", "item_id", "position", "Name", "Age"]
        );
        assert_eq!(
            workbook_headers(&mut workbook, "history_Family_children"),
            ["case_id", "parent_id", "item_id", "position", "Name", "Age",]
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
fn calculate_accepts_one_human_label() {
    let path = temp_path("runa");
    std::fs::write(
        &path,
        "# Input(value: Int)\n@ calculate(\"Income from sailor activities\")\n| calculate(input: Input) -> input.value\n",
    )
    .expect("write labelled calculation source");
    let output = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = parse_stdout(&output);
    assert_eq!(schema["label"], "Income from sailor activities");

    std::fs::write(
        &path,
        "# Input(value: Int)\n@ calculate(\"Sailor income\")\n| calculate(input: Input) -> input.value\n",
    )
    .expect("rewrite labelled calculation source");
    let changed = run(&["schema", path.to_str().expect("source path")]);
    std::fs::remove_file(&path).ok();
    assert!(changed.status.success());
    let changed_schema = parse_stdout(&changed);
    assert_eq!(changed_schema["label"], "Sailor income");
    assert_ne!(schema["schema_hash"], changed_schema["schema_hash"]);
}

#[test]
fn calculate_rejects_invalid_human_labels() {
    for (annotation, expected) in [
        ("@ calculate(42)", "accepts one human-readable string label"),
        ("@ calculate(\"\")", "label must not be empty"),
        (
            "@ calculate(\"First\", \"Second\")",
            "accepts at most one human-readable string label",
        ),
    ] {
        let path = temp_path("runa");
        std::fs::write(
            &path,
            format!(
                "# Input(value: Int)\n{annotation}\n| calculate(input: Input) -> input.value\n"
            ),
        )
        .expect("write invalid labelled calculation source");
        let output = run(&["check", path.to_str().expect("source path")]);
        std::fs::remove_file(&path).ok();
        assert!(
            !output.status.success(),
            "annotation unexpectedly passed: {annotation}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "stderr for {annotation}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn calculation_field_metadata_rejects_unknown_paths_and_duplicates() {
    let unknown_path = temp_path("runa");
    std::fs::write(
        &unknown_path,
        "# Input(value: Int)\n\
# CalculationField(path: String, label: String, question: String, help: String, unit: String)\n\
= bad_field = CalculationField(path = \"missing\", label = \"Missing\", question = \"\", help = \"\", unit = \"\")\n\
--@label:calculate::field:bad_field--\n\
@ calculate\n\
| calculate(input: Input) -> input.value\n",
    )
    .expect("write unknown field metadata source");
    let output = run(&["check", unknown_path.to_str().expect("source path")]);
    std::fs::remove_file(&unknown_path).ok();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown input path `missing`"));

    let duplicate_path = temp_path("runa");
    std::fs::write(
        &duplicate_path,
        "# Input(value: Int)\n\
# CalculationField(path: String, label: String, question: String, help: String, unit: String)\n\
= first_field = CalculationField(path = \"value\", label = \"First\", question = \"\", help = \"\", unit = \"\")\n\
= second_field = CalculationField(path = \"Input.value\", label = \"Second\", question = \"\", help = \"\", unit = \"\")\n\
--@label:calculate::field:first_field::field:second_field--\n\
@ calculate\n\
| calculate(input: Input) -> input.value\n",
    )
    .expect("write duplicate field metadata source");
    let output = run(&["check", duplicate_path.to_str().expect("source path")]);
    std::fs::remove_file(&duplicate_path).ok();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("duplicate field metadata for `value`")
    );
}

#[test]
fn changing_field_metadata_makes_an_xlsx_template_stale() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    let source = |label: &str| {
        format!(
            "# Input(value: Int)\n\
# CalculationField(path: String, label: String, question: String, help: String, unit: String)\n\
= value_field = CalculationField(path = \"value\", label = {label:?}, question = \"What is the value?\", help = \"\", unit = \"\")\n\
--@label:calculate::field:value_field--\n\
@ calculate\n\
| calculate(input: Input) -> input.value\n"
        )
    };
    std::fs::write(&source_path, source("Original label")).expect("write calculation source");
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

    std::fs::write(&source_path, source("Updated label")).expect("update calculation source");
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
    assert!(result["diagnostics"][0]["message"]
        .as_str()
        .expect("diagnostic")
        .contains("stale calculation template"));
}

#[test]
fn changing_calculation_label_makes_an_xlsx_template_stale() {
    let source_path = temp_path("runa");
    let input_path = temp_path("xlsx");
    let source = |label: &str| {
        format!(
            "# Input(value: Int)\n@ calculate({label:?})\n| calculate(input: Input) -> input.value\n"
        )
    };
    std::fs::write(&source_path, source("Original calculation")).expect("write calculation source");
    let template = run(&[
        "template",
        source_path.to_str().expect("source path"),
        "--format",
        "xlsx",
        "--output",
        input_path.to_str().expect("input path"),
    ]);
    assert!(template.status.success());

    std::fs::write(&source_path, source("Updated calculation")).expect("update calculation source");
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
    assert!(result["diagnostics"][0]["message"]
        .as_str()
        .expect("diagnostic")
        .contains("stale calculation template"));
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
        ("schema", "futuruna.calculate.xlsx.input.v5"),
        ("contract_schema", "futuruna.calculate.v1"),
        ("schema_hash", schema_hash),
        ("entry", "calculate_tax"),
        ("encoding", "futuruna-canonical-json-v1"),
        ("label", "Household tax calculation"),
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

fn workbook_column_paths(
    workbook: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>,
    sheet: &str,
) -> Vec<String> {
    let metadata = workbook
        .worksheet_range("_columns")
        .expect("column metadata");
    let headers = metadata.rows().next().expect("column metadata headers");
    let sheet_column = headers
        .iter()
        .position(|cell| cell.to_string() == "sheet")
        .expect("sheet metadata column");
    let path_column = headers
        .iter()
        .position(|cell| cell.to_string() == "path")
        .expect("path metadata column");
    metadata
        .rows()
        .skip(1)
        .filter(|row| {
            row.get(sheet_column)
                .is_some_and(|cell| cell.to_string() == sheet)
        })
        .filter_map(|row| row.get(path_column))
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
    let display_header = calculation_workbook_display_header(sheets, sheet, header)
        .unwrap_or_else(|| header.to_string());
    let rows = workbook_sheet_mut(sheets, sheet);
    let column = rows
        .first()
        .expect("header row")
        .iter()
        .position(|cell| cell.to_string() == display_header)
        .unwrap_or_else(|| {
            panic!("missing column {header} (displayed as {display_header}) on sheet {sheet}")
        });
    while rows.len() <= row {
        rows.push(Vec::new());
    }
    if rows[row].len() <= column {
        rows[row].resize(column + 1, Data::Empty);
    }
    rows[row][column] = value;
}

fn calculation_workbook_display_header(
    sheets: &[(String, Vec<Vec<Data>>)],
    sheet: &str,
    path: &str,
) -> Option<String> {
    let rows = &sheets.iter().find(|(name, _)| name == "_columns")?.1;
    let headers = rows.first()?;
    let column = |name: &str| headers.iter().position(|cell| cell.to_string() == name);
    let sheet_column = column("sheet")?;
    let path_column = column("path")?;
    let input_path_column = column("input_path")?;
    let sheet_rows = rows
        .iter()
        .skip(1)
        .filter(|row| row.get(sheet_column).map(ToString::to_string).as_deref() == Some(sheet))
        .collect::<Vec<_>>();
    let field_column = sheet_rows.iter().position(|row| {
        row.get(path_column).map(ToString::to_string).as_deref() == Some(path)
            || row
                .get(input_path_column)
                .map(ToString::to_string)
                .as_deref()
                == Some(path)
    })?;
    let visible_headers = sheets.iter().find(|(name, _)| name == sheet)?.1.first()?;
    let technical_columns = visible_headers.len().checked_sub(sheet_rows.len())?;
    visible_headers
        .get(technical_columns + field_column)
        .map(ToString::to_string)
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
