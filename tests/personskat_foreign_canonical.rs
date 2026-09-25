//! Foreign-employment allocation through the actual annual tax entry.
//! Every person, employment source and document reference is fictional.
use serde_json::{json, Value};
use std::process::{Command, Output};
#[path = "support/foreign_employment.rs"]
mod foreign_employment;

const MODEL: &str = "examples/danish-income-tax/personskat.calculate.runa";

fn execute(args: &[&str]) -> Output {
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .args(args)
        .output()
        .expect("run canonical foreign audit");
    assert!(
        output.status.success(),
        "{args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    output
}

fn run(args: &[&str]) -> Value {
    serde_json::from_slice(&execute(args).stdout).unwrap()
}

fn allocation(retained: i64, excluded: i64) -> Value {
    let post = |id: &str, amount: i64, abroad: bool| {
        json!({
            "identifikation":id,"indkomstkilde":id,"kildereference":"fictional-employment",
            "indkomstår":2026,"første_dag_i_året":1,"sidste_dag_i_året":365,"grundlag_kroner":amount,
            "forhold":{"$variant":"Ll9jAnsættelsesindkomst","dbo_hjemmehørende_udland":true,
                "arbejde_udført_udland":abroad,"udenlandsk_arbejdsgiver":true}
        })
    };
    json!({"$variant":"FordeltArbejdsfradragsgrundlag", "fordeling":{
        "alle_kilder_og_perioder_oplyst":true,"poster":[post("work-in-DK",retained,false),post("work-abroad",excluded,true)]}})
}

fn set_allocation(input: &mut Value, allocation: Value) {
    input["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"] = allocation;
}

#[test]
fn canonical_interview_and_workbook_preserve_foreign_facts_and_unknowns() {
    let schema = run(&["schema", MODEL]);
    let fields = schema["field_metadata"].as_array().unwrap();
    for prefix in ["lønmodtager", "ægtefælle.MedÆgtefælle.fakta.lønmodtager"] {
        let path = format!("{prefix}.ligningsfradrag.arbejdsfradrag_udland.");
        let selected: Vec<_> = fields
            .iter()
            .filter(|field| field["path"].as_str().unwrap().starts_with(&path))
            .collect();
        assert_eq!(selected.len(), 18, "{prefix}");
        for field in selected {
            for key in ["label", "question", "help"] {
                assert!(!field[key].as_str().unwrap().trim().is_empty(), "{field}");
            }
            let sources = field["sources"].as_array().unwrap();
            for binding in ["ll9j_udland_lovkilde", "ll9j_udland_bemærkninger"] {
                assert!(
                    sources.iter().any(|source| source["binding"] == binding),
                    "{field}"
                );
            }
        }
    }
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let ordinary = ordinary_facts(envelope["cases"][0]["input"].clone());
    let mut mixed = ordinary.clone();
    set_allocation(&mut mixed, allocation(300000, 300000));
    let mut unknown = ordinary.clone();
    set_allocation(
        &mut unknown,
        json!({"$variant":"ArbejdsfradragUdlandUoplyst"}),
    );
    envelope["cases"] = json!([
        {"case_id":"common","input":ordinary},
        {"case_id":"mixed","input":mixed},
        {"case_id":"unknown","input":unknown}
    ]);
    let base =
        std::env::temp_dir().join(format!("futuruna-foreign-workbook-{}", std::process::id()));
    let input_path = base.with_extension("json");
    let workbook_path = base.with_extension("xlsx");
    std::fs::write(&input_path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let direct = run(&["call", MODEL, "--input", input_path.to_str().unwrap()]);
    execute(&[
        "template",
        MODEL,
        "--input",
        input_path.to_str().unwrap(),
        "--format",
        "xlsx",
        "--output",
        workbook_path.to_str().unwrap(),
    ]);
    let workbook = run(&["call", MODEL, "--input", workbook_path.to_str().unwrap()]);
    std::fs::remove_file(input_path).unwrap();
    std::fs::remove_file(workbook_path).unwrap();
    assert_eq!(direct["diagnostics"], json!([]));
    assert_eq!(workbook["diagnostics"], json!([]));
    assert_eq!(direct["results"], workbook["results"]);
    assert_eq!(
        workbook["results"][0]["result"]["vurdering"]["alle_kontroller_gyldige"],
        true
    );
    assert_eq!(
        workbook["results"][1]["result"]["skat"]["beskæftigelsesfradrag_kroner"],
        38250
    );
    assert!(
        workbook["results"][2]["result"]["vurdering"]["slutskat_til_sammenligning_øre"].is_null()
    );
    println!("Danish interview fields and all 3 allocation branches round-trip through XLSX without changing results");
}

fn spouse(facts: &Value) -> Value {
    let mut input = serde_json::Map::new();
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
        input.insert(name.into(), facts[name].clone());
    }
    json!({"$variant":"MedÆgtefælle","fakta":input,"samlevende_ved_indkomstårets_udløb":true,"kildeskat25a_fordelinger":[]})
}

fn ordinary_facts(mut ordinary: Value) -> Value {
    assert_eq!(
        ordinary["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"]["$variant"],
        "ArbejdsfradragUdlandUoplyst"
    );
    ordinary["lønmodtager"]["skatteår"] = json!(2026);
    ordinary["lønmodtager"]["kommune"] = json!({"$variant":"København"});
    ordinary["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    ordinary["lønmodtager"]["kirkeskat"] = json!({"$variant": "IngenKirkeskatHeleÅret"});
    ordinary["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
    ordinary["lønmodtager"]["pension"]["atp"] = json!({"$variant":"IngenAtpIndbetalinger"});
    ordinary["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    ordinary["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] =
        json!({"$variant":"IntetEkstraBørnetilskud"});
    ordinary["lønmodtager"]["ligningsfradrag"]["boligjob"] =
        json!({"$variant":"IngenBoligjobudgifter"});
    set_allocation(&mut ordinary, foreign_employment::no_exclusion());
    ordinary
}

#[test]
fn canonical_tax_uses_filtered_work_basis_without_changing_am_or_ll9l() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let ordinary = ordinary_facts(envelope["cases"][0]["input"].clone());
    let mut cases = vec![json!({"case_id":"ordinary","input":ordinary})];
    let mut mixed = ordinary.clone();
    set_allocation(&mut mixed, allocation(300000, 300000));
    cases.push(json!({"case_id":"mixed","input":mixed}));
    let mut all_excluded = ordinary.clone();
    set_allocation(&mut all_excluded, allocation(0, 600000));
    cases.push(json!({"case_id":"all-excluded","input":all_excluded}));
    let mut senior = mixed.clone();
    senior["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(1961);
    cases.push(json!({"case_id":"senior","input":senior}));
    let mut parent = mixed.clone();
    parent["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] = json!({"$variant":"OplystEkstraBørnetilskud","oplysninger_for_året_komplette":true,
        "kvartaler":(1..=4).map(|quarter|json!({"indkomstår":2026,"kvartal":quarter,"berettiget":true,"modtaget":true,"kildereference":"fictional-benefit"})).collect::<Vec<_>>()});
    cases.push(json!({"case_id":"single-parent","input":parent}));
    for (name, mut facts, allocation) in [
        (
            "atp-ordinary",
            ordinary.clone(),
            foreign_employment::no_exclusion(),
        ),
        ("atp-mixed", mixed.clone(), allocation(300000, 301000)),
    ] {
        facts["lønmodtager"]["pension"]["atp"] = json!({"$variant":"OplysteAtpIndbetalinger","oplysninger_for_året_komplette":true,"poster":[{
            "identifikation":"foreign-job-atp","indkomstår":2026,"kildereference":"fictional-atp","grundlag":{"$variant":"AtpArbejdsgiverPar19Stk1"},"indberettet_før_am_kroner":1000,"indberettet_efter_am_kroner":920}]});
        set_allocation(&mut facts, allocation);
        cases.push(json!({"case_id":name,"input":facts}));
    }
    let mut household = ordinary.clone();
    household["ægtefælle"] = spouse(&mixed);
    cases.push(json!({"case_id":"spouse-mixed","input":household}));
    let mut unknown = ordinary.clone();
    set_allocation(
        &mut unknown,
        json!({"$variant":"ArbejdsfradragUdlandUoplyst"}),
    );
    cases.push(json!({"case_id":"unknown","input":unknown}));
    let mut unknown_spouse = ordinary.clone();
    unknown_spouse["ægtefælle"] = spouse(&unknown);
    cases.push(json!({"case_id":"spouse-unknown","input":unknown_spouse}));
    let mut wrong_total = mixed.clone();
    set_allocation(&mut wrong_total, allocation(300000, 299999));
    cases.push(json!({"case_id":"mismatch","input":wrong_total}));
    let mut blanket = ordinary.clone();
    set_allocation(
        &mut blanket,
        json!({"$variant":"IngenUdlandsudelukkelseIFællesForhold", "dbo_hjemmehørende_udland_i_nogen_periode":true,"noget_arbejde_udført_udland":true,"nogen_udenlandsk_arbejdsgiver":true,"kildereference":"fictional"}),
    );
    cases.push(json!({"case_id":"blanket-exclusion-rejected","input":blanket}));
    envelope["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-foreign-canonical-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), 12);
    let result = |id: &str| &results.iter().find(|row| row["case_id"] == id).unwrap()["result"];
    for id in [
        "ordinary",
        "mixed",
        "all-excluded",
        "senior",
        "single-parent",
        "atp-ordinary",
        "atp-mixed",
        "spouse-mixed",
    ] {
        assert_eq!(
            result(id)["vurdering"]["alle_kontroller_gyldige"],
            true,
            "{id}: {}",
            result(id)["vurdering"]["fejl"]
        );
        assert_eq!(
            result(id)["hovedskat_eksakt"]["arbejdsmarkedsbidrag_øre"],
            4800000,
            "{id}"
        );
    }
    assert_eq!(
        result("ordinary")["vurdering"]["slutskat_til_sammenligning_øre"],
        20872564
    );
    for id in ["mixed", "senior", "single-parent", "atp-mixed"] {
        assert_eq!(
            result(id)["skat"]["beskæftigelsesfradrag_kroner"],
            38250,
            "{id}"
        );
        assert_eq!(result(id)["skat"]["jobfradrag_kroner"], 2916, "{id}");
        assert_eq!(
            result(id)["arbejdsfradrag_udland"]["grundlag"]
                ["grundlag_efter_udlandsafgrænsning_kroner"],
            300000,
            "{id}"
        );
    }
    assert_eq!(
        result("all-excluded")["skat"]["beskæftigelsesfradrag_kroner"],
        0
    );
    assert_eq!(result("all-excluded")["skat"]["jobfradrag_kroner"], 0);
    assert_eq!(
        result("senior")["skat"]["seniorbeskæftigelsesfradrag_kroner"],
        4200
    );
    assert_eq!(
        result("single-parent")["enligforsørgerfradrag"]["fradrag_kroner"],
        34500
    );
    for id in ["atp-ordinary", "atp-mixed"] {
        assert_eq!(
            result(id)["skat"]["ekstra_pensionsfradrag_kroner"],
            110,
            "{id}"
        );
    }
    let spouse = &result("spouse-mixed")["ægtefælle"];
    assert_eq!(
        spouse["grundlag"]["arbejdsfradrag_udland"]["grundlag"]
            ["grundlag_efter_udlandsafgrænsning_kroner"],
        300000
    );
    assert_eq!(
        spouse["skat"]["ligningsfradrag"]["beskæftigelsesfradrag_kroner"],
        38250
    );
    for (id, prefix) in [
        ("unknown", ""),
        ("mismatch", ""),
        ("blanket-exclusion-rejected", ""),
        ("spouse-unknown", "ægtefælle.MedÆgtefælle.fakta."),
    ] {
        let assessment = &result(id)["vurdering"];
        assert_eq!(assessment["alle_kontroller_gyldige"], false, "{id}");
        assert!(
            assessment["slutskat_til_sammenligning_øre"].is_null(),
            "{id}"
        );
        assert!(
            assessment["fejl"]
                .as_array()
                .unwrap()
                .iter()
                .any(|control| control["sti"]
                    == format!("{prefix}lønmodtager.ligningsfradrag.arbejdsfradrag_udland")),
            "{id}: {assessment}"
        );
    }
    println!("12 canonical cases passed; ordinary tax preserved, filtered work deductions, unchanged AM/LL9L, spouse and unknown gates");
}

#[test]
fn part_year_preserves_source_exclusion_and_recomputes_annual_work_deductions() {
    const PART_YEAR: &str = "examples/danish-income-tax/personskat-par14.calculate.runa";
    const ENTRY: &str = "beregn_personskat_delår";
    let mut envelope = run(&["template", PART_YEAR, "--entry", ENTRY, "--format", "json"]);
    let mut facts = ordinary_facts(envelope["cases"][0]["input"]["personskat"].clone());
    facts["lønmodtager"]["bruttoløn_kroner"] = json!(300000);
    let mut split = allocation(150000, 150000);
    for post in split["fordeling"]["poster"].as_array_mut().unwrap() {
        post["sidste_dag_i_året"] = json!(181);
    }
    set_allocation(&mut facts, split);
    let source = |id: &str, field: &str, amount: i64| {
        json!({
            "identifikation":id,"beregningsfelt":{"$variant":field},"delårsbeløb_kroner":amount,
            "omregningsmetode":{"$variant":"Par14ForholdsmæssigtLøbendeBeløb"},"faktisk_helårsbeløb_kroner":null
        })
    };
    let input = json!({"personskat":facts,"skattepligtsændring":{"$variant":"FuldSkattepligtOphører"},
        "skattepligtsperiode":{"fra_dato":{"år":2026,"måned":1,"dag":1},"til_dato":{"år":2026,"måned":6,"dag":30}},
        "valg_afgivet_ved_oplysninger":false,"omvalg_dato":null,
        "kilder":[source("gross-wage","Par14Bruttoløn",300000),source("work-abroad","Par14Ll9jUdelukketUdenlandskAnsættelsesindkomst",150000)],
        "helårsgrundlag":{"$variant":"AfledtFraIdentificeredeKilder"}});
    let mut cases = vec![json!({"case_id":"recurring","input":input})];
    let mut one_off = input.clone();
    for source in one_off["kilder"].as_array_mut().unwrap() {
        source["omregningsmetode"] = json!({"$variant":"Par14UændretEngangsbeløb"});
    }
    cases.push(json!({"case_id":"one-off","input":one_off}));
    let mut documented = input.clone();
    let mut annual = facts.clone();
    annual["lønmodtager"]["bruttoløn_kroner"] = json!(604972);
    set_allocation(&mut annual, allocation(302486, 302486));
    documented["helårsgrundlag"] =
        json!({"$variant":"DokumenteretHelårsPersonskat","personskat":annual});
    cases.push(json!({"case_id":"documented","input":documented}));
    let mut missing = input.clone();
    missing["kilder"].as_array_mut().unwrap().pop();
    cases.push(json!({"case_id":"missing-exclusion","input":missing}));
    let mut wrong_source = input.clone();
    wrong_source["kilder"][1]["identifikation"] = json!("different-source");
    cases.push(json!({"case_id":"wrong-source","input":wrong_source}));
    let mut unknown = input.clone();
    set_allocation(
        &mut unknown["personskat"],
        json!({"$variant":"ArbejdsfradragUdlandUoplyst"}),
    );
    cases.push(json!({"case_id":"unknown","input":unknown}));
    let mut negative = input.clone();
    negative["kilder"][1]["omregningsmetode"] =
        json!({"$variant":"Par14DokumenteretRetvisendeHelårsbeløb","helårsbeløb_kroner":-1});
    cases.push(json!({"case_id":"negative-exclusion","input":negative}));
    let mut mismatched_annual = documented.clone();
    set_allocation(
        &mut mismatched_annual["helårsgrundlag"]["personskat"],
        allocation(302485, 302487),
    );
    cases.push(json!({"case_id":"annual-mismatch","input":mismatched_annual}));
    let mut outside = input.clone();
    set_allocation(
        &mut outside["personskat"],
        foreign_employment::no_exclusion(),
    );
    outside["personskat"]["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"]
        ["kildereference"] =
        json!("fictional Danish treaty residence during the Danish liability period only");
    outside["valg_afgivet_ved_oplysninger"] = json!(true);
    outside["kilder"][0]["faktisk_helårsbeløb_kroner"] = json!(600000);
    outside["kilder"][1]["delårsbeløb_kroner"] = json!(0);
    outside["kilder"][1]["faktisk_helårsbeløb_kroner"] = json!(300000);
    let mut actual_annual = facts.clone();
    actual_annual["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    let mut actual_split = allocation(300000, 300000);
    actual_split["fordeling"]["poster"][0]["sidste_dag_i_året"] = json!(181);
    actual_split["fordeling"]["poster"][0]["forhold"]["dbo_hjemmehørende_udland"] = json!(false);
    actual_split["fordeling"]["poster"][1]["første_dag_i_året"] = json!(182);
    set_allocation(&mut actual_annual, actual_split);
    outside["helårsgrundlag"] =
        json!({"$variant":"DokumenteretHelårsPersonskat","personskat":actual_annual});
    cases.push(json!({"case_id":"outside-period","input":outside}));
    let mut outside_unknown_source = outside.clone();
    outside_unknown_source["kilder"][1]["identifikation"] = json!("unidentified-annual-source");
    cases.push(json!({"case_id":"outside-unidentified","input":outside_unknown_source}));
    envelope["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-foreign-part-year-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&[
        "call",
        PART_YEAR,
        "--entry",
        ENTRY,
        "--input",
        path.to_str().unwrap(),
    ]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), 10);
    let result = |id: &str| &results.iter().find(|row| row["case_id"] == id).unwrap()["result"];
    for id in ["recurring", "documented", "one-off"] {
        let r = result(id);
        assert_eq!(r["input_gyldigt"], true, "{id}: {r}");
        assert_eq!(r["arbejdsmarkedsbidrag_for_delåret_øre"], 2400000, "{id}");
        assert_eq!(
            r["delårsinput"]["ll9j_udelukket_udenlandsk_ansættelsesindkomst_kroner"], 150000,
            "{id}"
        );
        assert_eq!(
            r["delårsresultat"]["skat"]["beskæftigelsesfradrag_kroner"], 19125,
            "{id}"
        );
        let annual_basis = if id == "one-off" { 150000 } else { 302486 };
        assert_eq!(
            r["helårsinput"]["ll9j_udelukket_udenlandsk_ansættelsesindkomst_kroner"], annual_basis,
            "{id}"
        );
        assert_eq!(
            r["helårsberegning"]["beskæftigelsesfradrag_kroner"],
            (annual_basis * 1275 / 100 + 99) / 100,
            "{id}"
        );
        assert_eq!(
            r["helårsberegning"]["jobfradrag_kroner"],
            if id == "one-off" { 0 } else { 3028 },
            "{id}"
        );
    }
    for id in [
        "missing-exclusion",
        "wrong-source",
        "unknown",
        "negative-exclusion",
        "annual-mismatch",
    ] {
        assert_eq!(result(id)["input_gyldigt"], false, "{id}");
    }
    for id in ["missing-exclusion", "wrong-source"] {
        assert_eq!(result(id)["kilder_afstemt_med_personskat"], false, "{id}");
    }
    assert_eq!(result("negative-exclusion")["kilder_gyldige"], false);
    assert_eq!(result("annual-mismatch")["helårsgrundlag_gyldigt"], false);
    assert_eq!(
        result("recurring")["slutskat_efter_par14_øre"],
        result("documented")["slutskat_efter_par14_øre"]
    );
    assert_eq!(
        result("outside-period")["input_gyldigt"],
        true,
        "{}",
        result("outside-period")
    );
    assert_eq!(
        result("outside-period")["delårsinput"]
            ["ll9j_udelukket_udenlandsk_ansættelsesindkomst_kroner"],
        0
    );
    assert_eq!(
        result("outside-period")["helårsinput"]
            ["ll9j_udelukket_udenlandsk_ansættelsesindkomst_kroner"],
        300000
    );
    assert_eq!(
        result("outside-period")["helårsberegning"]["beskæftigelsesfradrag_kroner"],
        38250
    );
    assert_eq!(result("outside-unidentified")["input_gyldigt"], false);
    assert_eq!(
        result("outside-unidentified")["kilder_afstemt_med_personskat"],
        false
    );
    println!("10 part-year cases passed: source-linked exclusion, recurring/one-off/documented/actual annual bases, outside-period earnings, unchanged AM and rejection of missing or mismatched facts");
}
