//! Synthetic report observations only: none of these figures are taxpayer fixtures.
//! A reconciliation must never certify the report's underlying legal facts.
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

const MODEL: &str = "examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa";

fn invoke(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .args(args)
        .output()
        .expect("execute runa");
    assert!(
        output.status.success(),
        "runa {args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).expect("calculation JSON")
}

fn baseline() -> Value {
    json!({
        "skatteår": 2023,
        "kommune": {"$variant": "København"},
        "betaler_kirkeskat": false,
        "ægtefællens_kommune": null,
        "gift_samlevende_ved_årets_udløb": null,
        "indkomstbro": {
            "personlig_indkomst_kroner": 400000,
            "kapitalindkomst_kroner": -10000,
            "ligningsmæssige_fradrag_kroner": 45000,
            "skattepligtig_indkomst_kroner": 333000,
            "øvrige_indkomstreguleringer_kroner": 0,
            "oplyst_ægtefælleunderskud_kroner": 12000
        },
        "ægtefællenedslag": {
            "statsligt_personfradrag_øre": 100000,
            "kommunalt_personfradrag_øre": 200000,
            "kirkeligt_personfradrag_øre": 0,
            "negativ_kapitalindkomst_øre": 80000,
            "eget_negativ_kapitalindkomstnedslag_øre": 80000
        },
        "skat": {
            "poster_uden_ægtefællenedslag": [{"navn": "Syntetisk øvrig skat", "beløb_øre": 10200000}],
            "poster_uden_ægtefællenedslag_komplette": true,
            "oplyst_beregnet_skat_øre": 9820000
        },
        "betaling": {
            "oplyst_forskudsskat_øre": 9900000,
            "oplyst_beregnet_skat_øre": 9820000,
            "oplyst_overskydende_skat_øre": 80000,
            "korrektioner_til_udbetaling": [{"navn": "Oplyst godtgørelse", "beløb_øre": 1234}],
            "korrektioner_komplette": true,
            "oplyst_udbetaling_kroner": 812
        }
    })
}

fn condition<'a>(result: &'a Value, name: &str) -> &'a Value {
    result["nødvendige_forudsætninger"]
        .as_array()
        .expect("conditions")
        .iter()
        .find(|value| value["navn"] == name)
        .unwrap_or_else(|| panic!("missing condition {name}: {result}"))
}

fn control<'a>(result: &'a Value, name: &str) -> &'a Value {
    result["kontroller"]
        .as_array()
        .expect("controls")
        .iter()
        .find(|value| value["navn"] == name)
        .unwrap_or_else(|| panic!("missing control {name}: {result}"))
}

