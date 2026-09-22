//! Pension deduction bases and payout timing through the public typed call.
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

fn fictional_input() -> (Value, Value) {
    let envelope = run(&["template", MODEL, "--format", "json"]);
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
    (envelope, input)
}

fn calculate(mut envelope: Value, cases: Vec<Value>) -> Vec<Value> {
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
    output["results"].as_array().unwrap().clone()
}

#[test]
fn employer_lifetime_and_rate_pensions_keep_gross_employment_basis() {
    let (envelope, mut baseline) = fictional_input();
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(200000);
    baseline["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    let mut cases = Vec::new();
    for name in [
        "employer-rate",
        "employer-lifetime",
        "private-lifetime",
        "previous-year-lifetime",
        "young-employer-lifetime",
        "split-employer-pension",
    ] {
        let mut input = baseline.clone();
        let mut payment = contribution();
        payment["identifikation"] = json!(name);
        payment["ordning"] = json!({"$variant": if name == "employer-rate" {
            "Pbl18Rateopsparing"
        } else { "Pbl18LivsvarigLivrente" }});
        payment["indbetalingskilde"] = json!({"$variant":"Pbl18Arbejdsgiverindbetaling"});
        payment["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(4000);
        match name {
            "private-lifetime" => {
                payment["indbetalingskilde"] = json!({"$variant":"Pbl18EgenIndbetaling"});
                payment["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(0);
                // Fictional continuing annual premium, no ten-year allocation.
                payment["forfaldne_ikke_tidligere_fratrukket_kroner"] = json!(50000);
            }
            "previous-year-lifetime" => {
                payment["betaling"]["forfaldsår"] = json!(2025);
                payment["betaling"]["betalingsår"] = json!(2025);
            }
            "young-employer-lifetime" => {
                input["lønmodtager"]["personfradrag_alder_status"] =
                    json!({"$variant":"Under18Ugift"});
                input["lønmodtager"]["pension"]["fødselsdato"] =
                    json!({"år":2009,"måned":1,"dag":1});
                payment["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(0);
            }
            "split-employer-pension" => {
                payment["betaling"]["beløb_kroner"] = json!(25000);
                payment["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(2000);
            }
            _ => {}
        }
        let mut payments = vec![payment.clone()];
        if name == "split-employer-pension" {
            payment["identifikation"] = json!("split-employer-rate");
            payment["ordning"] = json!({"$variant":"Pbl18Rateopsparing"});
            payments.push(payment);
        }
        input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!(payments);
        cases.push(json!({"case_id":name,"input":input}));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), 6);
    for row in &results {
        let result = &row["result"];
        let name = row["case_id"].as_str().unwrap();
        assert_eq!(
            result["vurdering"]["alle_kontroller_gyldige"], true,
            "{name}: {}",
            result["vurdering"]
        );
        assert!(result["vurdering"]["slutskat_til_sammenligning_øre"].is_i64());
        assert_eq!(result["vurdering"]["samlet_modeldækning_bekræftet"], false);
        let (employment, job, extra, wage_am, personal, lifetime_gross) = match name {
            "private-lifetime" => (25500, 0, 6000, 16000, 134000, 0),
            "previous-year-lifetime" => (25500, 0, 0, 16000, 184000, 0),
            "young-employer-lifetime" => (31875, 666, 6000, 0, 200000, 50000),
            "employer-rate" => (31875, 666, 5520, 16000, 184000, 0),
            "split-employer-pension" => (31875, 666, 5520, 16000, 184000, 25000),
            _ => (31875, 666, 5520, 16000, 184000, 50000),
        };
        // LL §§9 J/K use gross workplace contributions; §9 L uses after-AM.
        // https://www.lovtidende.dk/api/pdf/250970
        println!(
            "{}: employment={}, job={}, extra-pension={}, tax-øre={}",
            row["case_id"],
            result["skat"]["beskæftigelsesfradrag_kroner"],
            result["skat"]["jobfradrag_kroner"],
            result["skat"]["ekstra_pensionsfradrag_kroner"],
            result["vurdering"]["slutskat_til_sammenligning_øre"]
        );
        assert_eq!(
            result["skat"]["beskæftigelsesfradrag_kroner"], employment,
            "{}",
            row["case_id"]
        );
        assert_eq!(result["skat"]["jobfradrag_kroner"], job);
        assert_eq!(result["skat"]["ekstra_pensionsfradrag_kroner"], extra);
        assert_eq!(result["skat"]["arbejdsmarkedsbidrag_kroner"], wage_am);
        assert_eq!(
            result["skat"]["personlig_indkomst_efter_am_kroner"],
            personal
        );
        assert_eq!(
            result["pension"]["lønmodtager_pensionsfradrag"]
                ["øvrigt_arbejdsmarkedsbidragsgrundlag_med_indeholdt_bidrag_kroner"],
            lifetime_gross
        );
        if matches!(
            name,
            "employer-rate" | "employer-lifetime" | "split-employer-pension"
        ) {
            assert_eq!(
                result["vurdering"]["slutskat_til_sammenligning_øre"],
                5308213
            );
        }
    }
    assert_eq!(
        results[0]["result"]["vurdering"]["slutskat_til_sammenligning_øre"],
        results[1]["result"]["vurdering"]["slutskat_til_sammenligning_øre"]
    );
}

#[test]
fn young_worker_keeps_birth_date_before_pension_deductions() {
    let (envelope, mut input) = fictional_input();
    input["lønmodtager"]["bruttoløn_kroner"] = json!(100000);
    input["lønmodtager"]["personfradrag_alder_status"] = json!({"$variant":"Under18Ugift"});
    input["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":2009,"måned":12,"dag":31});
    input["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    let results = calculate(
        envelope,
        vec![json!({"case_id":"young-worker", "input":input})],
    );
    let result = &results[0]["result"];
    assert_eq!(result["vurdering"]["alle_kontroller_gyldige"], true);
    // LOV 96/2025 §1 and §7(4): zero AM throughout 2026 when turning 17.
    // https://www.retsinformation.dk/eli/lta/2025/96/pdf
    assert_eq!(result["skat"]["arbejdsmarkedsbidrag_kroner"], 0);
    assert_eq!(
        result["pension"]["pbl18_årsinput"]["personlig_indkomst_før_pensionsfradrag_kroner"],
        100000,
        "The preliminary pension basis must use the taxpayer's birth date, not a fictional adult"
    );
    assert_eq!(result["skat"]["personlig_indkomst_efter_am_kroner"], 50000);
}

#[test]
fn young_worker_year_and_birthday_boundaries_keep_the_income_basis() {
    let (envelope, baseline) = fictional_input();
    let specifications = [
        (
            "seventeen-before-reform",
            2025,
            2008,
            12,
            31,
            50000,
            8000,
            false,
        ),
        ("seventeen-january-birthday", 2026, 2009, 1, 1, 0, 0, false),
        (
            "eighteen-january-birthday",
            2026,
            2008,
            1,
            1,
            50000,
            8000,
            false,
        ),
        (
            "eighteen-december-birthday",
            2026,
            2008,
            12,
            31,
            50000,
            8000,
            false,
        ),
        ("adult-spouse", 2026, 2008, 1, 1, 50000, 8000, true),
    ];
    let mut cases = Vec::new();
    for (name, year, birth_year, month, day, paid, _, is_spouse) in specifications {
        let mut input = baseline.clone();
        input["lønmodtager"]["skatteår"] = json!(year);
        input["lønmodtager"]["bruttoløn_kroner"] = json!(100000);
        input["lønmodtager"]["personfradrag_alder_status"] = json!({"$variant":
            if year - birth_year < 18 { "Under18Ugift" } else { "Fyldt18EllerGift" }});
        input["lønmodtager"]["pension"]["fødselsdato"] =
            json!({"år":birth_year,"måned":month,"dag":day});
        input["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
            json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
        let mut payment = contribution();
        payment["betaling"]["beløb_kroner"] = json!(paid);
        payment["betaling"]["forfaldsår"] = json!(year);
        payment["betaling"]["betalingsår"] = json!(year);
        input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = if paid == 0 {
            json!([])
        } else {
            json!([payment])
        };
        if is_spouse {
            let mut facts = serde_json::Map::new();
            for field in [
                "lønmodtager",
                "kapitalindkomst",
                "aktieavance",
                "udenlandske_sociale_bidrag",
                "cfc",
                "skatteforhold",
                "underskudsforhold",
                "ejendomsskatter",
            ] {
                facts.insert(field.into(), input[field].clone());
            }
            input["ægtefælle"] = json!({"$variant":"MedÆgtefælle", "fakta":facts,
                "samlevende_ved_indkomstårets_udløb":true, "kildeskat25a_fordelinger":[]});
            input["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
        }
        cases.push(json!({"case_id":name,"input":input}));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), specifications.len());
    for (row, (name, year, _, _, _, paid, am, is_spouse)) in results.iter().zip(specifications) {
        assert_eq!(row["case_id"], name);
        let result = &row["result"];
        assert_eq!(
            result["vurdering"]["alle_kontroller_gyldige"], true,
            "{name}: {}",
            result["vurdering"]
        );
        assert_eq!(result["vurdering"]["samlet_modeldækning_bekræftet"], false);
        let pension = if is_spouse {
            &result["ægtefælle"]["grundlag"]["pension"]
        } else {
            &result["pension"]
        };
        assert_eq!(
            pension["pbl18_årsinput"]["personlig_indkomst_før_pensionsfradrag_kroner"],
            100000 - am,
            "{name}"
        );
        if !is_spouse {
            assert_eq!(result["skat"]["arbejdsmarkedsbidrag_kroner"], am, "{name}");
            assert_eq!(
                result["skat"]["personlig_indkomst_efter_am_kroner"],
                100000 - am - paid,
                "{name}"
            );
            // A zero contribution rate does not remove the wage income from
            // the employment-deduction basis (L117/2024-25 notes to §1).
            assert_eq!(
                result["skat"]["beskæftigelsesfradrag_kroner"],
                if year == 2025 { 12300 } else { 12750 },
                "{name}"
            );
        }
        eprintln!(
            "{name}: preliminary income {} kr.; AM {am} kr.; pension contribution {paid} kr.",
            100000 - am
        );
    }
}

#[test]
fn pension_deduction_uses_this_years_payouts_only_after_prior_eligible_payouts() {
    let (envelope, mut input) = fictional_input();
    input["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
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
    let results = calculate(envelope, cases);
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

#[test]
fn missing_pension_history_is_not_confirmed_zero() {
    let (envelope, input) = fictional_input();
    assert_eq!(
        input["lønmodtager"]["pension"]["udbetalingsoplysninger"],
        json!({"for_året_komplette":false,"for_foregående_år_komplette":false}),
        "Template completeness defaults must be unknown, not confirmed absence"
    );
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    // Complete current facts do not imply complete prior facts. Neither flag
    // replaces row validity or proves the underlying documents are true.
    for (
        name,
        current_complete,
        prior_complete,
        current_amount,
        prior_amount,
        prior_exempt,
        need_history,
        failure_suffix,
    ) in [
        (
            "unknown-current",
            false,
            false,
            0,
            0,
            false,
            false,
            Some("for_året_komplette"),
        ),
        ("known-no-current", true, false, 0, 0, false, false, None),
        (
            "unknown-prior",
            true,
            false,
            30000,
            0,
            false,
            true,
            Some("for_foregående_år_komplette"),
        ),
        (
            "confirmed-first-year",
            true,
            true,
            30000,
            0,
            false,
            true,
            None,
        ),
        (
            "known-prior-trigger",
            true,
            false,
            30000,
            1,
            false,
            false,
            None,
        ),
        (
            "exempt-prior-not-complete",
            true,
            false,
            30000,
            40000,
            true,
            true,
            Some("for_foregående_år_komplette"),
        ),
        (
            "exempt-prior-complete",
            true,
            true,
            30000,
            40000,
            true,
            true,
            None,
        ),
        (
            "current-exempt-history-irrelevant",
            true,
            false,
            30000,
            0,
            false,
            false,
            None,
        ),
        (
            "no-contribution-history-irrelevant",
            true,
            false,
            30000,
            0,
            false,
            false,
            None,
        ),
        (
            "cap-history-irrelevant",
            true,
            false,
            10000,
            0,
            false,
            false,
            None,
        ),
        (
            "sport-history-needed",
            true,
            false,
            30000,
            0,
            false,
            true,
            Some("for_foregående_år_komplette"),
        ),
        (
            "spouse-history-needed",
            true,
            false,
            30000,
            0,
            false,
            true,
            Some("for_foregående_år_komplette"),
        ),
        (
            "audit-2025",
            true,
            false,
            30000,
            0,
            false,
            true,
            Some("for_foregående_år_komplette"),
        ),
        (
            "partial-current-with-trigger",
            false,
            false,
            30000,
            1,
            false,
            false,
            Some("for_året_komplette"),
        ),
    ] {
        let mut case = input.clone();
        let pension = &mut case["lønmodtager"]["pension"];
        pension["udbetalingsoplysninger"] = json!({
            "for_året_komplette":current_complete,"for_foregående_år_komplette":prior_complete});
        let mut rows = Vec::new();
        if current_amount > 0 {
            rows.push(payout(
                "current",
                2026,
                current_amount,
                name == "current-exempt-history-irrelevant",
            ));
        }
        if prior_amount > 0 {
            rows.push(payout("prior", 2025, prior_amount, prior_exempt));
        }
        pension["øvrige_pbl20_årsgrundlag"]["udbetalinger"] = json!(rows);
        match name {
            "no-contribution-history-irrelevant" => pension["pbl18_indbetalinger"] = json!([]),
            "cap-history-irrelevant" => {
                // 50k private rate + 55.2k employer lifelong after AM: both
                // 105.2k and (105.2k-10k) exceed the2026 LL9L base cap87,800.
                let mut employer = contribution();
                employer["identifikation"] = json!("fictional-employer-lifelong");
                employer["indbetalingskilde"] = json!({"$variant":"Pbl18Arbejdsgiverindbetaling"});
                employer["ordning"] = json!({"$variant":"Pbl18LivsvarigLivrente"});
                employer["betaling"]["beløb_kroner"] = json!(60000);
                employer["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(4800);
                pension["pbl18_indbetalinger"]
                    .as_array_mut()
                    .unwrap()
                    .push(employer);
            }
            "sport-history-needed" => {
                pension["øvrige_pbl20_årsgrundlag"]["udbetalinger"] = json!([]);
                pension["pbl15b_årsgrundlag"] = sport_payout(2026, 30000);
            }
            "audit-2025" => {
                pension["pbl18_indbetalinger"][0]["betaling"]["forfaldsår"] = json!(2025);
                pension["pbl18_indbetalinger"][0]["betaling"]["betalingsår"] = json!(2025);
                pension["øvrige_pbl20_årsgrundlag"]["udbetalinger"][0]["indkomstår"] = json!(2025);
                case["lønmodtager"]["skatteår"] = json!(2025);
            }
            _ => {}
        }
        let spouse = name == "spouse-history-needed";
        if spouse {
            let facts: serde_json::Map<String, Value> = [
                "lønmodtager",
                "kapitalindkomst",
                "aktieavance",
                "udenlandske_sociale_bidrag",
                "cfc",
                "skatteforhold",
                "underskudsforhold",
                "ejendomsskatter",
            ]
            .into_iter()
            .map(|key| (key.into(), case[key].clone()))
            .collect();
            case = input.clone();
            case["lønmodtager"]["pension"]["udbetalingsoplysninger"]["for_året_komplette"] =
                json!(true);
            case["ægtefælle"] = json!({"$variant":"MedÆgtefælle", "fakta":facts,
                "samlevende_ved_indkomstårets_udløb":true,"kildeskat25a_fordelinger":[]});
        }
        cases.push(json!({"case_id":name,"input":case}));
        expected.push((name, need_history, failure_suffix, spouse, prior_complete));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), expected.len());
    for (row, (name, need_history, failure_suffix, spouse, prior_complete)) in
        results.iter().zip(expected)
    {
        let result = &row["result"];
        assert_eq!(row["case_id"], name);
        let gate = &result["vurdering"];
        let pension = if spouse {
            &result["ægtefælle"]["grundlag"]["pension"]
        } else {
            &result["pension"]
        };
        assert_eq!(
            pension["oplysningsstatus"]["foregående_oplysninger_nødvendige"], need_history,
            "{name}"
        );
        assert_eq!(
            pension["oplysningsstatus"]["foregående_oplysninger_komplette"], prior_complete,
            "{name}: enough information must not be relabelled complete history"
        );
        assert_eq!(gate["samlet_modeldækning_bekræftet"], false, "{name}");
        if name == "cap-history-irrelevant" {
            assert_eq!(result["skat"]["ekstra_pensionsfradrag_kroner"], 10536);
        }
        if let Some(suffix) = failure_suffix {
            let prefix = if spouse {
                "ægtefælle.MedÆgtefælle.fakta."
            } else {
                ""
            };
            let path = format!("{prefix}lønmodtager.pension.udbetalingsoplysninger.{suffix}");
            assert_eq!(gate["alle_kontroller_gyldige"], false, "{name}: {gate}");
            assert_eq!(
                gate["slutskat_til_sammenligning_øre"],
                Value::Null,
                "{name}"
            );
            assert!(
                gate["fejl"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["sti"] == path),
                "{name}: {gate}"
            );
        } else {
            assert_eq!(gate["alle_kontroller_gyldige"], true, "{name}: {gate}");
            assert!(gate["slutskat_til_sammenligning_øre"].is_i64(), "{name}");
        }
        eprintln!(
            "{name}: prior history can still matter={need_history}; comparison allowed={}",
            failure_suffix.is_none()
        );
    }
}
