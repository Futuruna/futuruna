//! Canonical public-boundary checks using only generated/synthetic input facts.
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

const MODEL: &str = "examples/danish-income-tax/personskat.calculate.runa";

fn run(args: &[&str]) -> Value {
    // Model-only iteration may reuse a verified binary from the same compiler
    // revision. CI/default runs always use Cargo's binary under test.
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .args(args)
        .output()
        .expect("run canonical calculation");
    assert!(
        output.status.success(),
        "runa {args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).expect("canonical JSON")
}

fn spouse(input: &Value) -> Value {
    let mut facts = serde_json::Map::new();
    for name in [
        "lønmodtager",
        "kapitalindkomst",
        "aktieavance",
        "udenlandske_sociale_bidrag",
        "cfc",
        "skatteforhold",
        "underskudsforhold",
        "ejendomsskatter",
    ] {
        facts.insert(name.into(), input[name].clone());
    }
    facts.get_mut("lønmodtager").unwrap()["bruttoløn_kroner"] = json!(0);
    json!({"$variant":"MedÆgtefælle", "fakta":facts, "samlevende_ved_indkomstårets_udløb":true, "kildeskat25a_fordelinger":[]})
}

fn commuting(days: i64) -> Value {
    json!({
        "identifikation":"synthetic-commute", "befordringsmål_identifikation":"synthetic-work",
        "arbejdsdage":days, "daglige_befordringskilometer":55,
        "bopæl_i_yderkommune_eller_lille_ø":false,
        "befordringsformål":{"$variant":"IndtægtsgivendeArbejdsplads"},
        "modtaget_skattefri_befordringsgodtgørelse_for_strækning":false,
        "modtaget_uddannelsesbefordringsrabat_eller_godtgørelse_for_strækning":false,
        "ligningslov9d":{"$variant":"UdenLigningslov9D"},
        "fradrag_udelukket_folketingshverv_m_v":false,
        "arbejdsgiverbetalt_befordring":{"$variant":"UdenArbejdsgiverbetaltBefordring"},
        "broer":{"storebælt_bil_motorcykel_passager":0,"storebælt_kollektiv_passager":0,"øresund_bil_motorcykel_passager":0,"øresund_kollektiv_passager":0,"dokumenteret_og_afholdt_af_skattepligtige":false},
        "særlig_transport":{"faktisk_dokumenteret_udgift_kroner":0,"geografiske_forhold_tidsforbrug_økonomisk_rimelighed_kræver_transporten":false}
    })
}

#[test]
fn canonical_results_gate_invalid_input_without_changing_valid_tax_amounts() {
    let mut template = run(&["template", MODEL, "--format", "json"]);
    let mut ordinary = template["cases"][0]["input"].clone();
    ordinary["lønmodtager"]["skatteår"] = json!(2025);
    ordinary["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    ordinary["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
    let mut cases = Vec::new();
    let mut expectations = Vec::new();
    let mut add =
        |name: &str, failure_path: Option<&str>, tax: Option<i64>, mutate: &dyn Fn(&mut Value)| {
            let mut input = ordinary.clone();
            mutate(&mut input);
            cases.push(json!({"case_id":name,"input":input}));
            expectations.push((name.to_string(), failure_path.map(str::to_string), tax));
        };
    for (year, tax) in [
        (2023, 21678330),
        (2024, 21556463),
        (2025, 21194454),
        (2026, 20872564),
    ] {
        add(&format!("ordinary-{year}"), None, Some(tax), &|v| {
            v["lønmodtager"]["skatteår"] = json!(year)
        });
    }
    for (name, date) in [
        ("zero-birth", json!({"år":0,"måned":0,"dag":0})),
        ("impossible-birth", json!({"år":1990,"måned":2,"dag":30})),
        ("future-birth", json!({"år":2030,"måned":1,"dag":1})),
    ] {
        add(
            name,
            Some("lønmodtager.pension.fødselsdato"),
            None,
            &|v| v["lønmodtager"]["pension"]["fødselsdato"] = date.clone(),
        );
    }
    add(
        "valid-commute-inactive-special-transport",
        None,
        None,
        &|v| v["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] = json!([commuting(220)]),
    );
    add(
        "negative-commute-days",
        Some("lønmodtager.ligningsfradrag"),
        None,
        &|v| v["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] = json!([commuting(-1)]),
    );
    add(
        "negative-interest-expense",
        Some("kapitalindkomst"),
        None,
        &|v| v["kapitalindkomst"]["renter"]["renteudgifter_kroner"] = json!(-1),
    );
    add("valid-spouse", None, None, &|v| v["ægtefælle"] = spouse(v));
    add(
        "invalid-spouse-birth",
        Some("ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.fødselsdato"),
        None,
        &|v| {
            v["ægtefælle"] = spouse(v);
            v["ægtefælle"]["fakta"]["lønmodtager"]["pension"]["fødselsdato"] =
                json!({"år":0,"måned":0,"dag":0});
        },
    );
    add(
        "different-spouse-year",
        Some("ægtefælle.MedÆgtefælle.fakta.lønmodtager.skatteår"),
        None,
        &|v| {
            v["ægtefælle"] = spouse(v);
            v["ægtefælle"]["fakta"]["lønmodtager"]["skatteår"] = json!(2024);
        },
    );
    template["cases"] = json!(cases);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "futuruna-personskat-validity-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expectations.len());
    for (row, (name, failure_path, tax)) in results.iter().zip(expectations) {
        assert_eq!(row["case_id"], name);
        let result = &row["result"];
        let gate = &result["vurdering"];
        let errors = gate["fejl"].as_array().expect("explicit domain errors");
        assert_eq!(gate["samlet_modeldækning_bekræftet"], false);
        assert!(!gate["forbehold"].as_array().unwrap().is_empty());
        if let Some(path) = failure_path {
            assert_eq!(
                gate["status"]["$variant"], "UgyldigtBeregningsgrundlag",
                "{name}: {gate}"
            );
            assert_eq!(gate["alle_kontroller_gyldige"], false);
            assert_eq!(gate["slutskat_til_sammenligning_øre"], Value::Null);
            assert!(
                errors.iter().any(|e| e["sti"] == path),
                "{name}: missing {path}: {errors:?}"
            );
        } else {
            assert_eq!(
                gate["status"]["$variant"], "BeregnetMedForbehold",
                "{name}: {gate}"
            );
            assert_eq!(gate["alle_kontroller_gyldige"], true);
            assert!(errors.is_empty(), "{name}: {errors:?}");
            assert_eq!(
                gate["slutskat_til_sammenligning_øre"],
                result["slutskat_øre"]
            );
        }
        if let Some(tax) = tax {
            assert_eq!(result["slutskat_øre"], tax, "{name}");
        }
        assert!(gate["kontroller"].as_array().unwrap().len() >= 26);
    }
}