#[test]
fn reports_expose_necessary_conditions_without_inventing_spouse_facts() {
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    let mut add = |name: &str, status: &str, edit: &dyn Fn(&mut Value)| {
        let mut input = baseline();
        edit(&mut input);
        cases.push(json!({"case_id": name, "input": input}));
        expected.push((name.to_string(), status.to_string()));
    };
    for year in 2023..=2026 {
        add(&format!("year-{year}"), "BetingetAfstemt", &|v| {
            v["skatteår"] = json!(year);
        });
    }
    add("unknown-loss", "Ufuldstændig", &|v| {
        v["indkomstbro"]["oplyst_ægtefælleunderskud_kroner"] = Value::Null;
    });
    add("unknown-all-transfers", "Ufuldstændig", &|v| {
        for key in [
            "statsligt_personfradrag_øre",
            "kommunalt_personfradrag_øre",
            "kirkeligt_personfradrag_øre",
            "negativ_kapitalindkomst_øre",
        ] {
            v["ægtefællenedslag"][key] = Value::Null;
        }
    });
    add("no-income-bridge", "Ufuldstændig", &|v| {
        v["indkomstbro"] = Value::Null
    });
    add("incomplete-tax-lines", "Ufuldstændig", &|v| {
        v["skat"]["poster_uden_ægtefællenedslag_komplette"] = json!(false);
    });
    // The baseline confirms completeness and retains nonzero assessed tax.
    add(
        "complete-empty-tax-lines-with-nonzero-tax",
        "Modstrid",
        &|v| {
            v["skat"]["poster_uden_ægtefællenedslag"] = json!([]);
        },
    );
    add("incomplete-corrections", "Ufuldstændig", &|v| {
        v["betaling"]["korrektioner_komplette"] = json!(false);
    });
    add("one-ore-transfer", "Modstrid", &|v| {
        v["ægtefællenedslag"]["statsligt_personfradrag_øre"] = json!(100001);
    });
    add("one-ore-tax-section", "Modstrid", &|v| {
        v["betaling"]["oplyst_beregnet_skat_øre"] = json!(9820001);
    });
    add("one-krone-payout", "Modstrid", &|v| {
        v["betaling"]["oplyst_udbetaling_kroner"] = json!(813);
    });
    add("previous-refund", "BetingetAfstemt", &|v| {
        v["betaling"]["korrektioner_til_udbetaling"]
            .as_array_mut()
            .unwrap()
            .push(json!({"navn": "Tidligere udbetalt", "beløb_øre": -80000}));
        v["betaling"]["oplyst_udbetaling_kroner"] = json!(12);
    });
    add("negative-settlement", "Ufuldstændig", &|v| {
        v["betaling"]["korrektioner_til_udbetaling"] =
            json!([{"navn": "Tidligere udbetalt", "beløb_øre": -90000}]);
    });
    add("restskat-with-positive-correction", "Ufuldstændig", &|v| {
        v["betaling"]["oplyst_forskudsskat_øre"] = json!(9819900);
        v["betaling"]["oplyst_overskydende_skat_øre"] = json!(-100);
    });
    add("unknown-prepaid", "Ufuldstændig", &|v| {
        v["betaling"]["oplyst_forskudsskat_øre"] = Value::Null;
    });
    add("known-no-cohabitation", "Modstrid", &|v| {
        v["gift_samlevende_ved_årets_udløb"] = json!(false);
    });
    add("known-no-spouse-no-transfers", "BetingetAfstemt", &|v| {
        v["gift_samlevende_ved_årets_udløb"] = json!(false);
        v["indkomstbro"]["skattepligtig_indkomst_kroner"] = json!(345000);
        v["indkomstbro"]["oplyst_ægtefælleunderskud_kroner"] = json!(0);
        for key in [
            "statsligt_personfradrag_øre",
            "kommunalt_personfradrag_øre",
            "kirkeligt_personfradrag_øre",
            "negativ_kapitalindkomst_øre",
        ] {
            v["ægtefællenedslag"][key] = json!(0);
        }
        v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(9820000);
    });
    add("negative-own-capital-credit", "Modstrid", &|v| {
        v["ægtefællenedslag"]["eget_negativ_kapitalindkomstnedslag_øre"] = json!(-1);
    });
    add("joint-capital-cap", "Modstrid", &|v| {
        v["ægtefællenedslag"]["eget_negativ_kapitalindkomstnedslag_øre"] = json!(720001);
    });
    add("state-cap", "Modstrid", &|v| {
        v["ægtefællenedslag"]["statsligt_personfradrag_øre"] = json!(578881);
        // Preserve all arithmetic: the legal bound must independently catch this.
        v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(10678881);
    });
    add("church-without-church-tax", "Modstrid", &|v| {
        v["ægtefællenedslag"]["kirkeligt_personfradrag_øre"] = json!(1);
        v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(10200001);
    });
    add("loss-exceeds-positive-income", "Modstrid", &|v| {
        v["indkomstbro"]["skattepligtig_indkomst_kroner"] = json!(-1);
        v["indkomstbro"]["oplyst_ægtefælleunderskud_kroner"] = json!(345001);
    });
    add("different-spouse-municipality", "BetingetAfstemt", &|v| {
        v["ægtefællens_kommune"] = json!({"$variant": "Ballerup"});
        v["ægtefællenedslag"]["kommunalt_personfradrag_øre"] = json!(1137600);
        v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(11137600);
    });
    add("recipient-municipal-cap", "Modstrid", &|v| {
        v["ægtefællens_kommune"] = json!({"$variant": "Ballerup"});
        v["ægtefællenedslag"]["kommunalt_personfradrag_øre"] = json!(1137601);
        v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(11137601);
    });
    add("duplicate-tax-posts", "UgyldigtRapportinput", &|v| {
        let post = v["skat"]["poster_uden_ægtefællenedslag"][0].clone();
        v["skat"]["poster_uden_ægtefællenedslag"]
            .as_array_mut()
            .unwrap()
            .push(post);
    });
    add("duplicate-corrections", "UgyldigtRapportinput", &|v| {
        let post = v["betaling"]["korrektioner_til_udbetaling"][0].clone();
        v["betaling"]["korrektioner_til_udbetaling"]
            .as_array_mut()
            .unwrap()
            .push(post);
    });
    add("overflow-bound", "UgyldigtRapportinput", &|v| {
        v["betaling"]["oplyst_forskudsskat_øre"] = json!(i64::MAX);
    });
    add("blank-post-name", "UgyldigtRapportinput", &|v| {
        v["skat"]["poster_uden_ægtefællenedslag"][0]["navn"] = json!("  ");
    });
    add("post-count-limit", "UgyldigtRapportinput", &|v| {
        v["skat"]["poster_uden_ægtefællenedslag"] = json!((0..1001)
            .map(|i| json!({"navn": format!("post-{i}"), "beløb_øre": 0}))
            .collect::<Vec<_>>());
    });
    add(
        "unsupported-year",
        "IkkeUnderstøttetÅrEllerKommune",
        &|v| {
            v["skatteår"] = json!(2027);
        },
    );

    let mut template = invoke(&["template", MODEL, "--format", "json"]);
    template["cases"] = json!(cases);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "futuruna-report-reconciliation-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let output = invoke(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().expect("results");
    assert_eq!(results.len(), expected.len());
    for (case, (name, status)) in results.iter().zip(&expected) {
        assert_eq!(case["case_id"], *name);
        assert_eq!(
            case["result"]["status"]["$variant"], *status,
            "{name}: {case}"
        );
        assert_eq!(
            case["result"]["uafhængig_skatteberegning_udført"], false,
            "{name}"
        );
        assert!(!case["result"]["uafklaret"].as_array().unwrap().is_empty());
    }
    let get = |name: &str| &results.iter().find(|r| r["case_id"] == name).unwrap()["result"];
    let loss_name = "Nødvendigt indkomstfradrag fra ægtefælle";
    let total_name = "Nødvendig samlet skattenedsættelse fra ægtefælle";
    assert_eq!(
        condition(get("unknown-loss"), loss_name)["nødvendigt_beløb"],
        12000
    );
    assert_eq!(
        control(get("unknown-loss"), "Indkomstbro og ægtefælleunderskud")["oplyst"],
        Value::Null
    );
    assert_eq!(
        condition(get("unknown-all-transfers"), total_name)["nødvendigt_beløb"],
        380000
    );
    assert_eq!(
        condition(
            get("year-2023"),
            "Mindste grundlag bag ægtefællens overførte kapitalnedslag"
        )["nødvendigt_beløb"],
        10000
    );
    assert_eq!(
        condition(
            get("year-2023"),
            "Ubrugt kommunal personfradragsværdi fra ægtefælle"
        )["højst"],
        1137600
    );
    assert_eq!(
        condition(
            get("different-spouse-municipality"),
            "Ubrugt kommunal personfradragsværdi fra ægtefælle"
        )["højst"],
        1137600
    );
    assert_eq!(
        control(
            get("one-ore-transfer"),
            "Samlet ægtefællenedslag i beregnet skat"
        )["difference"],
        1
    );
    assert_eq!(
        control(
            get("previous-refund"),
            "Udbetaling efter oplyste korrektioner og hele kroner"
        )["forventet"],
        12
    );
    assert!(!get("incomplete-tax-lines")["nødvendige_forudsætninger"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["navn"] == total_name));
}

