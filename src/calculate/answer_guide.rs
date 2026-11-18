//! The answer guide of an entry, generated from its law: one item per input
//! path pattern at which a need can arise, with the type of the value there
//! and the law's metadata (label, question, help, unit, sources, the key
//! fields of the list elements it lies in, and for a value another entry
//! provides, that entry and its subject). It says what an entry can need;
//! who answers a need is the integrator's concern.
//!
//! A pattern is spelled as the engine's need paths are, with every list or set
//! index written `[]` and every map key `*`: record fields joined with `.`
//! under their contract (wire) names, a sum type's tag as `.$variant`, and the
//! fields of a sum type's alternatives directly under the sum type's path,
//! without the alternative's name (`events[].amount`, as in
//! `events[2].amount`). Alternatives with a field of one name share its
//! pattern; the first, in declaration order, gives its type and metadata.

use super::{
    definition_substitutions, joined_column_path, product_variant, substitute_contract_type,
    CalculationContract, CalculationField, CalculationFieldMetadata, CalculationMetadataReference,
    CalculationNeedKind, CalculationTypeDefinition, CalculationTypeRef,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// One input path pattern of an entry at which a need can arise.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GuideNeed {
    /// The pattern, as a need's path with `[]` for list indices and `*` for map keys.
    pub pattern: String,
    /// `Entry` when another entry provides the value, else `Fact`.
    pub kind: CalculationNeedKind,
    /// The value's Futuruna type as written (`Int`, `Spouse?`, `List(Child)`,
    /// a unit's name for an `Int` of that unit, the sum type for a tag).
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The patterns of the key fields of the list elements the pattern lies
    /// in, outermost first (`accounts[].id`), where the element type declares one.
    pub keys: Vec<String>,
    pub sources: Vec<CalculationMetadataReference>,
    /// The entry that provides the value, for an entry need.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// The input path of the subject that entry runs for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// The answer guide of `contract`'s entry: every input path pattern at which
/// a need can arise, each once, in the order the input type declares them
/// (a value before what lies inside it).
pub fn answer_guide(contract: &CalculationContract) -> Vec<GuideNeed> {
    let mut found = Vec::new();
    Walk { contract }.value(
        &contract.input,
        &BTreeMap::new(),
        &At::default(),
        &mut Vec::new(),
        &mut found,
    );
    let mut guide: Vec<GuideNeed> = Vec::new();
    for (at, ty) in found {
        if guide.iter().any(|need| need.pattern == at.pattern) {
            continue;
        }
        guide.push(guide_need(contract, at, &ty));
    }
    guide
}

/// Where a value lies: its pattern, the metadata path of the field there
/// (alternatives named, no indices or map keys: `events.Trade.amount`), and
/// the key fields of the list elements above it.
#[derive(Clone, Default)]
struct At {
    pattern: String,
    metadata: String,
    keys: Vec<String>,
}

impl At {
    fn field(&self, name: &str) -> Self {
        At {
            pattern: joined_column_path(&self.pattern, name),
            metadata: joined_column_path(&self.metadata, name),
            keys: self.keys.clone(),
        }
    }

    fn element(&self) -> Self {
        At {
            pattern: format!("{}[]", self.pattern),
            ..self.clone()
        }
    }

    fn map_value(&self) -> Self {
        At {
            pattern: joined_column_path(&self.pattern, "*"),
            ..self.clone()
        }
    }

    fn alternative(&self, variant: &str) -> Self {
        At {
            metadata: joined_column_path(&self.metadata, variant),
            ..self.clone()
        }
    }
}

fn guide_need(contract: &CalculationContract, at: At, ty: &CalculationTypeRef) -> GuideNeed {
    let metadata: Option<&CalculationFieldMetadata> =
        contract.field_metadata_for_path(&at.metadata).or_else(|| {
            at.metadata
                .strip_suffix(".$variant")
                .and_then(|owner| contract.field_metadata_for_path(owner))
        });
    let provider = metadata.and_then(|metadata| metadata.provided_by.as_ref());
    GuideNeed {
        pattern: at.pattern,
        kind: if provider.is_some() {
            CalculationNeedKind::Entry
        } else {
            CalculationNeedKind::Fact
        },
        ty: written_type(ty),
        label: metadata.map(|metadata| metadata.label.clone()),
        question: metadata.and_then(|metadata| metadata.question.clone()),
        help: metadata.and_then(|metadata| metadata.help.clone()),
        unit: metadata.and_then(|metadata| metadata.unit.clone()),
        keys: at.keys,
        sources: metadata
            .map(|metadata| metadata.sources.clone())
            .unwrap_or_default(),
        entry: provider.map(|provider| provider.entry.clone()),
        subject: provider.map(|provider| provider.subject.clone()),
    }
}

struct Walk<'c> {
    contract: &'c CalculationContract,
}

