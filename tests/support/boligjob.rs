use serde_json::{json, Value};

pub fn post(year: i64, work: &str, labour: i64) -> Value {
    json!({
        "betaling": {
            "identifikation":"invoice-line-payment-1", "kildereference":"synthetic-invoice-1-line-1",
            "betalingsreference":"synthetic-bank-transfer-1", "boligreference":"home-1",
            "ydelse":{"$variant":work}, "arbejdsdato":{"år":year,"måned":12,"dag":1},
            "betalingsdato":{"år":year,"måned":12,"dag":15}, "land":{"$variant":"Ll8VDanmark"},
            "eksisterende_bolig":{"$variant":"Ll8VJa"},
            "leverandør":{"$variant":"Ll8VVirksomhed","moms_eller_tredjelandsregistrering":{"$variant":"Ll8VDanskRegistrering"},"udenlandsk_virksomhed_i_danmark":{"$variant":"Ll8VNej"},"rut_registreret":{"$variant":"Ll8VUoplyst"}},
            "betalt_arbejdsløn_øre":labour, "betalte_materialer_kørsel_m_v_øre":1000000,
            "betalingsform":{"$variant":"Ll8VKortEllerMobilbetaling"},
            "arbejdsbilag_med_påkrævede_oplysninger":{"$variant":"Ll8VJa"}, "betalingsbilag":{"$variant":"Ll8VJa"},
            "offentligt_tilskud":{"$variant":"Ll8VNej"}, "samme_udgift_fradraget_efter_andre_regler":{"$variant":"Ll8VNej"},
            "børnepasning_skattefri_efter_par7æ":{"$variant":"Ll8VNej"},
            "udfører_bor_i_helårsboligen_eller_ejer_fritidsboligen_eller_bor_med_ejer":{"$variant":"Ll8VNej"},
            "betalerreference":"hovedperson", "andele":[{"personreference":"hovedperson","arbejdsløn_øre":labour}],
            "særligt_forhold":{"$variant":"Ll8VAlmindeligUdgift"}
        },
        "boligforhold":{"$variant":"Ll8VHelårsbolig","fast_bopæl_ved_arbejdet":{"$variant":"Ll8VJa"},"individuel_råderet_og_vedligeholdelsesret":{"$variant":"Ll8VJa"}},
        "fællesøkonomi_med_betalende_ægtefælle_eller_samlever":{"$variant":"Ll8VUoplyst"},
        "indberettet_med_leverandøroplysninger":{"$variant":"Ll8VJa"}
    })
}

pub fn facts(rows: Vec<Value>) -> Value {
    json!({"$variant":"OplysteBoligjobudgifter", "poster":rows,
        "oplysninger_for_året_komplette":true,"skattepligt":{"$variant":"Ll8VFuldtSkattepligtig"},
        "personreference":"hovedperson"})
}
