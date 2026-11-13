//! TypeScript SDK generation for calculation contracts (`runa sdk ts`).
//!
//! Everything is derived from the contracts `runa schema` emits. The SDK reads
//! as ordinary TypeScript: camelCase field names, a sum type whose variants
//! carry no fields as a string union, and `Date` as an ISO `YYYY-MM-DD` string.
//! The wire is unchanged: the canonical JSON encoding that `decode_value`
//! reads and `encode_value` writes. Each entry's modules carry codec tables
//! (TypeScript name, wire name, encoding), and the shared runtime encodes a
//! case before a call and decodes the result and the needs' paths after it.
//! The partial-case types and the `Complete`/`Needs` result follow
//! `docs/rfcs/calculation-needs.md`; the client reaches an engine only through
//! a transport. The output depends only on the contracts, the source label and
//! the runtime path, so regenerating an unchanged model rewrites identical
//! files.

use super::{
    calculation_type_occurrence_paths, definition_substitutions, joined_column_path,
    product_variant, substitute_contract_type, CalculationContract, CalculationField,
    CalculationFieldMetadata, CalculationTypeDefinition, CalculationTypeRef, CalculationVariant,
    CONTRACT_SCHEMA, INPUT_SCHEMA, OUTPUT_SCHEMA,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// The output envelope schema of `runa call --partial`.
pub const PARTIAL_OUTPUT_SCHEMA: &str = "futuruna.calculate.partial-output.v1";

/// Where the runtime module goes, relative to the output directory, unless
/// `--runtime` says otherwise.
pub const DEFAULT_RUNTIME_PATH: &str = "runtime.ts";

/// The modules generated for the entries of one source file.
const ENTRY_MODULES: [&str; 5] = [
    "types.ts",
    "schema.ts",
    "partial.ts",
    "client.ts",
    "index.ts",
];

/// The fields of a Futuruna `Date`, in the order the engine sorts the paths of
/// a date's parts (`date.day` < `date.month` < `date.year`).
const DATE_PARTS: [&str; 3] = ["day", "month", "year"];

const PRIMITIVES: [&str; 5] = ["Int", "Float", "Bool", "String", "Char"];

const TS_RESERVED: [&str; 46] = [
    "abstract",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
];

/// One generated TypeScript module, named by its path relative to the output
/// directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    pub name: String,
    pub contents: String,
}

/// Generate the TypeScript SDK for every calculation entry of one source file.
///
/// `source` names the file in the generated headers. `runtime_path` is where
/// the runtime module goes, relative to the output directory (`runtime.ts`,
/// or `../runtime.ts` for one runtime shared by the SDKs of several files);
/// the entry modules import it from there. A file whose contracts cannot be
/// expressed (an unsupported type, two entries disagreeing on a type, two
/// fields with the same camelCase name, or a contract name that collides with
/// a generated name) is an error.
pub fn generate_typescript_sdk(
    contracts: &[CalculationContract],
    source: &str,
    runtime_path: &str,
) -> Result<Vec<GeneratedFile>, String> {
    let sdk = Sdk::new(contracts, source, runtime_specifier(runtime_path)?)?;
    Ok(vec![
        GeneratedFile {
            name: "types.ts".to_string(),
            contents: sdk.types_module(),
        },
        GeneratedFile {
            name: "schema.ts".to_string(),
            contents: sdk.schema_module(),
        },
        GeneratedFile {
            name: "partial.ts".to_string(),
            contents: sdk.partial_module(),
        },
        GeneratedFile {
            name: "client.ts".to_string(),
            contents: sdk.client_module(),
        },
        GeneratedFile {
            name: "index.ts".to_string(),
            contents: sdk.index_module(),
        },
        GeneratedFile {
            name: runtime_path.to_string(),
            contents: runtime_module(),
        },
    ])
}

/// The module specifier the entry modules import the runtime by:
/// `../runtime.ts` → `../runtime.js`, `runtime.ts` → `./runtime.js`.
fn runtime_specifier(path: &str) -> Result<String, String> {
    let invalid = |reason: &str| Err(format!("the runtime path `{path}` {reason}"));
    let Some(stem) = path.strip_suffix(".ts") else {
        return invalid("must name a `.ts` file");
    };
    if stem.is_empty() || stem.ends_with('/') || stem.ends_with(".d") {
        return invalid("must name a `.ts` file");
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return invalid("must be relative to the output directory, with `/` between directories");
    }
    let file = path.strip_prefix("./").unwrap_or(path);
    if ENTRY_MODULES.contains(&file) {
        return invalid("is the path of a generated entry module");
    }
    let specifier = format!("{stem}.js");
    if specifier.starts_with("./") || specifier.starts_with("../") {
        Ok(specifier)
    } else {
        Ok(format!("./{specifier}"))
    }
}

/// Where a type is written: inside `types.ts`, from another module (complete
/// types under the `types` namespace), or as its partial-case form.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Local,
    Qualified,
    Partial,
}

/// How a contract type appears in TypeScript.
#[derive(Clone, Copy)]
enum Shape<'d> {
    /// A record: an object with camelCase fields.
    Record(&'d CalculationVariant),
    /// A sum type whose variants carry no fields: the variant's name.
    Enum,
    /// A sum type with payload variants: a union tagged by `$variant`.
    Union,
    /// A type without values: `never`.
    Empty,
    /// The prelude's `Date(year: Int, month: Int, day: Int)`: an ISO date string.
    Date,
}

fn shape(definition: &CalculationTypeDefinition) -> Shape<'_> {
    if is_date(definition) {
        Shape::Date
    } else if let Some(variant) = product_variant(definition) {
        Shape::Record(variant)
    } else if definition.variants.is_empty() {
        Shape::Empty
    } else if definition
        .variants
        .iter()
        .all(|variant| variant.fields.is_empty())
    {
        Shape::Enum
    } else {
        Shape::Union
    }
}

fn is_date(definition: &CalculationTypeDefinition) -> bool {
    definition.name == "Date"
        && definition.parameters.is_empty()
        && product_variant(definition).is_some_and(|variant| {
            variant.fields.len() == 3
                && ["year", "month", "day"]
                    .iter()
                    .zip(&variant.fields)
                    .all(|(name, field)| {
                        field.name == *name
                            && matches!(&field.ty, CalculationTypeRef::Primitive { name, unit: None } if name == "Int")
                    })
        })
}

fn without_optional(ty: &CalculationTypeRef) -> &CalculationTypeRef {
    match ty {
        CalculationTypeRef::Optional { item } => without_optional(item),
        other => other,
    }
}

struct Entry<'a> {
    contract: &'a CalculationContract,
    /// `tax_report` → `taxReport`, the per-entry function prefix.
    value_name: String,
    /// `tax_report` → `TaxReport`, the per-entry type prefix.
    type_name: String,
    /// TypeScript input path pattern → TypeScript types of the values written there.
    patterns: BTreeMap<String, BTreeSet<String>>,
    /// A recursive input type has paths of any depth.
    open_paths: bool,
}

struct Sdk<'a> {
    source: &'a str,
    /// The module specifier of the runtime, such as `../runtime.js`.
    runtime: String,
    entries: Vec<Entry<'a>>,
    definitions: BTreeMap<&'a str, &'a CalculationTypeDefinition>,
    input_definitions: BTreeSet<&'a str>,
    /// The units of measure (`# unit Ore "DKK øre"`) the contracts use, by
    /// name, with their descriptions: each is a branded number type.
    units: BTreeMap<&'a str, &'a str>,
    /// (type, variant, field) → the field metadata of every input path the
    /// field occurs at, including the metadata of its `$variant` tag, and for
    /// a `Date` field without its own metadata, the metadata its parts share.
    field_metadata: BTreeMap<(&'a str, &'a str, &'a str), Vec<Cow<'a, CalculationFieldMetadata>>>,
}

impl<'a> Sdk<'a> {
    fn new(
        contracts: &'a [CalculationContract],
        source: &'a str,
        runtime: String,
    ) -> Result<Self, String> {
        if contracts.is_empty() {
            return Err(format!("{source} declares no `@ calculate` entry"));
        }
        let mut contracts: Vec<&CalculationContract> = contracts.iter().collect();
        contracts.sort_by(|left, right| left.entry.cmp(&right.entry));
        if let Some(pair) = contracts
            .windows(2)
            .find(|pair| pair[0].entry == pair[1].entry)
        {
            return Err(format!(
                "calculation entry `{}` is declared twice",
                pair[0].entry
            ));
        }

        let mut definitions: BTreeMap<&str, &CalculationTypeDefinition> = BTreeMap::new();
        let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
        for contract in &contracts {
            for definition in &contract.definitions {
                match definitions.get(definition.name.as_str()) {
                    Some(existing) if *existing != definition => {
                        return Err(format!(
                            "type `{}` differs between calculation entries `{}` and `{}`",
                            definition.name,
                            owners[definition.name.as_str()],
                            contract.entry
                        ));
                    }
                    Some(_) => {}
                    None => {
                        definitions.insert(&definition.name, definition);
                        owners.insert(&definition.name, &contract.entry);
                    }
                }
            }
        }

        let mut units: BTreeMap<&str, &str> = BTreeMap::new();
        for contract in &contracts {
            for unit in &contract.units {
                match units.insert(&unit.name, &unit.description) {
                    Some(other) if other != unit.description => {
                        return Err(format!(
                            "unit `{}` is described both as {other:?} and as {:?}",
                            unit.name, unit.description
                        ));
                    }
                    _ => {}
                }
            }
        }

        let mut sdk = Sdk {
            source,
            runtime,
            entries: Vec::new(),
            definitions,
            input_definitions: BTreeSet::new(),
            units,
            field_metadata: BTreeMap::new(),
        };
        for definition in sdk.definitions.values() {
            for variant in &definition.variants {
                let mut names: BTreeMap<String, &str> = BTreeMap::new();
                for field in &variant.fields {
                    sdk.validate_type(
                        &field.ty,
                        &definition.parameters,
                        &format!("field `{}` of `{}`", field.name, definition.name),
                    )?;
                    if variant.positional {
                        continue;
                    }
                    let name = camel_case(&field.name);
                    if let Some(other) = names.insert(name.clone(), &field.name) {
                        return Err(format!(
                            "fields `{other}` and `{}` of `{}` are both `{name}` in TypeScript; rename one",
                            field.name, definition.name
                        ));
                    }
                }
            }
        }
        for contract in &contracts {
            sdk.validate_type(
                &contract.input,
                &[],
                &format!("input of `{}`", contract.entry),
            )?;
            sdk.validate_type(
                &contract.output,
                &[],
                &format!("result of `{}`", contract.entry),
            )?;
        }

        let mut input_definitions = BTreeSet::new();
        for contract in &contracts {
            sdk.collect_definitions(&contract.input, &mut input_definitions);
        }
        sdk.input_definitions = input_definitions;

        for contract in &contracts {
            sdk.collect_field_metadata(contract);
        }
        for list in sdk.field_metadata.values_mut() {
            list.sort_by(|left, right| left.path.cmp(&right.path));
        }

        let mut entries = Vec::new();
        for contract in &contracts {
            let mut entry = Entry {
                contract,
                value_name: value_identifier(&contract.entry),
                type_name: pascal_case(&contract.entry),
                patterns: BTreeMap::new(),
                open_paths: false,
            };
            sdk.collect_paths(
                &contract.input,
                &BTreeMap::new(),
                "",
                &mut Vec::new(),
                &mut entry,
            );
            entries.push(entry);
        }
        sdk.entries = entries;
        sdk.check_exported_names()?;
        Ok(sdk)
    }

    fn validate_type(
        &self,
        ty: &CalculationTypeRef,
        parameters: &[String],
        context: &str,
    ) -> Result<(), String> {
        match ty {
            CalculationTypeRef::Primitive { name, unit } => {
                if !PRIMITIVES.contains(&name.as_str()) {
                    Err(format!("{context}: unsupported primitive type `{name}`"))
                } else if unit.is_some() && name != "Int" {
                    Err(format!(
                        "{context}: a unit of measure on `{name}`, not `Int`"
                    ))
                } else {
                    Ok(())
                }
            }
            CalculationTypeRef::Unit => Ok(()),
            CalculationTypeRef::Optional { item }
            | CalculationTypeRef::List { item }
            | CalculationTypeRef::Set { item } => self.validate_type(item, parameters, context),
            CalculationTypeRef::Map { key, value } => {
                if !matches!(key.as_ref(), CalculationTypeRef::Primitive { name, .. } if name == "String")
                {
                    return Err(format!("{context}: only `Map(String, T)` is supported"));
                }
                self.validate_type(value, parameters, context)
            }
            CalculationTypeRef::TypeParameter { name } => {
                if parameters.contains(name) {
                    Ok(())
                } else {
                    Err(format!("{context}: open type parameter `{name}`"))
                }
            }
            CalculationTypeRef::Named { name, arguments } => {
                let definition = self
                    .definitions
                    .get(name.as_str())
                    .ok_or_else(|| format!("{context}: the contract has no type `{name}`"))?;
                if definition.parameters.len() != arguments.len() {
                    return Err(format!(
                        "{context}: `{name}` takes {} type arguments, got {}",
                        definition.parameters.len(),
                        arguments.len()
                    ));
                }
                for argument in arguments {
                    self.validate_type(argument, parameters, context)?;
                }
                Ok(())
            }
        }
    }

    fn collect_definitions(&self, ty: &CalculationTypeRef, found: &mut BTreeSet<&'a str>) {
        match ty {
            CalculationTypeRef::Optional { item }
            | CalculationTypeRef::List { item }
            | CalculationTypeRef::Set { item } => self.collect_definitions(item, found),
            CalculationTypeRef::Map { value, .. } => self.collect_definitions(value, found),
            CalculationTypeRef::Named { name, arguments } => {
                for argument in arguments {
                    self.collect_definitions(argument, found);
                }
                if let Some((name, definition)) = self.definitions.get_key_value(name.as_str()) {
                    if found.insert(name) {
                        for variant in &definition.variants {
                            for field in &variant.fields {
                                self.collect_definitions(&field.ty, found);
                            }
                        }
                    }
                }
            }
            CalculationTypeRef::Primitive { .. }
            | CalculationTypeRef::TypeParameter { .. }
            | CalculationTypeRef::Unit => {}
        }
    }