#[test]
fn recipient_rates_and_combined_local_lines_match_official_synthetic_reports() {
    // Official anonymous 2025 calculator, observed 2026-09-25. Fictional married
    // adults born 1990: recipient Copenhagen, salary 400,000; spouse Ballerup,
    // no income, no church membership. No other income/deductions/payments.
    // The church-member recipient's printed municipal allowance includes church.
    // Sources and exact displayed rows: aarsopgoerelse-afstemning.md.
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    let mut add = |name: &str, church: bool, status: &str, edit: &dyn Fn(&mut Value)| {
        let mut input = baseline();
        let local = if church { 1253880 } else { 1212600 };
        let assessed = if church { 11548858 } else { 11378698 };
        input["skatteår"] = json!(2025);
        input["betaler_kirkeskat"] = json!(church);
        // Intentionally conceal the known fictional spouse facts from this
        // conditional report audit; its bounds must not require their report.
        input["indkomstbro"] = json!({
            "personlig_indkomst_kroner":368000, "kapitalindkomst_kroner":0,
            "ligningsmæssige_fradrag_kroner":52100, "skattepligtig_indkomst_kroner":315900,
            "øvrige_indkomstreguleringer_kroner":0, "oplyst_ægtefælleunderskud_kroner":0
        });
        input["ægtefællenedslag"] = json!({
            "statsligt_personfradrag_øre":619716,
            "kommunalt_personfradrag_øre":null, "kirkeligt_personfradrag_øre":null,
            "kommunalt_og_kirkeligt_personfradrag_øre":local,
            "negativ_kapitalindkomst_øre":0, "eget_negativ_kapitalindkomstnedslag_øre":0
        });
        let mut lines = vec![
            json!({"navn":"AM-bidrag", "beløb_øre":3200000}),
            json!({"navn":"Bundskat", "beløb_øre":4419680}),
            json!({"navn":"Kommuneskat", "beløb_øre":7423650}),
            json!({"navn":"Eget personfradrag, stat", "beløb_øre":-619716}),
            json!({"navn":"Eget personfradrag, kommune og kirke", "beløb_øre":-local}),
        ];
        if church {
            lines.push(json!({"navn":"Kirkeskat", "beløb_øre":252720}));
        }
        input["skat"]["poster_uden_ægtefællenedslag"] = json!(lines);
        input["skat"]["oplyst_beregnet_skat_øre"] = json!(assessed);
        input["betaling"] = json!({
            "oplyst_forskudsskat_øre":0, "oplyst_beregnet_skat_øre":assessed,
            "oplyst_overskydende_skat_øre":null, "korrektioner_til_udbetaling":[],
            "korrektioner_komplette":true, "oplyst_udbetaling_kroner":null,
            "restskat":{"oplyst_restskat_øre":assessed, "tillæg_til_slutskat":[],
                "tillæg_til_slutskat_komplette":true}
        });
        edit(&mut input);
        cases.push(json!({"case_id":name, "input":input}));
        expected.push((name.to_owned(), status.to_owned(), local, assessed));
    };
    for (church, name) in [(false, "official-no-church"), (true, "official-church")] {
        add(name, church, "BetingetAfstemt", &|_| {});
        add(
            &format!("{name}-known-spouse"),
            church,
            "BetingetAfstemt",
            &|v| {
                v["ægtefællens_kommune"] = json!({"$variant":"Ballerup"});
            },
        );
        add(&format!("{name}-cap-plus-ore"), church, "Modstrid", &|v| {
            let n = &mut v["ægtefællenedslag"]["kommunalt_og_kirkeligt_personfradrag_øre"];
            *n = json!(n.as_i64().unwrap() + 1);
            // Keep all arithmetic comparisons equal: only the bound may fail.
            v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(3200001);
        });
    }
    add("no-cohabitation", true, "Modstrid", &|v| {
        v["gift_samlevende_ved_årets_udløb"] = json!(false);
    });
    for field in ["kommunalt_personfradrag_øre", "kirkeligt_personfradrag_øre"] {
        add(
            &format!("duplicate-{field}"),
            true,
            "UgyldigtRapportinput",
            &|v| {
                v["ægtefællenedslag"][field] = json!(0);
            },
        );
    }
    add("combined-zero", true, "BetingetAfstemt", &|v| {
        v["ægtefællenedslag"]["kommunalt_og_kirkeligt_personfradrag_øre"] = json!(0);
        v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(3200000 - 1253880);
    });
    add("combined-negative", true, "Modstrid", &|v| {
        v["ægtefællenedslag"]["kommunalt_og_kirkeligt_personfradrag_øre"] = json!(-1);
        v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(3200000 - 1253881);
    });
    add(
        "combined-overflow-bound",
        true,
        "UgyldigtRapportinput",
        &|v| {
            v["ægtefællenedslag"]["kommunalt_og_kirkeligt_personfradrag_øre"] = json!(i64::MAX);
        },
    );
    add(
        "combined-known-state-missing",
        true,
        "Ufuldstændig",
        &|v| {
            v["ægtefællenedslag"]["statsligt_personfradrag_øre"] = Value::Null;
        },
    );
    add(
        "combined-known-state-cap-plus-ore",
        true,
        "Modstrid",
        &|v| {
            v["ægtefællenedslag"]["statsligt_personfradrag_øre"] = Value::Null;
            v["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(3200001);
        },
    );
    add(
        "separate-church-at-recipient-cap",
        true,
        "BetingetAfstemt",
        &|v| {
            v["ægtefællens_kommune"] = json!({"$variant":"Ballerup"});
            v["ægtefællenedslag"]["kommunalt_og_kirkeligt_personfradrag_øre"] = Value::Null;
            v["ægtefællenedslag"]["kommunalt_personfradrag_øre"] = json!(1212600);
            v["ægtefællenedslag"]["kirkeligt_personfradrag_øre"] = json!(41280);
        },
    );

    let mut template = invoke(&["template", MODEL, "--format", "json"]);
    template["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-recipient-rates-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let output = invoke(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expected.len());
    for (case, (name, status, local, assessed)) in results.iter().zip(expected) {
        assert_eq!(case["case_id"], name);
        let result = &case["result"];
        assert_eq!(result["status"]["$variant"], status, "{name}: {result}");
        assert_eq!(result["uafhængig_skatteberegning_udført"], false);
        if status == "UgyldigtRapportinput" {
            assert_eq!(result["kontroller"], json!([]));
            continue;
        }
        assert_eq!(
            control(result, "Restskat før renter og procenttillæg")["forventet"],
            assessed
        );
        if name == "separate-church-at-recipient-cap" {
            assert_eq!(
                condition(result, "Ubrugt kirkelig personfradragsværdi fra ægtefælle")["højst"],
                41280
            );
        } else {
            assert_eq!(
                condition(
                    result,
                    "Ubrugt kommunal og kirkelig personfradragsværdi fra ægtefælle samlet"
                )["højst"],
                local
            );
            assert!(!result["nødvendige_forudsætninger"]
                .as_array()
                .unwrap()
                .iter()
                .any(
                    |c| c["navn"] == "Ubrugt kirkelig personfradragsværdi fra ægtefælle"
                        || c["navn"] == "Ubrugt kommunal personfradragsværdi fra ægtefælle"
                ));
        }
        let sum = control(result, "Samlet ægtefællenedslag i beregnet skat");
        if name.starts_with("combined-known-state-") {
            assert_eq!(sum["oplyst"], Value::Null);
            let residual = condition(result, "Nødvendig sum af ikke-oplyste ægtefællenedslag");
            assert_eq!(residual["højst"], 619716);
            assert_eq!(
                residual["nødvendigt_beløb"],
                if status == "Modstrid" { 619717 } else { 619716 }
            );
        } else {
            assert_eq!(sum["difference"], 0, "{name}");
        }
    }
}

#[test]
fn partial_transfers_expose_residual_bounds_without_filling_missing_observations() {
    let fields = [
        "statsligt_personfradrag_øre",
        "kommunalt_personfradrag_øre",
        "kirkeligt_personfradrag_øre",
        "negativ_kapitalindkomst_øre",
    ];
    let amounts = [100000_i64, 200000, 0, 80000];
    // Independent 2023 bounds: 48,000 * 12.06%, recipient municipality 23.70%,
    // no church tax, and 100,000 * 8% less the observed own credit of 800 DKK.
    let ceilings = [Some(578880_i64), Some(1137600), Some(0), Some(720000)];
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    let mut add = |name: String, mask: usize, required: i64, complete: bool| {
        let mut input = baseline();
        let mut known = 0;
        let mut ceiling = Some(0);
        for (index, field) in fields.iter().enumerate() {
            if mask & (1 << index) != 0 {
                input["ægtefællenedslag"][field] = Value::Null;
                ceiling = ceiling.zip(ceilings[index]).map(|(a, b)| a + b);
            } else {
                known += amounts[index];
            }
        }
        input["skat"]["poster_uden_ægtefællenedslag"][0]["beløb_øre"] = json!(9820000 + required);
        input["skat"]["poster_uden_ægtefællenedslag_komplette"] = json!(complete);
        let residual = required - known;
        let possible = residual >= 0 && ceiling.is_none_or(|limit| residual <= limit);
        let status = if complete && !possible {
            "Modstrid"
        } else {
            "Ufuldstændig"
        };
        cases.push(json!({"case_id":name,"input":input}));
        expected.push((name, residual, ceiling, possible, complete, status));
    };
    // One missing municipality line must not hide that the known credits
    // already exceed the required total by one ore.
    add("known-exceeds-total".into(), 2, 179999, true);
    for mask in 1..16 {
        add(format!("missing-mask-{mask}"), mask, 380000, true);
    }
    for (name, mask, known, cap) in [
        ("state", 1, 280000, 578880),
        ("municipal", 2, 180000, 1137600),
        ("state-and-municipal", 3, 80000, 1716480),
        ("church", 4, 380000, 0),
        ("capital", 8, 300000, 720000),
        ("state-and-capital", 9, 200000, 1298880),
    ] {
        for delta in [-1, 0, 1] {
            add(
                format!("{name}-cap-{delta}"),
                mask,
                known + cap + delta,
                true,
            );
        }
    }
    for residual in [-1, 0, 1] {
        add(
            format!("unknown-spouse-known-municipal-cap-{residual}"),
            2,
            180000 + residual,
            true,
        );
    }
    add(
        "incomplete-does-not-infer-negative-residual".into(),
        2,
        179999,
        false,
    );
    add(
        "incomplete-does-not-infer-cap-conflict".into(),
        1,
        858881,
        false,
    );

    let mut template = invoke(&["template", MODEL, "--format", "json"]);
    template["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-partial-transfers-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let output = invoke(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expected.len());
    for (case, (name, residual, ceiling, possible, complete, status)) in
        results.iter().zip(expected)
    {
        assert_eq!(case["case_id"], name);
        let result = &case["result"];
        assert_eq!(result["status"]["$variant"], status, "{name}: {result}");
        assert_eq!(result["uafhængig_skatteberegning_udført"], false);
        assert!(!result["uafklaret"].as_array().unwrap().is_empty());
        let total = control(result, "Samlet ægtefællenedslag i beregnet skat");
        // A necessary residual is not an observation or permission to complete it.
        assert_eq!(total["oplyst"], Value::Null, "{name}");
        assert_eq!(total["difference"], Value::Null, "{name}");
        assert_eq!(total["status"]["$variant"], "IkkeOplyst", "{name}");
        let residual_name = "Nødvendig sum af ikke-oplyste ægtefællenedslag";
        if complete {
            let bound = condition(result, residual_name);
            assert_eq!(bound["enhed"], "øre", "{name}");
            assert_eq!(bound["nødvendigt_beløb"], residual, "{name}");
            assert_eq!(bound["mindst"], 0, "{name}");
            assert_eq!(bound["højst"], json!(ceiling), "{name}");
            assert_eq!(bound["inden_for_kontrollerede_grænser"], possible, "{name}");
        } else {
            assert_eq!(total["forventet"], Value::Null, "{name}");
            assert!(!result["nødvendige_forudsætninger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|bound| bound["navn"] == residual_name));
        }
    }
}

#[test]
fn confirmed_empty_tax_sections_preserve_zero_unknowns_and_contradictions() {
    let mut empty = baseline();
    empty["skatteår"] = json!(2026);
    empty["gift_samlevende_ved_årets_udløb"] = json!(false);
    empty["indkomstbro"] = json!({
        "personlig_indkomst_kroner":0, "kapitalindkomst_kroner":0,
        "ligningsmæssige_fradrag_kroner":0, "skattepligtig_indkomst_kroner":0,
        "øvrige_indkomstreguleringer_kroner":0, "oplyst_ægtefælleunderskud_kroner":0
    });
    empty["ægtefællenedslag"] = json!({
        "statsligt_personfradrag_øre":0, "kommunalt_personfradrag_øre":0,
        "kirkeligt_personfradrag_øre":0, "negativ_kapitalindkomst_øre":0,
        "eget_negativ_kapitalindkomstnedslag_øre":0
    });
    empty["skat"] = json!({
        "poster_uden_ægtefællenedslag":[],
        "poster_uden_ægtefællenedslag_komplette":true,
        "oplyst_beregnet_skat_øre":0
    });
    empty["betaling"] = json!({
        "oplyst_forskudsskat_øre":0, "oplyst_beregnet_skat_øre":0,
        "oplyst_overskydende_skat_øre":0, "korrektioner_til_udbetaling":[],
        "korrektioner_komplette":true, "oplyst_udbetaling_kroner":0, "restskat":null
    });
    // These are positively confirmed fictional observations, never template defaults.
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    let mut add = |name: &str, status: &str, edit: &dyn Fn(&mut Value)| {
        let mut input = empty.clone();
        edit(&mut input);
        cases.push(json!({"case_id":name,"input":input}));
        expected.push((name.to_string(), status.to_string()));
    };
    add("confirmed-empty", "BetingetAfstemt", &|_| {});
    add("explicit-zero-post", "BetingetAfstemt", &|v| {
        v["skat"]["poster_uden_ægtefællenedslag"] =
            json!([{"navn":"Fiktiv bekræftet nulpost","beløb_øre":0}]);
    });
    add("unconfirmed-empty", "Ufuldstændig", &|v| {
        v["skat"]["poster_uden_ægtefællenedslag_komplette"] = json!(false);
    });
    add("confirmed-empty-nonzero-tax", "Modstrid", &|v| {
        v["skat"]["oplyst_beregnet_skat_øre"] = json!(1);
        v["betaling"]["oplyst_beregnet_skat_øre"] = json!(1);
        v["betaling"]["oplyst_forskudsskat_øre"] = json!(1);
    });
    add("confirmed-empty-unknown-tax", "Ufuldstændig", &|v| {
        v["skat"]["oplyst_beregnet_skat_øre"] = Value::Null;
        v["betaling"]["oplyst_beregnet_skat_øre"] = Value::Null;
    });
    let unknown_transfers = |v: &mut Value| {
        for value in v["ægtefællenedslag"].as_object_mut().unwrap().values_mut() {
            *value = Value::Null;
        }
    };
    add(
        "confirmed-empty-unknown-transfers",
        "Ufuldstændig",
        &unknown_transfers,
    );
    add(
        "confirmed-empty-unknown-transfers-nonzero-tax",
        "Modstrid",
        &|v| {
            unknown_transfers(v);
            v["skat"]["oplyst_beregnet_skat_øre"] = json!(1);
            v["betaling"]["oplyst_beregnet_skat_øre"] = json!(1);
            v["betaling"]["oplyst_forskudsskat_øre"] = json!(1);
        },
    );
    let debt = |v: &mut Value| {
        v["betaling"]["oplyst_overskydende_skat_øre"] = Value::Null;
        v["betaling"]["oplyst_udbetaling_kroner"] = Value::Null;
        v["betaling"]["restskat"] = json!({
            "oplyst_restskat_øre":0, "tillæg_til_slutskat":[],
            "tillæg_til_slutskat_komplette":true
        });
    };
    add("debt-confirmed-empty", "BetingetAfstemt", &debt);
    add("debt-unconfirmed-empty", "Ufuldstændig", &|v| {
        debt(v);
        v["skat"]["poster_uden_ægtefællenedslag_komplette"] = json!(false);
    });
    add("debt-confirmed-empty-nonzero-tax", "Modstrid", &|v| {
        debt(v);
        v["skat"]["oplyst_beregnet_skat_øre"] = json!(1);
        v["betaling"]["oplyst_beregnet_skat_øre"] = json!(1);
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = json!(1);
    });

    let mut template = invoke(&["template", MODEL, "--format", "json"]);
    assert_eq!(
        template["cases"][0]["input"]["skat"]["poster_uden_ægtefællenedslag_komplette"],
        false
    );
    template["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-empty-report-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let output = invoke(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expected.len());
    for (case, (name, status)) in results.iter().zip(expected) {
        assert_eq!(case["case_id"], name);
        assert_eq!(
            case["result"]["status"]["$variant"], status,
            "{name}: {case}"
        );
        assert_eq!(case["result"]["uafhængig_skatteberegning_udført"], false);
        assert!(!case["result"]["uafklaret"].as_array().unwrap().is_empty());
    }
    assert_eq!(results[0]["result"], results[1]["result"]);
    let get = |name: &str| &results.iter().find(|r| r["case_id"] == name).unwrap()["result"];
    let total = "Nødvendig samlet skattenedsættelse fra ægtefælle";
    let comparison = "Samlet ægtefællenedslag i beregnet skat";
    assert_eq!(
        condition(get("confirmed-empty"), total)["nødvendigt_beløb"],
        0
    );
    let unknown = control(get("confirmed-empty-unknown-transfers"), comparison);
    assert_eq!(unknown["forventet"], 0);
    assert_eq!(unknown["oplyst"], Value::Null);
    assert_eq!(unknown["difference"], Value::Null);
    assert_eq!(
        control(get("unconfirmed-empty"), comparison)["forventet"],
        Value::Null
    );
    let contradiction = condition(get("confirmed-empty-unknown-transfers-nonzero-tax"), total);
    assert_eq!(contradiction["nødvendigt_beløb"], -1);
    assert_eq!(contradiction["inden_for_kontrollerede_grænser"], false);
    assert_eq!(
        control(get("confirmed-empty-nonzero-tax"), comparison)["difference"],
        1
    );
}

#[test]
fn tax_owed_reports_reconcile_principal_without_certifying_collection() {
    let mut ordinary = baseline();
    ordinary["betaling"] = json!({
        "oplyst_forskudsskat_øre": 9000000,
        "oplyst_beregnet_skat_øre": 9820000,
        "oplyst_overskydende_skat_øre": null,
        "korrektioner_til_udbetaling": [],
        "korrektioner_komplette": false,
        "oplyst_udbetaling_kroner": null,
        "restskat": {
            "oplyst_restskat_øre": 820000,
            "tillæg_til_slutskat": [],
            "tillæg_til_slutskat_komplette": true
        }
    });
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    let mut add = |name: &str, status: &str, edit: &dyn Fn(&mut Value)| {
        let mut input = ordinary.clone();
        edit(&mut input);
        cases.push(json!({"case_id": name, "input": input}));
        expected.push((name.to_string(), status.to_string()));
    };
    for year in 2023..=2026 {
        add(&format!("debt-{year}"), "BetingetAfstemt", &|v| {
            v["skatteår"] = json!(year);
        });
    }
    add("debt-one-ore-error", "Modstrid", &|v| {
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = json!(820001);
    });
    add("debt-unknown-observation", "Ufuldstændig", &|v| {
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = Value::Null;
    });
    add("debt-unknown-prepaid", "Ufuldstændig", &|v| {
        v["betaling"]["oplyst_forskudsskat_øre"] = Value::Null;
    });
    add("debt-unknown-additions", "Ufuldstændig", &|v| {
        v["betaling"]["restskat"]["tillæg_til_slutskat_komplette"] = json!(false);
    });
    add("debt-carried-tax", "BetingetAfstemt", &|v| {
        v["betaling"]["restskat"]["tillæg_til_slutskat"] =
            json!([{"navn": "Oplyst overført restskat", "beløb_øre": 50000}]);
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = json!(870000);
    });
    for amount in [0, 1] {
        add(
            &format!("debt-boundary-{amount}"),
            "BetingetAfstemt",
            &|v| {
                v["betaling"]["oplyst_forskudsskat_øre"] = json!(9820000 - amount);
                v["betaling"]["restskat"]["oplyst_restskat_øre"] = json!(amount);
            },
        );
    }
    add("refund-in-debt-route", "Modstrid", &|v| {
        v["betaling"]["oplyst_forskudsskat_øre"] = json!(9820001);
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = json!(0);
    });
    add("refund-with-unknown-debt", "Modstrid", &|v| {
        v["betaling"]["oplyst_forskudsskat_øre"] = json!(9820001);
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = Value::Null;
    });
    add("negative-debt", "UgyldigtRapportinput", &|v| {
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = json!(-1);
    });
    add("mixed-payout", "UgyldigtRapportinput", &|v| {
        v["betaling"]["oplyst_udbetaling_kroner"] = json!(0);
    });
    add("mixed-surplus", "UgyldigtRapportinput", &|v| {
        v["betaling"]["oplyst_overskydende_skat_øre"] = json!(0);
    });
    add("mixed-corrections", "UgyldigtRapportinput", &|v| {
        v["betaling"]["korrektioner_til_udbetaling"] =
            json!([{"navn": "Tidligere udbetalt", "beløb_øre": -50000}]);
    });
    add("negative-addition", "UgyldigtRapportinput", &|v| {
        v["betaling"]["restskat"]["tillæg_til_slutskat"] =
            json!([{"navn": "Negativt tillæg", "beløb_øre": -1}]);
    });
    add("duplicate-additions", "UgyldigtRapportinput", &|v| {
        v["betaling"]["restskat"]["tillæg_til_slutskat"] = json!([
            {"navn": "Samme tillæg", "beløb_øre": 1},
            {"navn": " Samme tillæg ", "beløb_øre": 1}
        ]);
    });
    add("overflow-debt", "UgyldigtRapportinput", &|v| {
        v["betaling"]["restskat"]["oplyst_restskat_øre"] = json!(i64::MAX);
    });
    add("overflow-addition", "UgyldigtRapportinput", &|v| {
        v["betaling"]["restskat"]["tillæg_til_slutskat"] =
            json!([{"navn": "For stort tillæg", "beløb_øre": i64::MAX}]);
    });

    let mut template = invoke(&["template", MODEL, "--format", "json"]);
    assert_eq!(
        template["cases"][0]["input"]["betaling"]["restskat"],
        Value::Null
    );
    template["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-report-debt-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&template).unwrap()).unwrap();
    let output = invoke(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expected.len());
    for (case, (name, status)) in results.iter().zip(&expected) {
        let result = &case["result"];
        assert_eq!(case["case_id"], *name);
        assert_eq!(result["status"]["$variant"], *status, "{name}: {result}");
        assert_eq!(result["uafhængig_skatteberegning_udført"], false);
        if status != "UgyldigtRapportinput" {
            assert_eq!(result["kontroller"].as_array().unwrap().len(), 4);
            assert!(result["uafklaret"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| { value.as_str().unwrap().contains("Kun restskat før renter") }));
        }
    }
    let get = |name: &str| &results.iter().find(|r| r["case_id"] == name).unwrap()["result"];
    let name = "Restskat før renter og procenttillæg";
    assert_eq!(control(get("debt-2023"), name)["forventet"], 820000);
    assert_eq!(control(get("debt-carried-tax"), name)["forventet"], 870000);
    assert_eq!(control(get("debt-one-ore-error"), name)["difference"], 1);
    assert_eq!(
        control(get("debt-unknown-observation"), name)["forventet"],
        820000
    );
    assert_eq!(
        control(get("debt-unknown-observation"), name)["oplyst"],
        Value::Null
    );
    assert_eq!(
        control(get("debt-unknown-additions"), name)["forventet"],
        Value::Null
    );
    assert_eq!(
        control(get("debt-unknown-prepaid"), name)["forventet"],
        Value::Null
    );
    assert_eq!(
        condition(
            get("refund-with-unknown-debt"),
            "Ikke-negativ restskat før renter og procenttillæg"
        )["inden_for_kontrollerede_grænser"],
        false
    );
}
