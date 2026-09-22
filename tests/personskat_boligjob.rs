//! Synthetic invoice facts only; no personal documents or official tax totals.
use serde_json::{json, Value};
use std::process::Command;

const MODEL: &str = "examples/danish-income-tax/boligjob.calculate.runa";

fn run(args: &[&str]) -> Value {
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .args(args)
        .output()
        .expect("run Boligjob calculation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON calculation output")
}

#[path = "support/boligjob.rs"]
mod boligjob;
use boligjob::{facts, post};

#[test]
fn invoice_facts_dates_allocations_and_caps() {
    let schema = run(&["schema", MODEL]);
    let metadata = schema["field_metadata"].as_array().unwrap();
    for path in [
        "udgifter.$variant",
        "udgifter.OplysteBoligjobudgifter.poster.betaling.betalt_arbejdsløn_øre",
        "udgifter.OplysteBoligjobudgifter.poster.betaling.arbejdsdato.år",
    ] {
        let field = metadata
            .iter()
            .find(|field| field["path"] == path)
            .expect("projected invoice metadata");
        assert!(field["question"].is_string());
        assert!(!field["sources"].as_array().unwrap().is_empty());
    }
    let mut template = run(&["template", MODEL, "--format", "json"]);
    assert_eq!(
        template["cases"][0]["input"]["udgifter"]["$variant"],
        "BoligjobUoplyst"
    );
    let mut rows = Vec::new();
    let mut expected = Vec::new();
    let mut add = |name: &str, year: i64, valid: bool, service: i64, craft: i64, input: Value| {
        rows.push(json!({"case_id":name,"input":{"indkomstår":year,"fødselsdato":{"år":1990,"måned":1,"dag":1},"udgifter":input}}));
        expected.push((name.to_string(), valid, service, craft));
    };
    for (year, cap) in [
        (2023, 660000),
        (2024, 1190000),
        (2025, 1750000),
        (2026, 1830000),
    ] {
        add(
            &format!("service-cap-{year}"),
            year,
            true,
            cap,
            0,
            facts(vec![post(year, "Ll8VRengøring", 3000000)]),
        );
    }
    for (year, cap) in [(2023, 0), (2024, 0), (2025, 860000), (2026, 900000)] {
        add(
            &format!("craft-cap-{year}"),
            year,
            true,
            0,
            cap,
            facts(vec![post(year, "Ll8VTagisolering", 3000000)]),
        );
    }
    for year in [2024, 2025] {
        add(
            &format!("new-service-{year}"),
            year,
            true,
            if year == 2025 { 100000 } else { 0 },
            0,
            facts(vec![post(year, "Ll8VTagrender", 100000)]),
        );
    }
    add(
        "labour-only",
        2026,
        true,
        100099,
        0,
        facts(vec![post(2026, "Ll8VVinduespudsning", 100099)]),
    );
    add(
        "zero-labour",
        2026,
        true,
        0,
        0,
        facts(vec![post(2026, "Ll8VVinduespudsning", 0)]),
    );
    add(
        "max-integer-capped",
        2026,
        true,
        1830000,
        0,
        facts(vec![post(2026, "Ll8VRengøring", i64::MAX)]),
    );
    let mut feb = post(2023, "Ll8VRengøring", 100000);
    feb["betaling"]["betalingsdato"] = json!({"år":2024,"måned":2,"dag":29});
    add(
        "leap-feb-work-year",
        2023,
        true,
        100000,
        0,
        facts(vec![feb.clone()]),
    );
    add(
        "leap-feb-not-payment-year",
        2024,
        true,
        0,
        0,
        facts(vec![feb]),
    );
    let mut march = post(2023, "Ll8VRengøring", 100000);
    march["betaling"]["betalingsdato"] = json!({"år":2024,"måned":3,"dag":1});
    add(
        "march-payment-year",
        2024,
        true,
        100000,
        0,
        facts(vec![march.clone()]),
    );
    add("march-not-work-year", 2023, true, 0, 0, facts(vec![march]));
    for (name, work, paid, category, valid, service) in [
        (
            "pre-regulation-work",
            (2022, 3, 31),
            (2023, 3, 1),
            "Ll8VRengøring",
            false,
            0,
        ),
        (
            "regulation-first-day",
            (2022, 4, 1),
            (2023, 3, 1),
            "Ll8VRengøring",
            true,
            100000,
        ),
        (
            "old-service-feb-not-2023",
            (2022, 12, 31),
            (2023, 2, 28),
            "Ll8VRengøring",
            true,
            0,
        ),
        (
            "old-service-march-2023",
            (2022, 12, 31),
            (2023, 3, 1),
            "Ll8VRengøring",
            true,
            100000,
        ),
        (
            "old-craft-not-revived",
            (2022, 4, 1),
            (2025, 3, 1),
            "Ll8VTagisolering",
            true,
            0,
        ),
        (
            "old-new-service-not-revived",
            (2022, 4, 1),
            (2025, 3, 1),
            "Ll8VTagrender",
            true,
            0,
        ),
    ] {
        let mut p = post(work.0, category, 100000);
        p["betaling"]["arbejdsdato"] = json!({"år":work.0,"måned":work.1,"dag":work.2});
        p["betaling"]["betalingsdato"] = json!({"år":paid.0,"måned":paid.1,"dag":paid.2});
        add(name, paid.0, valid, service, 0, facts(vec![p]));
    }
    for (name, field, value, valid) in [
        ("cash", "betalingsform", "Ll8VKontanter", true),
        ("check", "betalingsform", "Ll8VCheck", true),
        (
            "payment-unknown",
            "betalingsform",
            "Ll8VUoplystBetaling",
            false,
        ),
        ("subsidy", "offentligt_tilskud", "Ll8VJa", true),
        (
            "other-deduction",
            "samme_udgift_fradraget_efter_andre_regler",
            "Ll8VJa",
            true,
        ),
        (
            "household-worker",
            "udfører_bor_i_helårsboligen_eller_ejer_fritidsboligen_eller_bor_med_ejer",
            "Ll8VJa",
            true,
        ),
        (
            "unknown-subsidy",
            "offentligt_tilskud",
            "Ll8VUoplyst",
            false,
        ),
        (
            "missing-payment-evidence",
            "betalingsbilag",
            "Ll8VNej",
            true,
        ),
        ("unlisted-work", "ydelse", "Ll8VYdelseUdenForBilag1", true),
        ("unclassified-work", "ydelse", "Ll8VUafklaretYdelse", false),
        (
            "insurance-unimplemented",
            "særligt_forhold",
            "Ll8VForsikringEllerAndenUafklaretFordeling",
            false,
        ),
    ] {
        let mut p = post(2026, "Ll8VRengøring", 100000);
        p["betaling"][field] = json!({"$variant":value});
        add(name, 2026, valid, 0, 0, facts(vec![p]));
    }
    let mut duplicate = post(2026, "Ll8VRengøring", 100000);
    duplicate["betaling"]["identifikation"] = json!("different-display-id");
    add(
        "duplicate-source-payment",
        2026,
        false,
        0,
        0,
        facts(vec![post(2026, "Ll8VRengøring", 100000), duplicate]),
    );
    let mut over = post(2026, "Ll8VRengøring", 100000);
    over["betaling"]["andele"][0]["arbejdsløn_øre"] = json!(100001);
    add("overallocated", 2026, false, 0, 0, facts(vec![over]));
    let mut shared = post(2026, "Ll8VRengøring", 100000);
    shared["betaling"]["betalerreference"] = json!("ægtefælle");
    shared["betaling"]["andele"] = json!([{"personreference":"hovedperson","arbejdsløn_øre":40000},{"personreference":"ægtefælle","arbejdsløn_øre":60000}]);
    shared["fællesøkonomi_med_betalende_ægtefælle_eller_samlever"] = json!({"$variant":"Ll8VJa"});
    add(
        "shared-economy",
        2026,
        true,
        40000,
        0,
        facts(vec![shared.clone()]),
    );
    shared["fællesøkonomi_med_betalende_ægtefælle_eller_samlever"] = json!({"$variant":"Ll8VNej"});
    add("no-shared-economy", 2026, true, 0, 0, facts(vec![shared]));
    let mut holiday = post(2026, "Ll8VRengøring", 100000);
    holiday["boligforhold"] = json!({"$variant":"Ll8VFritidsbolig","ejer_og_ejendomsværdiskattepligtig_ved_arbejdet":{"$variant":"Ll8VJa"},"gift_og_samlevende_med_sådan_ejer_ved_arbejdet":{"$variant":"Ll8VUoplyst"},"udlejet_i_fradragsåret":{"$variant":"Ll8VJa"}});
    add(
        "rented-holiday-service",
        2026,
        true,
        0,
        0,
        facts(vec![holiday.clone()]),
    );
    holiday["betaling"]["ydelse"] = json!({"$variant":"Ll8VTagisolering"});
    add(
        "rented-holiday-craft",
        2026,
        true,
        0,
        100000,
        facts(vec![holiday]),
    );
    let mut private = post(2026, "Ll8VRengøring", 100000);
    private["betaling"]["leverandør"] = json!({"$variant":"Ll8VPrivatperson","fødselsdato":{"år":2008,"måned":12,"dag":31},"fuld_skattepligt_i":{"$variant":"Ll8VDanskRegistrering"}});
    add(
        "private-adult-service",
        2026,
        true,
        100000,
        0,
        facts(vec![private.clone()]),
    );
    private["betaling"]["ydelse"] = json!({"$variant":"Ll8VTagisolering"});
    add(
        "private-craft-ineligible",
        2026,
        true,
        0,
        0,
        facts(vec![private]),
    );
    add(
        "unknown-year-facts",
        2026,
        false,
        0,
        0,
        json!({"$variant":"BoligjobUoplyst"}),
    );
    add(
        "known-none",
        2026,
        true,
        0,
        0,
        json!({"$variant":"IngenBoligjobudgifter"}),
    );
    let mut invalid_date = post(2026, "Ll8VRengøring", 100000);
    invalid_date["betaling"]["betalingsdato"] = json!({"år":2027,"måned":2,"dag":29});
    add(
        "impossible-date",
        2026,
        false,
        0,
        0,
        facts(vec![invalid_date]),
    );
    let mut advance = post(2026, "Ll8VRengøring", 100000);
    advance["betaling"]["betalingsdato"] = json!({"år":2026,"måned":11,"dag":1});
    add(
        "advance-unimplemented",
        2026,
        false,
        0,
        0,
        facts(vec![advance]),
    );
    let mut old_craft = post(2024, "Ll8VTagisolering", 100000);
    old_craft["betaling"]["betalingsdato"] = json!({"år":2025,"måned":3,"dag":1});
    add(
        "old-craft-late-payment",
        2025,
        true,
        0,
        0,
        facts(vec![old_craft]),
    );
    let mut incomplete = facts(vec![post(2026, "Ll8VRengøring", 100000)]);
    incomplete["oplysninger_for_året_komplette"] = json!(false);
    add("incomplete-year", 2026, false, 0, 0, incomplete);
    let mut negative = post(2026, "Ll8VRengøring", 100000);
    negative["betaling"]["andele"][0]["arbejdsløn_øre"] = json!(-1);
    add("negative-share", 2026, false, 0, 0, facts(vec![negative]));
    let mut duplicate_person = post(2026, "Ll8VRengøring", 100000);
    duplicate_person["betaling"]["andele"] = json!([{"personreference":"hovedperson","arbejdsløn_øre":50000},{"personreference":"hovedperson","arbejdsløn_øre":50000}]);
    add(
        "duplicate-person",
        2026,
        false,
        0,
        0,
        facts(vec![duplicate_person]),
    );
    let mut foreign = post(2026, "Ll8VRengøring", 100000);
    foreign["betaling"]["land"] =
        json!({"$variant":"Ll8VEU_EØS","oplysningsudveksling":{"$variant":"Ll8VJa"}});
    foreign["betaling"]["leverandør"]["moms_eller_tredjelandsregistrering"] =
        json!({"$variant":"Ll8VEU_EØSRegistrering"});
    foreign["boligforhold"] = json!({"$variant":"Ll8VFritidsbolig","ejer_og_ejendomsværdiskattepligtig_ved_arbejdet":{"$variant":"Ll8VJa"},"gift_og_samlevende_med_sådan_ejer_ved_arbejdet":{"$variant":"Ll8VUoplyst"},"udlejet_i_fradragsåret":{"$variant":"Ll8VNej"}});
    add(
        "eu-holiday",
        2026,
        true,
        100000,
        0,
        facts(vec![foreign.clone()]),
    );
    foreign["betaling"]["land"]["oplysningsudveksling"] = json!({"$variant":"Ll8VNej"});
    add("no-exchange", 2026, true, 0, 0, facts(vec![foreign]));
    let mut no_rut = post(2026, "Ll8VTagisolering", 100000);
    no_rut["betaling"]["leverandør"]["udenlandsk_virksomhed_i_danmark"] =
        json!({"$variant":"Ll8VJa"});
    no_rut["betaling"]["leverandør"]["rut_registreret"] = json!({"$variant":"Ll8VNej"});
    add(
        "foreign-company-without-rut",
        2026,
        true,
        0,
        0,
        facts(vec![no_rut]),
    );
    let mut no_share = post(2026, "Ll8VRengøring", 100000);
    no_share["betaling"]["andele"] = json!([]);
    add(
        "omitted-allocation",
        2026,
        false,
        0,
        0,
        facts(vec![no_share]),
    );
    template["cases"] = json!(rows);
    let path = std::env::temp_dir().join(format!(
        "futuruna-boligjob-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let result = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    let outputs = result["results"].as_array().unwrap();
    assert_eq!(outputs.len(), expected.len());
    for (row, (name, valid, service, craft)) in outputs.iter().zip(expected) {
        assert_eq!(row["case_id"], name);
        let value = &row["result"];
        assert_eq!(value["input_gyldigt"], valid, "{name}: {value}");
        assert_eq!(value["servicefradrag_øre"], service, "{name}");
        assert_eq!(value["håndværkerfradrag_øre"], craft, "{name}");
        assert_eq!(
            value["samlet_fradrag_kroner"],
            service / 100 + craft / 100,
            "{name}"
        );
        for post in value["poster"].as_array().unwrap() {
            assert_eq!(post["betingelser"].as_array().unwrap().len(), 17, "{name}");
            if name == "old-service-feb-not-2023" {
                assert_eq!(post["fradragsår"], 2022, "{name}");
                assert_eq!(post["betingelser_opfyldt"], true, "{name}");
            }
            if name == "pre-regulation-work" {
                assert!(post["kontrolpunkter"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|reason| {
                        reason
                            .as_str()
                            .is_some_and(|text| text.contains("1. april 2022"))
                    }));
            }
            if name == "cash" {
                assert!(post["kontrolpunkter"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|reason| reason == "Elektronisk betaling, ikke kontanter eller check"));
            }
        }
    }
}
