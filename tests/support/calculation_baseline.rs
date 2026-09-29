//! A test-chosen baseline for calculation inputs. Generated templates leave
//! every value unfilled; model tests that vary a few facts start from this
//! explicit baseline instead: zero, `false`, empty text and collections,
//! `null` for optional values and the first alternative of each choice.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

fn definition<'a>(schema: &'a Value, name: &str) -> Option<&'a Value> {
    schema["definitions"]
        .as_array()?
        .iter()
        .find(|definition| definition["name"] == name)
}

fn substitute(ty: &Value, substitutions: &BTreeMap<String, Value>) -> Value {
    if ty["kind"] == "type_parameter" {
        if let Some(argument) = ty["name"].as_str().and_then(|name| substitutions.get(name)) {
            return argument.clone();
        }
    }
    ty.clone()
}

fn fields(
    fields: &Value,
    schema: &Value,
    substitutions: &BTreeMap<String, Value>,
    active: &mut BTreeSet<String>,
) -> Vec<(String, Value)> {
    fields
        .as_array()
        .into_iter()
        .flatten()
        .map(|field| {
            (
                field["name"].as_str().unwrap().to_string(),
                baseline_value(&field["type"], schema, substitutions, active),
            )
        })
        .collect()
}

fn baseline_value(
    ty: &Value,
    schema: &Value,
    substitutions: &BTreeMap<String, Value>,
    active: &mut BTreeSet<String>,
) -> Value {
    let ty = substitute(ty, substitutions);
    match ty["kind"].as_str().unwrap_or_default() {
        "primitive" => match ty["name"].as_str().unwrap_or_default() {
            "Int" => json!(0),
            "Float" => json!(0.0),
            "Bool" => json!(false),
            "Char" => json!("x"),
            _ => json!(""),
        },
        "list" | "set" => json!([]),
        "map" => json!({}),
        "named" => {
            let name = ty["name"].as_str().unwrap();
            let Some(definition) = definition(schema, name) else {
                return Value::Null;
            };
            if !active.insert(name.to_string()) {
                return Value::Null;
            }
            let local: BTreeMap<String, Value> = definition["parameters"]
                .as_array()
                .into_iter()
                .flatten()
                .zip(ty["arguments"].as_array().into_iter().flatten())
                .map(|(parameter, argument)| {
                    (
                        parameter.as_str().unwrap().to_string(),
                        substitute(argument, substitutions),
                    )
                })
                .collect();
            let variant = &definition["variants"][0];
            let product = definition["variants"].as_array().map_or(0, Vec::len) == 1
                && variant["name"] == name
                && variant["positional"] == false;
            let mut object = Map::new();
            let values = fields(&variant["fields"], schema, &local, active);
            if !product {
                object.insert("$variant".into(), variant["name"].clone());
            }
            if variant["positional"] == true {
                object.insert(
                    "$values".into(),
                    Value::Array(values.into_iter().map(|(_, value)| value).collect()),
                );
            } else {
                object.extend(values);
            }
            active.remove(name);
            Value::Object(object)
        }
        _ => Value::Null,
    }
}

/// The baseline input for the contract exported by `runa schema`.
pub fn baseline_input(schema: &Value) -> Value {
    baseline_value(
        &schema["input"],
        schema,
        &BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}

/// Replace every case input of a generated template with the baseline.
pub fn fill_template(template: &mut Value, schema: &Value) {
    let input = baseline_input(schema);
    for case in template["cases"].as_array_mut().expect("template cases") {
        case["input"] = input.clone();
    }
}
