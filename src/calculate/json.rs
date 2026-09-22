//! Strict JSON decoding for external calculation inputs.
use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

/// Decode JSON without silently overwriting any object member. Decoded key
/// equality is used, so escaped-equivalent names are duplicates too.
pub fn parse_calculation_json(source: &str) -> Result<Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let value = JsonAtPath("$".into()).deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

struct JsonAtPath(String);

impl<'de> DeserializeSeed<'de> for JsonAtPath {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for JsonAtPath {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("JSON with unique object member names")
    }

    fn visit_bool<E: Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.into()))
    }

    fn visit_string<E: Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) =
            sequence.next_element_seed(JsonAtPath(format!("{}[{}]", self.0, values.len())))?
        {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            let quoted_key = serde_json::to_string(&key).expect("JSON key serializes");
            let path = format!("{}[{quoted_key}]", self.0);
            if values.contains_key(&key) {
                return Err(A::Error::custom(format!(
                    "duplicate JSON object member at {path}"
                )));
            }
            let value = object.next_value_seed(JsonAtPath(path))?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_identical_conflicting_and_escaped_duplicate_keys() {
        for source in [
            r#"{"annual_income": 0, "annual_income": 500000}"#,
            r#"{"annual_income": 500000, "annual_income": 0}"#,
            r#"{"annual_income": 500000, "annual_income": 500000}"#,
            r#"{"annual_income": 500000, "\u0061nnual_income": 0}"#,
        ] {
            let message = parse_calculation_json(source).unwrap_err().to_string();
            assert!(
                message.contains(r#"duplicate JSON object member at $["annual_income"]"#),
                "{message}"
            );
            assert!(message.contains("line 1 column"));
        }
    }

    #[test]
    fn identifies_nested_case_and_map_or_array_path() {
        let source = r#"{"cases":[{"input":{"rows":[{"a.b[0]":1,"a.b[0]":2}]}}]}"#;
        let message = parse_calculation_json(source).unwrap_err().to_string();
        assert!(
            message.contains(r#"$["cases"][0]["input"]["rows"][0]["a.b[0]"]"#),
            "{message}"
        );
        assert!(parse_calculation_json(r#"{"ø":1,"\u00f8":2}"#).is_err());
    }

    #[test]
    fn retains_scalar_precision_and_normal_json_semantics() {
        for source in [
            r#"{"min":-9223372036854775808,"max":9223372036854775807,"u64":18446744073709551615}"#,
            r#"{"float":1.125,"tiny":1e-300,"large":1e300,"negative_zero":-0.0,"bool":true,"null":null}"#,
            r#"{"text":"annual_income: 1, annual_income: 2","list":[{"a":1},{"a":2}],"ø":"\u00f8"}"#,
            " [false, null, \"escaped \\\" quote\", [], {}] \n",
            "-0",
            "0",
            "1e2",
            "-1e-2",
            "1.0",
            "\"scalar\"",
        ] {
            let expected: Value = serde_json::from_str(source).unwrap();
            assert_eq!(
                parse_calculation_json(source).unwrap(),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_trailing_data_bad_syntax_and_excessive_depth() {
        for source in ["{} {}", "{} trailing", "[1,]", "{\"a\":}", "NaN", "1e999"] {
            assert!(parse_calculation_json(source).is_err(), "{source}");
        }
        let deep = format!("{}0{}", "[".repeat(140), "]".repeat(140));
        let message = parse_calculation_json(&deep).unwrap_err().to_string();
        assert!(message.contains("recursion limit"), "{message}");
    }

    #[test]
    fn rejects_the_reproduced_public_boundary_batch() {
        let source = r#"{"cases":[{"input":{"annual_income":500000,"annual_income":0}}]}"#;
        assert!(parse_calculation_json(source)
            .unwrap_err()
            .to_string()
            .contains(r#"$["cases"][0]["input"]["annual_income"]"#));
    }
}
