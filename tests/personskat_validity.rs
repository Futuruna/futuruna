//! Canonical public-boundary checks using only generated/synthetic input facts.
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

const MODEL: &str = "examples/danish-income-tax/personskat.calculate.runa";
#[path = "support/boligjob.rs"]
mod boligjob;
#[path = "support/foreign_employment.rs"]
mod foreign_employment;

fn run(args: &[&str]) -> Value {
    // Model-only iteration may reuse a verified binary from the same compiler
    // revision. Mint pins its freshly built release binary; unpinned focused
    // Cargo runs use the debug binary under test.
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

fn single_parent(year: i64, quarters: &[i64]) -> Value {
    json!({"$variant":"OplystEkstraBørnetilskud", "oplysninger_for_året_komplette":true,
        "kvartaler": quarters.iter().map(|quarter| json!({"indkomstår":year,"kvartal":quarter,
            "berettiget":true,"modtaget":true,"kildereference":"synthetic-benefit-record"})).collect::<Vec<_>>()})
}

#[test]
fn union_fee_taxpayer_status_matches_the_individual_assessment() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut baseline = envelope["cases"][0]["input"].clone();
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    baseline["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
    baseline["lønmodtager"]["pension"]["atp"] = json!({"$variant":"IngenAtpIndbetalinger"});
    baseline["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    baseline["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] =
        json!({"$variant":"IntetEkstraBørnetilskud"});
    baseline["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"] =
        foreign_employment::no_exclusion();
    baseline["lønmodtager"]["ligningsfradrag"]["boligjob"] =
        json!({"$variant":"IngenBoligjobudgifter"});
    // Fictional documented fees, not a deduction copied from an assessment.
    let fees = |year, status: &str, active| {
        json!({
            "skatteyderstatus":{"$variant":status},
            "kontingenter": if active { vec![json!({
                "identifikation":"fictional-annual-fee", "forening_identifikation":"fictional-union",
                "indkomstår":year,
                "periode":{"fra_dato":{"år":year,"måned":1,"dag":1},
                    "til_dato":{"år":year,"måned":12,"dag":31}},
                "foreningsart":{"$variant":"Ll13Fagforening"},
                "betalt_kontingent_kroner":10000,
                "foreningens_opgjorte_andel_til_faglige_økonomiske_interesser_kroner":10000,
                "foreningens_hovedformål_er_erhvervsgruppens_økonomiske_interesser":true,
                "skatteyder_hører_til_erhvervsgruppen":true,
                "indberetningsstatus":{"$variant":"Ll13IndberettetEfterSkatteindberetningslov31"}
            })] } else { vec![] }
        })
    };
    let mut cases = Vec::new();
    for year in [2023, 2024, 2025, 2026] {
        let mut input = baseline.clone();
        input["lønmodtager"]["skatteår"] = json!(year);
        input["lønmodtager"]["ligningsfradrag"]["faglige_kontingenter"] =
            fees(year, "Ll13Lønmodtager", true);
        cases.push(json!({"case_id":format!("employee-{year}"),"input":input}));
    }
    for (id, status, active) in [
        ("company", "Ll13JuridiskPerson", true),
        ("inactive-company", "Ll13JuridiskPerson", false),
        ("self-employed", "Ll13SelvstændigtErhvervsdrivende", true),
        ("spouse-company", "Ll13JuridiskPerson", true),
    ] {
        let mut input = baseline.clone();
        input["lønmodtager"]["skatteår"] = json!(2026);
        if id == "spouse-company" {
            input["ægtefælle"] = spouse(&input);
            input["ægtefælle"]["fakta"]["lønmodtager"]["ligningsfradrag"]["faglige_kontingenter"] =
                fees(2026, status, active);
        } else {
            input["lønmodtager"]["ligningsfradrag"]["faglige_kontingenter"] =
                fees(2026, status, active);
        }
        cases.push(json!({"case_id":id,"input":input}));
    }
    envelope["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-union-person-status-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), 8);
    for row in results {
        let id = row["case_id"].as_str().unwrap();
        let r = &row["result"];
        let invalid = id == "company" || id == "spouse-company";
        let gate = &r["vurdering"];
        println!(
            "{id}: valid={}, fees={}, comparison={}",
            gate["alle_kontroller_gyldige"],
            r["ligningsfradrag"]["faglige_kontingenter_fradrag_anvendt_kroner"],
            gate["slutskat_til_sammenligning_øre"]
        );
        assert_eq!(gate["alle_kontroller_gyldige"], !invalid, "{id}: {gate}");
        assert_eq!(gate["samlet_modeldækning_bekræftet"], false);
        if invalid {
            assert_eq!(gate["slutskat_til_sammenligning_øre"], Value::Null);
            let prefix = if id == "spouse-company" {
                "ægtefælle.MedÆgtefælle.fakta."
            } else {
                ""
            };
            let expected_path = format!(
                "{prefix}lønmodtager.ligningsfradrag.faglige_kontingenter.skatteyderstatus"
            );
            assert!(
                gate["fejl"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["sti"] == expected_path),
                "{id}: {gate}"
            );
            let deductions = if id == "spouse-company" {
                &r["ægtefælle"]["grundlag"]["ligningsfradrag"]
            } else {
                &r["ligningsfradrag"]
            };
            assert_eq!(deductions["alle_input_gyldige"], false);
        } else {
            let expected = match id {
                "employee-2023" => 6000,
                "self-employed" => 10000,
                "inactive-company" => 0,
                _ => 7000,
            };
            assert_eq!(
                r["ligningsfradrag"]["faglige_kontingenter_fradrag_anvendt_kroner"], expected,
                "{id}"
            );
            assert!(gate["slutskat_til_sammenligning_øre"].is_number());
        }
    }
}

#[test]
fn unsupported_year_stops_before_tax_evaluation_with_actionable_diagnostic() {
    let mut template = run(&["template", MODEL, "--format", "json"]);
    template["cases"][0]["input"]["lønmodtager"]["skatteår"] = json!(2027);
    let path = std::env::temp_dir().join(format!(
        "futuruna-unsupported-year-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .args(["call", MODEL, "--input", path.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["results"], json!([]));
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    let message = diagnostics[0]["message"].as_str().unwrap();
    println!("unsupported year: {message}");
    for expected in [
        "lønmodtager.skatteår",
        "2027",
        "2023–2026",
        "Ingen skat beregnet",
        "skatteaar-parametre.runa",
    ] {
        assert!(message.contains(expected), "missing {expected}: {message}");
    }
}

#[test]
fn unsupported_year_batch_preserves_supported_totals_and_spouse_boundary() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut input = envelope["cases"][0]["input"].clone();
    input["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    input["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
    input["lønmodtager"]["pension"]["atp"] = json!({"$variant":"IngenAtpIndbetalinger"});
    input["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    input["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] =
        json!({"$variant":"IntetEkstraBørnetilskud"});
    input["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"] =
        foreign_employment::no_exclusion();
    input["lønmodtager"]["ligningsfradrag"]["boligjob"] =
        json!({"$variant":"IngenBoligjobudgifter"});
    let mut cases = Vec::new();
    for year in [2023, 2022, 2024, 2027, 2025, 0, 2026, i64::MIN] {
        let mut facts = input.clone();
        facts["lønmodtager"]["skatteår"] = json!(year);
        cases.push(json!({"case_id":format!("year-{year}"),"input":facts}));
    }
    for year in [2022, 2027] {
        let mut facts = input.clone();
        facts["lønmodtager"]["skatteår"] = json!(2026);
        facts["ægtefælle"] = spouse(&facts);
        facts["ægtefælle"]["fakta"]["lønmodtager"]["skatteår"] = json!(year);
        cases.push(json!({"case_id":format!("spouse-{year}"),"input":facts}));
    }
    envelope["cases"] = json!(cases);
    let path =
        std::env::temp_dir().join(format!("futuruna-year-batch-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .args(["call", MODEL, "--input", path.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), 4);
    for (row, (year, tax)) in results.iter().zip([
        (2023, 21678330),
        (2024, 21556463),
        (2025, 21194454),
        (2026, 20872564),
    ]) {
        assert_eq!(row["case_id"], format!("year-{year}"));
        assert_eq!(
            row["result"]["vurdering"]["slutskat_til_sammenligning_øre"],
            tax
        );
        println!("supported year {year}: tax {tax} øre unchanged");
    }
    let diagnostics = output["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 6);
    for (row, (case, path)) in diagnostics.iter().zip([
        ("year-2022", "lønmodtager.skatteår"),
        ("year-2027", "lønmodtager.skatteår"),
        ("year-0", "lønmodtager.skatteår"),
        ("year--9223372036854775808", "lønmodtager.skatteår"),
        (
            "spouse-2022",
            "ægtefælle.MedÆgtefælle.fakta.lønmodtager.skatteår",
        ),
        (
            "spouse-2027",
            "ægtefælle.MedÆgtefælle.fakta.lønmodtager.skatteår",
        ),
    ]) {
        assert_eq!(row["case_id"], case);
        let message = row["message"].as_str().unwrap();
        assert!(
            message.contains(path) && message.contains("Ingen skat beregnet"),
            "{row}"
        );
        println!("{case}: rejected before tax at {path}");
    }
}

#[test]
fn canonical_results_gate_invalid_input_without_changing_valid_tax_amounts() {
    let mut template = run(&["template", MODEL, "--format", "json"]);
    let mut ordinary = template["cases"][0]["input"].clone();
    assert_eq!(
        ordinary["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"]["$variant"],
        "EkstraBørnetilskudUoplyst"
    );
    ordinary["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] =
        json!({"$variant":"IntetEkstraBørnetilskud"});
    assert_eq!(
        ordinary["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"]["$variant"],
        "ArbejdsfradragUdlandUoplyst"
    );
    ordinary["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"] =
        foreign_employment::no_exclusion();
    assert_eq!(
        ordinary["lønmodtager"]["ligningsfradrag"]["boligjob"]["$variant"],
        "BoligjobUoplyst"
    );
    ordinary["lønmodtager"]["ligningsfradrag"]["boligjob"] =
        json!({"$variant":"IngenBoligjobudgifter"});
    ordinary["lønmodtager"]["skatteår"] = json!(2025);
    ordinary["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    ordinary["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
    ordinary["lønmodtager"]["pension"]["atp"] = json!({"$variant":"IngenAtpIndbetalinger"});
    // Confirmed absence for this fictional fixture, not an input default.
    ordinary["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
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
    for (year, birth_year, tax) in [
        (2025, 1960, 21194454),
        (2026, 1959, 20872564),
        (2026, 1960, 20729885),
        (2026, 1961, 20729885),
        (2026, 1962, 20872564),
    ] {
        add(
            &format!("senior-{year}-{birth_year}"),
            None,
            Some(tax),
            &|v| {
                v["lønmodtager"]["skatteår"] = json!(year);
                v["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(birth_year);
            },
        );
    }
    add("senior-spouse", None, Some(20872564), &|v| {
        v["lønmodtager"]["skatteår"] = json!(2026);
        v["ægtefælle"] = spouse(v);
        v["ægtefælle"]["fakta"]["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
        v["ægtefælle"]["fakta"]["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(1960);
    });
    for (year, tax) in [
        (2023, 21100050),
        (2024, 20959383),
        (2025, 20059404),
        (2026, 19689030),
    ] {
        add(&format!("single-parent-{year}"), None, Some(tax), &|v| {
            v["lønmodtager"]["skatteår"] = json!(year);
            v["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] =
                single_parent(year, &[1, 2, 3, 4]);
        });
    }
    add("single-parent-half-year", None, Some(20280797), &|v| {
        v["lønmodtager"]["skatteår"] = json!(2026);
        v["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] = single_parent(2026, &[1, 2]);
    });
    add("senior-and-single-parent", None, Some(19546351), &|v| {
        v["lønmodtager"]["skatteår"] = json!(2026);
        v["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(1960);
        v["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] = single_parent(2026, &[1, 2, 3, 4]);
    });
    add("single-parent-spouse", None, Some(20872564), &|v| {
        v["lønmodtager"]["skatteår"] = json!(2026);
        v["ægtefælle"] = spouse(v);
        v["ægtefælle"]["fakta"]["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
        v["ægtefælle"]["fakta"]["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] =
            single_parent(2026, &[1, 2]);
    });
    for (name, fact) in [
        (
            "single-parent-unknown",
            json!({"$variant":"EkstraBørnetilskudUoplyst"}),
        ),
        (
            "single-parent-duplicate-quarter",
            single_parent(2025, &[1, 1]),
        ),
        ("single-parent-wrong-year", single_parent(2024, &[1])),
    ] {
        add(
            name,
            Some("lønmodtager.ligningsfradrag.enlig_forsørger"),
            None,
            &|v| {
                v["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] = fact.clone();
            },
        );
    }
    add("boligjob-service-2023", None, Some(21521910), &|v| {
        v["lønmodtager"]["skatteår"] = json!(2023);
        v["lønmodtager"]["ligningsfradrag"]["boligjob"] =
            boligjob::facts(vec![boligjob::post(2023, "Ll8VRengøring", 3000000)]);
    });
    for (name, month, day, covered) in [
        ("boligjob-old-covered", 4, 1, true),
        ("boligjob-old-uncovered", 3, 31, false),
    ] {
        add(
            name,
            if covered {
                None
            } else {
                Some("lønmodtager.ligningsfradrag.boligjob")
            },
            if covered { Some(21521910) } else { None },
            &|v| {
                v["lønmodtager"]["skatteår"] = json!(2023);
                let mut row = boligjob::post(2022, "Ll8VRengøring", 3000000);
                row["betaling"]["arbejdsdato"] = json!({"år":2022,"måned":month,"dag":day});
                row["betaling"]["betalingsdato"] = json!({"år":2023,"måned":3,"dag":1});
                v["lønmodtager"]["ligningsfradrag"]["boligjob"] = boligjob::facts(vec![row]);
            },
        );
    }
    add("boligjob-both-caps", None, Some(20234017), &|v| {
        v["lønmodtager"]["skatteår"] = json!(2026);
        let mut craft = boligjob::post(2026, "Ll8VTagisolering", 3000000);
        craft["betaling"]["identifikation"] = json!("invoice-line-payment-2");
        craft["betaling"]["kildereference"] = json!("synthetic-invoice-2-line-1");
        v["lønmodtager"]["ligningsfradrag"]["boligjob"] =
            boligjob::facts(vec![boligjob::post(2026, "Ll8VRengøring", 3000000), craft]);
    });
    for conflict in [false, true] {
        add(
            if conflict {
                "boligjob-spouse-conflict"
            } else {
                "boligjob-shared"
            },
            if conflict {
                Some("lønmodtager.ligningsfradrag.boligjob.fordeling")
            } else {
                None
            },
            if conflict { None } else { Some(20779004) },
            &|v| {
                v["lønmodtager"]["skatteår"] = json!(2026);
                v["ægtefælle"] = spouse(v);
                v["ægtefælle"]["fakta"]["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
                let mut row = boligjob::post(2026, "Ll8VRengøring", 1000000);
                row["betaling"]["andele"] = json!([{"personreference":"hovedperson","arbejdsløn_øre":400000},{"personreference":"ægtefælle","arbejdsløn_øre":600000}]);
                v["lønmodtager"]["ligningsfradrag"]["boligjob"] =
                    boligjob::facts(vec![row.clone()]);
                row["fællesøkonomi_med_betalende_ægtefælle_eller_samlever"] =
                    json!({"$variant":"Ll8VJa"});
                if conflict {
                    row["betaling"]["andele"][0]["arbejdsløn_øre"] = json!(300000);
                }
                let mut spouse_facts = boligjob::facts(vec![row]);
                spouse_facts["personreference"] = json!("ægtefælle");
                v["ægtefælle"]["fakta"]["lønmodtager"]["ligningsfradrag"]["boligjob"] =
                    spouse_facts;
            },
        );
    }
    add(
        "boligjob-unknown",
        Some("lønmodtager.ligningsfradrag.boligjob"),
        None,
        &|v| {
            v["lønmodtager"]["ligningsfradrag"]["boligjob"] = json!({"$variant":"BoligjobUoplyst"});
        },
    );
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
        let senior = matches!(
            name.as_str(),
            "senior-2026-1960" | "senior-2026-1961" | "senior-and-single-parent"
        );
        assert_eq!(
            result["skat"]["seniorbeskæftigelsesfradrag_kroner"],
            if senior { 6100 } else { 0 },
            "{name}"
        );
        if senior {
            assert_eq!(result["skat"]["beskæftigelsesfradrag_kroner"], 63300);
            assert_eq!(result["skat"]["jobfradrag_kroner"], 3100);
            assert_eq!(result["skat"]["ekstra_pensionsfradrag_kroner"], 0);
            assert_eq!(
                result["skat"]["samlede_ligningsmæssige_fradrag_kroner"],
                if name == "senior-and-single-parent" {
                    123100
                } else {
                    72500
                }
            );
        }
        if name == "senior-spouse" {
            assert_eq!(
                result["ægtefælle"]["skat"]["ligningsfradrag"]
                    ["seniorbeskæftigelsesfradrag_kroner"],
                6100
            );
            assert_eq!(
                result["ægtefælle"]["skat"]["ligningsfradrag"]
                    ["samlede_ligningsmæssige_fradrag_kroner"],
                72500
            );
        }
        let parent_deduction = match name.as_str() {
            "single-parent-2023" => 24400,
            "single-parent-2024" | "single-parent-half-year" => 25300,
            "single-parent-2025" => 48300,
            "single-parent-2026" | "senior-and-single-parent" => 50600,
            _ => 0,
        };
        assert_eq!(
            result["enligforsørgerfradrag"]["fradrag_kroner"], parent_deduction,
            "{name}"
        );
        if name == "single-parent-spouse" {
            assert_eq!(
                result["ægtefælle"]["grundlag"]["enligforsørgerfradrag"]["fradrag_kroner"],
                25300
            );
            assert_eq!(
                result["ægtefælle"]["skat"]["ligningsfradrag"]
                    ["samlede_ligningsmæssige_fradrag_kroner"],
                91700
            );
        }
        let boligjob_amount = match name.as_str() {
            "boligjob-service-2023" | "boligjob-old-covered" => 6600,
            "boligjob-both-caps" => 27300,
            "boligjob-shared" | "boligjob-spouse-conflict" => 4000,
            _ => 0,
        };
        assert_eq!(
            result["ligningsfradrag"]["boligjob"]["samlet_fradrag_kroner"], boligjob_amount,
            "{name}"
        );
        if name == "boligjob-shared" {
            assert_eq!(
                result["ægtefælle"]["skat"]["ligningsfradrag"]
                    ["samlede_ligningsmæssige_fradrag_kroner"],
                72400
            );
        }
        assert!(gate["kontroller"].as_array().unwrap().len() >= 26);
    }
}