impl Walk<'_> {
    /// The patterns of a value of `ty` at `at` at which a need can arise: the
    /// value itself when it is one fact, an optional's existence, a
    /// collection as a whole and its elements' facts, a sum type's tag and
    /// its alternatives' facts, a record's facts but never the record. A type
    /// that repeats on a path is one fact there, the cut of a recursive type.
    fn value(
        &self,
        ty: &CalculationTypeRef,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
        at: &At,
        active: &mut Vec<String>,
        out: &mut Vec<(At, CalculationTypeRef)>,
    ) {
        let ty = substitute_contract_type(ty, substitutions);
        match &ty {
            CalculationTypeRef::Optional { item } => {
                out.push((at.clone(), ty.clone()));
                let mut below = Vec::new();
                self.value(item, substitutions, at, active, &mut below);
                out.extend(
                    below
                        .into_iter()
                        .filter(|(path, _)| path.pattern != at.pattern),
                );
            }
            CalculationTypeRef::List { item } | CalculationTypeRef::Set { item } => {
                out.push((at.clone(), ty.clone()));
                let mut element = at.element();
                if let Some(key) = self.element_key(item, substitutions) {
                    element
                        .keys
                        .push(joined_column_path(&element.pattern, &key));
                }
                self.element(item, substitutions, &element, active, out);
            }
            CalculationTypeRef::Map { value, .. } => {
                out.push((at.clone(), ty.clone()));
                self.element(value, substitutions, &at.map_value(), active, out);
            }
            CalculationTypeRef::Named { .. } => self.named(&ty, at, false, active, out),
            CalculationTypeRef::Primitive { .. }
            | CalculationTypeRef::TypeParameter { .. }
            | CalculationTypeRef::Unit => out.push((at.clone(), ty.clone())),
        }
    }

    /// An element of a collection: a record's or an alternative's facts, a
    /// sum type's tag, or an enum or date left unknown inside the
    /// collection; a scalar element is never a need.
    fn element(
        &self,
        item: &CalculationTypeRef,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
        at: &At,
        active: &mut Vec<String>,
        out: &mut Vec<(At, CalculationTypeRef)>,
    ) {
        let item = substitute_contract_type(item, substitutions);
        match &item {
            CalculationTypeRef::Optional { item } => {
                self.element(item, substitutions, at, active, out)
            }
            CalculationTypeRef::Named { .. } => self.named(&item, at, true, active, out),
            _ => {}
        }
    }

    fn named(
        &self,
        ty: &CalculationTypeRef,
        at: &At,
        element: bool,
        active: &mut Vec<String>,
        out: &mut Vec<(At, CalculationTypeRef)>,
    ) {
        let CalculationTypeRef::Named { name, arguments } = ty else {
            return;
        };
        let Some(definition) = self.contract.definition(name) else {
            return;
        };
        if definition.variants.is_empty() {
            return;
        }
        if is_date(definition)
            || definition
                .variants
                .iter()
                .all(|variant| variant.fields.is_empty())
        {
            out.push((at.clone(), ty.clone()));
            return;
        }
        let key = ty.display_name();
        if active.contains(&key) {
            if !element {
                out.push((at.clone(), ty.clone()));
            }
            return;
        }
        active.push(key);
        let local = definition_substitutions(definition, arguments);
        if let Some(variant) = product_variant(definition) {
            for field in &variant.fields {
                self.field(field, &local, &at.field(&field.name), active, out);
            }
        } else {
            out.push((at.field("$variant"), ty.clone()));
            for variant in definition
                .variants
                .iter()
                .filter(|variant| !variant.positional)
            {
                let alternative = at.alternative(&variant.name);
                for field in &variant.fields {
                    self.field(field, &local, &alternative.field(&field.name), active, out);
                }
            }
        }
        active.pop();
    }

    /// A record field: its value, or for an open collection
    /// (`@ field(open = True)`), which is never asked whole, each entry by
    /// its key (`balances[]`, keyed `balances[].day`) and the entries' facts.
    fn field(
        &self,
        field: &CalculationField,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
        at: &At,
        active: &mut Vec<String>,
        out: &mut Vec<(At, CalculationTypeRef)>,
    ) {
        let ty = substitute_contract_type(&field.ty, substitutions);
        match &ty {
            CalculationTypeRef::List { item } if field.open => {
                let mut element = at.element();
                if let Some(key) = self.element_key(item, substitutions) {
                    element
                        .keys
                        .push(joined_column_path(&element.pattern, &key));
                }
                out.push((element.clone(), substitute_contract_type(item, substitutions)));
                self.element(item, substitutions, &element, active, out);
            }
            _ => self.value(&ty, substitutions, at, active, out),
        }
    }

    /// The key field a list element's type declares (`@ field(key = True)`).
    fn element_key(
        &self,
        item: &CalculationTypeRef,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
    ) -> Option<String> {
        let mut item = substitute_contract_type(item, substitutions);
        while let CalculationTypeRef::Optional { item: inner } = item {
            item = *inner;
        }
        let CalculationTypeRef::Named { name, .. } = &item else {
            return None;
        };
        self.contract.definition(name)?.key.clone()
    }
}

