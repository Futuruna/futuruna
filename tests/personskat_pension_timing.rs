//! Pension deduction bases and payout timing through the public typed call.
//! All facts are fictional; no amounts are inferred from an official assessment.
use serde_json::{json, Value};
use std::process::Command;
#[path = "support/foreign_employment.rs"]
mod foreign_employment;

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

fn employer_benefit_pension(id: &str, plan: &str, gross: Option<i64>, net: i64) -> Value {
    json!({"identifikation":id,"indkomstår":2026,
    "ydelse":{"$variant":"ArbejdsgiveradministreretPensionEfterPbl19","fakta":{
        "arbejdsgiverrelation":{"$variant":"Pbl19NuværendeArbejdsgiver"},
        "ordning":{"$variant":plan},
        "indberettet_indbetaling_før_indeholdt_arbejdsmarkedsbidrag_kroner":gross,
        "indberettet_indbetaling_efter_indeholdt_arbejdsmarkedsbidrag_kroner":net,
        "kapitalordningsstatus":{"$variant":"Pbl19IngenTidligereAfgiftsberigtigelse"}
    }}})
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
    input["lønmodtager"]["ligningsfradrag"]["arbejdsfradrag_udland"] =
        foreign_employment::no_exclusion();
    input["lønmodtager"]["ligningsfradrag"]["boligjob"] =
        json!({"$variant":"IngenBoligjobudgifter"});
    input["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1990,"måned":1,"dag":1});
    input["lønmodtager"]["pension"]["atp"] = json!({"$variant":"IngenAtpIndbetalinger"});
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
fn atp_template_distinguishes_unknown_from_confirmed_absence() {
    let template = run(&["template", MODEL, "--format", "json"]);
    assert_eq!(
        template["cases"][0]["input"]["lønmodtager"]["pension"]["atp"]["$variant"],
        "AtpUoplyst"
    );
}

#[test]
fn atp_source_bases_preserve_income_and_distinguish_public_contributions() {
    let (envelope, mut baseline) = fictional_input();
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(200000);
    baseline["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([]);
    baseline["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    let names = [
        "none",
        "employer",
        "public-benefit",
        "supp",
        "mandatory",
        "mixed",
        "job-threshold",
        "young",
        "senior",
        "with-private-rate",
        "payout-offset",
        "2025",
    ];
    let mut cases = Vec::new();
    for name in names {
        let mut input = baseline.clone();
        let mut row = atp_payment("atp-payment", "AtpArbejdsgiverPar19Stk1", 5000, Some(4600));
        match name {
            "public-benefit" => row["grundlag"] = json!({"$variant":"AtpOffentligYdelsePar19Stk2"}),
            "supp" => {
                row["grundlag"] = json!({"$variant":"AtpSupplerendeArbejdsmarkedspensionPar19Stk4"})
            }
            "mandatory" => {
                row["grundlag"] = json!({"$variant":"AtpObligatoriskPensionPar19Stk4"});
                row["indberettet_efter_am_kroner"] = json!(5000);
            }
            "job-threshold" => input["lønmodtager"]["bruttoløn_kroner"] = json!(235000),
            "young" => {
                input["lønmodtager"]["personfradrag_alder_status"] =
                    json!({"$variant":"Under18Ugift"});
                input["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(2009);
                row["indberettet_efter_am_kroner"] = json!(5000);
            }
            "senior" => input["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(1960),
            "with-private-rate" => {
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([contribution()])
            }
            "payout-offset" => {
                input["lønmodtager"]["pension"]["øvrige_pbl20_årsgrundlag"]["udbetalinger"] =
                    json!([
                        payout("prior", 2025, 1000, false),
                        payout("current", 2026, 3000, false)
                    ])
            }
            "2025" => {
                input["lønmodtager"]["skatteår"] = json!(2025);
                row["indkomstår"] = json!(2025);
            }
            _ => {}
        }
        let mut rows = vec![row];
        if name == "mixed" {
            rows.push(atp_payment(
                "public",
                "AtpOffentligYdelsePar19Stk2",
                5000,
                Some(4600),
            ));
            rows.push(atp_payment(
                "op",
                "AtpObligatoriskPensionPar19Stk4",
                5000,
                Some(5000),
            ));
        }
        if name != "none" {
            input["lønmodtager"]["pension"]["atp"] = json!({"$variant":"OplysteAtpIndbetalinger","oplysninger_for_året_komplette":true,"poster":rows});
        }
        cases.push(json!({"case_id":name,"input":input}));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), names.len());
    for (row, name) in results.iter().zip(names) {
        let result = &row["result"];
        assert_eq!(
            result["vurdering"]["alle_kontroller_gyldige"], true,
            "{name}: {}",
            result["vurdering"]
        );
        assert_eq!(result["vurdering"]["samlet_modeldækning_bekræftet"], false);
        let atp = &result["pension"]["atp_resultat"];
        let work = if matches!(name, "none" | "public-benefit" | "supp" | "mandatory") {
            0
        } else {
            5000
        };
        let net = match name {
            "none" => 0,
            "mandatory" | "young" => 5000,
            "mixed" => 14200,
            _ => 4600,
        };
        assert_eq!(atp["arbejdsfradragsgrundlag_kroner"], work, "{name}");
        assert_eq!(atp["ekstra_pensionsfradragsgrundlag_kroner"], net, "{name}");
        let wage = if name == "job-threshold" {
            235000
        } else {
            200000
        };
        let am = if name == "young" { 0 } else { wage * 8 / 100 };
        let private = if name == "with-private-rate" {
            50000
        } else {
            0
        };
        let taxable_payout = if name == "payout-offset" { 3000 } else { 0 };
        let extra = (net + private - taxable_payout) * if name == "senior" { 32 } else { 12 } / 100;
        assert_eq!(result["skat"]["bruttoløn_kroner"], wage, "{name}");
        assert_eq!(result["skat"]["arbejdsmarkedsbidrag_kroner"], am, "{name}");
        assert_eq!(
            result["skat"]["personlig_indkomst_efter_am_kroner"],
            wage - am - private + taxable_payout,
            "{name}"
        );
        assert_eq!(
            result["skat"]["beskæftigelsesfradrag_kroner"],
            (wage + work) * if name == "2025" { 1230 } else { 1275 } / 10000,
            "{name}"
        );
        assert_eq!(
            result["skat"]["jobfradrag_kroner"],
            if name == "job-threshold" { 216 } else { 0 },
            "{name}"
        );
        assert_eq!(
            result["skat"]["seniorbeskæftigelsesfradrag_kroner"],
            if name == "senior" { 2870 } else { 0 },
            "{name}"
        );
        assert_eq!(
            result["skat"]["ekstra_pensionsfradrag_kroner"], extra,
            "{name}"
        );
        assert_eq!(
            result["pension"]["pbl18_årsresultat"]["rate_og_ophørende_fradrag_kroner"], private,
            "{name}"
        );
        // ATP never consumes the ordinary rate-pension cap.
        assert_eq!(
            result["pension"]["arbejdsgiver_rate_resultat"]["indbetaling_efter_am_kroner"], 0,
            "{name}"
        );
        if name == "none" {
            assert_eq!(
                result["vurdering"]["slutskat_til_sammenligning_øre"],
                5602015
            );
        }
        println!(
            "ATP {name}: employment={}, extra={extra}, tax-øre={}",
            result["skat"]["beskæftigelsesfradrag_kroner"],
            result["vurdering"]["slutskat_til_sammenligning_øre"]
        );
    }
}

fn atp_payment(id: &str, basis: &str, gross: i64, net: Option<i64>) -> Value {
    json!({"identifikation":id,"indkomstår":2026,"kildereference":"fictional-ATP-record",
        "grundlag":{"$variant":basis},"indberettet_før_am_kroner":gross,"indberettet_efter_am_kroner":net})
}

#[test]
fn atp_unknown_incomplete_and_conflicting_sources_withhold_comparison() {
    let (envelope, mut baseline) = fictional_input();
    baseline["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([]);
    baseline["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    let specs = [
        ("unknown", "atp"),
        ("incomplete", "oplysninger_for_året_komplette"),
        ("empty", "poster"),
        ("net-unknown", "indberettet_efter_am_kroner"),
        ("mandatory-with-am", "indberettet_efter_am_kroner"),
        ("duplicate", "identifikation"),
        ("cross-route-duplicate", "identifikation"),
        ("wrong-year", "poster"),
        ("unsupported", "grundlag"),
        ("spouse-unknown", "atp"),
        ("history-needed", "for_foregående_år_komplette"),
    ];
    let mut cases = Vec::new();
    for (name, _) in specs {
        let mut input = baseline.clone();
        let mut post = atp_payment("atp-payment", "AtpArbejdsgiverPar19Stk1", 5000, Some(4600));
        match name {
            "net-unknown" => post["indberettet_efter_am_kroner"] = Value::Null,
            "mandatory-with-am" => {
                post["grundlag"] = json!({"$variant":"AtpObligatoriskPensionPar19Stk4"})
            }
            "wrong-year" => post["indkomstår"] = json!(2025),
            "unsupported" => post["grundlag"] = json!({"$variant":"AtpAndetEllerUoplystGrundlag"}),
            "cross-route-duplicate" => {
                let mut private = contribution();
                private["identifikation"] = json!("atp-payment");
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([private]);
            }
            "history-needed" => {
                input["lønmodtager"]["pension"]["udbetalingsoplysninger"]
                    ["for_foregående_år_komplette"] = json!(false);
                input["lønmodtager"]["pension"]["øvrige_pbl20_årsgrundlag"]["udbetalinger"] =
                    json!([payout("current", 2026, 3000, false)]);
            }
            _ => {}
        }
        let rows = match name {
            "empty" => vec![],
            "duplicate" => vec![post.clone(), post],
            _ => vec![post],
        };
        input["lønmodtager"]["pension"]["atp"] = if name.ends_with("unknown")
            && name != "net-unknown"
        {
            json!({"$variant":"AtpUoplyst"})
        } else {
            json!({"$variant":"OplysteAtpIndbetalinger","oplysninger_for_året_komplette":name != "incomplete","poster":rows})
        };
        if name == "spouse-unknown" {
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
            .map(|key| (key.into(), input[key].clone()))
            .collect();
            input = baseline.clone();
            input["ægtefælle"] = json!({"$variant":"MedÆgtefælle","fakta":facts,"samlevende_ved_indkomstårets_udløb":true,"kildeskat25a_fordelinger":[]});
        }
        cases.push(json!({"case_id":name,"input":input}));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), specs.len());
    for (row, (name, suffix)) in results.iter().zip(specs) {
        let result = &row["result"];
        let gate = &result["vurdering"];
        assert_eq!(gate["alle_kontroller_gyldige"], false, "{name}");
        assert_eq!(
            gate["slutskat_til_sammenligning_øre"],
            Value::Null,
            "{name}"
        );
        assert!(
            gate["fejl"].as_array().unwrap().iter().any(|error| {
                let path = error["sti"].as_str().unwrap();
                path.ends_with(suffix)
                    && (name != "spouse-unknown"
                        || path.starts_with("ægtefælle.MedÆgtefælle.fakta."))
            }),
            "{name}: {gate}"
        );
        if name == "history-needed" {
            assert_eq!(
                result["pension"]["oplysningsstatus"]["foregående_oplysninger_nødvendige"],
                true
            );
        }
        if name == "net-unknown" {
            assert_eq!(
                result["pension"]["atp_resultat"]["input"]["poster"][0]
                    ["indberettet_efter_am_kroner"],
                Value::Null
            );
        }
        println!("ATP {name}: comparison withheld at {suffix}");
    }
}

#[test]
fn employer_pension_routes_share_deductions_caps_and_private_priority() {
    let (envelope, mut baseline) = fictional_input();
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(200000);
    baseline["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    baseline["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([]);
    let names = [
        "alternate-lifetime",
        "alternate-rate",
        "alternate-over-cap",
        "mixed-rate-lifetime",
        "mixed-rate-private",
        "alternate-private-boundary",
        "canonical-private-boundary",
        "young-alternate-lifetime",
        "taxable-aldersopsparing",
    ];
    let mut cases = Vec::new();
    for name in names {
        let mut input = baseline.clone();
        let mut employer = contribution();
        employer["identifikation"] = json!("canonical-employer");
        employer["indbetalingskilde"] = json!({"$variant":"Pbl18Arbejdsgiverindbetaling"});
        employer["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(4000);
        let mut private = contribution();
        private["betaling"]["beløb_kroner"] = json!(22701);
        let (plan, gross, net) = match name {
            "alternate-lifetime" => ("Pbl18LivsvarigLivrente", 50000, 46000),
            "alternate-over-cap" => ("Pbl18Rateforsikring", 100000, 92000),
            "mixed-rate-lifetime" => {
                employer["betaling"]["beløb_kroner"] = json!(25000);
                employer["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(2000);
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([employer]);
                ("Pbl18LivsvarigLivrente", 25000, 23000)
            }
            "mixed-rate-private" => {
                private["betaling"]["beløb_kroner"] = json!(5000);
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([employer, private]);
                ("Pbl18OphørendeLivrente", 50000, 46000)
            }
            "alternate-private-boundary" => {
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([private]);
                ("Pbl18Rateopsparing", 50000, 46000)
            }
            "canonical-private-boundary" => {
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([employer, private]);
                ("Pbl18Rateopsparing", 0, 0)
            }
            "young-alternate-lifetime" => {
                input["lønmodtager"]["personfradrag_alder_status"] =
                    json!({"$variant":"Under18Ugift"});
                input["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(2009);
                ("Pbl18LivsvarigLivrente", 50000, 50000)
            }
            "taxable-aldersopsparing" => ("Pbl18Aldersopsparing", 990, 911),
            _ => ("Pbl18Rateopsparing", 50000, 46000),
        };
        if name != "canonical-private-boundary" {
            input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]["arbejdsgiverydelser"] =
                json!([employer_benefit_pension(name, plan, Some(gross), net)]);
        }
        cases.push(json!({"case_id":name,"input":input}));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), names.len());
    for (row, name) in results.iter().zip(names) {
        let result = &row["result"];
        assert_eq!(
            result["vurdering"]["alle_kontroller_gyldige"], true,
            "{name}: {}",
            result["vurdering"]
        );
        assert_eq!(result["vurdering"]["samlet_modeldækning_bekræftet"], false);
        let bridge = &result["pension"]["arbejdsgiverydelser_resultat"];
        assert_eq!(bridge["alle_input_gyldige"], true);
        assert_eq!(
            bridge["kildeposter"].as_array().unwrap().len(),
            usize::from(name != "canonical-private-boundary")
        );
        let (employment, job, extra, private, personal, tax) = match name {
            "alternate-over-cap" | "mixed-rate-private" => {
                (38250, 2916, 8244, 0, 207300, Some(5867580))
            }
            "alternate-private-boundary" | "canonical-private-boundary" => {
                (31875, 666, 8244, 22700, 161300, None)
            }
            "young-alternate-lifetime" => (31875, 666, 6000, 0, 200000, Some(4263386)),
            "taxable-aldersopsparing" => (25626, 0, 0, 0, 184911, None),
            _ => (31875, 666, 5520, 0, 184000, Some(5308213)),
        };
        assert_eq!(
            result["skat"]["beskæftigelsesfradrag_kroner"], employment,
            "{name}"
        );
        assert_eq!(result["skat"]["jobfradrag_kroner"], job, "{name}");
        assert_eq!(
            result["skat"]["ekstra_pensionsfradrag_kroner"], extra,
            "{name}"
        );
        assert_eq!(
            result["skat"]["personlig_indkomst_efter_am_kroner"], personal,
            "{name}"
        );
        assert_eq!(
            result["skat"]["arbejdsmarkedsbidrag_kroner"],
            if name == "young-alternate-lifetime" {
                0
            } else {
                16000
            },
            "{name}"
        );
        assert_eq!(
            result["pension"]["pbl18_årsresultat"]["rate_og_ophørende_fradrag_kroner"], private,
            "{name}"
        );
        if let Some(tax) = tax {
            assert_eq!(
                result["vurdering"]["slutskat_til_sammenligning_øre"], tax,
                "{name}"
            );
        }
        if name.ends_with("private-boundary") {
            assert_eq!(
                result["pension"]["pbl18_årsresultat"]
                    ["egne_indbetalinger_ikke_fratrukket_i_indkomståret_kroner"],
                1
            );
        }
        println!(
            "{name}: employment={employment}, extra={extra}, private={private}, tax-øre={}",
            result["vurdering"]["slutskat_til_sammenligning_øre"]
        );
    }
    // Same payment facts in either route produce identical final taxes and
    // private cap; the original source lists remain distinct in the trace.
    assert_eq!(results[5]["result"]["skat"], results[6]["result"]["skat"]);
    assert_eq!(
        results[5]["result"]["vurdering"]["slutskat_til_sammenligning_øre"],
        results[6]["result"]["vurdering"]["slutskat_til_sammenligning_øre"]
    );
}

#[test]
fn employer_pension_routes_reject_missing_facts_and_duplicate_payments() {
    let (envelope, mut baseline) = fictional_input();
    baseline["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    baseline["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([]);
    let specs = [
        (
            "missing-gross",
            "indberettet_indbetaling_før_indeholdt_arbejdsmarkedsbidrag_kroner",
        ),
        ("duplicate-across-routes", "identifikation"),
        ("unsupported-special-plan", "ordning"),
        ("gross-below-net", "arbejdsgiverydelser"),
        ("wrong-year", "arbejdsgiverydelser"),
        ("duplicate-in-route", "arbejdsgiverydelser"),
        (
            "spouse-missing-gross",
            "indberettet_indbetaling_før_indeholdt_arbejdsmarkedsbidrag_kroner",
        ),
    ];
    let mut cases = Vec::new();
    for (name, _) in specs {
        let mut input = baseline.clone();
        let mut post = employer_benefit_pension(
            "source-payment",
            "Pbl18LivsvarigLivrente",
            Some(50000),
            46000,
        );
        match name {
            "missing-gross" | "spouse-missing-gross" => {
                post["ydelse"]["fakta"]
                    ["indberettet_indbetaling_før_indeholdt_arbejdsmarkedsbidrag_kroner"] =
                    Value::Null
            }
            "gross-below-net" => {
                post["ydelse"]["fakta"]
                    ["indberettet_indbetaling_før_indeholdt_arbejdsmarkedsbidrag_kroner"] =
                    json!(45000)
            }
            "unsupported-special-plan" => {
                post["ydelse"]["fakta"]["ordning"] = json!({"$variant":"Pbl18Indeksordning"})
            }
            "wrong-year" => post["indkomstår"] = json!(2025),
            "duplicate-across-routes" => {
                let mut payment = contribution();
                payment["identifikation"] = json!("source-payment");
                payment["ordning"] = json!({"$variant":"Pbl18LivsvarigLivrente"});
                payment["indbetalingskilde"] = json!({"$variant":"Pbl18Arbejdsgiverindbetaling"});
                payment["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(4000);
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([payment]);
            }
            _ => {}
        }
        input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]["arbejdsgiverydelser"] =
            if name == "duplicate-in-route" {
                json!([post.clone(), post])
            } else {
                json!([post])
            };
        if name == "spouse-missing-gross" {
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
            .map(|key| (key.into(), input[key].clone()))
            .collect();
            input = baseline.clone();
            input["ægtefælle"] = json!({"$variant":"MedÆgtefælle","fakta":facts,"samlevende_ved_indkomstårets_udløb":true,"kildeskat25a_fordelinger":[]});
        }
        cases.push(json!({"case_id":name,"input":input}));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), specs.len());
    for (row, (name, suffix)) in results.iter().zip(specs) {
        let gate = &row["result"]["vurdering"];
        assert_eq!(gate["alle_kontroller_gyldige"], false, "{name}");
        assert_eq!(
            gate["slutskat_til_sammenligning_øre"],
            Value::Null,
            "{name}"
        );
        let prefix = if name.starts_with("spouse-") {
            "ægtefælle.MedÆgtefælle.fakta."
        } else {
            ""
        };
        assert!(
            gate["fejl"].as_array().unwrap().iter().any(|error| {
                let path = error["sti"].as_str().unwrap();
                path.starts_with(prefix) && path.ends_with(suffix)
            }),
            "{name}: {gate}"
        );
        if name == "missing-gross" {
            assert_eq!(
                row["result"]["pension"]["arbejdsgiverydelser_resultat"]["kildeposter"][0]
                    ["ydelse"]["fakta"]
                    ["indberettet_indbetaling_før_indeholdt_arbejdsmarkedsbidrag_kroner"],
                Value::Null
            );
        }
        println!("{name}: comparison withheld, actionable source-path diagnostic");
    }
}

#[test]
fn employer_rate_above_cap_is_taxable_without_second_am_charge() {
    let (envelope, mut input) = fictional_input();
    input["lønmodtager"]["bruttoløn_kroner"] = json!(200000);
    input["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    let mut payment = contribution();
    payment["indbetalingskilde"] = json!({"$variant":"Pbl18Arbejdsgiverindbetaling"});
    payment["betaling"]["beløb_kroner"] = json!(100000);
    payment["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(8000);
    input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([payment]);
    let results = calculate(
        envelope,
        vec![json!({"case_id":"employer-over-cap","input":input})],
    );
    let result = &results[0]["result"];
    assert_eq!(result["vurdering"]["alle_kontroller_gyldige"], true);
    println!(
        "extra={}, personal={}, AM={}, employment={}, job={}, tax-øre={}",
        result["skat"]["ekstra_pensionsfradrag_kroner"],
        result["skat"]["personlig_indkomst_efter_am_kroner"],
        result["skat"]["arbejdsmarkedsbidrag_kroner"],
        result["skat"]["beskæftigelsesfradrag_kroner"],
        result["skat"]["jobfradrag_kroner"],
        result["vurdering"]["slutskat_til_sammenligning_øre"]
    );
    // 92000 after AM, 68700 exempt, 23300 taxable (SKAT box 347).
    // LL9L: 68700 * 12%; the excess has no further pension deduction.
    assert_eq!(result["skat"]["ekstra_pensionsfradrag_kroner"], 8244);
    assert_eq!(result["skat"]["personlig_indkomst_efter_am_kroner"], 207300);
    assert_eq!(
        result["pension"]["pbl18_årsinput"]["personlig_indkomst_før_pensionsfradrag_kroner"],
        207300
    );
    assert_eq!(result["skat"]["arbejdsmarkedsbidrag_kroner"], 16000);
    assert_eq!(result["skat"]["beskæftigelsesfradrag_kroner"], 38250);
    assert_eq!(result["skat"]["jobfradrag_kroner"], 2916);
    assert_eq!(
        result["pension"]["arbejdsgiver_rate_resultat"],
        json!({
            "indkomstår":2026, "input_gyldigt":true,
            "indbetaling_før_am_kroner":100000, "indeholdt_am_kroner":8000,
            "indbetaling_efter_am_kroner":92000, "fælles_rateloft_kroner":68700,
            "bortseelsesberettiget_efter_am_kroner":68700,
            "skattepligtig_personlig_indkomst_uden_nyt_am_kroner":23300
        })
    );
}

#[test]
fn employer_rate_year_cap_boundary_and_private_priority() {
    let (envelope, mut baseline) = fictional_input();
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(200000);
    baseline["lønmodtager"]["pension"]["udbetalingsoplysninger"] =
        json!({"for_året_komplette":true,"for_foregående_år_komplette":true});
    let specs = [
        ("2025-cap", 2025, 100000, 8000, 65500, false),
        ("zero-am-at-cap", 2026, 68700, 0, 68700, false),
        ("zero-am-one-over", 2026, 68701, 0, 68700, false),
        ("split-and-private", 2026, 100000, 8000, 68700, true),
    ];
    let mut cases = Vec::new();
    for (name, year, gross, am, _, split) in specs {
        let mut input = baseline.clone();
        input["lønmodtager"]["skatteår"] = json!(year);
        if am == 0 {
            input["lønmodtager"]["personfradrag_alder_status"] = json!({"$variant":"Under18Ugift"});
            input["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":2009,"måned":1,"dag":1});
        }
        let mut payment = contribution();
        payment["identifikation"] = json!(name);
        payment["indbetalingskilde"] = json!({"$variant":"Pbl18Arbejdsgiverindbetaling"});
        payment["betaling"]["beløb_kroner"] = json!(if split { gross / 2 } else { gross });
        payment["betaling"]["arbejdsmarkedsbidrag_kroner"] = json!(if split { am / 2 } else { am });
        payment["betaling"]["forfaldsår"] = json!(year);
        payment["betaling"]["betalingsår"] = json!(year);
        if year == 2025 {
            payment["ordning"] = json!({"$variant":"Pbl18Rateforsikring"});
        }
        let mut payments = vec![payment.clone()];
        if split {
            payment["identifikation"] = json!("second-employer-terminating-annuity");
            payment["ordning"] = json!({"$variant":"Pbl18OphørendeLivrente"});
            payments.push(payment);
            let mut private = contribution();
            private["betaling"]["beløb_kroner"] = json!(5000);
            payments.push(private);
        }
        input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!(payments);
        cases.push(json!({"case_id":name,"input":input}));
    }
    let results = calculate(envelope, cases);
    assert_eq!(results.len(), specs.len());
    for (row, (name, year, gross, am, cap, split)) in results.iter().zip(specs) {
        assert_eq!(row["case_id"], name);
        let result = &row["result"];
        assert_eq!(
            result["vurdering"]["alle_kontroller_gyldige"], true,
            "{name}: {}",
            result["vurdering"]
        );
        assert!(result["vurdering"]["slutskat_til_sammenligning_øre"].is_i64());
        assert_eq!(result["vurdering"]["samlet_modeldækning_bekræftet"], false);
        let wage_am = if am == 0 { 0 } else { 16000 };
        let excess = gross - am - cap;
        let personal = 200000 - wage_am + excess;
        assert_eq!(result["skat"]["arbejdsmarkedsbidrag_kroner"], wage_am);
        assert_eq!(
            result["skat"]["personlig_indkomst_efter_am_kroner"],
            personal
        );
        assert_eq!(
            result["pension"]["pbl18_årsinput"]["personlig_indkomst_før_pensionsfradrag_kroner"],
            personal
        );
        assert_eq!(
            result["skat"]["ekstra_pensionsfradrag_kroner"],
            cap * 12 / 100
        );
        assert_eq!(
            result["skat"]["beskæftigelsesfradrag_kroner"],
            (200000 + gross) * if year == 2025 { 1230 } else { 1275 } / 10000
        );
        let job = ((200000 + gross - if year == 2025 { 224500 } else { 235200 }) * 450 / 10000)
            .min(if year == 2025 { 2900 } else { 3100 });
        assert_eq!(result["skat"]["jobfradrag_kroner"], job);
        let rate = &result["pension"]["arbejdsgiver_rate_resultat"];
        assert_eq!(rate["indbetaling_før_am_kroner"], gross);
        assert_eq!(rate["indeholdt_am_kroner"], am);
        assert_eq!(rate["fælles_rateloft_kroner"], cap);
        assert_eq!(rate["bortseelsesberettiget_efter_am_kroner"], cap);
        assert_eq!(
            rate["skattepligtig_personlig_indkomst_uden_nyt_am_kroner"],
            excess
        );
        let private = &result["pension"]["pbl18_årsresultat"];
        assert_eq!(private["rate_og_ophørende_fradrag_kroner"], 0);
        assert_eq!(
            private["egne_indbetalinger_ikke_fratrukket_i_indkomståret_kroner"],
            if split { 5000 } else { 0 }
        );
        assert_eq!(
            result["pension"]["lønmodtager_pensionsfradrag"]
                ["pbl19_rate_ophørende_arbejdsindkomst_før_am_kroner"],
            gross
        );
        println!(
            "{name}: exempt={cap}, excess={excess}, personal={personal}, tax-øre={}",
            result["vurdering"]["slutskat_til_sammenligning_øre"]
        );
    }
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
