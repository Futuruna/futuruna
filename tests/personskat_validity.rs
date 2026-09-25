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
fn commuting_rejects_negative_bridge_counts_and_impossible_calendar_days() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut baseline = fictional_spouse_rates_input(&envelope);
    baseline["ægtefælle"] = json!({"$variant":"UdenÆgtefælle"});
    baseline["lønmodtager"]["skatteår"] = json!(2026);
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    baseline["lønmodtager"]["betaler_kirkeskat"] = json!(false);
    let bridge_fields = [
        "storebælt_bil_motorcykel_passager",
        "storebælt_kollektiv_passager",
        "øresund_bil_motorcykel_passager",
        "øresund_kollektiv_passager",
    ];
    // Year, travel days, bridge counts, spouse route, expected deduction.
    // Expectations use the existing annual nearest-krone LL9C projection;
    // these domain tests do not establish SKAT rounding conformance.
    let fixtures = [
        ("zero", 2026, 0, [0, 0, 0, 0], false, Some(0)),
        ("ordinary-365", 2026, 365, [0, 0, 0, 0], false, Some(35869)),
        ("bridges", 2026, 220, [1, 1, 1, 1], false, Some(21802)),
        (
            "negative-storebaelt-car",
            2026,
            220,
            [-1, 0, 0, 0],
            false,
            None,
        ),
        (
            "negative-storebaelt-rail",
            2026,
            220,
            [0, -1, 0, 0],
            false,
            None,
        ),
        (
            "negative-oresund-car",
            2026,
            220,
            [0, 0, -1, 0],
            false,
            None,
        ),
        (
            "negative-oresund-rail",
            2026,
            220,
            [0, 0, 0, -1],
            false,
            None,
        ),
        ("ordinary-366", 2026, 366, [0, 0, 0, 0], false, None),
        ("leap-366", 2024, 366, [0, 0, 0, 0], false, Some(25302)),
        ("leap-367", 2024, 367, [0, 0, 0, 0], false, None),
        (
            "spouse-negative-oresund-rail",
            2026,
            220,
            [0, 0, 0, -1],
            true,
            None,
        ),
    ];
    envelope["cases"] = json!(fixtures
        .iter()
        .map(|(id, year, days, bridges, active_spouse, _)| {
            let mut input = baseline.clone();
            input["lønmodtager"]["skatteår"] = json!(year);
            let mut route = commuting(*days);
            for (field, count) in bridge_fields.iter().zip(bridges) {
                route["broer"][*field] = json!(count);
            }
            route["broer"]["dokumenteret_og_afholdt_af_skattepligtige"] = json!(true);
            if *active_spouse {
                input["ægtefælle"] = spouse(&input);
                input["ægtefælle"]["fakta"]["lønmodtager"]["ligningsfradrag"]["befordring"]
                    ["forhold"] = json!([route]);
            } else {
                input["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] = json!([route]);
            }
            json!({"case_id":id, "input":input})
        })
        .collect::<Vec<_>>());
    let path = std::env::temp_dir().join(format!(
        "futuruna-commuting-validity-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), fixtures.len());
    for row in results {
        println!(
            "{}: valid={}, comparison={}",
            row["case_id"],
            row["result"]["vurdering"]["alle_kontroller_gyldige"],
            row["result"]["vurdering"]["slutskat_til_sammenligning_øre"]
        );
    }
    for (row, (id, _, _, _, active_spouse, deduction)) in results.iter().zip(fixtures) {
        assert_eq!(row["case_id"], id);
        let result = &row["result"];
        let gate = &result["vurdering"];
        assert_eq!(
            gate["alle_kontroller_gyldige"],
            deduction.is_some(),
            "{id}: {gate}"
        );
        let deductions = if active_spouse {
            &result["ægtefælle"]["grundlag"]["ligningsfradrag"]
        } else {
            &result["ligningsfradrag"]
        };
        if let Some(expected) = deduction {
            assert!(gate["slutskat_til_sammenligning_øre"].is_number());
            assert_eq!(
                deductions["befordring"]["samlet_ligningsfradrag_kroner"], expected,
                "{id}: {deductions}"
            );
        } else {
            assert_eq!(gate["slutskat_til_sammenligning_øre"], Value::Null);
            assert_eq!(deductions["befordring"]["alle_input_gyldige"], false);
            let prefix = if active_spouse {
                "ægtefælle.MedÆgtefælle.fakta."
            } else {
                ""
            };
            assert!(
                gate["fejl"].as_array().unwrap().iter().any(|error| {
                    error["sti"] == format!("{prefix}lønmodtager.ligningsfradrag.befordring")
                        && error["forklaring"]
                            .as_str()
                            .unwrap()
                            .contains("bropassager")
                        && error["forklaring"]
                            .as_str()
                            .unwrap()
                            .contains("kalenderdage")
                }),
                "{id}: {gate}"
            );
        }
    }
}

fn fictional_spouse_rates_input(envelope: &Value) -> Value {
    // Fictional source facts matching SKAT's anonymous 2025 calculator.
    // No reported deduction or tax amount is inserted into the input.
    let mut baseline = envelope["cases"][0]["input"].clone();
    baseline["lønmodtager"]["skatteår"] = json!(2025);
    baseline["lønmodtager"]["kommune"] = json!({"$variant":"København"});
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(400000);
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
    baseline["ægtefælle"] = spouse(&baseline);
    baseline["ægtefælle"]["fakta"]["lønmodtager"]["kommune"] = json!({"$variant":"Ballerup"});
    baseline["ægtefælle"]["fakta"]["lønmodtager"]["betaler_kirkeskat"] = json!(false);
    baseline
}

#[test]
fn public_early_pensions_preserve_old_exemptions_and_separate_atp_bases() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut baseline = fictional_spouse_rates_input(&envelope);
    baseline["ægtefælle"] = json!({"$variant":"UdenÆgtefælle"});
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(0);
    baseline["lønmodtager"]["betaler_kirkeskat"] = json!(false);
    baseline["lønmodtager"]["pension"]["fødselsdato"]["år"] = json!(1960);
    let payment = |id: &str, kind: &str, amount: i64| {
        json!({
            "$variant":"PersonskatSocialpensionsudbetaling", "fakta":{
                "identifikation":id,"indkomstår":2025,"kildereference":"fictional-public-pension-notice",
                "art":{"$variant":kind},"indkomst_før_skat_kroner":amount,
                "ordinær_dansk_udbetaling_uden_korrektioner":true
            }
        })
    };
    let fixtures = [
        "old",
        "old-exempt",
        "new",
        "senior",
        "early",
        "job",
        "public-atp",
        "commuter",
        "spouse",
        "unknown-kind",
        "scope-unknown",
        "wrong-year",
        "negative",
        "missing-source",
        "duplicate",
        "atp-unknown",
        "spouse-invalid",
    ];
    envelope["cases"] = json!(fixtures.iter().map(|id| {
        let mut input = baseline.clone();
        let kind = match *id {
            "senior" => "SocialSeniorpension",
            "early" => "SocialTidligPension",
            _ => "SocialFørtidspensionEfterNyeRegler",
        };
        let mut posts = vec![payment("pension", kind, 180000)];
        match *id {
            "old" => posts = vec![
                payment("basic", "GammelFørtidspensionGrundbeløb", 80000),
                payment("supplement", "GammelFørtidspensionPensionstillæg", 60000),
                payment("incapacity", "GammelFørtidspensionErhvervsudygtighedsbeløb", 40000),
                payment("invalidity", "GammelFørtidspensionInvaliditetsbeløb", 30000),
            ],
            "old-exempt" => posts = [
                "GammelFørtidspensionInvaliditetsbeløb", "GammelFørtidspensionInvaliditetsydelse",
                "GammelFørtidspensionFørtidsbeløb", "GammelFørtidspensionEkstraTillægsydelse",
                "GammelFørtidspensionBistandsOgPlejetillæg", "GammelFørtidspensionPersonligtTillæg",
                "GammelFørtidspensionHelbredstillæg", "GammelFørtidspensionTillægEfterPar62",
            ].iter().map(|kind| payment(kind, kind, 1000)).collect(),
            "job" => input["lønmodtager"]["bruttoløn_kroner"] = json!(100000),
            "public-atp" => input["lønmodtager"]["pension"]["atp"] = json!({
                "$variant":"OplysteAtpIndbetalinger","oplysninger_for_året_komplette":true,
                "poster":[
                    {"identifikation":"atp","indkomstår":2025,"kildereference":"fictional-atp",
                     "grundlag":{"$variant":"AtpOffentligYdelsePar19Stk2"},
                     "indberettet_før_am_kroner":5000,"indberettet_efter_am_kroner":4600},
                    {"identifikation":"supp","indkomstår":2025,"kildereference":"fictional-supp",
                     "grundlag":{"$variant":"AtpSupplerendeArbejdsmarkedspensionPar19Stk4"},
                     "indberettet_før_am_kroner":5000,"indberettet_efter_am_kroner":4600},
                    {"identifikation":"mandatory","indkomstår":2025,"kildereference":"fictional-mandatory",
                     "grundlag":{"$variant":"AtpObligatoriskPensionPar19Stk4"},
                     "indberettet_før_am_kroner":800,"indberettet_efter_am_kroner":800}
                ]
            }),
            "commuter" => {
                input["lønmodtager"]["skatteår"] = json!(2026);
                input["lønmodtager"]["bruttoløn_kroner"] = json!(300000);
                input["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] = json!([commuting(220)]);
                posts[0]["fakta"]["indkomstår"] = json!(2026);
            }
            "unknown-kind" => posts[0]["fakta"]["art"] = json!({"$variant":"SocialpensionsartUoplystEllerUdenForModellen"}),
            "scope-unknown" | "spouse-invalid" => posts[0]["fakta"]["ordinær_dansk_udbetaling_uden_korrektioner"] = json!(false),
            "wrong-year" => posts[0]["fakta"]["indkomstår"] = json!(2024),
            "negative" => posts[0]["fakta"]["indkomst_før_skat_kroner"] = json!(-1),
            "missing-source" => posts[0]["fakta"]["kildereference"] = json!(" "),
            "duplicate" => posts.push(posts[0].clone()),
            "atp-unknown" => input["lønmodtager"]["pension"]["atp"] = json!({"$variant":"AtpUoplyst"}),
            _ => {}
        }
        input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]["forenings_og_arbejdsløshedsydelser"] = json!(posts);
        if id.starts_with("spouse") {
            input["ægtefælle"] = spouse(&input);
            input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]["forenings_og_arbejdsløshedsydelser"] = json!([]);
        }
        json!({"case_id":id,"input":input})
    }).collect::<Vec<_>>());
    let path = std::env::temp_dir().join(format!(
        "futuruna-socialpension-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let rows = output["results"].as_array().unwrap();
    assert_eq!(rows.len(), fixtures.len());
    for (index, (row, id)) in rows.iter().zip(fixtures).enumerate() {
        assert_eq!(row["case_id"], id);
        let result = &row["result"];
        let gate = &result["vurdering"];
        let valid = index < 9;
        println!(
            "{id}: valid={}, tax={}",
            gate["alle_kontroller_gyldige"], gate["slutskat_til_sammenligning_øre"]
        );
        assert_eq!(gate["alle_kontroller_gyldige"], valid, "{id}: {gate}");
        if !valid {
            assert_eq!(gate["slutskat_til_sammenligning_øre"], Value::Null);
            let error_path = match id {
                "atp-unknown" => "lønmodtager.pension.atp",
                "spouse-invalid" => "ægtefælle.MedÆgtefælle.fakta.lønmodtager.personlig_indkomst",
                _ => "lønmodtager.personlig_indkomst",
            };
            assert!(
                gate["fejl"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["sti"] == error_path),
                "{id}: {gate}"
            );
            continue;
        }
        assert!(gate["slutskat_til_sammenligning_øre"].is_number());
        let person = if id == "spouse" {
            &result["ægtefælle"]["grundlag"]
        } else {
            result
        };
        let income = if id == "old-exempt" { 0 } else { 180000 };
        assert_eq!(
            person["personlig_indkomst"]["personlig_indkomst_uden_nyt_arbejdsmarkedsbidrag_kroner"],
            income,
            "{id}"
        );
        let posts = person["personlig_indkomst"]["ordinære_forhold"]
            ["forenings_og_arbejdsløshedsydelser"]
            .as_array()
            .unwrap();
        let exempt: i64 = posts
            .iter()
            .map(|p| p["resultat"]["skattefrit_beløb_kroner"].as_i64().unwrap())
            .sum();
        assert_eq!(
            exempt,
            match id {
                "old" => 30000,
                "old-exempt" => 8000,
                _ => 0,
            },
            "{id}"
        );
        let wage = match id {
            "job" => 100000,
            "commuter" => 300000,
            _ => 0,
        };
        assert_eq!(
            person["ligningsfradrag"]["befordring"]["aftrapningsindkomst_kroner"], wage,
            "{id}"
        );
        assert_eq!(
            person["ligningsfradrag"]["befordring"]["aftrapningsindkomst_afklaret"], true,
            "{id}"
        );
        assert_eq!(
            person["ligningsfradrag"]["befordring"]["lavindkomsttillæg_kroner"],
            if id == "commuter" { 13836 } else { 0 },
            "{id}"
        );
        assert_eq!(
            person["pension"]["udbetalingsresultat"]
                ["samlet_modregningspligtig_pbl20_i_året_kroner"],
            0,
            "{id}"
        );
        let tax = if id == "spouse" {
            &result["ægtefælle"]["skat"]["indkomstgrundlag"]
        } else {
            &result["skat"]
        };
        let deductions = if id == "spouse" {
            &result["ægtefælle"]["skat"]["ligningsfradrag"]
        } else {
            &result["skat"]
        };
        assert_eq!(
            tax["personlig_indkomst_efter_am_kroner"],
            income + wage * 92 / 100,
            "{id}"
        );
        assert_eq!(tax["arbejdsmarkedsbidrag_kroner"], wage * 8 / 100, "{id}");
        assert_eq!(
            deductions["beskæftigelsesfradrag_kroner"],
            match id {
                "job" => 12300,
                "commuter" => 38250,
                _ => 0,
            },
            "{id}"
        );
        assert_eq!(
            deductions["ekstra_pensionsfradrag_kroner"],
            if id == "public-atp" { 3200 } else { 0 },
            "{id}"
        );
        assert_eq!(
            person["pension"]["atp_resultat"]["arbejdsfradragsgrundlag_kroner"], 0,
            "{id}"
        );
        // Independent anonymous 2025 calculator observations on 2026-09-25,
        // born 1960, Copenhagen, unmarried, no church, public pension selected.
        // Rubrik 16A pension 180000; optional wage 100000. Before green check.
        // Old-regime split and the other social-pension kinds are model checks;
        // the public calculator does not independently classify those components.
        if matches!(id, "new" | "job") {
            assert_eq!(
                tax["almindelig_skattepligtig_indkomst_kroner"],
                if id == "job" { 259700 } else { 180000 },
                "{id}"
            );
            assert_eq!(
                result["hovedskat_eksakt"]["før_nedsættelser"]["kommuneskat_øre"],
                if id == "job" { 6102950 } else { 4230000 },
                "{id}"
            );
            assert_eq!(
                gate["slutskat_til_sammenligning_øre"],
                if id == "job" { 8337354 } else { 4559484 },
                "{id}"
            );
        }
    }
}

#[test]
fn folkepension_and_exempt_supplements_preserve_tax_and_pension_deduction_bases() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut baseline = fictional_spouse_rates_input(&envelope);
    baseline["ægtefælle"] = json!({"$variant":"UdenÆgtefælle"});
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(0);
    baseline["lønmodtager"]["betaler_kirkeskat"] = json!(false);
    baseline["lønmodtager"]["pension"]["fødselsdato"] = json!({"år":1955,"måned":1,"dag":1});
    let payment = |id: &str, art: &str, amount: i64| {
        json!({"$variant":"PersonskatFolkepensionsudbetaling", "fakta":{
            "identifikation":id, "indkomstår":2025, "kildereference":"fictional-pension-notice",
            "art":{"$variant":art}, "beløb_før_skat_kroner":amount,
            "ordinær_dansk_udbetaling_uden_korrektioner":true
        }})
    };
    let private_payout = |year: i64| {
        json!({"identifikation":format!("private-{year}"), "indkomstår":year,
            "ordning":{"$variant":"Pbl20Rateopsparing"},
            "udbetalingsret":{"$variant":"Pbl20TilEjerEfterVilkår"},
            "ligningslov9l_art":{"$variant":"Pbl20OrdinærPensionsudbetaling"},
            "bruttoudbetaling_kroner":40000, "del_fra_indbetalinger_før_1955_56_kroner":0,
            "dokumenteret_uden_fradrags_eller_bortseelsesret_kroner":0,
            "eu_eøs_erklæring":{"$variant":"Pbl20IngenEuEøsErklæring"},
            "udenlandsk_indkomstskat_kroner":0})
    };
    let fixtures = [
        "pension",
        "job",
        "exempt-supplements",
        "only-exempt",
        "private-contribution",
        "private-payout",
        "commuter",
        "spouse",
        "unknown-kind",
        "negative",
        "wrong-year",
        "blank-id",
        "missing-source",
        "scope-unknown",
        "duplicate",
        "spouse-invalid",
    ];
    envelope["cases"] = json!(fixtures.iter().map(|id| {
        let mut input = baseline.clone();
        let mut posts = vec![
            payment("basic", "FolkepensionGrundbeløb", 80000),
            payment("supplement", "FolkepensionPensionstillæg", 80000),
            payment("elder-cheque", "FolkepensionÆldrecheck", 20000),
        ];
        if *id == "only-exempt" { posts.clear(); }
        if matches!(*id, "exempt-supplements" | "only-exempt") {
            posts.extend([
                payment("personal", "FolkepensionPersonligtTillægEfterPar14", 3000),
                payment("heating", "FolkepensionVarmetillægEfterPar14", 4000),
                payment("health", "FolkepensionHelbredstillægEfterPar14A", 5000),
            ]);
        }
        match *id {
            "job" => input["lønmodtager"]["bruttoløn_kroner"] = json!(100000),
            "commuter" => {
                input["lønmodtager"]["skatteår"] = json!(2026);
                input["lønmodtager"]["bruttoløn_kroner"] = json!(300000);
                input["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] = json!([commuting(220)]);
                for post in &mut posts { post["fakta"]["indkomstår"] = json!(2026); }
            }
            "private-contribution" | "private-payout" => {
                input["lønmodtager"]["pension"]["pbl18_indbetalinger"] = json!([{
                    "identifikation":"fictional-private-rate", "ordning":{"$variant":"Pbl18Rateopsparing"},
                    "indbetalingskilde":{"$variant":"Pbl18EgenIndbetaling"},
                    "fradragsretshaver":{"$variant":"Pbl18OrdningensEjer"},
                    "betaling":{"beløb_kroner":20000,"forfaldsår":2025,"betalingsår":2025,
                        "betalt_senest_bankjusteret_1_april_efter_forfald":true,
                        "hidrører_fra_par22e_tilbagebetaling":false,
                        "par15a_fradragsplacering":{"$variant":"Pbl18IkkePar15APlacering"},
                        "arbejdsmarkedsbidrag_kroner":0},
                    "fordelingsforløb":{"$variant":"Pbl18IngenTiårsfordeling"},
                    "indeksvalg":{"fradragsvalgte_kontraktbidrag_kroner":[]},
                    "indeksordningsgrundlag":{"$variant":"Pbl18IkkeIndeksordning"},
                    "forfaldne_ikke_tidligere_fratrukket_kroner":0,
                    "særligt_ordningsgrundlag":{"$variant":"Pbl18IntetSærligtOrdningsgrundlag"},
                    "begrænsninger":{"pbl54_personkreds_opfyldt":true,
                        "afgiftspligt_for_hele_ordningen_indtrådt":false,
                        "udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens":false}
                }]);
                let mut payouts = vec![private_payout(2024)];
                if *id == "private-payout" { payouts.push(private_payout(2025)); }
                input["lønmodtager"]["pension"]["øvrige_pbl20_årsgrundlag"]["udbetalinger"] = json!(payouts);
            }
            "unknown-kind" => posts[0]["fakta"]["art"] = json!({"$variant":"FolkepensionsartUoplystEllerUdenForModellen"}),
            "negative" => posts[0]["fakta"]["beløb_før_skat_kroner"] = json!(-1),
            "wrong-year" => posts[0]["fakta"]["indkomstår"] = json!(2024),
            "blank-id" => posts[0]["fakta"]["identifikation"] = json!(" "),
            "missing-source" => posts[0]["fakta"]["kildereference"] = json!(" "),
            "scope-unknown" | "spouse-invalid" => posts[0]["fakta"]["ordinær_dansk_udbetaling_uden_korrektioner"] = json!(false),
            "duplicate" => posts[1]["fakta"]["identifikation"] = json!("basic"),
            _ => {}
        }
        input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]["forenings_og_arbejdsløshedsydelser"] = json!(posts);
        if id.starts_with("spouse") {
            input["ægtefælle"] = spouse(&input);
            input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]["forenings_og_arbejdsløshedsydelser"] = json!([]);
        }
        json!({"case_id":id, "input":input})
    }).collect::<Vec<_>>());
    let path =
        std::env::temp_dir().join(format!("futuruna-folkepension-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let rows = output["results"].as_array().unwrap();
    assert_eq!(rows.len(), fixtures.len());
    for (index, (row, id)) in rows.iter().zip(fixtures).enumerate() {
        assert_eq!(row["case_id"], id);
        let result = &row["result"];
        let gate = &result["vurdering"];
        let valid = index < 8;
        println!(
            "{id}: valid={}, tax={}",
            gate["alle_kontroller_gyldige"], gate["slutskat_til_sammenligning_øre"]
        );
        assert_eq!(gate["alle_kontroller_gyldige"], valid, "{id}: {gate}");
        if !valid {
            assert_eq!(gate["slutskat_til_sammenligning_øre"], Value::Null);
            let prefix = if id == "spouse-invalid" {
                "ægtefælle.MedÆgtefælle.fakta."
            } else {
                ""
            };
            assert!(
                gate["fejl"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["sti"] == format!("{prefix}lønmodtager.personlig_indkomst")),
                "{id}: {gate}"
            );
            continue;
        }
        assert!(gate["slutskat_til_sammenligning_øre"].is_number());
        let person = if id == "spouse" {
            &result["ægtefælle"]["grundlag"]
        } else {
            result
        };
        let pension = if id == "only-exempt" { 0 } else { 180000 };
        assert_eq!(
            person["personlig_indkomst"]["personlig_indkomst_uden_nyt_arbejdsmarkedsbidrag_kroner"],
            pension,
            "{id}"
        );
        let wage = match id {
            "job" => 100000,
            "commuter" => 300000,
            _ => 0,
        };
        assert_eq!(
            person["ligningsfradrag"]["befordring"]["aftrapningsindkomst_kroner"], wage,
            "{id}"
        );
        assert_eq!(
            person["ligningsfradrag"]["befordring"]["aftrapningsindkomst_afklaret"], true,
            "{id}"
        );
        assert_eq!(
            person["ligningsfradrag"]["befordring"]["lavindkomsttillæg_kroner"],
            if id == "commuter" { 13836 } else { 0 },
            "{id}"
        );
        let payouts = &person["pension"]["udbetalingsresultat"];
        assert_eq!(
            payouts["samlet_modregningspligtig_pbl20_i_året_kroner"],
            if id == "private-payout" { 40000 } else { 0 },
            "{id}"
        );
        if id != "spouse" {
            assert_eq!(
                result["skat"]["arbejdsmarkedsbidrag_kroner"],
                wage * 8 / 100,
                "{id}"
            );
            assert_eq!(
                result["skat"]["beskæftigelsesfradrag_kroner"],
                match id {
                    "job" => 12300,
                    "commuter" => 38250,
                    _ => 0,
                },
                "{id}"
            );
            assert_eq!(
                result["skat"]["ekstra_pensionsfradrag_kroner"],
                if id == "private-contribution" {
                    6400
                } else {
                    0
                },
                "{id}"
            );
        }
        // Independent anonymous annual-calculator observations, 2026-09-25:
        // 2025 Copenhagen, born 1955, unmarried, no church/ATP/private pension.
        // Tax before green check, advance payments and settlement additions.
        // Adding exempt supplements is a model invariance check, not a third oracle.
        let observed = match id {
            "pension" | "exempt-supplements" => Some((180000, 180000, 4230000, 4559484)),
            "job" => Some((272000, 259700, 6102950, 8337354)),
            _ => None,
        };
        if let Some((personal, taxable, municipal, tax)) = observed {
            assert_eq!(
                result["skat"]["personlig_indkomst_efter_am_kroner"], personal,
                "{id}"
            );
            assert_eq!(
                result["skat"]["almindelig_skattepligtig_indkomst_kroner"], taxable,
                "{id}"
            );
            assert_eq!(
                result["hovedskat_eksakt"]["før_nedsættelser"]["kommuneskat_øre"], municipal,
                "{id}"
            );
            assert_eq!(gate["slutskat_til_sammenligning_øre"], tax, "{id}");
        }
    }
}

#[test]
fn su_grants_and_loans_reach_canonical_tax_without_wage_deductions() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut baseline = fictional_spouse_rates_input(&envelope);
    baseline["ægtefælle"] = json!({"$variant":"UdenÆgtefælle"});
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(0);
    baseline["lønmodtager"]["betaler_kirkeskat"] = json!(false);
    let su = |id: &str, art: &str, year: i64, amount: i64| {
        json!({
            "$variant":"PersonskatUddannelsesstøtte", "fakta":{
                "identifikation":id, "indkomstår":year, "kildereference":"fictional-SU-record",
                "art":{"$variant":art}, "beløb_før_skat_kroner":amount,
                "uden_udlandsforhold_og_korrektioner":true
            }
        })
    };
    let fixtures = [
        "stipend",
        "job-and-stipend",
        "loan",
        "stipend-and-loan",
        "commuter",
        "spouse",
        "unknown-kind",
        "negative",
        "wrong-year",
        "missing-source",
        "scope-unknown",
        "duplicate",
        "spouse-invalid",
    ];
    envelope["cases"] = json!(fixtures
        .iter()
        .map(|id| {
            let mut input = baseline.clone();
            let mut grant = su("grant", "SuStipendiumEfterDanskSuLov", 2025, 80000);
            match *id {
                "job-and-stipend" => input["lønmodtager"]["bruttoløn_kroner"] = json!(100000),
                "commuter" => {
                    input["lønmodtager"]["skatteår"] = json!(2026);
                    input["lønmodtager"]["bruttoløn_kroner"] = json!(300000);
                    input["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] =
                        json!([commuting(220)]);
                    grant["fakta"]["indkomstår"] = json!(2026);
                    grant["fakta"]["beløb_før_skat_kroner"] = json!(100000);
                }
                "unknown-kind" => {
                    grant["fakta"]["art"] = json!({"$variant":"SuUoplystEllerUdenForModellen"})
                }
                "negative" => grant["fakta"]["beløb_før_skat_kroner"] = json!(-1),
                "wrong-year" => grant["fakta"]["indkomstår"] = json!(2024),
                "missing-source" => grant["fakta"]["kildereference"] = json!(" "),
                "scope-unknown" | "spouse-invalid" => {
                    grant["fakta"]["uden_udlandsforhold_og_korrektioner"] = json!(false)
                }
                _ => {}
            }
            let posts = match *id {
                "loan" => vec![su("loan", "SuLånEfterDanskSuLov", 2025, 30000)],
                "stipend-and-loan" => vec![grant, su("loan", "SuLånEfterDanskSuLov", 2025, 30000)],
                "duplicate" => vec![grant, su("grant", "SuLånEfterDanskSuLov", 2025, 30000)],
                _ => vec![grant],
            };
            input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]
                ["forenings_og_arbejdsløshedsydelser"] = json!(posts);
            if id.starts_with("spouse") {
                input["ægtefælle"] = spouse(&input);
                input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]
                    ["forenings_og_arbejdsløshedsydelser"] = json!([]);
            }
            json!({"case_id":id, "input":input})
        })
        .collect::<Vec<_>>());
    let path = std::env::temp_dir().join(format!("futuruna-su-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let rows = output["results"].as_array().unwrap();
    assert_eq!(rows.len(), fixtures.len());
    for (row, id) in rows.iter().zip(fixtures) {
        assert_eq!(row["case_id"], id);
        let result = &row["result"];
        let gate = &result["vurdering"];
        let valid = matches!(
            id,
            "stipend" | "job-and-stipend" | "loan" | "stipend-and-loan" | "commuter" | "spouse"
        );
        println!(
            "{id}: valid={}, tax={}",
            gate["alle_kontroller_gyldige"], gate["slutskat_til_sammenligning_øre"]
        );
        assert_eq!(gate["alle_kontroller_gyldige"], valid, "{id}: {gate}");
        if !valid {
            assert_eq!(gate["slutskat_til_sammenligning_øre"], Value::Null);
            let prefix = if id == "spouse-invalid" {
                "ægtefælle.MedÆgtefælle.fakta."
            } else {
                ""
            };
            let path = format!("{prefix}lønmodtager.personlig_indkomst");
            assert!(
                gate["fejl"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["sti"] == path),
                "{id}: {gate}"
            );
            continue;
        }
        assert!(gate["slutskat_til_sammenligning_øre"].is_number());
        let person = if id == "spouse" {
            &result["ægtefælle"]["grundlag"]
        } else {
            result
        };
        let grant_amount = match id {
            "loan" => 0,
            "commuter" => 100000,
            _ => 80000,
        };
        assert_eq!(
            person["personlig_indkomst"]["personlig_indkomst_uden_nyt_arbejdsmarkedsbidrag_kroner"],
            grant_amount,
            "{id}"
        );
        let wage = match id {
            "job-and-stipend" => 100000,
            "commuter" => 300000,
            _ => 0,
        };
        let commute = &person["ligningsfradrag"]["befordring"];
        assert_eq!(commute["aftrapningsindkomst_kroner"], wage, "{id}");
        assert_eq!(commute["aftrapningsindkomst_afklaret"], true, "{id}");
        assert_eq!(
            commute["lavindkomsttillæg_kroner"],
            if id == "commuter" { 13836 } else { 0 },
            "{id}"
        );
        if id != "spouse" {
            assert_eq!(
                result["skat"]["arbejdsmarkedsbidrag_kroner"],
                wage * 8 / 100,
                "{id}"
            );
            assert_eq!(
                result["skat"]["personlig_indkomst_efter_am_kroner"],
                wage * 92 / 100 + grant_amount,
                "{id}"
            );
            assert_eq!(
                result["skat"]["beskæftigelsesfradrag_kroner"],
                match id {
                    "job-and-stipend" => 12300,
                    "commuter" => 38250,
                    _ => 0,
                },
                "{id}"
            );
        }
        // Independent anonymous SKAT annual-calculator observations, 2026-09-25.
        // Copenhagen 2025, born 1990, unmarried, no church/pension/ATP/other facts.
        // Before green check, advance payments and settlement additions.
        // Adding a loan must preserve the stipend-only observation; that case
        // is a model regression, not a third external calculator observation.
        let official = match id {
            "stipend" | "stipend-and-loan" => Some((80000, 1880000, 1008484)),
            "job-and-stipend" => Some((159700, 3752950, 4786354)),
            _ => None,
        };
        if let Some((taxable, municipal, tax)) = official {
            assert_eq!(
                result["skat"]["almindelig_skattepligtig_indkomst_kroner"], taxable,
                "{id}"
            );
            assert_eq!(
                result["hovedskat_eksakt"]["før_nedsættelser"]["kommuneskat_øre"], municipal,
                "{id}"
            );
            assert_eq!(gate["slutskat_til_sammenligning_øre"], tax, "{id}");
        }
    }
}

#[test]
fn commuting_income_uses_benefit_sources_and_annual_business_basis() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut baseline = fictional_spouse_rates_input(&envelope);
    baseline["ægtefælle"] = json!({"$variant":"UdenÆgtefælle"});
    baseline["lønmodtager"]["skatteår"] = json!(2026);
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(300000);
    baseline["lønmodtager"]["betaler_kirkeskat"] = json!(false);
    baseline["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] = json!([commuting(220)]);
    let benefit = |art: Value| {
        json!({"$variant":"PersonskatLovbestemteDagpenge", "fakta":{
            "identifikation":"fictional-benefit", "indkomstår":2026,
            "kildereference":"fictional-payment-record", "art":art,
            "indkomst_før_skat_kroner":100000
        }})
    };
    let sick = |b: Value, voluntary: Value| {
        benefit(json!({
            "$variant":"SygedagpengeEfterDanskSygedagpengelov",
            "erstatter_b_indkomst":b, "frivillig_sikring_efter_par45":voluntary
        }))
    };
    let parental = |b: Value| {
        benefit(json!({"$variant":"BarselsdagpengeEfterDanskBarselslov", "erstatter_b_indkomst":b}))
    };
    let insurance = |art: &str| {
        json!({"$variant":"PersonskatArbejdsløshedsforsikringsudbetalingEfterPbl49", "fakta":{
            "identifikation":"fictional-benefit", "indkomstår":2026, "beløb_kroner":100000,
            "art":{"$variant":art}, "skatteyderrelation":{"$variant":"Pbl49SkatteyderenErEjer"},
            "undtagelse":{"$variant":"Ll30Og31UdenUndtagelse"}
        }})
    };
    let unemployment =
        benefit(json!({"$variant":"ArbejdsløshedsdagpengeEfterDanskArbejdsløshedsforsikringslov"}));
    let mut wrong_year = unemployment.clone();
    wrong_year["fakta"]["indkomstår"] = json!(2025);
    // Source-law expectations, not observations of SKAT rounding:
    // 220 * (55 - 24) * 3.17 = 21619.40 -> existing 21619 DKK projection.
    // At 300000 the additional deduction is floor(21619 * .64) = 13836.
    // At 400000 it is fully phased out. Benefits must not add AM or employment deduction.
    // (ID, benefits, known income, income classified, deduction classified, valid, bonus.)
    let fixtures = vec![
        ("baseline", vec![], 300000, true, true, true, 13836),
        (
            "unemployment",
            vec![unemployment.clone()],
            400000,
            true,
            true,
            true,
            0,
        ),
        (
            "g-days",
            vec![benefit(
                json!({"$variant":"GDageEfterDanskArbejdsløshedsforsikringslov84"}),
            )],
            400000,
            true,
            true,
            true,
            0,
        ),
        (
            "sickness",
            vec![sick(json!(false), json!(false))],
            400000,
            true,
            true,
            true,
            0,
        ),
        (
            "parental",
            vec![parental(json!(false))],
            400000,
            true,
            true,
            true,
            0,
        ),
        (
            "sickness-b-income",
            vec![sick(json!(true), Value::Null)],
            300000,
            true,
            true,
            true,
            13836,
        ),
        (
            "sickness-voluntary",
            vec![sick(Value::Null, json!(true))],
            300000,
            true,
            true,
            true,
            13836,
        ),
        (
            "parental-b-income",
            vec![parental(json!(true))],
            300000,
            true,
            true,
            true,
            13836,
        ),
        (
            "unknown-sickness",
            vec![sick(Value::Null, json!(false))],
            300000,
            false,
            false,
            false,
            0,
        ),
        (
            "unknown-high-wage",
            vec![sick(Value::Null, json!(false))],
            400000,
            false,
            true,
            true,
            0,
        ),
        (
            "legacy-akasse",
            vec![insurance("Pbl49UdbetalingFraArbejdsløshedskasse")],
            300000,
            false,
            false,
            false,
            0,
        ),
        (
            "private-insurance",
            vec![insurance(
                "Pbl49UdbetalingFraPrivatArbejdsløshedsforsikring",
            )],
            300000,
            true,
            true,
            true,
            13836,
        ),
        (
            "wrong-year",
            vec![wrong_year],
            300000,
            false,
            false,
            false,
            0,
        ),
        (
            "duplicate",
            vec![unemployment.clone(), unemployment.clone()],
            300000,
            false,
            false,
            false,
            0,
        ),
        (
            "cross-branch-duplicate",
            vec![
                unemployment.clone(),
                insurance("Pbl49UdbetalingFraArbejdsløshedskasse"),
            ],
            300000,
            false,
            false,
            false,
            0,
        ),
        ("spouse", vec![unemployment], 400000, true, true, true, 0),
        ("business-netting", vec![], 300000, true, true, true, 13836),
        ("business-carry", vec![], 300000, true, true, true, 13836),
        (
            "unknown-no-commute",
            vec![parental(Value::Null)],
            300000,
            false,
            true,
            true,
            0,
        ),
    ];
    let business = |id: &str, revenue: i64, expense: i64| {
        json!({
            "identifikation":id, "indkomstår":2026,
            "indtægter":[{"identifikation":format!("{id}-revenue"), "art":{"$variant":"OrdinærDriftsindtægt"}, "beløb_kroner":revenue}],
            "udgifter":[{"identifikation":format!("{id}-expense"), "afgrænsning":{"$variant":"SelvstændigErhvervsindkomstUdgift"}, "beløb_kroner":expense}],
            "ligningslovsfradrag_efter_par3_stk2_nr2":[], "erhvervsposter_efter_par3_stk2_nr4_til_10":[]
        })
    };
    envelope["cases"] = json!(fixtures
        .iter()
        .map(|(id, benefits, _, _, _, _, _)| {
            let mut input = baseline.clone();
            if *id == "unknown-high-wage" {
                input["lønmodtager"]["bruttoløn_kroner"] = json!(400000);
            }
            if *id == "unknown-no-commute" {
                input["lønmodtager"]["ligningsfradrag"]["befordring"]["forhold"] = json!([]);
            }
            input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]
                ["forenings_og_arbejdsløshedsydelser"] = json!(benefits);
            if *id == "business-netting" {
                input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]
                    ["virksomheder_uden_virksomhedsordning"] =
                    json!([business("profit", 100000, 0), business("loss", 0, 100000)]);
            }
            if *id == "business-carry" {
                input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]
                    ["virksomheder_uden_virksomhedsordning"] = json!([business("profit", 100000, 0)]);
                input["kapitalindkomst"]["virksomhedskapital"]["selvstændig_arbejdsmarkedsbidrag"] = json!({
                    "$variant":"AmblPar4UdenVirksomhedsordning", "fremført_negativ_personlig_indkomst":[{
                        "identifikation":"fictional-prior-loss", "oprindelsesår":2025,
                        "resterende_beløb_primo_kroner":100000,
                        "oprindelse":{"$variant":"AmblFremførtNegativFraAndet"},
                        "dokumentreference":"fictional-prior-income-loss-ledger"
                    }]
                });
            }
            if *id == "spouse" {
                input["ægtefælle"] = spouse(&input);
                input["ægtefælle"]["fakta"]["lønmodtager"]["bruttoløn_kroner"] = json!(300000);
                input["lønmodtager"]["personlig_indkomst"]["ordinære_forhold"]
                    ["forenings_og_arbejdsløshedsydelser"] = json!([]);
            }
            json!({"case_id":id, "input":input})
        })
        .collect::<Vec<_>>());
    let path = std::env::temp_dir().join(format!(
        "futuruna-commuting-income-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let rows = output["results"].as_array().unwrap();
    assert_eq!(rows.len(), fixtures.len());
    for (row, (id, _, income, classified, bonus_classified, valid, bonus)) in
        rows.iter().zip(fixtures)
    {
        assert_eq!(row["case_id"], id);
        let result = &row["result"];
        let gate = &result["vurdering"];
        let person = if id == "spouse" {
            &result["ægtefælle"]["grundlag"]
        } else {
            result
        };
        let commute = &person["ligningsfradrag"]["befordring"];
        println!(
            "{id}: income={}, classified={}, bonus={}, valid={}",
            commute["aftrapningsindkomst_kroner"],
            commute["aftrapningsindkomst_afklaret"],
            commute["lavindkomsttillæg_kroner"],
            gate["alle_kontroller_gyldige"]
        );
        assert_eq!(gate["alle_kontroller_gyldige"], valid, "{id}: {gate}");
        assert_eq!(commute["aftrapningsindkomst_kroner"], income, "{id}");
        assert_eq!(commute["aftrapningsindkomst_afklaret"], classified, "{id}");
        assert_eq!(
            commute["lavindkomsttillæg_afklaret"], bonus_classified,
            "{id}"
        );
        assert_eq!(commute["lavindkomsttillæg_kroner"], bonus, "{id}");
        if valid {
            assert!(gate["slutskat_til_sammenligning_øre"].is_number());
            if id != "spouse" {
                let wage = if id == "unknown-high-wage" {
                    400000
                } else {
                    300000
                };
                assert_eq!(
                    result["skat"]["arbejdsmarkedsbidrag_kroner"],
                    wage * 8 / 100,
                    "{id}"
                );
                assert_eq!(
                    result["skat"]["beskæftigelsesfradrag_kroner"],
                    wage * 1275 / 10000,
                    "{id}"
                );
                // The carry fixture tests AM/commuting composition, not the separate PSL loss ledger.
                let benefit_income = if id == "baseline" || id == "business-netting" {
                    0
                } else {
                    100000
                };
                assert_eq!(
                    result["skat"]["personlig_indkomst_efter_am_kroner"],
                    wage * 92 / 100 + benefit_income,
                    "{id}"
                );
            }
        } else {
            assert_eq!(gate["slutskat_til_sammenligning_øre"], Value::Null);
            assert!(gate["fejl"].as_array().unwrap().iter().any(|error| error["sti"] == "lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser"), "{id}: {gate}");
        }
    }
}

#[test]
fn spouse_loss_uses_recipient_rates_from_source_facts() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let mut baseline = fictional_spouse_rates_input(&envelope);
    baseline["lønmodtager"]["bruttoløn_kroner"] = json!(600000);
    baseline["ægtefælle"]["fakta"]["kapitalindkomst"]["renter"]["renteudgifter_kroner"] =
        json!(503500);
    // Independent anonymous SKAT 2025 observations, 2026-09-25:
    // recipient wage 600000, Copenhagen; spouse zero wage, Ballerup,
    // other-debt interest (rubrik 44) 503500. No reported tax is an input.
    // (Recipient church, donor church, final tax in ore, loss credit in kroner.)
    let expectations = [
        (false, false, 6729888, 2350),
        (true, false, 6639328, 2430),
        (false, true, 6729888, 2350),
    ];
    envelope["cases"] =
        json!(expectations.iter().map(|(church, donor_church, _, _)| {
        let mut input = baseline.clone();
        input["lønmodtager"]["betaler_kirkeskat"] = json!(church);
        input["ægtefælle"]["fakta"]["lønmodtager"]["betaler_kirkeskat"] = json!(donor_church);
        json!({"case_id":format!("loss-recipient-{church}-donor-{donor_church}"),"input":input})
    }).collect::<Vec<_>>());
    let path =
        std::env::temp_dir().join(format!("futuruna-spouse-loss-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expectations.len());
    for (row, (church, donor_church, expected, credit)) in results.iter().zip(expectations) {
        let result = &row["result"];
        let gate = &result["vurdering"];
        println!("loss recipient church={church}, donor church={donor_church}: tax={}, expected={expected}", gate["slutskat_til_sammenligning_øre"]);
        assert_eq!(gate["alle_kontroller_gyldige"], true, "{gate}");
        assert_eq!(gate["slutskat_til_sammenligning_øre"], expected);
        assert_eq!(
            result["indgående_ægtefælle"]["par13_indkomstfradrag_kroner"],
            493500
        );
        assert_eq!(
            result["indgående_ægtefælle"]["par13_skattemodregning_kroner"],
            credit
        );
        let loss = &result["ægtefælle"]["skat"]["par13_underskud"];
        assert_eq!(loss["fremført_efter_ægtefælle_kroner"], 0);
        assert_eq!(
            loss["ægtefælle_skattemodregning"]["dækket_underskud_ved_skattemodregning_kroner"],
            10000
        );
        let exact = &result["hovedskat_eksakt"];
        assert_eq!(
            exact["før_nedsættelser"]["par6_skat_øre"].as_i64().unwrap()
                - exact["efter_par13"]["par6_skat_øre"].as_i64().unwrap(),
            credit * 100
        );
    }
}

#[test]
fn spouse_allowance_uses_recipient_rates_from_source_facts() {
    let mut envelope = run(&["template", MODEL, "--format", "json"]);
    let baseline = fictional_spouse_rates_input(&envelope);
    // Observed independently in the anonymous calculator on 2026-09-25.
    // (Recipient church membership, donor church membership, donor wage, tax.)
    let expectations = [
        (false, false, 0, 11378698),
        (true, false, 0, 11548858),
        (false, false, 40000, 12569846),
        (false, false, 60000, 13162040),
        (false, false, 60001, 13162040),
        (true, true, 60000, 13370647),
    ];
    envelope["cases"] = json!(expectations
        .iter()
        .map(|(church, donor_church, wage, _)| {
            let mut input = baseline.clone();
            input["lønmodtager"]["betaler_kirkeskat"] = json!(church);
            input["ægtefælle"]["fakta"]["lønmodtager"]["betaler_kirkeskat"] = json!(donor_church);
            input["ægtefælle"]["fakta"]["lønmodtager"]["bruttoløn_kroner"] = json!(wage);
            json!({"case_id":format!("recipient-{church}-donor-{donor_church}-{wage}"),"input":input})
        })
        .collect::<Vec<_>>());
    let path =
        std::env::temp_dir().join(format!("futuruna-spouse-rates-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let output = run(&["call", MODEL, "--input", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert_eq!(output["diagnostics"], json!([]));
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), expectations.len());
    for (row, (church, donor_church, wage, expected)) in results.iter().zip(expectations) {
        let gate = &row["result"]["vurdering"];
        println!(
            "recipient church={church}, donor church={donor_church}, wage={wage}: valid={}, tax={}, expected={expected}",
            gate["alle_kontroller_gyldige"], gate["slutskat_til_sammenligning_øre"]
        );
        assert_eq!(gate["alle_kontroller_gyldige"], true, "{gate}");
        assert_eq!(gate["slutskat_til_sammenligning_øre"], expected);
    }
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
