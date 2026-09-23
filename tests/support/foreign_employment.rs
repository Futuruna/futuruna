use serde_json::{json, Value};

/// Explicit fictional fact, never a template default or inferred legal status.
pub fn no_exclusion() -> Value {
    json!({"$variant":"IngenUdlandsudelukkelseIFællesForhold",
        "dbo_hjemmehørende_udland_i_nogen_periode":false,
        "noget_arbejde_udført_udland":null,
        "nogen_udenlandsk_arbejdsgiver":null,
        "kildereference":"fictional Danish treaty residence throughout year"})
}