    /// The shape of a named type, seen through `Optional`; `None` for other types.
    fn named_shape(&self, ty: &CalculationTypeRef) -> Option<Shape<'a>> {
        match without_optional(ty) {
            CalculationTypeRef::Named { name, .. } => self
                .definitions
                .get(name.as_str())
                .map(|definition| shape(definition)),
            _ => None,
        }
    }

    fn is_date_type(&self, ty: &CalculationTypeRef) -> bool {
        matches!(self.named_shape(ty), Some(Shape::Date))
    }

    fn is_enum_type(&self, ty: &CalculationTypeRef) -> bool {
        matches!(self.named_shape(ty), Some(Shape::Enum))
    }

    /// An enum or a `Date` is one value in TypeScript but a record on the
    /// wire, so inside a list or map its partial form adds `Unknown`, `{}`.
    fn is_atomic_record(&self, ty: &CalculationTypeRef) -> bool {
        matches!(self.named_shape(ty), Some(Shape::Enum | Shape::Date))
    }

    /// Field metadata is keyed by input path, with the variant name between a
    /// sum type's path and its fields (`events.AccountCharge.amount_ore`).
    fn collect_field_metadata(&mut self, contract: &'a CalculationContract) {
        for (name, paths) in calculation_type_occurrence_paths(contract) {
            let Some(definition) = self.definitions.get(name.as_str()).copied() else {
                continue;
            };
            let product = product_variant(definition).is_some();
            for path in &paths {
                for variant in &definition.variants {
                    let base = if product {
                        path.clone()
                    } else {
                        joined_column_path(path, &variant.name)
                    };
                    for field in &variant.fields {
                        let field_path = joined_column_path(&base, &field.name);
                        let mut found: Vec<Cow<'a, CalculationFieldMetadata>> =
                            [field_path.clone(), format!("{field_path}.$variant")]
                                .iter()
                                .filter_map(|candidate| contract.field_metadata_for_path(candidate))
                                .map(Cow::Borrowed)
                                .collect();
                        if found.is_empty() && self.is_date_type(&field.ty) {
                            let parts: Vec<&CalculationFieldMetadata> = DATE_PARTS
                                .iter()
                                .filter_map(|part| {
                                    contract
                                        .field_metadata_for_path(&format!("{field_path}.{part}"))
                                })
                                .collect();
                            if !parts.is_empty() {
                                found.push(Cow::Owned(date_metadata(&field_path, &parts)));
                            }
                        }
                        if found.is_empty() {
                            continue;
                        }
                        let list = self
                            .field_metadata
                            .entry((
                                definition.name.as_str(),
                                variant.name.as_str(),
                                field.name.as_str(),
                            ))
                            .or_default();
                        for metadata in found {
                            if !list.contains(&metadata) {
                                list.push(metadata);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Input path patterns in TypeScript terms: record fields and a sum
    /// type's fields join with `.` under their camelCase names, list and set
    /// elements are `[]`, map values are `*`, and the tag of a sum type with
    /// payload variants is `.$variant`. An enum or a `Date` is one value at its
    /// own path.
    fn collect_paths(
        &self,
        ty: &CalculationTypeRef,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
        path: &str,
        active: &mut Vec<String>,
        entry: &mut Entry<'a>,
    ) {
        let ty = substitute_contract_type(ty, substitutions);
        match &ty {
            CalculationTypeRef::Optional { item } => {
                self.collect_paths(item, substitutions, path, active, entry)
            }
            CalculationTypeRef::List { item } | CalculationTypeRef::Set { item } => {
                let element = format!("{path}[]");
                self.record_path(entry, &element, item, true);
                self.collect_paths(item, substitutions, &element, active, entry);
            }
            CalculationTypeRef::Map { value, .. } => {
                let element = joined_column_path(path, "*");
                self.record_path(entry, &element, value, true);
                self.collect_paths(value, substitutions, &element, active, entry);
            }
            CalculationTypeRef::Named { name, arguments } => {
                let definition = self.definitions[name.as_str()];
                let shape = shape(definition);
                if matches!(shape, Shape::Enum | Shape::Date | Shape::Empty) {
                    return;
                }
                let key = ty.display_name();
                if active.contains(&key) {
                    entry.open_paths = true;
                    return;
                }
                active.push(key);
                let local = definition_substitutions(definition, arguments);
                if let Shape::Record(variant) = shape {
                    for field in &variant.fields {
                        let field_ty = substitute_contract_type(&field.ty, &local);
                        let field_path = joined_column_path(path, &camel_case(&field.name));
                        self.record_path(entry, &field_path, &field_ty, false);
                        self.collect_paths(&field_ty, &local, &field_path, active, entry);
                    }
                } else {
                    let tag = joined_column_path(path, "$variant");
                    let names = definition
                        .variants
                        .iter()
                        .map(|variant| string_literal(&variant.name))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    entry.patterns.entry(tag).or_default().insert(names);
                    for variant in &definition.variants {
                        if variant.positional {
                            let values = joined_column_path(path, "$values");
                            let tuple = format!(
                                "[{}]",
                                variant
                                    .fields
                                    .iter()
                                    .map(|field| self.ts_type(
                                        &substitute_contract_type(&field.ty, &local),
                                        Side::Partial
                                    ))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                            entry
                                .patterns
                                .entry(values.clone())
                                .or_default()
                                .insert(tuple);
                            for (index, field) in variant.fields.iter().enumerate() {
                                let field_ty = substitute_contract_type(&field.ty, &local);
                                let field_path = format!("{values}[{index}]");
                                self.record_path(entry, &field_path, &field_ty, false);
                                self.collect_paths(&field_ty, &local, &field_path, active, entry);
                            }
                        } else {
                            for field in &variant.fields {
                                let field_ty = substitute_contract_type(&field.ty, &local);
                                let field_path = joined_column_path(path, &camel_case(&field.name));
                                self.record_path(entry, &field_path, &field_ty, false);
                                self.collect_paths(&field_ty, &local, &field_path, active, entry);
                            }
                        }
                    }
                }
                active.pop();
            }
            CalculationTypeRef::Primitive { .. }
            | CalculationTypeRef::TypeParameter { .. }
            | CalculationTypeRef::Unit => {}
        }
    }

    /// `element` is a list element or map value, where an enum or a `Date`
    /// may also be `Unknown`.
    fn record_path(
        &self,
        entry: &mut Entry<'a>,
        pattern: &str,
        ty: &CalculationTypeRef,
        element: bool,
    ) {
        let rendered = if element {
            self.element_type(ty, Side::Partial)
        } else {
            self.ts_type(ty, Side::Partial)
        };
        entry
            .patterns
            .entry(pattern.to_string())
            .or_default()
            .insert(rendered);
    }

    fn check_exported_names(&self) -> Result<(), String> {
        let mut names: Vec<(String, &str)> = Vec::new();
        for (name, _) in RUNTIME_EXPORTS {
            names.push((name.to_string(), "the runtime"));
        }
        names.push(("SCHEMA_HASH".to_string(), "schema.ts"));
        for unit in self.units.keys() {
            names.push((unit.to_string(), "types.ts"));
            names.push((format!("{unit}Schema"), "schema.ts"));
            names.push((value_identifier(unit), "schema.ts"));
        }
        for (name, definition) in &self.definitions {
            match shape(definition) {
                Shape::Date => {}
                Shape::Enum | Shape::Empty => {
                    names.push((name.to_string(), "types.ts"));
                    names.push((format!("{name}Schema"), "schema.ts"));
                }
                Shape::Record(_) | Shape::Union => {
                    names.push((name.to_string(), "types.ts"));
                    names.push((format!("{name}Schema"), "schema.ts"));
                    names.push((format!("{name}Codec"), "schema.ts"));
                }
            }
        }
        for name in self.partial_definitions() {
            names.push((format!("Partial{name}"), "partial.ts"));
            names.push((format!("Partial{name}Schema"), "partial.ts"));
        }
        for entry in &self.entries {
            let (types, values) = (&entry.type_name, &entry.value_name);
            for suffix in [
                "InputPaths",
                "InputPath",
                "InputPatterns",
                "InputPathSchema",
                "Need",
                "NeedSchema",
                "QueryResult",
                "QueryResultSchema",
            ] {
                names.push((format!("{types}{suffix}"), "partial.ts"));
            }
            names.push((format!("{values}SetAt"), "partial.ts"));
            names.push((values.clone(), "client.ts"));
            names.push((format!("{values}Query"), "client.ts"));
            names.push((format!("{values}Explained"), "client.ts"));
            names.push((format!("{values}QueryExplained"), "client.ts"));
            names.push((format!("{values}Entry"), "client.ts"));
            names.push((format!("{types}Resolvers"), "client.ts"));
        }
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for (name, module) in &names {
            if let Some(first) = seen.insert(name.as_str(), *module) {
                return Err(format!(
                    "the generated name `{name}` is needed in both {first} and {module}; rename the contract type or entry that produces it"
                ));
            }
        }
        Ok(())
    }

    /// The input types with a partial form: records and sum types with
    /// payload variants. An enum and a `Date` are partial only as a whole.
    fn partial_definitions(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.input_definitions.iter().copied().filter(|name| {
            matches!(
                shape(self.definitions[name]),
                Shape::Record(_) | Shape::Union | Shape::Empty
            )
        })
    }

    fn ts_type(&self, ty: &CalculationTypeRef, side: Side) -> String {
        match ty {
            CalculationTypeRef::Primitive {
                unit: Some(unit), ..
            } => match side {
                Side::Local => unit.clone(),
                Side::Qualified | Side::Partial => format!("types.{unit}"),
            },
            CalculationTypeRef::Primitive { name, unit: None } => match name.as_str() {
                "Int" | "Float" => "number",
                "Bool" => "boolean",
                _ => "string",
            }
            .to_string(),
            CalculationTypeRef::Unit => "null".to_string(),
            CalculationTypeRef::Optional { item } => {
                format!("{} | null", self.ts_type(item, side))
            }
            CalculationTypeRef::List { item } | CalculationTypeRef::Set { item } => {
                let item = self.element_type(item, side);
                if item.contains(" | ") {
                    format!("({item})[]")
                } else {
                    format!("{item}[]")
                }
            }
            CalculationTypeRef::Map { value, .. } => {
                format!("{{ [key: string]: {} }}", self.element_type(value, side))
            }
            CalculationTypeRef::TypeParameter { name } => name.clone(),
            CalculationTypeRef::Named { name, arguments } => {
                let shape = shape(self.definitions[name.as_str()]);
                if matches!(shape, Shape::Date) {
                    return "IsoDate".to_string();
                }
                let arguments = if arguments.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        arguments
                            .iter()
                            .map(|argument| self.ts_type(argument, side))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                match (side, shape) {
                    (Side::Local, _) => format!("{name}{arguments}"),
                    (Side::Qualified, _) | (Side::Partial, Shape::Enum) => {
                        format!("types.{name}{arguments}")
                    }
                    (Side::Partial, _) => format!("Partial{name}{arguments}"),
                }
            }
        }
    }

    /// The type of a list element or map value: in a partial case an enum or
    /// a `Date` there may also be `Unknown`.
    fn element_type(&self, item: &CalculationTypeRef, side: Side) -> String {
        let rendered = self.ts_type(item, side);
        if side == Side::Partial && self.is_atomic_record(item) {
            format!(
                "{rendered} | Unknown<{}>",
                self.ts_type(without_optional(item), side)
            )
        } else {
            rendered
        }
    }

    /// The Zod expression for `ty`; `Local` means inside `schema.ts`.
    fn zod(&self, ty: &CalculationTypeRef, side: Side) -> String {
        match ty {
            CalculationTypeRef::Primitive {
                unit: Some(unit), ..
            } => match side {
                Side::Local => format!("{unit}Schema"),
                Side::Qualified | Side::Partial => format!("schema.{unit}Schema"),
            },
            CalculationTypeRef::Primitive { name, unit: None } => match name.as_str() {
                "Int" => "IntSchema",
                "Float" => "FloatSchema",
                "Char" => "CharSchema",
                "Bool" => "z.boolean()",
                _ => "z.string()",
            }
            .to_string(),
            CalculationTypeRef::Unit => "z.null()".to_string(),
            CalculationTypeRef::Optional { item } => format!("{}.nullable()", self.zod(item, side)),
            CalculationTypeRef::List { item } => {
                format!("z.array({})", self.element_zod(item, side))
            }
            CalculationTypeRef::Set { item } => {
                format!("SetSchema({})", self.element_zod(item, side))
            }
            CalculationTypeRef::Map { value, .. } => {
                format!("z.record(z.string(), {})", self.element_zod(value, side))
            }
            CalculationTypeRef::TypeParameter { name } => format!("schema_{name}"),
            CalculationTypeRef::Named { name, arguments } => {
                let shape = shape(self.definitions[name.as_str()]);
                let schema = match (side, shape) {
                    (_, Shape::Date) => return "IsoDateSchema".to_string(),
                    (Side::Local, _) => format!("{name}Schema"),
                    (Side::Qualified, _) | (Side::Partial, Shape::Enum) => {
                        format!("schema.{name}Schema")
                    }
                    (Side::Partial, _) => format!("Partial{name}Schema"),
                };
                if arguments.is_empty() {
                    schema
                } else {
                    format!(
                        "{schema}({})",
                        arguments
                            .iter()
                            .map(|argument| self.zod(argument, side))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
        }
    }

    fn element_zod(&self, item: &CalculationTypeRef, side: Side) -> String {
        let schema = self.zod(item, side);
        if side == Side::Partial && self.is_atomic_record(item) {
            format!("z.union([{schema}, UnknownSchema])")
        } else {
            schema
        }
    }

    /// The codec expression for `ty`; `qualified` reaches the codecs of
    /// named types through the `schema` namespace. Optional values need no
    /// codec of their own: `null` is the same on both sides.
    fn codec(&self, ty: &CalculationTypeRef, qualified: bool) -> String {
        let wrap = |constructor: &str, item: &CalculationTypeRef| {
            let item = self.codec(item, qualified);
            if item == "valueCodec" {
                item
            } else {
                format!("{constructor}({item})")
            }
        };
        match ty {
            CalculationTypeRef::Primitive { .. } | CalculationTypeRef::Unit => {
                "valueCodec".to_string()
            }
            CalculationTypeRef::Optional { item } => self.codec(item, qualified),
            CalculationTypeRef::List { item } | CalculationTypeRef::Set { item } => {
                wrap("listCodec", item)
            }
            CalculationTypeRef::Map { value, .. } => wrap("mapCodec", value),
            CalculationTypeRef::TypeParameter { name } => format!("codec_{name}"),
            CalculationTypeRef::Named { name, arguments } => {
                match shape(self.definitions[name.as_str()]) {
                    Shape::Date => "isoDateCodec".to_string(),
                    Shape::Enum => "enumCodec".to_string(),
                    Shape::Empty => "valueCodec".to_string(),
                    Shape::Record(_) | Shape::Union => {
                        let prefix = if qualified { "schema." } else { "" };
                        if arguments.is_empty() {
                            format!("() => {prefix}{name}Codec")
                        } else {
                            format!(
                                "() => {prefix}{name}Codec({})",
                                arguments
                                    .iter()
                                    .map(|argument| self.codec(argument, qualified))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                    }
                }
            }
        }
    }

    fn header(&self, out: &mut String, purpose: &str) {
        let _ = writeln!(
            out,
            "// Generated by `runa sdk ts` from {}. Do not edit; regenerate instead.",
            self.source
        );
        let _ = writeln!(out, "// {purpose}");
        let _ = writeln!(out, "// Contract {CONTRACT_SCHEMA}; schema hash per entry:");
        for entry in &self.entries {
            let _ = writeln!(
                out,
                "//   {}: {}",
                entry.contract.entry, entry.contract.schema_hash
            );
        }
        out.push('\n');
    }

    /// The import lines of a module whose code is `body`: each namespace in
    /// `namespaces` (name, specifier, type-only) and each runtime export the
    /// code uses.
    fn imports(&self, body: &str, namespaces: &[(&str, &str, bool)], type_only: bool) -> String {
        let code: Vec<&str> = body
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !(line.starts_with("//") || line.starts_with("/*") || line.starts_with('*'))
            })
            .collect();
        let uses = |name: &str| code.iter().any(|line| uses_identifier(line, name));
        let mut out = String::new();
        if uses("z") {
            out.push_str("import { z } from \"zod\";\n");
        }
        for (name, specifier, type_only) in namespaces {
            if code.iter().any(|line| line.contains(&format!("{name}."))) {
                let keyword = if *type_only { "import type" } else { "import" };
                let _ = writeln!(out, "{keyword} * as {name} from \"{specifier}\";");
            }
        }
        let used: Vec<String> = RUNTIME_EXPORTS
            .iter()
            .filter(|(name, _)| uses(name))
            .map(|(name, is_type)| {
                if *is_type && !type_only {
                    format!("type {name}")
                } else {
                    name.to_string()
                }
            })
            .collect();
        if !used.is_empty() {
            let keyword = if type_only { "import type" } else { "import" };
            let specifier = string_literal(&self.runtime);
            let line = format!("{keyword} {{ {} }} from {specifier};", used.join(", "));
            if line.len() <= 100 {
                let _ = writeln!(out, "{line}");
            } else {
                let _ = writeln!(out, "{keyword} {{");
                for name in &used {
                    let _ = writeln!(out, "  {name},");
                }
                let _ = writeln!(out, "}} from {specifier};");
            }
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    fn types_module(&self) -> String {
        let mut body = String::new();
        for (unit, description) in &self.units {
            write_doc(
                &mut body,
                "",
                &[
                    sentence(description),
                    String::new(),
                    format!(
                        "A Futuruna `{unit}`: a safe integer of this unit, on the wire the same JSON number. Only `{}(n)` and the validators make one; a bare number or a value of another unit is not one.",
                        value_identifier(unit)
                    ),
                ],
            );
            let _ = writeln!(
                body,
                "export type {unit} = number & {{ readonly __unit: {} }};\n",
                string_literal(unit)
            );
        }
        for definition in self.definitions.values() {
            if matches!(shape(definition), Shape::Date) {
                continue;
            }
            self.write_type(&mut body, definition);
            body.push('\n');
        }
        let mut out = String::new();
        self.header(
            &mut out,
            "Types of every calculation input and result. The SDK writes them in runa call's JSON on the wire.",
        );
        out.push_str(
            "// Field names are camelCase (snake_case on the wire). A sum type whose variants carry\n// no fields is its variant's name; other sum types are unions tagged by `$variant`.\n// A `Date` is an ISO `YYYY-MM-DD` string. Int is `number` and holds safe integers\n// only; the validators in schema.ts reject anything else. An optional value is `T | null`.\n// An Int with a unit of measure (`# unit Ore \"DKK øre\"`) is the branded number type `Ore`.\n\n",
        );
        out.push_str(&self.imports(&body, &[], true));
        out.push_str(&body);
        out
    }

    fn write_type(&self, out: &mut String, definition: &CalculationTypeDefinition) {
        let parameters = type_parameters(definition);
        let name = &definition.name;
        match shape(definition) {
            Shape::Record(variant) => {
                write_doc(
                    out,
                    "",
                    &[format!(
                        "Futuruna record `{}`.",
                        display_definition(definition)
                    )],
                );
                let _ = writeln!(out, "export type {name}{parameters} = {{");
                for field in &variant.fields {
                    self.write_field(out, "  ", definition, variant, field);
                }
                out.push_str("};\n");
            }
            Shape::Empty => {
                write_doc(
                    out,
                    "",
                    &[format!(
                        "Futuruna type `{}`, which has no values.",
                        display_definition(definition)
                    )],
                );
                let _ = writeln!(out, "export type {name}{parameters} = never;");
            }
            Shape::Enum => {
                write_doc(
                    out,
                    "",
                    &[format!(
                        "Futuruna sum type `{}`; its variants carry no fields, so a value is its variant's name.",
                        display_definition(definition)
                    )],
                );
                let _ = writeln!(
                    out,
                    "export type {name}{parameters} = {};",
                    variant_names(definition)
                );
            }
            Shape::Union => {
                write_doc(
                    out,
                    "",
                    &[format!(
                        "Futuruna sum type `{}`, tagged by `$variant`: {}.",
                        display_definition(definition),
                        variant_list(definition)
                    )],
                );
                let _ = writeln!(out, "export type {name}{parameters} =");
                let mut members = Vec::new();
                for variant in &definition.variants {
                    let tag = string_literal(&variant.name);
                    if variant.positional {
                        members.push(format!(
                            "  | {{ $variant: {tag}; $values: [{}] }}",
                            variant
                                .fields
                                .iter()
                                .map(|field| self.ts_type(&field.ty, Side::Local))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    } else if variant.fields.is_empty() {
                        members.push(format!("  | {{ $variant: {tag} }}"));
                    } else {
                        let mut member = String::from("  | {\n");
                        let _ = writeln!(member, "      $variant: {tag};");
                        for field in &variant.fields {
                            self.write_field(&mut member, "      ", definition, variant, field);
                        }
                        member.push_str("    }");
                        members.push(member);
                    }
                }
                out.push_str(&members.join("\n"));
                out.push_str(";\n");
            }
            Shape::Date => {}
        }
    }

    fn write_field(
        &self,
        out: &mut String,
        indent: &str,
        definition: &CalculationTypeDefinition,
        variant: &CalculationVariant,
        field: &CalculationField,
    ) {
        write_doc(out, indent, &self.field_doc(definition, variant, field));
        let _ = writeln!(
            out,
            "{indent}{}: {};",
            property_name(&camel_case(&field.name)),
            self.ts_type(&field.ty, Side::Local)
        );
    }

    /// The field's metadata (label, question, help, unit and sources) at every
    /// input path it occurs at, followed by its Futuruna type.
    fn field_doc(
        &self,
        definition: &CalculationTypeDefinition,
        variant: &CalculationVariant,
        field: &CalculationField,
    ) -> Vec<String> {
        // An enum's `$variant` is the value itself, not a tag.
        let tag_label = !self.is_enum_type(&field.ty);
        let mut groups: Vec<(Vec<String>, Vec<&str>)> = Vec::new();
        if let Some(list) = self.field_metadata.get(&(
            definition.name.as_str(),
            variant.name.as_str(),
            field.name.as_str(),
        )) {
            for metadata in list {
                let block = metadata_block(metadata, tag_label);
                match groups.iter_mut().find(|(existing, _)| *existing == block) {
                    Some((_, paths)) => paths.push(&metadata.path),
                    None => groups.push((block, vec![&metadata.path])),
                }
            }
        }
        let mut lines = Vec::new();
        if groups.len() == 1 {
            lines.extend(groups.remove(0).0);
            lines.push(String::new());
        } else {
            for (block, paths) in groups {
                lines.push(format!(
                    "At {}:",
                    paths
                        .iter()
                        .map(|path| format!("`{}`", camel_case_path(path)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                lines.extend(block);
                lines.push(String::new());
            }
        }
        let mut notes = Vec::new();
        if mentions_int(&field.ty) {
            notes.push("Int values are safe integers");
        }
        if self.mentions_date(&field.ty) {
            notes.push("a `Date` is an ISO `YYYY-MM-DD` string");
        }
        let mut type_line = format!("Futuruna type: `{}`", futuruna_type(&field.ty));
        for note in notes {
            type_line.push_str("; ");
            type_line.push_str(note);
        }
        type_line.push('.');
        lines.push(type_line);
        lines
    }

    fn mentions_date(&self, ty: &CalculationTypeRef) -> bool {
        match ty {
            CalculationTypeRef::Optional { item }
            | CalculationTypeRef::List { item }
            | CalculationTypeRef::Set { item } => self.mentions_date(item),
            CalculationTypeRef::Map { value, .. } => self.mentions_date(value),
            CalculationTypeRef::Named { arguments, .. } => {
                self.is_date_type(ty)
                    || arguments
                        .iter()
                        .any(|argument| self.mentions_date(argument))
            }
            CalculationTypeRef::Primitive { .. }
            | CalculationTypeRef::TypeParameter { .. }
            | CalculationTypeRef::Unit => false,
        }
    }

    fn schema_module(&self) -> String {
        let mut body = String::new();
        body.push_str("/** The contract fingerprint of each calculation entry. */\nexport const SCHEMA_HASH = {\n");
        for entry in &self.entries {
            let _ = writeln!(
                body,
                "  {}: {},",
                property_name(&entry.contract.entry),
                string_literal(&entry.contract.schema_hash)
            );
        }
        body.push_str("} as const;\n");
        for (unit, description) in &self.units {
            let _ = writeln!(
                body,
                "\n/** Validates a `{unit}` ({}): a safe integer, typed `{unit}` once parsed. */\nexport const {unit}Schema: z.ZodType<types.{unit}> = unitSchema<types.{unit}>();\n",
                escape_doc(description)
            );
            let _ = writeln!(
                body,
                "/** `value` as a `{unit}`; throws a `RangeError` unless it is a safe integer. */\nexport function {}(value: number): types.{unit} {{\n  return unitValue<types.{unit}>({}, value);\n}}",
                value_identifier(unit),
                string_literal(unit)
            );
        }
        for definition in self.definitions.values() {
            if matches!(shape(definition), Shape::Date) {
                continue;
            }
            body.push('\n');
            self.write_schema(&mut body, definition, Side::Local);
        }
        body.push_str(
            "\n// Wire codecs: each TypeScript field name with the field's name in runa call's JSON\n// and its codec. The runtime's toWire and fromWire read them.\n",
        );
        for definition in self.definitions.values() {
            if matches!(shape(definition), Shape::Record(_) | Shape::Union) {
                body.push('\n');
                self.write_codec(&mut body, definition);
            }
        }
        let mut out = String::new();
        self.header(
            &mut out,
            "Zod validators and wire codecs for every calculation input and result type.",
        );
        out.push_str(&self.imports(&body, &[("types", "./types.js", true)], false));
        out.push_str(&body);
        out
    }

    /// `side` is `Local` for the complete schema in `schema.ts` and `Partial`
    /// for the partial-case schema in `partial.ts`.
    fn write_schema(&self, out: &mut String, definition: &CalculationTypeDefinition, side: Side) {
        let (type_name, schema_name) = match side {
            Side::Partial => (
                format!("Partial{}", definition.name),
                format!("Partial{}Schema", definition.name),
            ),
            _ => (
                format!("types.{}", definition.name),
                format!("{}Schema", definition.name),
            ),
        };
        if let Shape::Enum = shape(definition) {
            let names = definition
                .variants
                .iter()
                .map(|variant| string_literal(&variant.name))
                .collect::<Vec<_>>()
                .join(", ");
            if definition.parameters.is_empty() {
                let _ = writeln!(
                    out,
                    "export const {schema_name}: z.ZodType<{type_name}> = z.enum([{names}]);"
                );
            } else {
                let parameters = definition.parameters.join(", ");
                let arguments = definition
                    .parameters
                    .iter()
                    .map(|parameter| format!("_schema_{parameter}: z.ZodType<{parameter}>"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "export const {schema_name} = <{parameters}>({arguments}): z.ZodType<{type_name}<{parameters}>> =>\n  z.enum([{names}]);"
                );
            }
            return;
        }
        let body = self.schema_body(definition, side);
        if definition.parameters.is_empty() {
            let _ = writeln!(
                out,
                "export const {schema_name}: z.ZodType<{type_name}> = z.lazy(() =>\n{body},\n);"
            );
        } else {
            let parameters = definition.parameters.join(", ");
            let arguments = definition
                .parameters
                .iter()
                .map(|parameter| format!("schema_{parameter}: z.ZodType<{parameter}>"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "export const {schema_name} = <{parameters}>({arguments}): z.ZodType<{type_name}<{parameters}>> =>\n  z.lazy(() =>\n{body},\n  ) as unknown as z.ZodType<{type_name}<{parameters}>>;"
            );
        }
    }

    fn schema_body(&self, definition: &CalculationTypeDefinition, side: Side) -> String {
        let partial = side == Side::Partial;
        let field_schema = |field: &CalculationField| {
            let schema = self.zod(&field.ty, side);
            if partial {
                format!("{schema}.optional()")
            } else {
                schema
            }
        };
        if let Some(variant) = product_variant(definition) {
            let mut body = String::from("  z\n    .object({\n");
            for field in &variant.fields {
                let _ = writeln!(
                    body,
                    "      {}: {},",
                    property_name(&camel_case(&field.name)),
                    field_schema(field)
                );
            }
            body.push_str("    })\n    .strict()");
            return body;
        }
        if definition.variants.is_empty() {
            return "  z.never()".to_string();
        }
        let objects = definition
            .variants
            .iter()
            .map(|variant| {
                let tag = format!("$variant: z.literal({})", string_literal(&variant.name));
                if variant.positional {
                    let values = format!(
                        "z.tuple([{}])",
                        variant
                            .fields
                            .iter()
                            .map(|field| self.zod(&field.ty, side))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    let values = if partial {
                        format!("{values}.optional()")
                    } else {
                        values
                    };
                    format!("      z.object({{ {tag}, $values: {values} }}).strict(),\n")
                } else if variant.fields.is_empty() {
                    format!("      z.object({{ {tag} }}).strict(),\n")
                } else {
                    let mut object = format!("      z\n        .object({{\n          {tag},\n");
                    for field in &variant.fields {
                        let _ = writeln!(
                            object,
                            "          {}: {},",
                            property_name(&camel_case(&field.name)),
                            field_schema(field)
                        );
                    }
                    object.push_str("        })\n        .strict(),\n");
                    object
                }
            })
            .collect::<String>();
        let union = format!("z.discriminatedUnion(\"$variant\", [\n{objects}    ])");
        if partial {
            format!("  z.union([\n    UnknownSchema,\n    {union},\n  ])")
        } else {
            format!("  {union}")
        }
    }

    fn write_codec(&self, out: &mut String, definition: &CalculationTypeDefinition) {
        let name = &definition.name;
        let fields = |out: &mut String, indent: &str, variant: &CalculationVariant| {
            for field in &variant.fields {
                let _ = writeln!(
                    out,
                    "{indent}{}: [{}, {}],",
                    property_name(&camel_case(&field.name)),
                    string_literal(&field.name),
                    self.codec(&field.ty, false)
                );
            }
        };
        let mut body = String::new();
        match shape(definition) {
            Shape::Record(variant) => {
                body.push_str("recordCodec({\n");
                fields(&mut body, "  ", variant);
                body.push_str("})");
            }
            _ => {
                body.push_str("unionCodec({\n");
                for variant in &definition.variants {
                    let tag = property_name(&variant.name);
                    if variant.positional {
                        let _ = writeln!(
                            body,
                            "  {tag}: {{ $values: [{}] }},",
                            variant
                                .fields
                                .iter()
                                .map(|field| self.codec(&field.ty, false))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    } else if variant.fields.is_empty() {
                        let _ = writeln!(body, "  {tag}: {{}},");
                    } else {
                        let _ = writeln!(body, "  {tag}: {{");
                        fields(&mut body, "    ", variant);
                        body.push_str("  },\n");
                    }
                }
                body.push_str("})");
            }
        }
        if definition.parameters.is_empty() {
            let _ = writeln!(out, "export const {name}Codec: Codec = {body};");
        } else {
            let arguments = definition
                .parameters
                .iter()
                .map(|parameter| format!("codec_{parameter}: CodecRef"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "export const {name}Codec = ({arguments}): Codec =>\n  {};",
                body.replace('\n', "\n  ")
            );
        }
    }

    fn partial_module(&self) -> String {
        let mut body = String::new();
        let mut first = true;
        for name in self.partial_definitions() {
            let definition = self.definitions[name];
            if !first {
                body.push('\n');
            }
            first = false;
            self.write_partial_type(&mut body, definition);
            body.push('\n');
            self.write_schema(&mut body, definition, Side::Partial);
        }
        for entry in &self.entries {
            if !first {
                body.push('\n');
            }
            first = false;
            self.write_entry_paths(&mut body, entry);
        }
        let mut out = String::new();
        self.header(
            &mut out,
            "Partial cases, the Complete/Needs result of a partial call, typed needs and setAt.",
        );
        out.push_str(&self.imports(
            &body,
            &[
                ("types", "./types.js", true),
                ("schema", "./schema.js", false),
            ],
            false,
        ));
        out.push_str(&body);
        out
    }

    fn write_partial_type(&self, out: &mut String, definition: &CalculationTypeDefinition) {
        let parameters = type_parameters(definition);
        let name = &definition.name;
        match shape(definition) {
            Shape::Record(variant) => {
                write_doc(
                    out,
                    "",
                    &[format!(
                        "{{@link types.{name}}} in a partial case: a field left out is unknown, and `{{}}` is a record whose fields are all unknown."
                    )],
                );
                let _ = writeln!(out, "export type Partial{name}{parameters} = {{");
                for field in &variant.fields {
                    write_doc(out, "  ", &self.field_doc(definition, variant, field));
                    let _ = writeln!(
                        out,
                        "  {}?: {} | undefined;",
                        property_name(&camel_case(&field.name)),
                        self.ts_type(&field.ty, Side::Partial)
                    );
                }
                out.push_str("};\n");
            }
            Shape::Union => {
                write_doc(
                    out,
                    "",
                    &[format!(
                        "{{@link types.{name}}} in a partial case: `{{}}` (or the field left out) is a variant not known yet; once `$variant` is given, a field left out is unknown."
                    )],
                );
                let _ = writeln!(out, "export type Partial{name}{parameters} =");
                let mut members = vec![format!("  | Unknown<types.{name}{parameters}>")];
                for variant in &definition.variants {
                    let tag = string_literal(&variant.name);
                    if variant.positional {
                        members.push(format!(
                            "  | {{ $variant: {tag}; $values?: [{}] | undefined }}",
                            variant
                                .fields
                                .iter()
                                .map(|field| self.ts_type(&field.ty, Side::Partial))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    } else if variant.fields.is_empty() {
                        members.push(format!("  | {{ $variant: {tag} }}"));
                    } else {
                        let mut member = format!("  | {{\n      $variant: {tag};\n");
                        for field in &variant.fields {
                            write_doc(
                                &mut member,
                                "      ",
                                &self.field_doc(definition, variant, field),
                            );
                            let _ = writeln!(
                                member,
                                "      {}?: {} | undefined;",
                                property_name(&camel_case(&field.name)),
                                self.ts_type(&field.ty, Side::Partial)
                            );
                        }
                        member.push_str("    }");
                        members.push(member);
                    }
                }
                out.push_str(&members.join("\n"));
                out.push_str(";\n");
            }
            Shape::Empty => {
                let _ = writeln!(out, "export type Partial{name}{parameters} = never;");
            }
            Shape::Enum | Shape::Date => {}
        }
    }

    fn write_entry_paths(&self, out: &mut String, entry: &Entry<'_>) {
        let contract = entry.contract;
        let types = &entry.type_name;
        let values = &entry.value_name;
        let input = self.ts_type(&contract.input, Side::Partial);
        let output = self.ts_type(&contract.output, Side::Qualified);
        let output_schema = self.zod(&contract.output, Side::Qualified);
        let _ = writeln!(out, "// `{}`\n", contract.entry);

        write_doc(
            out,
            "",
            &[
                format!(
                    "Every input path of `{}` a need can name, with the value an answer writes there.",
                    contract.entry
                ),
                "List indices are `[${number}]` and map keys `${string}`.".to_string(),
            ],
        );
        let _ = writeln!(out, "export interface {types}InputPaths {{");
        for (pattern, value_types) in &entry.patterns {
            // A map key is `${string}`, which also matches every longer path
            // through that map; TypeScript requires their values to fit it.
            let mut value_types = value_types.clone();
            if pattern.contains('*') {
                for (other, other_types) in &entry.patterns {
                    if other != pattern && template_matches(pattern, other) {
                        value_types.extend(other_types.iter().cloned());
                    }
                }
            }
            let value = value_types.into_iter().collect::<Vec<_>>().join(" | ");
            if pattern.contains("[]") || pattern.contains('*') {
                let template = pattern
                    .replace("[]", "[${number}]")
                    .replace('*', "${string}");
                let _ = writeln!(out, "  [path: `{template}`]: {value};");
            } else {
                let _ = writeln!(out, "  {}: {value};", string_literal(pattern));
            }
        }
        if entry.open_paths {
            out.push_str("  [path: string]: unknown;\n");
        }
        out.push_str("}\n\n");
        let _ = writeln!(
            out,
            "/** A concrete input path of `{}`, with list indices and map keys filled in. */",
            contract.entry
        );
        let _ = writeln!(
            out,
            "export type {types}InputPath = Extract<keyof {types}InputPaths, string>;\n"
        );

        write_doc(
            out,
            "",
            &[format!(
                "The input path patterns of `{}` (list indices as `[]`, map keys as `*`), keying resolvers.",
                contract.entry
            )],
        );
        let _ = writeln!(out, "export interface {types}InputPatterns {{");
        for (pattern, value_types) in &entry.patterns {
            let value = value_types.iter().cloned().collect::<Vec<_>>().join(" | ");
            let _ = writeln!(out, "  {}: {value};", string_literal(pattern));
        }
        if entry.open_paths {
            out.push_str("  [pattern: string]: unknown;\n");
        }
        out.push_str("}\n\n");

        if entry.open_paths {
            let _ = writeln!(
                out,
                "export const {types}InputPathSchema: z.ZodType<{types}InputPath> = z.string();\n"
            );
        } else {
            let _ = writeln!(
                out,
                "export const {types}InputPathSchema = inputPathSchema<{types}InputPath>(["
            );
            for pattern in entry.patterns.keys() {
                let _ = writeln!(out, "  {},", string_literal(pattern));
            }
            out.push_str("]);\n\n");
        }

        let _ = writeln!(
            out,
            "/** A need of `{}`: its concrete path and the field's metadata. */",
            contract.entry
        );
        let _ = writeln!(out, "export type {types}Need = Need<{types}InputPath>;");
        let _ = writeln!(
            out,
            "export const {types}NeedSchema = needSchema({types}InputPathSchema);\n"
        );
        let _ = writeln!(
            out,
            "/** The result of a partial call of `{}`. */",
            contract.entry
        );
        let _ = writeln!(
            out,
            "export type {types}QueryResult = QueryResult<{output}, {types}InputPath>;"
        );
        let _ = writeln!(
            out,
            "export const {types}QueryResultSchema = queryResultSchema({output_schema}, {types}InputPathSchema);\n"
        );
        write_doc(
            out,
            "",
            &[format!(
                "{{@link setAt}} for a `{}` case, typed by the path.",
                contract.entry
            )],
        );
        let _ = writeln!(
            out,
            "export function {values}SetAt<K extends {types}InputPath>(\n  partialCase: {input},\n  path: K,\n  value: {types}InputPaths[K],\n): {input} {{\n  return setAt(partialCase, path, value);\n}}"
        );
    }

    fn client_module(&self) -> String {
        let mut body = String::new();
        let mut first = true;
        for entry in &self.entries {
            let contract = entry.contract;
            let types = &entry.type_name;
            let values = &entry.value_name;
            let input = self.ts_type(&contract.input, Side::Qualified);
            let input_schema = self.zod(&contract.input, Side::Qualified);
            let partial_input = qualify_partial(&self.ts_type(&contract.input, Side::Partial));
            let partial_schema = qualify_partial(&self.zod(&contract.input, Side::Partial));
            let output = self.ts_type(&contract.output, Side::Qualified);
            let output_schema = self.zod(&contract.output, Side::Qualified);
            let title = match &contract.label {
                Some(label) => format!("`{}`: {}.", contract.entry, escape_doc(label)),
                None => format!("`{}`.", contract.entry),
            };
            if !first {
                body.push('\n');
            }
            first = false;
            let _ = writeln!(body, "// `{}`\n", contract.entry);
            let _ = writeln!(
                body,
                "/** The `{}` entry: its name, contract hash, and the wire codecs of its input and result. */",
                contract.entry
            );
            let _ = writeln!(
                body,
                "export const {values}Entry: CalculationEntry = {{\n  name: {},\n  schemaHash: schema.SCHEMA_HASH{},\n  input: {},\n  output: {},\n}};\n",
                string_literal(&contract.entry),
                property_access(&contract.entry),
                self.codec(&contract.input, true),
                self.codec(&contract.output, true)
            );
            write_doc(
                &mut body,
                "",
                &[
                    format!("Calculate {title}"),
                    String::new(),
                    "Validates the case, encodes it for the engine, runs it through `transport.call`, checks the envelope's schema hash, and decodes and validates the result.".to_string(),
                ],
            );
            let _ = writeln!(
                body,
                "export function {values}(transport: EngineTransport, input: {input}): Promise<{output}> {{\n  return callEntry(transport, {values}Entry, {input_schema}, {output_schema}, input);\n}}\n"
            );
            write_doc(
                &mut body,
                "",
                &[
                    format!("Query {title}"),
                    String::new(),
                    "Runs a partial case through `transport.callPartial` and returns `Complete` with the result, or `Needs` with every fact and entry that blocks it, at TypeScript paths.".to_string(),
                ],
            );
            let _ = writeln!(
                body,
                "export function {values}Query(\n  transport: EngineTransport,\n  input: {partial_input},\n): Promise<partial.{types}QueryResult> {{\n  return queryEntry(transport, {values}Entry, {partial_schema}, partial.{types}QueryResultSchema, input);\n}}\n"
            );
            write_doc(
                &mut body,
                "",
                &[
                    format!("Calculate and explain {title}"),
                    String::new(),
                    "As the plain call, through `transport.callExplained` (`runa call --explain`): also returns where every leaf of the result came from, at TypeScript paths.".to_string(),
                ],
            );
            let _ = writeln!(
                body,
                "export function {values}Explained(\n  transport: EngineTransport,\n  input: {input},\n  depth: ExplainDepth = \"direct\",\n): Promise<Explained<{output}>> {{\n  return explainEntry(transport, {values}Entry, depth, {input_schema}, {output_schema}, input);\n}}\n"
            );
            write_doc(
                &mut body,
                "",
                &[
                    format!("Query and explain {title}"),
                    String::new(),
                    "As the query, through `transport.callPartialExplained` (`runa call --partial --explain`): a `Complete` result comes with its explanation.".to_string(),
                ],
            );
            let _ = writeln!(
                body,
                "export function {values}QueryExplained(\n  transport: EngineTransport,\n  input: {partial_input},\n  depth: ExplainDepth = \"direct\",\n): Promise<ExplainedQuery<partial.{types}QueryResult>> {{\n  return explainQueryEntry(transport, {values}Entry, depth, {partial_schema}, partial.{types}QueryResultSchema, input);\n}}\n"
            );
            let _ = writeln!(
                body,
                "/** Resolvers for the needs of `{}`, keyed by path pattern. */",
                contract.entry
            );
            let _ = writeln!(
                body,
                "export type {types}Resolvers = {{\n  readonly [K in keyof partial.{types}InputPatterns]?: Resolver<\n    {partial_input},\n    partial.{types}InputPatterns[K],\n    partial.{types}InputPath\n  >;\n}};"
            );
        }
        let mut out = String::new();
        self.header(
            &mut out,
            "Typed calls of each calculation entry through an engine transport.",
        );
        out.push_str(&self.imports(
            &body,
            &[
                ("types", "./types.js", true),
                ("schema", "./schema.js", false),
                ("partial", "./partial.js", false),
            ],
            false,
        ));
        out.push_str(&body);
        out
    }

    fn index_module(&self) -> String {
        let mut out = String::new();
        self.header(
            &mut out,
            "The SDK's public surface, with the runtime it imports.",
        );
        out.push_str("export * from \"./types.js\";\nexport * from \"./schema.js\";\nexport * from \"./partial.js\";\nexport * from \"./client.js\";\n");
        let _ = writeln!(out, "export * from {};", string_literal(&self.runtime));
        out
    }
}

/// The runtime module: the same text for every model, so the SDKs of several
/// source files can share one copy.
fn runtime_module() -> String {
    format!(
        "// Generated by `runa sdk ts`. Do not edit; regenerate instead.\n// The runtime of the SDKs `runa sdk ts` generates: the wire codec, paths and setAt,\n// needs, calls through an engine transport, and the resolver loop. It depends on\n// no model, so the SDKs generated by one `runa` can share one copy (`--runtime`).\n\n{}",
        RUNTIME
            .replace("{INPUT_SCHEMA}", INPUT_SCHEMA)
            .replace("{OUTPUT_SCHEMA}", OUTPUT_SCHEMA)
            .replace("{PARTIAL_OUTPUT_SCHEMA}", PARTIAL_OUTPUT_SCHEMA)
    )
}

/// The metadata of a `Date` field from the metadata of its parts: the label
/// they share (their common beginning, without trailing separators, such as
/// `Booking date` from `Booking date: year` and `Booking date: month`), and
/// the question, help, unit and sources where all parts agree. The runtime
/// merges the needs of a date's parts by the same rule.
fn date_metadata(path: &str, parts: &[&CalculationFieldMetadata]) -> CalculationFieldMetadata {
    let first = parts[0];
    let agreed = |get: fn(&CalculationFieldMetadata) -> &Option<String>| {
        let value = get(first);
        parts
            .iter()
            .all(|part| get(part) == value)
            .then(|| value.clone())
            .flatten()
    };
    CalculationFieldMetadata {
        path: path.to_string(),
        label: shared_label(parts.iter().map(|part| part.label.as_str())),
        question: agreed(|metadata| &metadata.question),
        help: agreed(|metadata| &metadata.help),
        unit: agreed(|metadata| &metadata.unit),
        anchor: first.anchor.clone(),
        binding: first.binding.clone(),
        sources: if parts.iter().all(|part| part.sources == first.sources) {
            first.sources.clone()
        } else {
            Vec::new()
        },
        provided_by: None,
    }
}

/// The label several labels share: all of it when they are equal, otherwise
/// their common beginning without trailing whitespace, `:`, `;`, `,`, `(`,
/// `-` or `–`, or the first label when nothing is left.
fn shared_label<'s>(mut labels: impl Iterator<Item = &'s str>) -> String {
    let Some(first) = labels.next() else {
        return String::new();
    };
    let mut prefix: Vec<char> = first.chars().collect();
    let mut equal = true;
    for label in labels {
        if label != first {
            equal = false;
        }
        let common = prefix
            .iter()
            .zip(label.chars())
            .take_while(|(left, right)| *left == right)
            .count();
        prefix.truncate(common);
    }
    if equal {
        return first.to_string();
    }
    let shared: String = prefix.into_iter().collect();
    let trimmed = shared.trim_end_matches(|c: char| c.is_whitespace() || ":;,(-–".contains(c));
    if trimmed.is_empty() {
        first.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Partial types and schemas live in `partial.ts`; from `client.ts` they are
/// reached through its namespace import.
fn qualify_partial(rendered: &str) -> String {
    let mut out = String::with_capacity(rendered.len());
    let mut rest = rendered;
    while let Some(index) = rest.find("Partial") {
        let boundary = rest[..index].chars().last().is_none_or(|previous| {
            !(previous.is_alphanumeric() || previous == '_' || previous == '.')
        });
        out.push_str(&rest[..index]);
        if boundary {
            out.push_str("partial.");
        }
        out.push_str("Partial");
        rest = &rest[index + "Partial".len()..];
    }
    out.push_str(rest);
    out
}

/// Whether `line` uses the identifier `name`: not as part of a longer
/// identifier, and not as a property (`.name`) or a key (`name:`).
fn uses_identifier(line: &str, name: &str) -> bool {
    let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    let mut start = 0;
    while let Some(found) = line[start..].find(name) {
        let at = start + found;
        let end = at + name.len();
        let before = line[..at].chars().next_back();
        let after = line[end..].chars().next();
        if !before.is_some_and(|c| identifier(c) || c == '.')
            && !after.is_some_and(|c| identifier(c) || c == ':')
        {
            return true;
        }
        start = end;
    }
    false
}

/// Whether every concrete path of pattern `other` also matches pattern
/// `pattern` as TypeScript reads their templates: `*` (`${string}`) matches
/// any text and `[]` (`[${number}]`) a list index.
fn template_matches(pattern: &str, other: &str) -> bool {
    if let Some(rest) = pattern.strip_prefix('*') {
        return (0..=other.len())
            .filter(|&split| other.is_char_boundary(split))
            .any(|split| template_matches(rest, &other[split..]));
    }
    if let Some(rest) = pattern.strip_prefix("[]") {
        if let Some(other_rest) = other.strip_prefix("[]") {
            return template_matches(rest, other_rest);
        }
        let Some(digits) = other.strip_prefix('[') else {
            return false;
        };
        let end = digits
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(digits.len());
        return end > 0
            && digits[end..].starts_with(']')
            && template_matches(rest, &digits[end + 1..]);
    }
    match (pattern.chars().next(), other.chars().next()) {
        (None, None) => true,
        (Some(left), Some(right)) if left == right && right != '*' => {
            template_matches(&pattern[left.len_utf8()..], &other[right.len_utf8()..])
        }
        _ => false,
    }
}

fn metadata_block(metadata: &CalculationFieldMetadata, tag_label: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let label = if tag_label && metadata.path.ends_with(".$variant") {
        format!("Variant: {}", metadata.label)
    } else {
        metadata.label.clone()
    };
    lines.push(sentence(&label));
    for text in [&metadata.question, &metadata.help].into_iter().flatten() {
        lines.push(String::new());
        lines.extend(text.lines().map(str::to_string));
    }
    if let Some(unit) = &metadata.unit {
        lines.push(String::new());
        lines.push(format!("Unit: {unit}."));
    }
    if !metadata.sources.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Sources: {}.",
            metadata
                .sources
                .iter()
                .map(|source| source.label.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    lines
}

fn sentence(text: &str) -> String {
    let text = text.trim_end();
    if text.ends_with(['.', '?', '!', ':']) {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

/// The Futuruna type as the model writes it: an `Int` with a unit is the unit.
fn futuruna_type(ty: &CalculationTypeRef) -> String {
    match ty {
        CalculationTypeRef::Primitive {
            unit: Some(unit), ..
        } => unit.clone(),
        CalculationTypeRef::Optional { item } => format!("{}?", futuruna_type(item)),
        CalculationTypeRef::List { item } => format!("List({})", futuruna_type(item)),
        CalculationTypeRef::Set { item } => format!("Set({})", futuruna_type(item)),
        CalculationTypeRef::Map { key, value } => {
            format!("Map({}, {})", futuruna_type(key), futuruna_type(value))
        }
        CalculationTypeRef::Named { name, arguments } if !arguments.is_empty() => format!(
            "{name}({})",
            arguments
                .iter()
                .map(futuruna_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => ty.display_name(),
    }
}

fn mentions_int(ty: &CalculationTypeRef) -> bool {
    match ty {
        CalculationTypeRef::Primitive { name, .. } => name == "Int",
        CalculationTypeRef::Optional { item }
        | CalculationTypeRef::List { item }
        | CalculationTypeRef::Set { item } => mentions_int(item),
        CalculationTypeRef::Map { value, .. } => mentions_int(value),
        CalculationTypeRef::Named { arguments, .. } => arguments.iter().any(mentions_int),
        CalculationTypeRef::TypeParameter { .. } | CalculationTypeRef::Unit => false,
    }
}

fn write_doc(out: &mut String, indent: &str, lines: &[String]) {
    let mut lines: Vec<String> = lines.iter().map(|line| escape_doc(line)).collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    match lines.as_slice() {
        [] => {}
        [line] => {
            let _ = writeln!(out, "{indent}/** {line} */");
        }
        lines => {
            let _ = writeln!(out, "{indent}/**");
            for line in lines {
                if line.trim().is_empty() {
                    let _ = writeln!(out, "{indent} *");
                } else {
                    let _ = writeln!(out, "{indent} * {}", line.trim_end());
                }
            }
            let _ = writeln!(out, "{indent} */");
        }
    }
}

fn escape_doc(text: &str) -> String {
    text.replace("*/", "*\\/")
}

fn display_definition(definition: &CalculationTypeDefinition) -> String {
    if definition.parameters.is_empty() {
        definition.name.clone()
    } else {
        format!("{}({})", definition.name, definition.parameters.join(", "))
    }
}

fn variant_list(definition: &CalculationTypeDefinition) -> String {
    definition
        .variants
        .iter()
        .map(|variant| format!("`{}`", variant.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn variant_names(definition: &CalculationTypeDefinition) -> String {
    definition
        .variants
        .iter()
        .map(|variant| string_literal(&variant.name))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn type_parameters(definition: &CalculationTypeDefinition) -> String {
    if definition.parameters.is_empty() {
        String::new()
    } else {
        format!("<{}>", definition.parameters.join(", "))
    }
}

fn string_literal(text: &str) -> String {
    serde_json::to_string(text).expect("strings serialize")
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|next| next.is_ascii_alphanumeric() || next == '_' || next == '$')
}

fn property_name(name: &str) -> String {
    if is_identifier(name) {
        name.to_string()
    } else {
        string_literal(name)
    }
}

/// `.name`, or `["name"]` for a name that is not an identifier.
fn property_access(name: &str) -> String {
    if is_identifier(name) {
        format!(".{name}")
    } else {
        format!("[{}]", string_literal(name))
    }
}

/// `opening_cash_ore` → `openingCashOre`: each `_` is dropped and the letter
/// after it raised; leading underscores stay. A name without `_` (a variant
/// name, `$variant`) is unchanged.
fn camel_case(name: &str) -> String {
    let body = name.trim_start_matches('_');
    let mut out = String::with_capacity(name.len());
    out.push_str(&name[..name.len() - body.len()]);
    let mut raise = false;
    for c in body.chars() {
        if c == '_' {
            raise = true;
        } else if raise {
            out.extend(c.to_uppercase());
            raise = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// A contract metadata path (`events.AccountCharge.amount_ore`) with its
/// field names in camelCase (`events.AccountCharge.amountOre`).
fn camel_case_path(path: &str) -> String {
    path.split('.')
        .map(camel_case)
        .collect::<Vec<_>>()
        .join(".")
}

fn pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars.next().expect("non-empty part");
            first.to_uppercase().chain(chars).collect::<String>()
        })
        .collect()
}

fn value_identifier(name: &str) -> String {
    let pascal = pascal_case(name);
    let mut chars = pascal.chars();
    let camel = match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect::<String>(),
        None => "calculation".to_string(),
    };
    if TS_RESERVED.contains(&camel.as_str()) {
        format!("{camel}_")
    } else {
        camel
    }
}

/// Every name the runtime exports, and whether it is a type only.
const RUNTIME_EXPORTS: [(&str, bool); 82] = [
    ("INPUT_SCHEMA", false),
    ("OUTPUT_SCHEMA", false),
    ("PARTIAL_OUTPUT_SCHEMA", false),
    ("IntSchema", false),
    ("FloatSchema", false),
    ("CharSchema", false),
    ("SetSchema", false),
    ("IsoDateSchema", false),
    ("UnknownSchema", false),
    ("valueCodec", false),
    ("isoDateCodec", false),
    ("enumCodec", false),
    ("listCodec", false),
    ("mapCodec", false),
    ("recordCodec", false),
    ("unionCodec", false),
    ("toWire", false),
    ("fromWire", false),
    ("fromWirePath", false),
    ("WireError", false),
    ("PathError", false),
    ("parsePath", false),
    ("setAt", false),
    ("pathPattern", false),
    ("matchesPathPattern", false),
    ("inputPathSchema", false),
    ("SourceReferenceSchema", false),
    ("needSchema", false),
    ("queryResultSchema", false),
    ("unitSchema", false),
    ("unitValue", false),
    ("CalculationError", false),
    ("callEntry", false),
    ("queryEntry", false),
    ("explainEntry", false),
    ("explainQueryEntry", false),
    ("ask", false),
    ("answer", false),
    ("ResolveError", false),
    ("ResolveCycleError", false),
    ("resolve", false),
    ("IsoDate", true),
    ("Unknown", true),
    ("CodecRef", true),
    ("CodecFields", true),
    ("PositionalCodecs", true),
    ("Codec", true),
    ("WireIssue", true),
    ("PathSegment", true),
    ("SourceReference", true),
    ("FactNeed", true),
    ("EntryNeed", true),
    ("Need", true),
    ("Complete", true),
    ("Needs", true),
    ("QueryResult", true),
    ("EngineTransport", true),
    ("CalculationEntry", true),
    ("CalculationErrorKind", true),
    ("CalculationDiagnostic", true),
    ("ExplainDepth", true),
    ("ExplainedLeaf", true),
    ("CitedParagraph", true),
    ("ExplainedRule", true),
    ("RuleContributor", true),
    ("InputContributor", true),
    ("Contributor", true),
    ("ExplainedBy", true),
    ("ExplainedValue", true),
    ("ExplainedSection", true),
    ("Explanation", true),
    ("Explained", true),
    ("ExplainedQuery", true),
    ("EntryFrame", true),
    ("ResolverAnswer", true),
    ("ResolverContext", true),
    ("Query", true),
    ("Resolver", true),
    ("Resolvers", true),
    ("PendingEntry", true),
    ("Resolution", true),
    ("ResolveOptions", true),
];

const RUNTIME: &str = r#"import { z } from "zod";

export const INPUT_SCHEMA = "{INPUT_SCHEMA}";
export const OUTPUT_SCHEMA = "{OUTPUT_SCHEMA}";
export const PARTIAL_OUTPUT_SCHEMA = "{PARTIAL_OUTPUT_SCHEMA}";

// Values

/** `Int`: a safe integer. Values beyond ±(2^53 − 1) are rejected, never rounded. */
export const IntSchema = z.number().refine(Number.isSafeInteger, "expected a safe integer (Int within ±(2^53 − 1))");

/** `Float`: a finite number. */
export const FloatSchema = z.number().refine(Number.isFinite, "expected a finite number");

/** `Char`: a string of exactly one character. */
export const CharSchema = z.string().refine((text) => [...text].length === 1, "expected exactly one character");

/** `Set(T)`: an array without duplicate values. */
export const SetSchema = <T>(item: z.ZodType<T>): z.ZodType<T[]> =>
  z.array(item).refine(
    (values) => new Set(values.map(canonicalJson)).size === values.length,
    "expected a set without duplicate values",
  );

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    const entries = Object.entries(value).sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0));
    return `{${entries.map(([key, item]) => `${JSON.stringify(key)}:${canonicalJson(item)}`).join(",")}}`;
  }
  return String(JSON.stringify(value));
}

/**
 * A Futuruna `Date`: an ISO 8601 calendar date `YYYY-MM-DD`, years 0000 to
 * 9999. On the wire it is the record `{ "year": 2026, "month": 3, "day": 1 }`.
 */
export type IsoDate = string;

const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

function isoDateParts(text: string): { year: number; month: number; day: number } | undefined {
  const match = ISO_DATE.exec(text);
  if (match === null) return undefined;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const days = month === 2 ? (year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28) : [4, 6, 9, 11].includes(month) ? 30 : 31;
  return month >= 1 && month <= 12 && day >= 1 && day <= days ? { year, month, day } : undefined;
}

/** `Date`: an ISO date `YYYY-MM-DD` that is on the calendar (`2026-02-30` is rejected). */
export const IsoDateSchema: z.ZodType<IsoDate> = z
  .string()
  .refine((text) => isoDateParts(text) !== undefined, "expected an ISO date YYYY-MM-DD on the calendar");

declare const unknownOf: unique symbol;

/**
 * A `T` in a partial case that is not known yet, written `{}`. `T` is a sum
 * type, whose variant the engine then needs at `path.$variant`, or, inside a
 * list or map, a value that is a record on the wire (an enum, a `Date`).
 */
export type Unknown<T> = { readonly [key: string]: never; readonly [unknownOf]?: T };

/** Validates {@link Unknown}: exactly `{}`. */
export const UnknownSchema: z.ZodType<Unknown<never>> = z.object({}).strict();

// The wire codec

/** A codec, or a function returning one: the codecs of named types refer to each other lazily. */
export type CodecRef = Codec | (() => Codec);

/** The fields of a record or variant: each TypeScript name with the field's wire name and codec. */
export type CodecFields = { readonly [name: string]: readonly [wireName: string, codec: CodecRef] };

/** The values of a positional variant, `$values` on both sides. */
export type PositionalCodecs = { readonly $values: readonly CodecRef[] };

/**
 * How a TypeScript value is written in the canonical JSON `runa call` reads and
 * writes. `null` is the same on both sides, so optional values need no codec.
 */
export type Codec =
  | { readonly kind: "value" }
  | { readonly kind: "date" }
  | { readonly kind: "enum" }
  | { readonly kind: "list"; readonly item: CodecRef }
  | { readonly kind: "map"; readonly item: CodecRef }
  | { readonly kind: "record"; readonly fields: CodecFields }
  | { readonly kind: "union"; readonly variants: { readonly [variant: string]: CodecFields | PositionalCodecs } };

/** A value written as it is: a number, string, boolean or `null`, or an array or object of them. */
export const valueCodec: Codec = { kind: "value" };

/** A `Date`: `"2026-03-01"` is `{ "year": 2026, "month": 3, "day": 1 }` on the wire. */
export const isoDateCodec: Codec = { kind: "date" };

/** A sum type whose variants carry no fields: `"Deposit"` is `{ "$variant": "Deposit" }` on the wire. */
export const enumCodec: Codec = { kind: "enum" };

/** A list or set whose items use `item`. */
export const listCodec = (item: CodecRef): Codec => ({ kind: "list", item });

/** A `Map(String, T)` whose values use `item`; keys are the same on both sides. */
export const mapCodec = (item: CodecRef): Codec => ({ kind: "map", item });

/** A record: each TypeScript field name with its wire name and codec. */
export const recordCodec = (fields: CodecFields): Codec => ({ kind: "record", fields });

/** A sum type tagged by `$variant`: the fields of each variant, or its positional `$values`. */
export const unionCodec = (variants: { readonly [variant: string]: CodecFields | PositionalCodecs }): Codec => ({
  kind: "union",
  variants,
});

function codecOf(ref: CodecRef): Codec {
  return typeof ref === "function" ? ref() : ref;
}

function isObject(value: unknown): value is { readonly [key: string]: unknown } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(object: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(object, key);
}

function isPositional(spec: CodecFields | PositionalCodecs): spec is PositionalCodecs {
  return hasOwn(spec, "$values");
}

/** Set an own property, also for a key such as `__proto__`. */
function put(target: { [key: string]: unknown }, key: string, value: unknown): void {
  Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true });
}

const wireNames = new WeakMap<CodecFields, ReadonlyMap<string, readonly [string, CodecRef]>>();

/** The fields by wire name: wire name → TypeScript name and codec. */
function byWireName(fields: CodecFields): ReadonlyMap<string, readonly [string, CodecRef]> {
  let names = wireNames.get(fields);
  if (names === undefined) {
    names = new Map(Object.entries(fields).map(([name, [wireName, codec]]) => [wireName, [name, codec] as const]));
    wireNames.set(fields, names);
  }
  return names;
}

/** A field a codec does not know, at its path in the value being converted. */
export type WireIssue = { readonly path: readonly (string | number)[]; readonly message: string };

/** A value with a field its codec does not know: the contract has no such field there. */
export class WireError extends Error {
  readonly issues: readonly WireIssue[];

  constructor(issues: readonly WireIssue[]) {
    super(issueSummary(issues));
    this.name = "WireError";
    this.issues = issues;
  }
}

type Conversion = { readonly encode: boolean; readonly path: (string | number)[]; readonly issues: WireIssue[] };

/**
 * Encode a TypeScript value, complete or partial, as `runa call` JSON. A field
 * the codec does not know throws {@link WireError}.
 */
export function toWire(codec: CodecRef, value: unknown): unknown {
  return converted(codec, value, true);
}

/**
 * Decode `runa call` JSON into its TypeScript value. A field the codec does
 * not know throws {@link WireError}; any other mismatch (a string where a
 * number belongs, an unknown variant) stays as it is, for the validators to
 * reject.
 */
export function fromWire(codec: CodecRef, value: unknown): unknown {
  return converted(codec, value, false);
}

function converted(codec: CodecRef, value: unknown, encode: boolean): unknown {
  const conversion: Conversion = { encode, path: [], issues: [] };
  const out = convert(codec, value, conversion);
  if (conversion.issues.length > 0) throw new WireError(conversion.issues);
  return out;
}

function convert(ref: CodecRef, value: unknown, conversion: Conversion): unknown {
  if (value === undefined || value === null) return value;
  const codec = codecOf(ref);
  const { encode, path } = conversion;
  switch (codec.kind) {
    case "value":
      return value;
    case "date":
      return encode ? (typeof value === "string" && isoDateParts(value)) || value : dateFromWire(value);
    case "enum":
      if (encode) return typeof value === "string" ? { $variant: value } : value;
      return isObject(value) && typeof value["$variant"] === "string" && Object.keys(value).length === 1
        ? value["$variant"]
        : value;
    case "list": {
      if (!Array.isArray(value)) return value;
      const out: unknown[] = [];
      for (const [index, item] of value.entries()) {
        path.push(index);
        out.push(convert(codec.item, item, conversion));
        path.pop();
      }
      return out;
    }
    case "map": {
      if (!isObject(value)) return value;
      const out: { [key: string]: unknown } = {};
      for (const [key, item] of Object.entries(value)) {
        path.push(key);
        put(out, key, convert(codec.item, item, conversion));
        path.pop();
      }
      return out;
    }
    case "record":
      return isObject(value) ? convertFields(codec.fields, value, false, conversion) : value;
    case "union": {
      if (!isObject(value)) return value;
      const variant = value["$variant"];
      if (typeof variant !== "string" || !hasOwn(codec.variants, variant)) return value;
      const spec = codec.variants[variant] as CodecFields | PositionalCodecs;
      if (!isPositional(spec)) return convertFields(spec, value, true, conversion);
      const values = value["$values"];
      if (!Array.isArray(values)) return value;
      const out: { [key: string]: unknown } = { ...value };
      path.push("$values");
      out["$values"] = values.map((item: unknown, index) => {
        path.push(index);
        const next = convert(spec.$values[index] ?? valueCodec, item, conversion);
        path.pop();
        return next;
      });
      path.pop();
      return out;
    }
  }
}

/** `tagged`: the fields of a variant, next to its `$variant`. */
function convertFields(
  fields: CodecFields,
  value: { readonly [key: string]: unknown },
  tagged: boolean,
  conversion: Conversion,
): unknown {
  const { encode, path, issues } = conversion;
  const names = encode ? undefined : byWireName(fields);
  const out: { [key: string]: unknown } = {};
  for (const [key, item] of Object.entries(value)) {
    if (item === undefined) continue;
    if (tagged && key === "$variant") {
      out["$variant"] = item;
      continue;
    }
    const field = names === undefined ? (hasOwn(fields, key) ? fields[key] : undefined) : names.get(key);
    if (field === undefined) {
      issues.push({ path: [...path, key], message: `unknown field \`${key}\`` });
      continue;
    }
    const name = encode ? key : field[0];
    path.push(name);
    put(out, encode ? field[0] : name, convert(field[1], item, conversion));
    path.pop();
  }
  return out;
}

function dateFromWire(value: unknown): unknown {
  if (!isObject(value) || Object.keys(value).length !== 3) return value;
  const { year, month, day } = value;
  if (![year, month, day].every((part) => Number.isSafeInteger(part))) return value;
  const pad = (part: unknown, width: number) => String(part).padStart(width, "0");
  return `${Number(year) >= 0 ? pad(year, 4) : String(year)}-${pad(month, 2)}-${pad(day, 2)}`;
}

const DATE_PARTS: Record<string, true> = { year: true, month: true, day: true };

/**
 * The TypeScript path of an input path the engine reports:
 * `events[0].amount_ore` → `events[0].amountOre`; an enum's tag is the enum
 * (`events[0].direction.$variant` → `events[0].direction`); a part of a `Date`
 * is the date (`events[0].date.year` → `events[0].date`). `wireCase` is the
 * encoded case, which names the variant of each sum type on the way. The part
 * of a path the codec does not describe stays as it is.
 */
export function fromWirePath(codec: CodecRef, wireCase: unknown, path: string): string {
  return wirePath(codec, wireCase, path).path;
}

/** A TypeScript path, and the part of the `Date` there that the wire path named. */
type LocatedPath = { readonly path: string; readonly part?: string | undefined };

function wirePath(codec: CodecRef, wireCase: unknown, path: string): LocatedPath {
  let segments: PathSegment[];
  try {
    segments = parsePath(path);
  } catch {
    return { path };
  }
  const out: PathSegment[] = [];
  let node: Codec | PositionalCodecs | undefined = codecOf(codec);
  let value: unknown = wireCase;
  const step = (segment: PathSegment, next: CodecRef | undefined, key: string | number) => {
    out.push(segment);
    node = next === undefined ? undefined : codecOf(next);
    value = typeof key === "number" ? (Array.isArray(value) ? value[key] : undefined) : isObject(value) ? value[key] : undefined;
  };
  for (const segment of segments) {
    const current: Codec | PositionalCodecs | undefined = node;
    if (current === undefined) {
      out.push(segment);
    } else if ("index" in segment) {
      if ("$values" in current) step(segment, current.$values[segment.index], segment.index);
      else if (current.kind === "list") step(segment, current.item, segment.index);
      else step(segment, undefined, segment.index);
    } else if ("$values" in current) {
      step(segment, undefined, segment.field);
    } else {
      const field = segment.field;
      switch (current.kind) {
        case "date":
          if (hasOwn(DATE_PARTS, field)) return { path: formatPath(out), part: field };
          step(segment, undefined, field);
          break;
        case "enum":
          if (field === "$variant") return { path: formatPath(out) };
          step(segment, undefined, field);
          break;
        case "map":
          step(segment, current.item, field);
          break;
        case "record": {
          const named = byWireName(current.fields).get(field);
          if (named === undefined) step(segment, undefined, field);
          else step({ field: named[0] }, named[1], field);
          break;
        }
        case "union": {
          const variant = isObject(value) ? value["$variant"] : undefined;
          const spec: CodecFields | PositionalCodecs | undefined =
            typeof variant === "string" && hasOwn(current.variants, variant) ? current.variants[variant] : undefined;
          if (field === "$values" && spec !== undefined && isPositional(spec)) {
            out.push(segment);
            node = spec;
            value = isObject(value) ? value["$values"] : undefined;
            break;
          }
          const candidates = spec !== undefined ? [spec] : Object.values(current.variants);
          const named = candidates
            .filter((candidate): candidate is CodecFields => !isPositional(candidate))
            .map((candidate) => byWireName(candidate).get(field))
            .find((found) => found !== undefined);
          if (named === undefined) step(segment, undefined, field);
          else step({ field: named[0] }, named[1], field);
          break;
        }
        default:
          step(segment, undefined, field);
      }
    }
  }
  return { path: formatPath(out) };
}

function formatPath(segments: readonly PathSegment[]): string {
  return segments
    .map((segment, at) => ("index" in segment ? `[${segment.index}]` : at === 0 ? segment.field : `.${segment.field}`))
    .join("");
}

// Paths and setAt

/** One step of a concrete input path: a field (record field, map key, `$variant`, `$values`) or a list index. */
export type PathSegment = { readonly field: string } | { readonly index: number };

/** A path that does not parse, or that {@link setAt} cannot write. */
export class PathError extends Error {
  readonly path: string;

  constructor(path: string, message: string) {
    super(`${path}: ${message}`);
    this.name = "PathError";
    this.path = path;
  }
}

const FIRST_FIELD = /^[^.[\]]+/;
const NEXT_FIELD = /^\.([^.[\]]+)/;
const LIST_INDEX = /^\[(\d+)\]/;

/** Split a concrete input path such as `accounts[2].closingCashOre` into its steps. */
export function parsePath(path: string): PathSegment[] {
  const first = FIRST_FIELD.exec(path);
  if (first === null) throw new PathError(path, "a path starts with a field name");
  const segments: PathSegment[] = [{ field: first[0] }];
  let rest = path.slice(first[0].length);
  while (rest.length > 0) {
    const index = LIST_INDEX.exec(rest);
    if (index !== null) {
      segments.push({ index: Number(index[1]) });
      rest = rest.slice(index[0].length);
      continue;
    }
    const field = NEXT_FIELD.exec(rest);
    if (field === null) throw new PathError(path, `unexpected \`${rest}\``);
    segments.push({ field: field[1] as string });
    rest = rest.slice(field[0].length);
  }
  return segments;
}

/**
 * Write `value` at a concrete input path, such as `accounts[2].closingCashOre`
 * or `taxpayer.outsideEndavu.$variant`, and return the new case. The case
 * passed in is not changed. A record on the way that was left out is created as
 * `{}`; a list element must already exist. Writing `$variant` keeps the object
 * when the variant is unchanged, and otherwise starts the new variant with its
 * fields unknown.
 */
export function setAt<C>(partialCase: C, path: string, value: unknown): C {
  return writeAt(partialCase, parsePath(path), 0, value, path) as C;
}

function writeAt(node: unknown, segments: readonly PathSegment[], at: number, value: unknown, path: string): unknown {
  const segment = segments[at];
  if (segment === undefined) return value;
  if ("index" in segment) {
    if (!Array.isArray(node) || segment.index >= node.length) {
      throw new PathError(path, `there is no list element [${segment.index}] to write into`);
    }
    const copy: unknown[] = node.slice();
    copy[segment.index] = writeAt(node[segment.index], segments, at + 1, value, path);
    return copy;
  }
  if (node === null) {
    throw new PathError(path, `\`${segment.field}\` is inside a value given as null ("none")`);
  }
  if (node !== undefined && (typeof node !== "object" || Array.isArray(node))) {
    throw new PathError(path, `\`${segment.field}\` is inside a value that is not an object`);
  }
  const record = (node ?? {}) as { readonly [key: string]: unknown };
  if (segment.field === "$variant" && at === segments.length - 1) {
    return record["$variant"] === value ? record : { $variant: value };
  }
  return { ...record, [segment.field]: writeAt(record[segment.field], segments, at + 1, value, path) };
}

/** The pattern of a concrete path: list indices become `[]` (`accounts[2].kind` → `accounts[].kind`). */
export function pathPattern(path: string): string {
  return path.replace(/\[\d+\]/g, "[]");
}

const patternExpressions = new Map<string, RegExp>();

/** Whether a concrete path matches a pattern, where `[]` stands for any list index and `*` for any map key. */
export function matchesPathPattern(path: string, pattern: string): boolean {
  let expression = patternExpressions.get(pattern);
  if (expression === undefined) {
    const source = pattern
      .split("[]")
      .map((part) => part.split("*").map(escapeRegExp).join("[^.[\\]]+"))
      .join("\\[\\d+\\]");
    expression = new RegExp(`^${source}$`);
    patternExpressions.set(pattern, expression);
  }
  return expression.test(path);
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** A validator for the concrete input paths matching one of `patterns`. */
export function inputPathSchema<P extends string>(patterns: readonly string[]): z.ZodType<P> {
  return z.custom<P>(
    (value) => typeof value === "string" && patterns.some((pattern) => matchesPathPattern(value, pattern)),
    "not an input path of this calculation",
  );
}

// Units of measure

/** The validator of a unit of measure: a safe integer, typed `U` once parsed. */
export function unitSchema<U extends number>(): z.ZodType<U> {
  return IntSchema as unknown as z.ZodType<U>;
}

/** `value` as a `U` of the unit `unit`; throws a `RangeError` unless it is a safe integer. */
export function unitValue<U extends number>(unit: string, value: number): U {
  if (!Number.isSafeInteger(value)) throw new RangeError(`${unit}: expected a safe integer, got ${value}`);
  return value as U;
}

// Needs and the result of a partial call

/** A source reference from the contract's field metadata. */
export type SourceReference = {
  label: string;
  role: string;
  binding: string;
  attachmentPath?: string | undefined;
  type?: string | undefined;
  value?: string | undefined;
  data?: unknown;
  text?: string | undefined;
  symbols?: string[] | undefined;
};

export const SourceReferenceSchema: z.ZodType<SourceReference> = z
  .object({
    label: z.string(),
    role: z.string(),
    binding: z.string(),
    attachmentPath: z.string().optional(),
    type: z.string().optional(),
    value: z.string().optional(),
    data: z.unknown().optional(),
    text: z.string().optional(),
    symbols: z.array(z.string()).optional(),
  })
  .strict();

/** The field metadata a need carries; each part is left out when the contract does not give it. */
type NeedMetadata = {
  label?: string | undefined;
  question?: string | undefined;
  help?: string | undefined;
  unit?: string | undefined;
  sources?: SourceReference[] | undefined;
  /**
   * The rule or function the need waits for: the innermost one on the stack
   * when the path was first read that is declared in a law section (else in a
   * section with a quote); without one, the innermost rule family (or the entry).
   */
  waitingRule: string;
  /** The law section (anchor label) of `waitingRule`. */
  section?: string | undefined;
  /** The paragraphs `waitingRule` cites on the clause being tried, as `§ 11, stk. 2`. */
  paragraphs?: string[] | undefined;
};

/** A fact the client answers, at `path`. */
export type FactNeed<P extends string = string> = NeedMetadata & {
  kind: "fact";
  /** The concrete input path to answer, with list indices. */
  path: P;
};

/** A value another calculation entry provides: run `entry` for `subject` and write its result at `path`. */
export type EntryNeed<P extends string = string> = NeedMetadata & {
  kind: "entry";
  /** The concrete input path the entry's result is written at. */
  path: P;
  /** The calculation entry that provides the value. */
  entry: string;
  /** The input path of the subject the entry runs for. */
  subject: string;
};

/** Something a partial case still needs: a fact, or another entry's result. */
export type Need<P extends string = string> = FactNeed<P> | EntryNeed<P>;

export function needSchema<P extends string>(path: z.ZodType<P>): z.ZodType<Need<P>> {
  const metadata = {
    label: z.string().optional(),
    question: z.string().optional(),
    help: z.string().optional(),
    unit: z.string().optional(),
    sources: z.array(SourceReferenceSchema).optional(),
    waitingRule: z.string(),
    section: z.string().optional(),
    paragraphs: z.array(z.string()).optional(),
  };
  return z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("fact"), path, ...metadata }).strict(),
    z.object({ kind: z.literal("entry"), path, entry: z.string(), subject: z.string(), ...metadata }).strict(),
  ]) as unknown as z.ZodType<Need<P>>;
}

/** A partial case whose result no completion of its unknown fields can change. */
export type Complete<R, P extends string = string> = {
  $variant: "Complete";
  result: R;
  /** The provided input paths evaluation never read, sorted. */
  unusedPaths: P[];
};

/** A partial case that cannot be decided yet. */
export type Needs<P extends string = string> = {
  $variant: "Needs";
  /** Every fact and entry that blocks the result, sorted by path, each path once. */
  needs: Need<P>[];
};

/** The result of a partial call. */
export type QueryResult<R, P extends string = string> = Complete<R, P> | Needs<P>;

export function queryResultSchema<R, P extends string>(
  result: z.ZodType<R>,
  path: z.ZodType<P>,
): z.ZodType<QueryResult<R, P>> {
  return z.discriminatedUnion("$variant", [
    z.object({ $variant: z.literal("Complete"), result, unusedPaths: z.array(path) }).strict(),
    z.object({ $variant: z.literal("Needs"), needs: z.array(needSchema(path)) }).strict(),
  ]) as unknown as z.ZodType<QueryResult<R, P>>;
}

/** Wire names of the needs' and sources' fields that differ in TypeScript. */
const NEED_NAMES: { readonly [wireName: string]: string } = { waiting_rule: "waitingRule" };
const SOURCE_NAMES: { readonly [wireName: string]: string } = { attachment_path: "attachmentPath" };
const RESULT_NAMES: { readonly [wireName: string]: string } = { unused_paths: "unusedPaths" };

function renamed(value: { readonly [key: string]: unknown }, names: { readonly [wireName: string]: string }) {
  const out: { [key: string]: unknown } = {};
  for (const [key, item] of Object.entries(value)) put(out, hasOwn(names, key) ? (names[key] as string) : key, item);
  return out;
}

/** The TypeScript form of a partial call's result, its paths at TypeScript paths of the case. */
function queryResultFromWire(entry: CalculationEntry, wireCase: unknown, result: unknown): unknown {
  if (!isObject(result)) return result;
  const out = renamed(result, RESULT_NAMES);
  const path = (item: unknown) => (typeof item === "string" ? fromWirePath(entry.input, wireCase, item) : item);
  if (result["$variant"] === "Complete") {
    if (hasOwn(out, "result")) out["result"] = fromWire(entry.output, out["result"]);
    const unused = out["unusedPaths"];
    if (Array.isArray(unused)) {
      // The parts of a date are one path in TypeScript.
      out["unusedPaths"] = [...new Set(unused.map(path))].sort(byText);
    }
  } else if (result["$variant"] === "Needs" && Array.isArray(out["needs"])) {
    const needs = out["needs"].map((need: unknown) => {
      if (!isObject(need)) return need;
      const translated = renamed(need, NEED_NAMES);
      if (hasOwn(translated, "path")) translated["path"] = path(translated["path"]);
      if (hasOwn(translated, "subject")) translated["subject"] = path(translated["subject"]);
      const sources = translated["sources"];
      if (Array.isArray(sources)) {
        translated["sources"] = sources.map((source: unknown) => (isObject(source) ? renamed(source, SOURCE_NAMES) : source));
      }
      return translated;
    });
    out["needs"] = mergeNeeds(needs);
  }
  return out;
}

function byText(left: unknown, right: unknown): number {
  const [a, b] = [String(left), String(right)];
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * The needs of a date's parts share one TypeScript path; they become one need
 * there, labelled by what their labels share (`Booking date` from `Booking
 * date: year` and `Booking date: month`), with the question, help, unit and
 * sources where all of them agree.
 */
function mergeNeeds(needs: readonly unknown[]): unknown[] {
  const groups = new Map<unknown, { [key: string]: unknown }[]>();
  const rest: unknown[] = [];
  for (const need of needs) {
    if (!isObject(need) || typeof need["path"] !== "string") {
      rest.push(need);
      continue;
    }
    const group = groups.get(need["path"]);
    if (group === undefined) groups.set(need["path"], [{ ...need }]);
    else group.push({ ...need });
  }
  const merged = [...groups.values()].map((group) => {
    const first = group[0] as { [key: string]: unknown };
    if (group.length === 1) return first;
    const labels = group.map((need) => need["label"]).filter((label): label is string => typeof label === "string");
    if (labels.length > 0) first["label"] = sharedLabel(labels);
    for (const key of ["question", "help", "unit", "sources"]) {
      if (!group.every((need) => canonicalJson(need[key]) === canonicalJson(first[key]))) delete first[key];
    }
    return first;
  });
  return [...merged.sort((left, right) => byText(left["path"], right["path"])), ...rest];
}

function sharedLabel(labels: readonly string[]): string {
  const first = labels[0] as string;
  if (labels.every((label) => label === first)) return first;
  let prefix = [...first];
  for (const label of labels) {
    const chars = [...label];
    let common = 0;
    while (common < prefix.length && prefix[common] === chars[common]) common += 1;
    prefix = prefix.slice(0, common);
  }
  const shared = prefix.join("").replace(/[\s:;,(\-–]+$/u, "");
  return shared.length > 0 ? shared : first;
}

// Calls through an engine transport

const CASE_ID = "case-1";

/**
 * How the SDK reaches an engine. Each method receives the input document
 * `runa call --input` reads (one case, marked ready) and resolves to the output
 * document `runa call` writes, as an object or as its JSON text.
 */
export interface EngineTransport {
  /** Run `entry` on complete input, as `runa call`. */
  call(entry: string, input: unknown): Promise<unknown>;
  /** Run `entry` on a partial case, as `runa call --partial`. */
  callPartial(entry: string, input: unknown): Promise<unknown>;
  /**
   * Run `entry` and explain every leaf of the result, as `runa call --explain`
   * (`--explain=full` when `full`). Needed by the `...Explained` calls only.
   */
  callExplained?(entry: string, input: unknown, full: boolean): Promise<unknown>;
  /** As `callExplained`, on a partial case: `runa call --partial --explain`. */
  callPartialExplained?(entry: string, input: unknown, full: boolean): Promise<unknown>;
}

/** One calculation entry: its name, its contract hash, and the wire codecs of its input and result. */
export type CalculationEntry = {
  readonly name: string;
  readonly schemaHash: string;
  readonly input: CodecRef;
  readonly output: CodecRef;
};

/**
 * Why a call failed: the input did not validate, the output did not validate,
 * the envelope is for another contract or entry, or the engine reported
 * diagnostics for the case.
 */
export type CalculationErrorKind = "input" | "output" | "envelope" | "diagnostics";

/** A diagnostic the engine reported for the case, at its path in the input document. */
export type CalculationDiagnostic = { caseId: string; path: string; message: string };

export class CalculationError extends Error {
  readonly kind: CalculationErrorKind;
  readonly entry: string;
  /** The validation issues, for `input` and `output`. */
  readonly issues: readonly unknown[];
  /** The engine's diagnostics, for `diagnostics`. */
  readonly diagnostics: readonly CalculationDiagnostic[];

  constructor(
    kind: CalculationErrorKind,
    entry: string,
    message: string,
    details: { issues?: readonly unknown[]; diagnostics?: readonly CalculationDiagnostic[] } = {},
  ) {
    super(`${entry}: ${message}`);
    this.name = "CalculationError";
    this.kind = kind;
    this.entry = entry;
    this.issues = details.issues ?? [];
    this.diagnostics = details.diagnostics ?? [];
  }
}

function issueSummary(issues: readonly { path: readonly PropertyKey[]; message: string }[]): string {
  const first = issues[0];
  if (first === undefined) return "invalid";
  const at = first.path.length === 0 ? "$" : first.path.map(String).join(".");
  return `${issues.length} issue${issues.length === 1 ? "" : "s"}; first at ${at}: ${first.message}`;
}

/** The one case of an output document (`result`, and `explanation` when explained), its envelope checked. */
function caseOutput(
  entry: CalculationEntry,
  outputSchema: string,
  document: unknown,
): { readonly [key: string]: unknown } {
  const parsed: unknown = typeof document === "string" ? JSON.parse(document) : document;
  if (!isObject(parsed) || !("$futuruna" in parsed)) return { result: parsed };
  const envelope = parsed["$futuruna"];
  const name = entry.name;
  if (!isObject(envelope)) throw new CalculationError("envelope", name, "`$futuruna` is not an object");
  if (envelope["schema_hash"] !== undefined && envelope["schema_hash"] !== entry.schemaHash) {
    throw new CalculationError(
      "envelope",
      name,
      `the engine runs contract ${String(envelope["schema_hash"])}, but this SDK was generated for ${entry.schemaHash}`,
    );
  }
  if (envelope["entry"] !== undefined && envelope["entry"] !== name) {
    throw new CalculationError("envelope", name, `the output is for entry ${String(envelope["entry"])}`);
  }
  if (envelope["schema"] !== undefined && envelope["schema"] !== outputSchema) {
    throw new CalculationError("envelope", name, `expected a ${outputSchema} document, got ${String(envelope["schema"])}`);
  }
  const diagnostics = parsed["diagnostics"];
  if (Array.isArray(diagnostics) && diagnostics.length > 0) {
    const list = diagnostics.filter(isObject).map((diagnostic) => ({
      caseId: String(diagnostic["case_id"]),
      path: String(diagnostic["path"]),
      message: String(diagnostic["message"]),
    }));
    const first = list[0];
    const summary = first === undefined ? "" : `; first at ${first.path}: ${first.message}`;
    throw new CalculationError("diagnostics", name, `${diagnostics.length} diagnostic(s)${summary}`, {
      diagnostics: list,
    });
  }
  const results = parsed["results"];
  const only: unknown = Array.isArray(results) && results.length === 1 ? results[0] : undefined;
  if (!isObject(only) || !("result" in only)) {
    throw new CalculationError("envelope", name, "the output document does not hold exactly one case result");
  }
  return only;
}

/** Validate a case and encode it as the input document `runa call` reads. */
function inputDocument<I>(entry: CalculationEntry, inputSchema: z.ZodType<I>, input: I) {
  const checked = inputSchema.safeParse(input);
  if (!checked.success) {
    const issues = checked.error.issues;
    throw new CalculationError("input", entry.name, `invalid input: ${issueSummary(issues)}`, { issues });
  }
  const wireCase = toWire(entry.input, checked.data);
  const document = {
    $futuruna: { schema: INPUT_SCHEMA, schema_hash: entry.schemaHash, entry: entry.name },
    cases: [{ case_id: CASE_ID, input_status: "ready", input: wireCase }],
  };
  return { wireCase, document };
}

/** Decode a result with `decode` and validate it; either failing is an `output` error. */
function validated<O>(entry: CalculationEntry, schema: z.ZodType<O>, decode: () => unknown): O {
  let value: unknown;
  try {
    value = decode();
  } catch (error) {
    if (!(error instanceof WireError)) throw error;
    throw new CalculationError("output", entry.name, `invalid result: ${error.message}`, { issues: error.issues });
  }
  const checked = schema.safeParse(value);
  if (!checked.success) {
    const issues = checked.error.issues;
    throw new CalculationError("output", entry.name, `invalid result: ${issueSummary(issues)}`, { issues });
  }
  return checked.data;
}

/**
 * Calculate `entry` on a complete case: validate it, encode it, run it through
 * `transport.call`, check the envelope, and decode and validate the result.
 */
export async function callEntry<I, O>(
  transport: EngineTransport,
  entry: CalculationEntry,
  inputSchema: z.ZodType<I>,
  outputSchema: z.ZodType<O>,
  input: I,
): Promise<O> {
  const { document } = inputDocument(entry, inputSchema, input);
  const output = await transport.call(entry.name, document);
  const result = caseOutput(entry, OUTPUT_SCHEMA, output)["result"];
  return validated(entry, outputSchema, () => fromWire(entry.output, result));
}

/**
 * Query `entry` on a partial case through `transport.callPartial`: `Complete`
 * or `Needs`, with the result decoded and every path a TypeScript path.
 */
export async function queryEntry<I, R, P extends string>(
  transport: EngineTransport,
  entry: CalculationEntry,
  inputSchema: z.ZodType<I>,
  resultSchema: z.ZodType<QueryResult<R, P>>,
  input: I,
): Promise<QueryResult<R, P>> {
  const { wireCase, document } = inputDocument(entry, inputSchema, input);
  const output = await transport.callPartial(entry.name, document);
  const result = caseOutput(entry, PARTIAL_OUTPUT_SCHEMA, output)["result"];
  return validated(entry, resultSchema, () => queryResultFromWire(entry, wireCase, result));
}

// Explanations

/** How much an explained call reports: `direct` contributors of each value, or also the `full` derivation. */
export type ExplainDepth = "direct" | "full";

/** A leaf of a result or a case: a number, string or boolean, an enum's variant, or a `Date` as an ISO date. */
export type ExplainedLeaf = string | number | boolean | null;

/** A paragraph a rule clause cites, with its quoted text. */
export type CitedParagraph = { paragraph: string; source?: string | undefined; text: string };

/** The rule clause that produced a value: its law section and cited paragraphs. */
export type ExplainedRule = {
  $variant: "Rule";
  rule: string;
  /** The RuleScope of a scoped rule. */
  scope?: string | undefined;
  clause: "default" | "exception" | "clause";
  /** The label of the exception that applied. */
  exception?: string | undefined;
  /** `file:line` of the clause. */
  location?: string | undefined;
  /** The law section around the clause; a key of {@link Explanation.sections}. */
  section?: string | undefined;
  cites?: CitedParagraph[] | undefined;
};

/** A rule a value was computed from, with the value it gave. */
export type RuleContributor = Omit<ExplainedRule, "cites"> & {
  paragraphs?: string[] | undefined;
  value: ExplainedLeaf;
  /** The node in {@link Explanation.derivation}, with depth `full`. */
  node?: number | undefined;
};

/** An input a value was computed from, at its TypeScript path in the case. */
export type InputContributor = { $variant: "Input"; path: string; value: ExplainedLeaf };

export type Contributor = RuleContributor | InputContributor;

/**
 * What produced a value: a rule clause, the input at a TypeScript path of the
 * case, a computation outside any rule, or a constant of the model.
 */
export type ExplainedBy =
  | ExplainedRule
  | { $variant: "Input"; path: string }
  | { $variant: "Computed" }
  | { $variant: "Constant" };

/** One leaf of a result and where it came from. */
export type ExplainedValue = {
  value: ExplainedLeaf;
  by: ExplainedBy;
  from?: Contributor[] | undefined;
  /** Direct contributors beyond the first 32, with depth `direct`. */
  fromOmitted?: number | undefined;
};

/** A law section: its anchor, quoted documents, attachments and quote. */
export type ExplainedSection = {
  location: string;
  sources: string[];
  attachments: { role: string; binding?: string | undefined; value: unknown }[];
  quote: string;
};

/**
 * Where every leaf of a result came from (`docs/rfcs/explanations.md`), at the
 * TypeScript paths of the result and the case: an enum is one leaf, and a
 * `Date` is one leaf, its ISO date, unless its parts came from different
 * places; then each part is explained at `<date>.year`, `.month` and `.day`.
 */
export type Explanation = {
  /** Every leaf of the result by its TypeScript path. */
  values: { [path: string]: ExplainedValue };
  sections: { [label: string]: ExplainedSection };
  /** With depth `full`: every rule node a value was computed from. */
  derivation?: { [node: string]: { value: ExplainedLeaf; by: ExplainedRule; from: Contributor[] } } | undefined;
};

/** A result with its explanation. */
export type Explained<R> = { readonly result: R; readonly explanation: Explanation };

/** A partial call's result; a `Complete` result carries an explanation. */
export type ExplainedQuery<R> = { readonly result: R; readonly explanation?: Explanation | undefined };

/** The ISO date of a `Date`'s parts, when they are one. */
function isoDateOf(parts: { readonly [part: string]: unknown }): string | undefined {
  const date = dateFromWire({ year: parts["year"], month: parts["month"], day: parts["day"] });
  return typeof date === "string" && isoDateParts(date) !== undefined ? date : undefined;
}

/** Input contributors at TypeScript paths; the three parts of one input `Date` become the date. */
function contributorsFromWire(input: (path: string) => LocatedPath, from: readonly unknown[]): unknown[] {
  const out: unknown[] = [];
  const dates = new Map<string, { at: number; parts: Map<string, { [key: string]: unknown }> }>();
  for (const item of from) {
    if (!isObject(item) || item["$variant"] !== "Input" || typeof item["path"] !== "string") {
      out.push(item);
      continue;
    }
    const located = input(item["path"]);
    const translated: { [key: string]: unknown } = { ...item, path: located.path };
    if (located.part === undefined) {
      out.push(translated);
      continue;
    }
    let date = dates.get(located.path);
    if (date === undefined) {
      date = { at: out.length, parts: new Map() };
      dates.set(located.path, date);
      out.push(undefined);
    }
    date.parts.set(located.part, translated);
  }
  for (const [path, { at, parts }] of dates) {
    const value = parts.size === 3 ? isoDateOf(Object.fromEntries([...parts].map(([part, item]) => [part, item["value"]]))) : undefined;
    out[at] =
      value !== undefined
        ? [{ $variant: "Input", path, value }]
        : [...parts].map(([part, item]) => ({ ...item, path: `${path}.${part}` }));
  }
  const seen = new Set<string>();
  return out.flat(1).filter((item) => {
    const key = canonicalJson(item);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function explainedFromWire(input: (path: string) => LocatedPath, value: unknown): { [key: string]: unknown } {
  const out = isObject(value) ? renamed(value, EXPLAINED_NAMES) : {};
  const by = out["by"];
  if (isObject(by) && by["$variant"] === "Input" && typeof by["path"] === "string") {
    out["by"] = { ...by, path: input(by["path"]).path };
  }
  if (Array.isArray(out["from"])) out["from"] = contributorsFromWire(input, out["from"]);
  return out;
}

const EXPLAINED_NAMES: { readonly [wireName: string]: string } = { from_omitted: "fromOmitted" };

/** The explained parts of one `Date` of a result as one value, when they share what produced them. */
function explainedDate(parts: ReadonlyMap<string, { [key: string]: unknown }>): { [key: string]: unknown } | undefined {
  if (parts.size !== 3) return undefined;
  const explained = [...parts.values()];
  const first = explained[0] as { [key: string]: unknown };
  if (!explained.every((part) => canonicalJson(part["by"]) === canonicalJson(first["by"]))) return undefined;
  const value = isoDateOf(Object.fromEntries([...parts].map(([part, item]) => [part, item["value"]])));
  if (value === undefined) return undefined;
  const out: { [key: string]: unknown } = { value, by: first["by"] };
  const seen = new Set<string>();
  const from = explained
    .flatMap((part) => (Array.isArray(part["from"]) ? part["from"] : []))
    .filter((item: unknown) => {
      const key = canonicalJson(item);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  if (explained.some((part) => part["from"] !== undefined)) out["from"] = from;
  const omitted = explained.map((part) => part["fromOmitted"]).filter((count): count is number => typeof count === "number");
  if (omitted.length > 0) out["fromOmitted"] = Math.max(...omitted);
  return out;
}

/**
 * The TypeScript form of an explanation: result and input paths are the
 * TypeScript paths of `wireResult` and `wireCase` (an enum at its own path, a
 * `Date` as a whole) and `from_omitted` is `fromOmitted`. Leaf values are
 * already TypeScript values: numbers, strings, booleans and variant names.
 */
function explanationFromWire(entry: CalculationEntry, wireCase: unknown, wireResult: unknown, explanation: unknown): Explanation {
  if (!isObject(explanation) || !isObject(explanation["values"]) || !isObject(explanation["sections"])) {
    throw new CalculationError("output", entry.name, "the explanation is not an object with values and sections");
  }
  const input = (path: string) => wirePath(entry.input, wireCase, path);
  const groups = new Map<string, Map<string, { [key: string]: unknown }>>();
  const values: [string, unknown][] = [];
  for (const [path, value] of Object.entries(explanation["values"])) {
    const located = wirePath(entry.output, wireResult, path);
    const explained = explainedFromWire(input, value);
    if (located.part === undefined) {
      values.push([located.path, explained]);
      continue;
    }
    let parts = groups.get(located.path);
    if (parts === undefined) {
      parts = new Map();
      groups.set(located.path, parts);
    }
    parts.set(located.part, explained);
  }
  for (const [path, parts] of groups) {
    const date = explainedDate(parts);
    if (date !== undefined) values.push([path, date]);
    else for (const [part, explained] of parts) values.push([`${path}.${part}`, explained]);
  }
  const out: { [key: string]: unknown } = { ...explanation, values: {} };
  for (const [path, value] of values.sort(([left], [right]) => byText(left, right))) {
    put(out["values"] as { [key: string]: unknown }, path, value);
  }
  const derivation = explanation["derivation"];
  if (isObject(derivation)) {
    const nodes: { [key: string]: unknown } = {};
    for (const [node, value] of Object.entries(derivation)) {
      const from = isObject(value) ? value["from"] : undefined;
      put(nodes, node, isObject(value) && Array.isArray(from) ? { ...value, from: contributorsFromWire(input, from) } : value);
    }
    out["derivation"] = nodes;
  }
  return out as unknown as Explanation;
}

function explainingCall(
  transport: EngineTransport,
  entry: CalculationEntry,
  partialCall: boolean,
): (entry: string, input: unknown, full: boolean) => Promise<unknown> {
  const call = partialCall ? transport.callPartialExplained : transport.callExplained;
  if (call === undefined) {
    const method = partialCall ? "callPartialExplained" : "callExplained";
    throw new CalculationError("envelope", entry.name, `the transport has no ${method}`);
  }
  return call.bind(transport);
}

/**
 * Calculate `entry` and explain it, through `transport.callExplained`
 * (`runa call --explain`): the result as {@link callEntry} returns it, and
 * where every leaf of it came from.
 */
export async function explainEntry<I, O>(
  transport: EngineTransport,
  entry: CalculationEntry,
  depth: ExplainDepth,
  inputSchema: z.ZodType<I>,
  outputSchema: z.ZodType<O>,
  input: I,
): Promise<Explained<O>> {
  const call = explainingCall(transport, entry, false);
  const { wireCase, document } = inputDocument(entry, inputSchema, input);
  const output = caseOutput(entry, OUTPUT_SCHEMA, await call(entry.name, document, depth === "full"));
  const wireResult = output["result"];
  const result = validated(entry, outputSchema, () => fromWire(entry.output, wireResult));
  if (output["explanation"] === undefined) {
    throw new CalculationError("output", entry.name, "the engine returned no explanation");
  }
  return { result, explanation: explanationFromWire(entry, wireCase, wireResult, output["explanation"]) };
}

/**
 * Query `entry` and explain a `Complete` result, through
 * `transport.callPartialExplained` (`runa call --partial --explain`).
 */
export async function explainQueryEntry<I, R, P extends string>(
  transport: EngineTransport,
  entry: CalculationEntry,
  depth: ExplainDepth,
  inputSchema: z.ZodType<I>,
  resultSchema: z.ZodType<QueryResult<R, P>>,
  input: I,
): Promise<ExplainedQuery<QueryResult<R, P>>> {
  const call = explainingCall(transport, entry, true);
  const { wireCase, document } = inputDocument(entry, inputSchema, input);
  const output = caseOutput(entry, PARTIAL_OUTPUT_SCHEMA, await call(entry.name, document, depth === "full"));
  const wire = output["result"];
  const result = validated(entry, resultSchema, () => queryResultFromWire(entry, wireCase, wire));
  if (output["explanation"] === undefined) return { result };
  const wireResult = isObject(wire) ? wire["result"] : undefined;
  return { result, explanation: explanationFromWire(entry, wireCase, wireResult, output["explanation"]) };
}

// The resolver loop

/** One calculation entry running for one subject, on the resolver stack. */
export type EntryFrame = { readonly entry: string; readonly subject: string };

/**
 * What a resolver returns: an answer to write at the need's path, `ask` when
 * only the customer knows, or the resolution of another entry
 * ({@link ResolverContext.runEntry}).
 */
export type ResolverAnswer<V = unknown> =
  | { readonly kind: "answer"; readonly value: V }
  | { readonly kind: "ask" }
  | { readonly kind: "entry"; readonly resolution: Resolution<unknown, V, string> };

/** The resolver reply for a need only the customer can answer. */
export const ask: { readonly kind: "ask" } = Object.freeze({ kind: "ask" });

/** The resolver reply that answers a need with `value`. */
export function answer<V>(value: V): ResolverAnswer<V> {
  return { kind: "answer", value };
}

export interface ResolverContext<C> {
  /** The case sent in this round. */
  readonly partialCase: C;
  /** The entries running above this loop, outermost first. */
  readonly frames: readonly EntryFrame[];
  /**
   * Resolve another entry for this need, through the same loop. The frame is
   * the need's `entry` and `subject` unless given; a frame already on the
   * stack is a cycle and throws {@link ResolveCycleError}.
   */
  runEntry<C2, R2, P2 extends string>(
    query: Query<C2, R2, P2>,
    initialCase: C2,
    resolvers: Resolvers<C2, P2>,
    frame?: EntryFrame,
  ): Promise<ResolverAnswer<R2>>;
}

/** A partial call of one entry, such as `(c) => taxReportQuery(transport, c)`. */
export type Query<C, R, P extends string = string> = (partialCase: C) => Promise<QueryResult<R, P>>;

export type Resolver<C, V, P extends string = string> = (
  need: Need<P>,
  context: ResolverContext<C>,
) => ResolverAnswer<V> | Promise<ResolverAnswer<V>>;

/**
 * Resolvers keyed by path pattern: a concrete path (`taxpayer.married`), or a
 * pattern with `[]` for any list index (`accounts[].closingCashOre`) and `*`
 * for any map key. A need no resolver matches is a question.
 */
export type Resolvers<C, P extends string = string> = {
  readonly [pattern: string]: Resolver<C, unknown, P> | undefined;
};

/** An entry need whose own loop stopped at questions about its subject. */
export type PendingEntry = { readonly need: Need; readonly resolution: Resolution<unknown, unknown, string> };

/**
 * Where a resolver loop stopped: `Complete`, or `Questions` when a round
 * produced no answer. The partial case holds every answer written so far.
 */
export type Resolution<C, R, P extends string = string> =
  | {
      readonly $variant: "Complete";
      readonly result: R;
      readonly unusedPaths: readonly P[];
      readonly partialCase: C;
      readonly rounds: number;
    }
  | {
      readonly $variant: "Questions";
      /** The needs to ask the customer. */
      readonly questions: readonly Need<P>[];
      /** Entry needs waiting on questions in another entry's case. */
      readonly pending: readonly PendingEntry[];
      readonly partialCase: C;
      readonly rounds: number;
    };

export type ResolveOptions = {
  /** The entries already running above this loop; nested loops pass their stack. */
  readonly frames?: readonly EntryFrame[];
};

/** The resolver loop cannot continue. */
export class ResolveError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ResolveError";
  }
}

/** Two entries need each other for the same subjects: a model error; join them into one entry. */
export class ResolveCycleError extends ResolveError {
  readonly frames: readonly EntryFrame[];

  constructor(frames: readonly EntryFrame[]) {
    super(
      `calculation entries need each other: ${frames.map((frame) => `${frame.entry}(${frame.subject})`).join(" → ")}; model them as one joint entry`,
    );
    this.name = "ResolveCycleError";
    this.frames = frames;
  }
}

function findResolver<C, P extends string>(
  resolvers: Resolvers<C, P>,
  patterns: readonly string[],
  path: string,
): Resolver<C, unknown, P> | undefined {
  if (hasOwn(resolvers, path) && resolvers[path] !== undefined) return resolvers[path];
  const pattern = patterns.find((candidate) => resolvers[candidate] !== undefined && matchesPathPattern(path, candidate));
  return pattern === undefined ? undefined : resolvers[pattern];
}

/**
 * Resolve a partial case: query it, let the resolvers answer its needs, write
 * the answers into the case with {@link setAt}, and query again. Stops with
 * `Complete`, or with `Questions` when a round adds no answer. A path that is
 * still needed after it was answered is a {@link ResolveError}; an entry that
 * needs itself for the same subject is a {@link ResolveCycleError}.
 */
export async function resolve<C, R, P extends string>(
  query: Query<C, R, P>,
  initialCase: C,
  resolvers: Resolvers<C, P>,
  options: ResolveOptions = {},
): Promise<Resolution<C, R, P>> {
  const frames = options.frames ?? [];
  const patterns = Object.keys(resolvers).sort();
  const answered = new Set<string>();
  let partialCase = initialCase;
  for (let rounds = 1; ; rounds += 1) {
    const result = await query(partialCase);
    if (result.$variant === "Complete") {
      return { $variant: "Complete", result: result.result, unusedPaths: result.unusedPaths, partialCase, rounds };
    }
    const questions: Need<P>[] = [];
    const pending: PendingEntry[] = [];
    let next = partialCase;
    for (const need of result.needs) {
      if (answered.has(need.path)) {
        throw new ResolveError(`the engine still needs \`${need.path}\` after it was answered`);
      }
      const resolver = findResolver(resolvers, patterns, need.path);
      if (resolver === undefined) {
        questions.push(need);
        continue;
      }
      const reply = await resolver(need, {
        partialCase,
        frames,
        async runEntry(nestedQuery, nestedCase, nestedResolvers, frame) {
          const target = frame ?? (need.kind === "entry" ? { entry: need.entry, subject: need.subject } : undefined);
          if (target === undefined) {
            throw new ResolveError(`\`${need.path}\` is a fact need; give runEntry the frame of the entry it runs`);
          }
          const stack = [...frames, target];
          if (frames.some((active) => active.entry === target.entry && active.subject === target.subject)) {
            throw new ResolveCycleError(stack);
          }
          const resolution = await resolve(nestedQuery, nestedCase, nestedResolvers, { frames: stack });
          return { kind: "entry", resolution };
        },
      });
      if (reply.kind === "ask") {
        questions.push(need);
      } else if (reply.kind === "answer") {
        next = setAt(next, need.path, reply.value);
        answered.add(need.path);
      } else if (reply.resolution.$variant === "Complete") {
        next = setAt(next, need.path, reply.resolution.result);
        answered.add(need.path);
      } else {
        pending.push({ need, resolution: reply.resolution });
      }
    }
    if (next === partialCase) return { $variant: "Questions", questions, pending, partialCase, rounds };
    partialCase = next;
  }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser};

    fn contracts(source: &str) -> Vec<CalculationContract> {
        let statements = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect("parse calculation source");
        super::super::extract_calculation_contracts(&statements, source, None)
            .expect("extract calculation contracts")
    }

    fn generate(source: &str) -> BTreeMap<String, String> {
        generate_typescript_sdk(
            &contracts(source),
            "model.calculate.runa",
            DEFAULT_RUNTIME_PATH,
        )
        .expect("generate SDK")
        .into_iter()
        .map(|file| (file.name, file.contents))
        .collect()
    }

    fn assert_contains(text: &str, expected: &str) {
        assert!(
            text.contains(expected),
            "expected to find:\n{expected}\n--- in ---\n{text}"
        );
    }

    const META: &str = r#"# SourceInfo(url: String, section: String)
# CalculationField(path: String, label: String, question: String?, help: String?, unit: String?)
# CalculationMetaRole(a) = Source(value: a)
# CalculationMeta(a) = CalculationMeta(fields: List(CalculationField), attachments: a)
# impl MetaRole for CalculationMetaRole {}
# impl Meta for CalculationMeta {}
"#;

    fn records_source() -> String {
        format!(
            r#"{META}
# Child(name: String, age: Int)
# Household(income: Int, rate: Float, initial: Char, deduction: Int?, children: List(Child), grants: Map(String, Int), codes: Set(String), resident: Bool)
# Result(tax: Int)

= law = SourceInfo(url = "https://example.invalid/law", section = "§ 1")
= income_field = CalculationField(path = "income", label = "Yearly income", question = Some("What did you earn this year?"), help = Some("Before tax."), unit = Some("DKK øre"))
= age_field = CalculationField(path = "children.age", label = "Child's age", question = None, help = None, unit = Some("years"))
= household_meta = CalculationMeta(fields = [income_field, age_field], attachments = (Source(value = law),))
--@label:household_tax::meta:household_meta--

@ calculate("Household tax")
> household_tax(input: Household) -> Result {{ Result(tax = input.income / 4) }}
"#
        )
    }

    fn ledger_source() -> String {
        format!(
            r#"{META}
# Date(year: Int, month: Int, day: Int)
# Direction = Deposit | Withdrawal
# Movement(booked_on: Date, direction: Direction, amount_ore: Int, settled_on: Date?)
# Ledger(opening_cash_ore: Int, movements: List(Movement), directions: List(Direction), holidays: Map(String, Date))
# Summary(closing_cash_ore: Int, last_booked_on: Date)

= year_field = CalculationField(path = "movements.booked_on.year", label = "Booking date: year", question = None, help = None, unit = None)
= month_field = CalculationField(path = "movements.booked_on.month", label = "Booking date: month", question = None, help = None, unit = None)
= day_field = CalculationField(path = "movements.booked_on.day", label = "Booking date: day", question = None, help = None, unit = None)
= direction_field = CalculationField(path = "movements.direction", label = "In or out", question = Some("Did the money go in or out?"), help = None, unit = None)
= ledger_meta = CalculationMeta(fields = [year_field, month_field, day_field, direction_field], attachments = ())
--@label:ledger_balance::meta:ledger_meta--

@ calculate
> ledger_balance(input: Ledger) -> Summary {{ Summary(closing_cash_ore = input.opening_cash_ore, last_booked_on = Date(year = 2026, month = 1, day = 1)) }}
"#
        )
    }

    #[test]
    fn optional_record_members_carry_their_docs_in_types_and_partial_types() {
        let files = generate(&format!(
            r#"{META}
# Spouse(income: Int, cohabiting: Bool)
# Household(income: Int, spouse: Spouse?)
# Result(tax: Int)

= cohabiting_field = CalculationField(path = pathof(Household::spouse::cohabiting), label = "Spouse cohabiting", question = Some("Did you live together at the end of the year?"), help = None, unit = None)
= household_meta = CalculationMeta(fields = [cohabiting_field], attachments = ())
--@label:household_tax::meta:household_meta--

@ calculate
> household_tax(input: Household) -> Result {{ Result(tax = input.income / 4) }}
"#
        ));
        let doc = "  /**\n   * Spouse cohabiting.\n   *\n   * Did you live together at the end of the year?\n   *\n   * Futuruna type: `Bool`.\n   */\n";
        assert_contains(&files["types.ts"], &format!("{doc}  cohabiting: boolean;"));
        assert_contains(
            &files["partial.ts"],
            &format!("{doc}  cohabiting?: boolean | undefined;"),
        );
    }

    #[test]
    fn records_optionals_lists_maps_and_sets_follow_the_json_encoding() {
        let files = generate(&records_source());
        let types = &files["types.ts"];
        assert_contains(types, "export type Household = {");
        assert_contains(types, "  income: number;");
        assert_contains(types, "  rate: number;");
        assert_contains(types, "  initial: string;");
        assert_contains(types, "  deduction: number | null;");
        assert_contains(types, "  children: Child[];");
        assert_contains(types, "  grants: { [key: string]: number };");
        assert_contains(types, "  codes: string[];");
        assert_contains(types, "  resident: boolean;");
        assert!(!types.contains("import"), "types.ts needs no runtime here");

        let schema = &files["schema.ts"];
        assert_contains(
            schema,
            "import { z } from \"zod\";\nimport type * as types from \"./types.js\";\nimport {",
        );
        assert_contains(schema, "} from \"./runtime.js\";");
        assert_contains(
            schema,
            "export const HouseholdSchema: z.ZodType<types.Household> = z.lazy(() =>",
        );
        assert_contains(schema, "      income: IntSchema,");
        assert_contains(schema, "      rate: FloatSchema,");
        assert_contains(schema, "      initial: CharSchema,");
        assert_contains(schema, "      deduction: IntSchema.nullable(),");
        assert_contains(schema, "      children: z.array(ChildSchema),");
        assert_contains(schema, "      grants: z.record(z.string(), IntSchema),");
        assert_contains(schema, "      codes: SetSchema(z.string()),");
        assert_contains(schema, "    .strict()");
        assert_contains(schema, "  household_tax: \"");
        assert_contains(
            schema,
            "  children: [\"children\", listCodec(() => ChildCodec)],",
        );
        assert_contains(schema, "  grants: [\"grants\", valueCodec],");

        let partial = &files["partial.ts"];
        assert_contains(partial, "export type PartialHousehold = {");
        assert_contains(partial, "  deduction?: number | null | undefined;");
        assert_contains(partial, "  children?: PartialChild[] | undefined;");
        assert_contains(partial, "      deduction: IntSchema.nullable().optional(),");
        assert!(
            !partial.contains("PartialResult"),
            "result-only types get no partial form"
        );
    }

    #[test]
    fn field_docs_carry_label_question_help_unit_and_sources() {
        let types = &generate(&records_source())["types.ts"];
        assert_contains(
            types,
            "  /**\n   * Yearly income.\n   *\n   * What did you earn this year?\n   *\n   * Before tax.\n   *\n   * Unit: DKK øre.\n   *\n   * Sources: household_tax.\n   *\n   * Futuruna type: `Int`; Int values are safe integers.\n   */\n  income: number;",
        );
        assert_contains(
            types,
            "  /**\n   * Child's age.\n   *\n   * Unit: years.\n   *\n   * Sources: household_tax.\n   *\n   * Futuruna type: `Int`; Int values are safe integers.\n   */\n  age: number;",
        );
        assert_contains(
            types,
            "  /** Futuruna type: `Bool`. */\n  resident: boolean;",
        );
    }

    #[test]
    fn fields_are_camel_case_enums_are_names_and_dates_are_iso_strings() {
        let files = generate(&ledger_source());
        let types = &files["types.ts"];
        assert_contains(types, "import type { IsoDate } from \"./runtime.js\";");
        assert!(!types.contains("export type Date"), "a Date is an IsoDate");
        assert_contains(
            types,
            "/** Futuruna sum type `Direction`; its variants carry no fields, so a value is its variant's name. */\nexport type Direction = \"Deposit\" | \"Withdrawal\";",
        );
        assert_contains(types, "  openingCashOre: number;");
        assert_contains(types, "  settledOn: IsoDate | null;");
        assert_contains(types, "  holidays: { [key: string]: IsoDate };");
        // The parts' labels give the date's label; an enum's label has no `Variant:`.
        assert_contains(
            types,
            "  /**\n   * Booking date.\n   *\n   * Futuruna type: `Date`; a `Date` is an ISO `YYYY-MM-DD` string.\n   */\n  bookedOn: IsoDate;",
        );
        assert_contains(
            types,
            "  /**\n   * In or out.\n   *\n   * Did the money go in or out?\n   *\n   * Futuruna type: `Direction`.\n   */\n  direction: Direction;",
        );

        let schema = &files["schema.ts"];
        assert_contains(
            schema,
            "export const DirectionSchema: z.ZodType<types.Direction> = z.enum([\"Deposit\", \"Withdrawal\"]);",
        );
        assert_contains(schema, "      bookedOn: IsoDateSchema,");
        assert!(
            !schema.contains("DateSchema: "),
            "no schema of its own for Date"
        );
        assert_contains(
            schema,
            "export const MovementCodec: Codec = recordCodec({\n  bookedOn: [\"booked_on\", isoDateCodec],\n  direction: [\"direction\", enumCodec],\n  amountOre: [\"amount_ore\", valueCodec],\n  settledOn: [\"settled_on\", isoDateCodec],\n});",
        );
        assert_contains(
            schema,
            "  directions: [\"directions\", listCodec(enumCodec)],",
        );
        assert_contains(
            schema,
            "  holidays: [\"holidays\", mapCodec(isoDateCodec)],",
        );

        let partial = &files["partial.ts"];
        assert!(
            !partial.contains("PartialDirection"),
            "an enum is partial only as a whole"
        );
        assert!(
            !partial.contains("PartialDate"),
            "a date is partial only as a whole"
        );
        assert_contains(partial, "  direction?: types.Direction | undefined;");
        assert_contains(partial, "  bookedOn?: IsoDate | undefined;");
        assert_contains(
            partial,
            "  directions?: (types.Direction | Unknown<types.Direction>)[] | undefined;",
        );
        assert_contains(
            partial,
            "      directions: z.array(z.union([schema.DirectionSchema, UnknownSchema])).optional(),",
        );
        assert_contains(partial, "  \"movements[].bookedOn\": IsoDate;");
        assert_contains(partial, "  \"movements[].direction\": types.Direction;");
        assert_contains(partial, "  \"openingCashOre\": number;");
        assert!(
            !partial.contains("bookedOn.year"),
            "a date's parts have no paths"
        );
        assert!(
            !partial.contains("direction.$variant"),
            "an enum is its own value"
        );

        let client = &files["client.ts"];
        assert_contains(
            client,
            "export const ledgerBalanceEntry: CalculationEntry = {\n  name: \"ledger_balance\",\n  schemaHash: schema.SCHEMA_HASH.ledger_balance,\n  input: () => schema.LedgerCodec,\n  output: () => schema.SummaryCodec,\n};",
        );
    }

    #[test]
    fn two_fields_with_one_camel_case_name_are_an_error() {
        let error = generate_typescript_sdk(
            &contracts("# Input(tax_year: Int, taxYear: Int)\n@ calculate\n> both(input: Input) -> Int { input.tax_year }\n"),
            "model.calculate.runa",
            DEFAULT_RUNTIME_PATH,
        )
        .expect_err("`tax_year` and `taxYear` collide");
        assert!(error.contains("are both `taxYear`"), "{error}");
    }

    #[test]
    fn sum_types_with_payloads_are_unions_tagged_by_variant() {
        let files = generate(
            r#"# Shape = Circle(Int) | Rect(w: Int, h: Int) | Empty
# Pair(a, b) = Pair(first: a, second: b)
# Input(shape: Shape, pair: Pair(Int, Bool), maybe: Shape?)
@ calculate
> area(input: Input) -> Pair(Int, String) { Pair(first = 1, second = "x") }
"#,
        );
        let types = &files["types.ts"];
        assert_contains(
            types,
            "export type Shape =\n  | { $variant: \"Circle\"; $values: [number] }\n  | {\n      $variant: \"Rect\";",
        );
        assert_contains(types, "  | { $variant: \"Empty\" };");
        assert_contains(types, "export type Pair<a, b> = {");
        assert_contains(types, "  first: a;");
        assert_contains(types, "  pair: Pair<number, boolean>;");
        assert_contains(types, "  maybe: Shape | null;");

        let schema = &files["schema.ts"];
        assert_contains(schema, "z.discriminatedUnion(\"$variant\", [");
        assert_contains(
            schema,
            "z.object({ $variant: z.literal(\"Circle\"), $values: z.tuple([IntSchema]) }).strict(),",
        );
        assert_contains(
            schema,
            "z.object({ $variant: z.literal(\"Empty\") }).strict(),",
        );
        assert_contains(
            schema,
            "export const PairSchema = <a, b>(schema_a: z.ZodType<a>, schema_b: z.ZodType<b>): z.ZodType<types.Pair<a, b>> =>",
        );
        assert_contains(schema, "      pair: PairSchema(IntSchema, z.boolean()),");
        assert_contains(
            schema,
            "export const ShapeCodec: Codec = unionCodec({\n  Circle: { $values: [valueCodec] },\n  Rect: {\n    w: [\"w\", valueCodec],\n    h: [\"h\", valueCodec],\n  },\n  Empty: {},\n});",
        );
        assert_contains(
            schema,
            "export const PairCodec = (codec_a: CodecRef, codec_b: CodecRef): Codec =>\n  recordCodec({\n    first: [\"first\", codec_a],",
        );
        assert_contains(
            schema,
            "  pair: [\"pair\", () => PairCodec(valueCodec, valueCodec)],",
        );

        let partial = &files["partial.ts"];
        assert_contains(
            partial,
            "export type PartialShape =\n  | Unknown<types.Shape>\n  | { $variant: \"Circle\"; $values?: [number] | undefined }",
        );
        assert_contains(partial, "    UnknownSchema,");
        assert_contains(
            partial,
            "  \"shape.$variant\": \"Circle\" | \"Rect\" | \"Empty\";",
        );
        assert_contains(partial, "  \"shape.w\": number;");
        assert_contains(partial, "  \"shape.$values[0]\": number;");
        assert_contains(partial, "  \"pair.first\": number;");
        assert_contains(
            partial,
            "export const AreaQueryResultSchema = queryResultSchema(schema.PairSchema(IntSchema, z.string()), AreaInputPathSchema);",
        );
        assert_contains(
            &files["client.ts"],
            "  output: () => schema.PairCodec(valueCodec, valueCodec),",
        );
    }

    #[test]
    fn paths_use_list_indices_and_map_keys() {
        let partial = &generate(&records_source())["partial.ts"];
        assert_contains(partial, "  [path: `children[${number}].age`]: number;");
        assert_contains(partial, "  [path: `grants.${string}`]: number;");
        assert_contains(partial, "  \"children[].age\": number;");
        assert_contains(partial, "  \"grants.*\": number;");
        assert_contains(partial, "  \"deduction\": number | null;");
        assert_contains(
            partial,
            "export function householdTaxSetAt<K extends HouseholdTaxInputPath>(",
        );
    }

    #[test]
    fn several_entries_each_get_calls_hashes_and_paths() {
        let source = r#"# Person(name: String, income: Int)
# Couple(first: Person, second: Person)
@ calculate("One person")
> person_tax(input: Person) -> Int { input.income / 4 }
@ calculate
> couple_tax(input: Couple) -> Int { input.first.income + input.second.income }
"#;
        let contracts = contracts(source);
        let files = generate(source);
        let client = &files["client.ts"];
        assert_contains(
            client,
            "export function personTax(transport: EngineTransport, input: types.Person): Promise<number> {\n  return callEntry(transport, personTaxEntry, schema.PersonSchema, IntSchema, input);",
        );
        assert_contains(
            client,
            "export function coupleTaxQuery(\n  transport: EngineTransport,\n  input: partial.PartialCouple,\n): Promise<partial.CoupleTaxQueryResult> {",
        );
        assert_contains(client, "/**\n * Calculate `person_tax`: One person.");
        assert_contains(client, "export type PersonTaxResolvers = {");
        for expected in [
            "export function personTaxExplained(\n  transport: EngineTransport,\n  input: types.Person,\n  depth: ExplainDepth = \"direct\",\n): Promise<Explained<number>> {\n  return explainEntry(transport, personTaxEntry, depth, schema.PersonSchema, IntSchema, input);",
            "export function coupleTaxQueryExplained(\n  transport: EngineTransport,\n  input: partial.PartialCouple,\n  depth: ExplainDepth = \"direct\",\n): Promise<ExplainedQuery<partial.CoupleTaxQueryResult>> {",
            "type ExplainDepth",
            "type Explained,",
            "type ExplainedQuery",
        ] {
            assert_contains(client, expected);
        }
        let schema = &files["schema.ts"];
        for contract in &contracts {
            assert_contains(
                schema,
                &format!("  {}: \"{}\",", contract.entry, contract.schema_hash),
            );
            assert_contains(
                &files["index.ts"],
                &format!("//   {}: {}", contract.entry, contract.schema_hash),
            );
        }
        assert_contains(&files["index.ts"], "export * from \"./runtime.js\";");
        // Entries are ordered by name, so declaration order does not matter.
        assert!(schema.find("  couple_tax:").unwrap() < schema.find("  person_tax:").unwrap());
        let mut reversed = contracts.clone();
        reversed.reverse();
        assert_eq!(
            generate_typescript_sdk(&contracts, "model.calculate.runa", DEFAULT_RUNTIME_PATH)
                .unwrap(),
            generate_typescript_sdk(&reversed, "model.calculate.runa", DEFAULT_RUNTIME_PATH)
                .unwrap()
        );
    }

    #[test]
    fn the_runtime_is_one_module_every_model_shares() {
        let ledger = generate_typescript_sdk(
            &contracts(&ledger_source()),
            "ledger.calculate.runa",
            "../runtime.ts",
        )
        .unwrap();
        let household = generate_typescript_sdk(
            &contracts(&records_source()),
            "household.calculate.runa",
            "../runtime.ts",
        )
        .unwrap();
        let runtime = |files: &[GeneratedFile]| {
            files
                .iter()
                .find(|file| file.name == "../runtime.ts")
                .expect("the runtime is generated")
                .contents
                .clone()
        };
        assert_eq!(runtime(&ledger), runtime(&household));
        assert_contains(
            &runtime(&ledger),
            "export async function resolve<C, R, P extends string>(",
        );
        assert!(!runtime(&ledger).contains("{INPUT_SCHEMA}"));
        for file in &ledger {
            if file.name != "../runtime.ts" {
                assert!(
                    !file.contents.contains("export class"),
                    "{} holds runtime code",
                    file.name
                );
                assert!(
                    !file.contents.contains("\"./runtime.js\""),
                    "{} imports the default runtime",
                    file.name
                );
            }
        }
        let index = &ledger
            .iter()
            .find(|file| file.name == "index.ts")
            .unwrap()
            .contents;
        assert_contains(index, "export * from \"../runtime.js\";");

        assert_eq!(runtime_specifier("runtime.ts").unwrap(), "./runtime.js");
        assert_eq!(
            runtime_specifier("./shared/sdk.ts").unwrap(),
            "./shared/sdk.js"
        );
        for invalid in [
            "/tmp/runtime.ts",
            "runtime.js",
            "types.ts",
            "./index.ts",
            ".ts",
            "lib/.d.ts",
            "a\\b.ts",
        ] {
            assert!(runtime_specifier(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn a_date_label_is_what_its_parts_share() {
        assert_eq!(
            shared_label(
                [
                    "Booking date: day",
                    "Booking date: month",
                    "Booking date: year"
                ]
                .into_iter()
            ),
            "Booking date"
        );
        assert_eq!(
            shared_label(["Payment date (day)", "Payment date (year)"].into_iter()),
            "Payment date"
        );
        assert_eq!(
            shared_label(["Same: label", "Same: label"].into_iter()),
            "Same: label"
        );
        assert_eq!(shared_label(["Day", "Month"].into_iter()), "Day");
        assert_eq!(camel_case("opening_cash_ore"), "openingCashOre");
        assert_eq!(camel_case("_private_note"), "_privateNote");
        assert_eq!(camel_case("AccountCharge"), "AccountCharge");
        assert_eq!(
            camel_case_path("events.AccountCharge.amount_ore.$variant"),
            "events.AccountCharge.amountOre.$variant"
        );
    }

    #[test]
    fn map_key_signatures_cover_longer_paths_through_the_map() {
        let partial = &generate(
            "# Item(code: String, amount: Int?)\n# Input(items: Map(String, Item))\n@ calculate\n> count(input: Input) -> Int { 0 }\n",
        )["partial.ts"];
        assert_contains(
            partial,
            "  [path: `items.${string}`]: PartialItem | number | null | string;",
        );
        assert_contains(
            partial,
            "  [path: `items.${string}.amount`]: number | null;",
        );
        assert_contains(partial, "  \"items.*\": PartialItem;");
        assert!(template_matches("a.*", "a.*.b[]"));
        assert!(template_matches("a[].b", "a[3].b"));
        assert!(!template_matches("a.*.b", "a.*"));
        assert!(!template_matches("a[0]", "a[]"));
    }

    #[test]
    fn recursive_input_types_have_open_paths() {
        let files = generate(
            r#"# Node = Leaf(value: Int) | Branch(left: Node, right: Node)
@ calculate
> total(input: Node) -> Int { 0 }
"#,
        );
        let partial = &files["partial.ts"];
        assert_contains(partial, "  [path: string]: unknown;");
        assert_contains(
            partial,
            "export const TotalInputPathSchema: z.ZodType<TotalInputPath> = z.string();",
        );
        assert_contains(&files["types.ts"], "      left: Node;");
        assert_contains(
            &files["schema.ts"],
            "    left: [\"left\", () => NodeCodec],",
        );
    }

    #[test]
    fn a_contract_name_that_collides_with_a_generated_name_is_an_error() {
        for (source, name) in [
            ("# Need(amount: Int)\n@ calculate\n> needed(input: Need) -> Int { input.amount }\n", "`Need`"),
            ("# Unknown(amount: Int)\n@ calculate\n> needed(input: Unknown) -> Int { input.amount }\n", "`Unknown`"),
        ] {
            let error = generate_typescript_sdk(&contracts(source), "model.calculate.runa", DEFAULT_RUNTIME_PATH)
                .expect_err("the name collides with a runtime export");
            assert!(error.contains(name), "{error}");
        }
    }

    #[test]
    fn units_of_measure_are_branded_number_types() {
        let source = r#"# unit Ore "DKK øre"
# unit Micro "millionths of a share"
# Holding(quantity_micro: Micro, value_ore: Ore, count: Int, history: List(Ore)?)
@ calculate
> holding_value(input: Holding) -> Ore { input.value_ore }
"#;
        let files = generate(source);
        let types = &files["types.ts"];
        assert_contains(
            types,
            "/**\n * DKK øre.\n *\n * A Futuruna `Ore`: a safe integer of this unit, on the wire the same JSON number. Only `ore(n)` and the validators make one; a bare number or a value of another unit is not one.\n */\nexport type Ore = number & { readonly __unit: \"Ore\" };",
        );
        assert_contains(
            types,
            "export type Micro = number & { readonly __unit: \"Micro\" };",
        );
        assert_contains(types, "  quantityMicro: Micro;");
        assert_contains(types, "  count: number;");
        assert_contains(types, "  history: Ore[] | null;");
        assert_contains(types, "Futuruna type: `List(Ore)?`");
        let schema = &files["schema.ts"];
        assert_contains(
            schema,
            "export const OreSchema: z.ZodType<types.Ore> = unitSchema<types.Ore>();",
        );
        assert_contains(
            schema,
            "export function ore(value: number): types.Ore {\n  return unitValue<types.Ore>(\"Ore\", value);\n}",
        );
        assert_contains(schema, "      valueOre: OreSchema,");
        assert_contains(schema, "      count: IntSchema,");
        let partial = &files["partial.ts"];
        assert_contains(partial, "  \"valueOre\": types.Ore;");
        assert_contains(partial, "  [path: `history[${number}]`]: types.Ore;");
        assert_contains(partial, "      valueOre: schema.OreSchema.optional(),");
        assert_contains(
            &files["client.ts"],
            "export function holdingValue(transport: EngineTransport, input: types.Holding): Promise<types.Ore> {\n  return callEntry(transport, holdingValueEntry, schema.HoldingSchema, schema.OreSchema, input);",
        );
        let error = generate_typescript_sdk(
            &contracts("# unit Answer \"replies\"\n# In(n: Answer)\n@ calculate\n> f(input: In) -> Int { 0 }\n"),
            "model.calculate.runa",
            DEFAULT_RUNTIME_PATH,
        )
        .expect_err("`answer` is a runtime export");
        assert!(error.contains("`answer`"), "{error}");
    }

    #[test]
    fn partial_type_names_are_qualified_from_the_client() {
        assert_eq!(
            qualify_partial("PartialPair<PartialChild, number>[]"),
            "partial.PartialPair<partial.PartialChild, number>[]"
        );
        assert_eq!(
            qualify_partial("PartialPairSchema(IntSchema)"),
            "partial.PartialPairSchema(IntSchema)"
        );
        assert!(uses_identifier("x: Need<P>;", "Need"));
        assert!(!uses_identifier("x: TaxReportNeed;", "Need"));
        assert!(!uses_identifier("  name: \"a\",", "name"));
        assert!(!uses_identifier("schema.IntSchema", "IntSchema"));
    }
}