/// The prelude's `Date(year: Int, month: Int, day: Int)`: one fact, an ISO
/// date in the SDK.
pub(super) fn is_date(definition: &CalculationTypeDefinition) -> bool {
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

/// A contract type as Futuruna writes it, an `Int` of a unit as the unit's name.
pub(super) fn written_type(ty: &CalculationTypeRef) -> String {
    match ty {
        CalculationTypeRef::Primitive {
            unit: Some(unit), ..
        } => unit.clone(),
        CalculationTypeRef::Optional { item } => format!("{}?", written_type(item)),
        CalculationTypeRef::List { item } => format!("List({})", written_type(item)),
        CalculationTypeRef::Set { item } => format!("Set({})", written_type(item)),
        CalculationTypeRef::Map { key, value } => {
            format!("Map({}, {})", written_type(key), written_type(value))
        }
        CalculationTypeRef::Named { name, arguments } if !arguments.is_empty() => format!(
            "{name}({})",
            arguments
                .iter()
                .map(written_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => ty.display_name(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser};

    fn contract(source: &str, entry: &str) -> CalculationContract {
        let statements = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect("parse calculation source");
        super::super::extract_calculation_contracts(&statements, source, None)
            .expect("extract calculation contracts")
            .into_iter()
            .find(|contract| contract.entry == entry)
            .expect("the entry")
    }

    const HOUSEHOLD: &str = r#"# Spouse(income: Int, cohabiting: Bool)
# Child(@ field(label = "Name", key = True) name: String, @ field(label = "Age", question = "How old is the child?", help = "In whole years.", unit = "years") age: Int)
# Event = Trade(@ field(label = "Amount") amount: Int, fee: Int) | Dividend(amount: Int)
# Household(
    @ field(label = "Income") income: Int,
    married: Bool,
    spouse: Option(Spouse),
    children: List(Child),
    events: List(Event),
    @ field(label = "Spouse's share income", entry = refof(share_income), subject = pathof(Household::spouse))
    spouse_share: Int
)
# Result(tax: Int)

@ entry
| share_income(s: Spouse) -> s.income

@ entry
| household_tax(h: Household) -> Result(tax = h.income - h.spouse_share)
"#;

    #[test]
    fn the_guide_has_every_pattern_a_need_can_name_with_the_laws_metadata() {
        let guide = answer_guide(&contract(HOUSEHOLD, "household_tax"));
        let patterns: Vec<(&str, &str)> = guide
            .iter()
            .map(|need| (need.pattern.as_str(), need.ty.as_str()))
            .collect();
        assert_eq!(
            patterns,
            [
                ("income", "Int"),
                ("married", "Bool"),
                ("spouse", "Spouse?"),
                ("spouse.income", "Int"),
                ("spouse.cohabiting", "Bool"),
                ("children", "List(Child)"),
                ("children[].name", "String"),
                ("children[].age", "Int"),
                ("events", "List(Event)"),
                ("events[].$variant", "Event"),
                ("events[].amount", "Int"),
                ("events[].fee", "Int"),
                ("spouse_share", "Int"),
            ]
        );
        let need = |pattern: &str| {
            guide
                .iter()
                .find(|need| need.pattern == pattern)
                .expect("the pattern")
        };
        let age = need("children[].age");
        assert_eq!(age.kind, CalculationNeedKind::Fact);
        assert_eq!(age.label.as_deref(), Some("Age"));
        assert_eq!(age.question.as_deref(), Some("How old is the child?"));
        assert_eq!(age.help.as_deref(), Some("In whole years."));
        assert_eq!(age.unit.as_deref(), Some("years"));
        assert_eq!(age.keys, ["children[].name"]);
        assert_eq!(need("income").label.as_deref(), Some("Income"));
        assert_eq!(need("married").label, None);
        assert!(need("married").keys.is_empty());
        assert_eq!(need("events[].amount").label.as_deref(), Some("Amount"));
        let share = need("spouse_share");
        assert_eq!(share.kind, CalculationNeedKind::Entry);
        assert_eq!(share.entry.as_deref(), Some("share_income"));
        assert_eq!(share.subject.as_deref(), Some("spouse"));
    }

    #[test]
    fn a_recursive_type_is_one_fact_at_its_cut_and_a_date_is_one_fact() {
        let source = "# Date(year: Int, month: Int, day: Int)\n# Node(value: Int, next: Option(Node), on: Date)\n@ entry\n| total(n: Node) -> n.value\n";
        let guide = answer_guide(&contract(source, "total"));
        let patterns: Vec<(&str, &str)> = guide
            .iter()
            .map(|need| (need.pattern.as_str(), need.ty.as_str()))
            .collect();
        assert_eq!(
            patterns,
            [("value", "Int"), ("next", "Node?"), ("on", "Date"),]
        );
    }
}
