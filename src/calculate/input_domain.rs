//! The input domain: the values an integrator can send, which are the values
//! the validators of `runa sdk ts` accept. The engine decodes any `i64` and
//! any `Date` record (a law may validate its dates itself); the SDK holds an
//! `Int` as a JavaScript number and a `Date` as an ISO `YYYY-MM-DD` string,
//! so it accepts less:
//!
//! - an `Int` (with a unit or a named scalar type too) is a safe integer,
//!   within ±(2^53 − 1) (`IntSchema`, `unitSchema`, `scalarSchema` of the
//!   runtime, `Number.isSafeInteger`);
//! - a `Date` is a day on the calendar in the years 0000 to 9999
//!   (`IsoDateSchema`, `isoDateParts`).
//!
//! Every other check the SDK's validators make (a finite `Float`, one `Char`,
//! a set without duplicates, closed objects) the engine's decoder makes too.
//! This module is the one statement of the domain on the Rust side: the SDK
//! generator's `Date` shape, the witnesses of `runa cases` (as SMT) and the
//! check of its cases read it, and a test holds it to the runtime's
//! validators.

use super::{
    definition_substitutions, product_variant, substitute_contract_type, CalculationContract,
    CalculationTypeDefinition, CalculationTypeRef,
};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// The largest safe integer, 2^53 − 1: the largest `Int` the SDK holds exactly.
pub const SAFE_INTEGER_MAX: i64 = (1 << 53) - 1;
/// The smallest safe integer, −(2^53 − 1).
pub const SAFE_INTEGER_MIN: i64 = -SAFE_INTEGER_MAX;
/// The years of an ISO `YYYY-MM-DD` date: four digits.
pub const DATE_YEAR_MIN: i64 = 0;
pub const DATE_YEAR_MAX: i64 = 9999;

/// The prelude's `Date(year: Int, month: Int, day: Int)`, its fields in that
/// order: one fact, an ISO date in the SDK. Another record named `Date` is a
/// record of integers there.
pub fn is_date(definition: &CalculationTypeDefinition) -> bool {
    definition.name == "Date"
        && definition.parameters.is_empty()
        && product_variant(definition).is_some_and(|variant| {
            variant.fields.len() == 3
                && ["year", "month", "day"]
                    .iter()
                    .zip(&variant.fields)
                    .all(|(name, field)| {
                        field.name == *name
                            && matches!(&field.ty, CalculationTypeRef::Primitive { name, unit: None, scalar: None } if name == "Int")
                    })
        })
}

/// Whether `ty` is a [`is_date`] `Date` of `contract`.
pub fn is_date_type(contract: &CalculationContract, ty: &CalculationTypeRef) -> bool {
    matches!(ty, CalculationTypeRef::Named { name, arguments } if arguments.is_empty()
        && contract.definition(name).is_some_and(is_date))
}

/// Whether an `Int` is in the domain.
pub fn int_in_domain(value: i64) -> bool {
    (SAFE_INTEGER_MIN..=SAFE_INTEGER_MAX).contains(&value)
}

/// The days of a month of the Gregorian calendar.
pub fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Whether a date is in the domain: a day on the calendar, years 0000 to 9999.
pub fn date_in_domain(year: i64, month: i64, day: i64) -> bool {
    (DATE_YEAR_MIN..=DATE_YEAR_MAX).contains(&year)
        && (1..=12).contains(&month)
        && day >= 1
        && day <= days_in_month(year, month)
}

/// The domain as SMT-LIB functions over `Int`: `(runa_int_in_domain n)` and
/// `(runa_valid_date year month day)`.
pub fn smt_definitions() -> String {
    format!(
        "(define-fun runa_int_in_domain ((n Int)) Bool (and (<= (- {max}) n) (<= n {max})))\n\
(define-fun runa_valid_date ((y Int) (m Int) (d Int)) Bool \
(and (<= {year_min} y {year_max}) (<= 1 m 12) (<= 1 d) (<= d (ite (= m 2) (ite (or (and (= (mod y 4) 0) (not (= (mod y 100) 0))) (= (mod y 400) 0)) 29 28) \
(ite (or (= m 4) (= m 6) (= m 9) (= m 11)) 30 31)))))\n",
        max = SAFE_INTEGER_MAX,
        year_min = DATE_YEAR_MIN,
        year_max = DATE_YEAR_MAX,
    )
}

impl CalculationContract {
    /// The first value of a complete input (canonical JSON, one the engine
    /// decodes) outside the domain: its path and why.
    pub fn input_domain_error(&self, input: &JsonValue) -> Option<String> {
        outside(input, &self.input, self, &BTreeMap::new(), "$")
    }

    /// The first value of a result (canonical JSON) outside the domain.
    pub fn output_domain_error(&self, output: &JsonValue) -> Option<String> {
        outside(output, &self.output, self, &BTreeMap::new(), "$")
    }
}

