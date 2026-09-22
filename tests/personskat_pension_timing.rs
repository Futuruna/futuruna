//! LL §9 L: current-year amounts, previous-year eligibility, public typed call.
//! All facts are fictional; no amounts are inferred from an official assessment.
use serde_json::{json, Value};
use std::process::Command;

const MODEL: &str = "examples/danish-income-tax/personskat.calculate.runa";

fn run(args: &[&str]) -> Value {
    let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("FUTURUNA_CALCULATION_JOBS", "1")
        .args(args)
        .output()
        .expect("run canonical pension calculation");
    assert!(
        output.status.success(),
        "{args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).expect("canonical JSON")
}

fn contribution() -> Value {
    json!({
        "identifikation":"fictional-private-rate",
        "ordning":{"$variant":"Pbl18Rateopsparing"},
        "indbetalingskilde":{"$variant":"Pbl18EgenIndbetaling"},
        "fradragsretshaver":{"$variant":"Pbl18OrdningensEjer"},
        "betaling":{
            "beløb_kroner":50000, "forfaldsår":2026, "betalingsår":2026,
            "betalt_senest_bankjusteret_1_april_efter_forfald":true,
            "hidrører_fra_par22e_tilbagebetaling":false,
            "par15a_fradragsplacering":{"$variant":"Pbl18IkkePar15APlacering"},
            "arbejdsmarkedsbidrag_kroner":0
        },
        "fordelingsforløb":{"$variant":"Pbl18IngenTiårsfordeling"},
        "indeksvalg":{"fradragsvalgte_kontraktbidrag_kroner":[]},
        "indeksordningsgrundlag":{"$variant":"Pbl18IkkeIndeksordning"},
        "forfaldne_ikke_tidligere_fratrukket_kroner":0,
        "særligt_ordningsgrundlag":{"$variant":"Pbl18IntetSærligtOrdningsgrundlag"},
        "begrænsninger":{
            "pbl54_personkreds_opfyldt":true,
            "afgiftspligt_for_hele_ordningen_indtrådt":false,
            "udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens":false
        }
    })
}

fn payout(id: &str, year: i64, amount: i64, exempt: bool) -> Value {
    json!({
        "identifikation":id, "indkomstår":year,
        "ordning":{"$variant":"Pbl20Rateopsparing"},
        "udbetalingsret":{"$variant":"Pbl20TilEjerEfterVilkår"},
        "ligningslov9l_art":{"$variant":if exempt {
            "Pbl20RateopsparingVedVarigtNedsatArbejdsevneMedFørtidsEllerSeniorpension"
        } else { "Pbl20OrdinærPensionsudbetaling" }},
        "bruttoudbetaling_kroner":amount,
        "del_fra_indbetalinger_før_1955_56_kroner":0,
        "dokumenteret_uden_fradrags_eller_bortseelsesret_kroner":0,
        "eu_eøs_erklæring":{"$variant":"Pbl20IngenEuEøsErklæring"},
        "udenlandsk_indkomstskat_kroner":0
    })
}

fn sport_payout(year: i64, amount: i64) -> Value {
    // One complete, one-year savings plan; no duplicate PBL20 input row.
    json!({"indkomstposter":[], "tidligere_indbetalinger":[],
        "ordninger":[{
            "identifikation":"fictional-sport", "oprettelsesår":2024,
            "art":{"$variant":"Pbl15BRateopsparing"}, "fødselsår":1990,
            "påtegnet_som_sportspension":true,
            "udbetalingsplan":{"$variant":"Pbl15BRateopsparingsplan",
                "startår":year, "aftalte_rateår":1,
                "ordningsværdi_ved_start_kroner":amount,
                "valgt_udbetalingsgrundlag_kroner":amount,
                "metode":{"$variant":"Pbl11APrimoVærdiDivideretMedResterendeÅr"}}
        }],
        "rateudbetalinger":[{
            "identifikation":"fictional-sport-rate", "ordning_identifikation":"fictional-sport",
            "indkomstår":year, "udbetalt_kroner":amount,
            "beregningsfakta":{"$variant":"Pbl15BRateopsparingEfterPar11A",
                "fakta":{"$variant":"Pbl11ARateEfterPrimoVærdi",
                    "ordningsværdi_ved_årets_begyndelse_kroner":amount}}
        }]
    })
}

#[test]
fn pension_deduction_uses_this_years_payouts_only_after_prior_eligible_payouts() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut input = envelope["cases"][0]["input"].clone();
    // Fictional resident employee, no church tax/spouse/other income/deductions.
    input["lønmodtager"]["skatteår"] = json!(2026);
    input["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    input["lønmodtager"]["kommune"] = json!({"$variant":"København"});
    input["lønmodtager"]["betaler_kirkeskat"] = json!(false);
    input["lønmodtager"]["ligningsfradrag"]["enlig_forsørger"] =
        json!({"$variant":"IntetEkstraBørnetilskud"});
    input["lønmodtager"]["ligningsfradrag"]["boligjob"] =
        json!({"$variant":"IngenBoligjobudgifter"});
    input["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
    input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([contribution()]);
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    // Independently derived from LL9L and L238 amendment notes pp5-6:
    // https://www.retsinformation.dk/api/pdf/201576
    // tuple = previous amount, current amount, previous/current exemption,
    //         extra deduction, effective current offset, current taxable income.
    for (name, prior, current, prior_exempt, current_exempt, extra, offset, taxable) in [
        ("stopped-payouts", 40000, 0, false, false, 6000, 0, 0),
        ("no-payouts", 0, 0, false, false, 6000, 0, 0),
        ("first-year", 0, 30000, false, false, 6000, 0, 30000),
        ("increased", 10000, 30000, false, false, 2400, 30000, 30000),
        ("decreased", 40000, 10000, false, false, 4800, 10000, 10000),
        ("unchanged", 20000, 20000, false, false, 3600, 20000, 20000),
        ("one-krone-trigger", 1, 50000, false, false, 0, 50000, 50000),
        ("zero-floor", 1, 50001, false, false, 0, 50001, 50001),
        ("prior-exempt", 40000, 30000, true, false, 6000, 0, 30000),
        ("current-exempt", 40000, 30000, false, true, 6000, 0, 30000),
        (
            "mixed-current",
            10000,
            30000,
            false,
            false,
            2400,
            30000,
            50000,
        ),
        (
            "partly-tax-exempt",
            10000,
            30000,
            false,
            false,
            3600,
            20000,
            20000,
        ),
        (
            "near-pension-age",
            10000,
            30000,
            false,
            false,
            6400,
            30000,
            30000,
        ),
        ("invalid-prior", 10000, 30000, false, false, 0, 0, 30000),
        (
            "unknown-deductions",
            10000,
            30000,
            false,
            false,
            0,
            30000,
            30000,
        ),
        ("sport-current", 10000, 0, false, false, 2400, 30000, 30000),
        ("sport-prior", 0, 30000, false, false, 2400, 30000, 30000),
        ("sport-first-year", 0, 0, false, false, 6000, 0, 30000),
    ] {
        let mut case = input.clone();
        let mut payouts = Vec::new();
        if prior > 0 {
            payouts.push(payout("different-prior-plan", 2025, prior, prior_exempt));
        }
        if current > 0 {
            payouts.push(payout(
                "different-current-plan",
                2026,
                current,
                current_exempt,
            ));
        }
        match name {
            "mixed-current" => payouts.push(payout("exempt-current", 2026, 20000, true)),
            "partly-tax-exempt" => {
                payouts[1]["dokumenteret_uden_fradrags_eller_bortseelsesret_kroner"] = json!(10000)
            }
            "near-pension-age" => case["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(1960),
            "invalid-prior" => payouts[0]["bruttoudbetaling_kroner"] = json!(-1),
            "unknown-deductions" => {
                case["lønmodtager"]["ligningsfradrag"]["boligjob"] =
                    json!({"$variant":"BoligjobUoplyst"})
            }
            "sport-current" | "sport-first-year" => {
                case["lønmodtager"]["pension"]["pbl15b_årsgrundlag"] = sport_payout(2026, 30000)
            }
            "sport-prior" => {
                case["lønmodtager"]["pension"]["pbl15b_årsgrundlag"] = sport_payout(2025, 10000)
            }
            _ => {}
        }
        case["lønmodtager"]["pension"]["øvrige_pbl20_årsgrundlag"]["udbetalinger"] = json!(payouts);
        cases.push(json!({"case_id":name, "input":case}));
        expected.push((name, extra, offset, taxable));
    }
    envelope["cases"] = json!(cases);
    let path = std::env::temp_dir().join(format!(
        "futuruna-pension-timing-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expected.len());
    for (row, (name, extra, offset, taxable)) in results.iter().zip(expected) {
        assert_eq!(row["case_id"], name);
        let result = &row["result"];
        let gate = &result["vurdering"];
        assert_eq!(gate["samlet_modeldækning_bekræftet"], false, "{name}");
        assert!(!gate["forbehold"].as_array().unwrap().is_empty(), "{name}");
        if matches!(name, "invalid-prior" | "unknown-deductions") {
            assert_eq!(gate["alle_kontroller_gyldige"], false, "{name}");
            assert_eq!(
                gate["status"]["$variant"], "UgyldigtBeregningsgrundlag",
                "{name}"
            );
            assert_eq!(
                gate["slutskat_til_sammenligning_øre"],
                Value::Null,
                "{name}"
            );
            if name == "invalid-prior" {
                assert_eq!(
                    result["pension"]["udbetalingsresultat"]["foregående_input_gyldige"],
                    false
                );
            }
            continue;
        }
        assert_eq!(gate["alle_kontroller_gyldige"], true, "{name}: {gate}");
        assert_eq!(gate["status"]["$variant"], "BeregnetMedForbehold", "{name}");
        assert!(gate["slutskat_til_sammenligning_øre"].is_i64(), "{name}");
        assert_eq!(
            result["pension"]["pbl18_årsresultat"]["rate_og_ophørende_fradrag_kroner"], 50000,
            "{name}"
        );
        assert_eq!(
            result["skat"]["ekstra_pensionsfradrag_kroner"], extra,
            "{name}"
        );
        assert_eq!(
            result["pension"]["udbetalingsresultat"]["modregnet_efter_ligningslov9l_i_året_kroner"],
            offset,
            "{name}"
        );
        assert_eq!(
            result["pension"]["lønmodtager_pensionsfradrag"]
                ["pbl20_indkomstskattepligtig_udbetaling_kroner"],
            offset,
            "{name}"
        );
        assert_eq!(
            result["pension"]["udbetalingsresultat"]
                ["samlet_personlig_indkomst_uden_nyt_arbejdsmarkedsbidrag_i_året_kroner"],
            taxable,
            "{name}"
        );
        assert_eq!(
            result["skat"]["personlig_indkomst_efter_am_kroner"],
            502000 + taxable,
            "{name}"
        );
        eprintln!("{name}: extra deduction {extra} kr.; current offset {offset} kr.; current taxable payout {taxable} kr.");
    }
}