/// The first value under `path` outside the domain. The shape is the
/// decoder's to check; a value of another shape is passed over.
fn outside(
    value: &JsonValue,
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
) -> Option<String> {
    let ty = substitute_contract_type(ty, substitutions);
    match &ty {
        CalculationTypeRef::Primitive { name, .. } if name == "Int" => {
            let number = value.as_i64()?;
            (!int_in_domain(number)).then(|| {
                format!("{path}: {number} is not a safe integer (Int within ±(2^53 − 1))")
            })
        }
        CalculationTypeRef::Primitive { .. }
        | CalculationTypeRef::Unit
        | CalculationTypeRef::TypeParameter { .. } => None,
        CalculationTypeRef::Optional { item } => {
            if value.is_null() {
                None
            } else {
                outside(value, item, contract, substitutions, path)
            }
        }
        CalculationTypeRef::List { item } | CalculationTypeRef::Set { item } => value
            .as_array()?
            .iter()
            .enumerate()
            .find_map(|(index, element)| {
                outside(element, item, contract, substitutions, &format!("{path}[{index}]"))
            }),
        CalculationTypeRef::Map { value: item, .. } => {
            value.as_object()?.iter().find_map(|(key, element)| {
                outside(element, item, contract, substitutions, &format!("{path}.{key}"))
            })
        }
        CalculationTypeRef::Named { name, arguments } => {
            let definition = contract.definition(name)?;
            let object = value.as_object()?;
            if is_date(definition) {
                let part = |name: &str| object.get(name).and_then(JsonValue::as_i64);
                let (year, month, day) = (part("year")?, part("month")?, part("day")?);
                return (!date_in_domain(year, month, day)).then(|| {
                    format!("{path}: {year}-{month}-{day} is not a date on the calendar in the years 0000 to 9999")
                });
            }
            let local = definition_substitutions(definition, arguments);
            let variant = match product_variant(definition) {
                Some(variant) => variant,
                None => {
                    let chosen = object.get("$variant")?.as_str()?;
                    definition
                        .variants
                        .iter()
                        .find(|variant| variant.name == chosen)?
                }
            };
            if variant.positional {
                let values = object.get("$values")?.as_array()?;
                values
                    .iter()
                    .zip(&variant.fields)
                    .enumerate()
                    .find_map(|(index, (element, field))| {
                        outside(
                            element,
                            &field.ty,
                            contract,
                            &local,
                            &format!("{path}.$values[{index}]"),
                        )
                    })
            } else {
                variant.fields.iter().find_map(|field| {
                    outside(
                        object.get(&field.name)?,
                        &field.ty,
                        contract,
                        &local,
                        &format!("{path}.{}", field.name),
                    )
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The runtime's validators, which the SDKs import: the bounds above
    /// are theirs.
    const RUNTIME: &str = include_str!("sdk_ts_runtime.ts");

    #[test]
    fn the_domain_is_the_runtime_validators() {
        assert_eq!(SAFE_INTEGER_MAX, 9_007_199_254_740_991);
        for validator in [
            "export const IntSchema = z.number().refine(Number.isSafeInteger,",
            "return z.custom<U>((value) => Number.isSafeInteger(value),",
            "return of === \"Int\" ? Number.isSafeInteger(value) : typeof value === \"string\";",
            "const ISO_DATE = /^(\\d{4})-(\\d{2})-(\\d{2})$/;",
            "const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);",
            "const days = month === 2 ? (leap ? 29 : 28) : [4, 6, 9, 11].includes(month) ? 30 : 31;",
            "return month >= 1 && month <= 12 && day >= 1 && day <= days ? { year, month, day } : undefined;",
        ] {
            assert!(RUNTIME.contains(validator), "the runtime's validator changed: {validator}");
        }
    }

    #[test]
    fn dates_and_integers_in_the_domain() {
        assert!(int_in_domain(SAFE_INTEGER_MAX) && int_in_domain(SAFE_INTEGER_MIN));
        assert!(!int_in_domain(SAFE_INTEGER_MAX + 1) && !int_in_domain(SAFE_INTEGER_MIN - 1));
        assert!(date_in_domain(2024, 2, 29) && date_in_domain(2000, 2, 29));
        assert!(!date_in_domain(2026, 2, 29) && !date_in_domain(1900, 2, 29));
        assert!(!date_in_domain(2026, 4, 31) && !date_in_domain(2026, 13, 1));
        assert!(!date_in_domain(2026, 3, 0) && !date_in_domain(-404, 2, 1));
        assert!(date_in_domain(0, 1, 1) && date_in_domain(9999, 12, 31));
        assert!(!date_in_domain(10000, 1, 1));
    }
}
