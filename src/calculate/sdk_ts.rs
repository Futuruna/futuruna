//! TypeScript SDK generation for calculation contracts (`runa sdk ts`), and
//! the runtime package the SDKs import (`runa sdk runtime`).
//!
//! Everything is derived from the contracts `runa schema` emits. The SDK reads
//! as ordinary TypeScript: camelCase field names, a sum type whose variants
//! carry no fields as a string union, and `Date` as an ISO `YYYY-MM-DD` string.
//! The wire is unchanged: the canonical JSON encoding that `decode_value`
//! reads and `encode_value` writes. Each entry's modules carry codec tables
//! (TypeScript name, wire name, encoding), and the runtime package
//! `@futuruna/runtime` encodes a case before a call and decodes the result and
//! the needs' paths after it. The partial-case types and the
//! `Complete`/`Needs` result follow `docs/rfcs/calculation-needs.md`; the
//! client reaches an engine only through a transport. Every entry is also
//! answered round by round: `<entry>Answer` calls one `resolve` per round with
//! every need. The output depends only on the contracts and the source label,
//! so regenerating an unchanged model rewrites identical files.

use super::answer_guide::{answer_guide, is_date, written_type, GuideNeed};
use super::{
    calculation_type_occurrence_paths, definition_substitutions, joined_column_path,
    product_variant, substitute_contract_type, CalculationContract, CalculationField,
    CalculationFieldMetadata, CalculationNeedKind, CalculationScalar, CalculationTypeDefinition,
    CalculationTypeRef, CalculationUnit, CalculationVariant, CONTRACT_SCHEMA, INPUT_SCHEMA,
    OUTPUT_SCHEMA,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// The output envelope schema of `runa call --partial`.
pub const PARTIAL_OUTPUT_SCHEMA: &str = "futuruna.entry.partial-output.v1";

/// The npm package of the runtime every generated SDK imports.
pub const RUNTIME_PACKAGE: &str = "@futuruna/runtime";

/// The version of the runtime package: Futuruna's.
pub const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// Generate the TypeScript SDK for every entry of one source file.
///
/// `source` names the file in the generated headers. The modules import the
/// runtime from the package `@futuruna/runtime` ([`runtime_package`]). A file
/// whose contracts cannot be expressed (an unsupported type, two entries
/// disagreeing on a type, two fields with the same camelCase name, or a
/// contract name that collides with a generated name) is an error.
pub fn generate_typescript_sdk(
    contracts: &[CalculationContract],
    source: &str,
) -> Result<Vec<GeneratedFile>, String> {
    let sdk = Sdk::new(contracts, source)?;
    let files = vec![
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
    ];
    Ok(files)
}

/// The npm package `@futuruna/runtime` (`runa sdk runtime --out DIR`): what
/// every generated SDK shares, as an ES module with its declarations, at
/// Futuruna's version. It depends on no model; `zod` is its peer dependency,
/// so the SDK and the runtime validate with one `zod`.
pub fn runtime_package() -> Vec<GeneratedFile> {
    let header = format!(
        "// {RUNTIME_PACKAGE} {RUNTIME_VERSION}, written by `runa sdk runtime`. Do not edit.\n// The runtime of the SDKs `runa sdk ts` generates: the wire codec, paths and setAt,\n// needs, calls through an engine transport, answering an entry and unit converters.\n"
    );
    let package = format!(
        r#"{{
  "name": {name},
  "version": {version},
  "description": "The runtime of the TypeScript SDKs that `runa sdk ts` generates from Futuruna models.",
  "license": "MIT",
  "type": "module",
  "exports": {{
    ".": {{
      "types": "./index.d.ts",
      "default": "./index.js"
    }}
  }},
  "types": "./index.d.ts",
  "files": ["index.js", "index.d.ts"],
  "sideEffects": false,
  "peerDependencies": {{
    "zod": "^4.0.0"
  }}
}}
"#,
        name = string_literal(RUNTIME_PACKAGE),
        version = string_literal(RUNTIME_VERSION),
    );
    vec![
        GeneratedFile {
            name: "package.json".to_string(),
            contents: package,
        },
        GeneratedFile {
            name: "index.js".to_string(),
            contents: format!("{header}{}", with_schemas(RUNTIME_JS)),
        },
        GeneratedFile {
            name: "index.d.ts".to_string(),
            contents: format!("{header}{}", with_schemas(RUNTIME_DTS)),
        },
    ]
}

/// The runtime's text with the wire schema ids filled in.
fn with_schemas(text: &str) -> String {
    text.replace("{INPUT_SCHEMA}", INPUT_SCHEMA)
        .replace("{OUTPUT_SCHEMA}", OUTPUT_SCHEMA)
        .replace("{PARTIAL_OUTPUT_SCHEMA}", PARTIAL_OUTPUT_SCHEMA)
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
    /// The lists whose elements declare a key field, by element pattern.
    keys: BTreeMap<String, ElementKey>,
    /// A recursive input type has paths of any depth.
    open_paths: bool,
}

/// The key field a list's element type declares (`@ field(key = True)`): its
/// camelCase name, and its complete type and validator as `partial.ts` writes them.
#[derive(Clone)]
struct ElementKey {
    field: String,
    ty: String,
    zod: String,
}

struct Sdk<'a> {
    source: &'a str,
    entries: Vec<Entry<'a>>,
    definitions: BTreeMap<&'a str, &'a CalculationTypeDefinition>,
    input_definitions: BTreeSet<&'a str>,
    /// The units of measure (`# unit Ore "DKK øre"`) the contracts use, by
    /// name: each is a branded number type, and one with a scale whose
    /// values are all finite decimals also has decimal converters.
    units: BTreeMap<&'a str, &'a CalculationUnit>,
    /// The named scalar types (`# Isin: String "..." where ...`) the
    /// contracts use, by name: each is a branded string or number type.
    scalars: BTreeMap<&'a str, &'a CalculationScalar>,
    /// (type, variant, field) → the field metadata of every input path the
    /// field occurs at, including the metadata of its `$variant` tag, and for
    /// a `Date` field without its own metadata, the metadata its parts share.
    field_metadata: BTreeMap<(&'a str, &'a str, &'a str), Vec<Cow<'a, CalculationFieldMetadata>>>,
}

impl<'a> Sdk<'a> {
    fn new(contracts: &'a [CalculationContract], source: &'a str) -> Result<Self, String> {
        if contracts.is_empty() {
            return Err(format!("{source} declares no `@ entry`"));
        }
        let mut contracts: Vec<&CalculationContract> = contracts.iter().collect();
        contracts.sort_by(|left, right| left.entry.cmp(&right.entry));
        if let Some(pair) = contracts
            .windows(2)
            .find(|pair| pair[0].entry == pair[1].entry)
        {
            return Err(format!("entry `{}` is declared twice", pair[0].entry));
        }

        let mut definitions: BTreeMap<&str, &CalculationTypeDefinition> = BTreeMap::new();
        let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
        for contract in &contracts {
            for definition in &contract.definitions {
                match definitions.get(definition.name.as_str()) {
                    Some(existing) if *existing != definition => {
                        return Err(format!(
                            "type `{}` differs between entries `{}` and `{}`",
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

        let mut units: BTreeMap<&str, &CalculationUnit> = BTreeMap::new();
        for contract in &contracts {
            for unit in &contract.units {
                match units.insert(&unit.name, unit) {
                    Some(other) if other != unit => {
                        return Err(format!(
                            "unit `{}` is declared differently by two entries: {:?} and {:?}",
                            unit.name, other, unit
                        ));
                    }
                    _ => {}
                }
            }
        }

        let mut scalars: BTreeMap<&str, &CalculationScalar> = BTreeMap::new();
        for contract in &contracts {
            for scalar in &contract.scalars {
                match scalars.insert(&scalar.name, scalar) {
                    Some(other) if other != scalar => {
                        return Err(format!(
                            "named scalar type `{}` differs between entries",
                            scalar.name
                        ));
                    }
                    _ => {}
                }
            }
        }

        let mut sdk = Sdk {
            source,
            entries: Vec::new(),
            definitions,
            input_definitions: BTreeSet::new(),
            units,
            scalars,
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
                keys: BTreeMap::new(),
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
            CalculationTypeRef::Primitive { name, unit, scalar } => {
                if !PRIMITIVES.contains(&name.as_str()) {
                    Err(format!("{context}: unsupported primitive type `{name}`"))
                } else if unit.is_some() && name != "Int" {
                    Err(format!(
                        "{context}: a unit of measure on `{name}`, not `Int`"
                    ))
                } else if let Some(scalar) = scalar
                    .as_deref()
                    .filter(|scalar| unit.is_some() || !self.scalars.contains_key(scalar))
                {
                    Err(format!(
                        "{context}: undeclared named scalar type `{scalar}`"
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
                if let Some(key) = self.element_key(item) {
                    entry.keys.insert(element.clone(), key);
                }
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

    /// The key field of a list element whose type declares one: a record's
    /// field, or the field every variant of a sum type declares alike.
    fn element_key(&self, item: &CalculationTypeRef) -> Option<ElementKey> {
        let CalculationTypeRef::Named { name, arguments } = without_optional(item) else {
            return None;
        };
        let definition = self.definitions[name.as_str()];
        let key = definition.key.as_deref()?;
        let field = definition
            .variants
            .first()?
            .fields
            .iter()
            .find(|field| field.name == key)?;
        let ty =
            substitute_contract_type(&field.ty, &definition_substitutions(definition, arguments));
        Some(ElementKey {
            field: camel_case(key),
            ty: self.ts_type(&ty, Side::Qualified),
            zod: self.zod(&ty, Side::Qualified),
        })
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
        for (name, _) in runtime_exports() {
            names.push((name.to_string(), "the runtime"));
        }
        names.push(("SCHEMA_HASH".to_string(), "schema.ts"));
        for unit in self.units.keys() {
            names.push((unit.to_string(), "types.ts"));
            names.push((format!("{unit}Schema"), "schema.ts"));
            names.push((value_identifier(unit), "schema.ts"));
        }
        for scalar in self.scalars.keys() {
            names.push((scalar.to_string(), "types.ts"));
            names.push((format!("{scalar}Schema"), "schema.ts"));
            names.push((value_identifier(scalar), "schema.ts"));
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
                "InputPatterns",
                "InputPattern",
                "InputPatternList",
                "InputPath",
                "InputPathSchema",
                "ElementKey",
                "Need",
                "NeedSchema",
                "QueryResult",
                "QueryResultSchema",
            ] {
                names.push((format!("{types}{suffix}"), "partial.ts"));
            }
            names.push((format!("{values}SetAt"), "partial.ts"));
            names.push((format!("{values}Guide"), "partial.ts"));
            names.push((values.clone(), "client.ts"));
            names.push((format!("{values}Query"), "client.ts"));
            names.push((format!("{values}Explained"), "client.ts"));
            names.push((format!("{values}QueryExplained"), "client.ts"));
            names.push((format!("{values}Entry"), "client.ts"));
            names.push((format!("{types}Resolve"), "client.ts"));
            names.push((format!("{values}Answer"), "client.ts"));
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
            CalculationTypeRef::Primitive {
                scalar: Some(scalar),
                ..
            } => match side {
                Side::Local => scalar.clone(),
                Side::Qualified | Side::Partial => format!("types.{scalar}"),
            },
            CalculationTypeRef::Primitive {
                name,
                unit: None,
                scalar: None,
            } => match name.as_str() {
                "Int" | "Float" => "number",
                "Bool" => "boolean",
                _ => "string",
            }
            .to_string(),
            CalculationTypeRef::Unit => "null".to_string(),
            // In a partial case "none" is `none`, so a bare `null` cannot pass for it.
            CalculationTypeRef::Optional { item } => match side {
                Side::Partial => format!("{} | None", self.ts_type(item, side)),
                Side::Local | Side::Qualified => format!("{} | null", self.ts_type(item, side)),
            },
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
            CalculationTypeRef::Primitive {
                scalar: Some(scalar),
                ..
            } => match side {
                Side::Local => format!("{scalar}Schema"),
                Side::Qualified | Side::Partial => format!("schema.{scalar}Schema"),
            },
            CalculationTypeRef::Primitive {
                name,
                unit: None,
                scalar: None,
            } => match name.as_str() {
                "Int" => "IntSchema",
                "Float" => "FloatSchema",
                "Char" => "CharSchema",
                "Bool" => "z.boolean()",
                _ => "z.string()",
            }
            .to_string(),
            CalculationTypeRef::Unit => "z.null()".to_string(),
            CalculationTypeRef::Optional { item } => match side {
                Side::Partial => format!("z.union([{}, NoneSchema])", self.zod(item, side)),
                Side::Local | Side::Qualified => format!("{}.nullable()", self.zod(item, side)),
            },
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
            CalculationTypeRef::Optional { item } => {
                format!("optionalCodec({})", self.codec(item, qualified))
            }
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
        let _ = writeln!(
            out,
            "// Imports {RUNTIME_PACKAGE} {RUNTIME_VERSION}, the runtime of the runa that generated it."
        );
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
        // String literals (paths, variant names) name no identifiers.
        let code: Vec<String> = body
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !(line.starts_with("//") || line.starts_with("/*") || line.starts_with('*'))
            })
            .map(without_strings)
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
        let used: Vec<String> = runtime_exports()
            .filter(|(name, _)| uses(name))
            .map(|(name, is_type)| {
                if is_type && !type_only {
                    format!("type {name}")
                } else {
                    name.to_string()
                }
            })
            .collect();
        if !used.is_empty() {
            let keyword = if type_only { "import type" } else { "import" };
            let specifier = string_literal(RUNTIME_PACKAGE);
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
        for (unit, declared) in &self.units {
            let mut doc = vec![
                sentence(&declared.description),
                String::new(),
                format!(
                    "A Futuruna `{unit}`: a safe integer of this unit, on the wire the same JSON number. Only `{}(n)` and the validators make one; a bare number or a value of another unit is not one.",
                    value_identifier(unit)
                ),
            ];
            if let Some(scale) = &declared.scale {
                doc.push(String::new());
                doc.push(match decimal_places(scale.per) {
                    Some(_) => format!(
                        "One `{unit}` is 1/{}{}. `{name}.fromDecimal` and `{name}.toDecimal` convert a decimal amount exactly.",
                        scale.per,
                        scale_base(&scale.of),
                        name = value_identifier(unit)
                    ),
                    None => format!(
                        "One `{unit}` is 1/{}{}, which is no finite decimal: the SDK has no decimal converters for it.",
                        scale.per,
                        scale_base(&scale.of)
                    ),
                });
            }
            write_doc(&mut body, "", &doc);
            let _ = writeln!(
                body,
                "export type {unit} = number & {{ readonly __unit: {} }};\n",
                string_literal(unit)
            );
        }
        for scalar in self.scalars.values() {
            write_doc(&mut body, "", &scalar_doc(scalar));
            let _ = writeln!(
                body,
                "export type {} = {} & {{ readonly __scalar: {} }};\n",
                scalar.name,
                scalar_wire_type(scalar),
                string_literal(&scalar.name)
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
            "// Field names are camelCase (snake_case on the wire). A sum type whose variants carry\n// no fields is its variant's name; other sum types are unions tagged by `$variant`.\n// A `Date` is an ISO `YYYY-MM-DD` string (`IsoDate`, made by `isoDate`). Int is `number`\n// and holds safe integers only; the validators in schema.ts reject anything else. An\n// optional value is `T | null` (in a partial case, `T | None`: \"none\" is `none`).\n// An Int with a unit of measure (`# unit Ore \"DKK øre\"`) is the branded number type `Ore`;\n// a named scalar type (`# Isin: String \"...\" where ...`) is the branded type `Isin`.\n\n",
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
        let mut type_line = format!("Futuruna type: `{}`", written_type(&field.ty));
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
        body.push_str(
            "/** The contract fingerprint of each entry. */\nexport const SCHEMA_HASH = {\n",
        );
        for entry in &self.entries {
            let _ = writeln!(
                body,
                "  {}: {},",
                property_name(&entry.contract.entry),
                string_literal(&entry.contract.schema_hash)
            );
        }
        body.push_str("} as const;\n");
        for (unit, declared) in &self.units {
            let _ = writeln!(
                body,
                "\n/** Validates a `{unit}` ({}): a safe integer, typed `{unit}` once parsed. */\nexport const {unit}Schema: z.ZodType<types.{unit}> = unitSchema<types.{unit}>({});\n",
                escape_doc(&declared.description),
                string_literal(unit)
            );
            let name = value_identifier(unit);
            let _ = writeln!(
                body,
                "/** `value` as a `{unit}`; throws a `RangeError` unless it is a safe integer. */\nexport function {name}(value: number): types.{unit} {{\n  return unitValue<types.{unit}>({}, value);\n}}",
                string_literal(unit)
            );
            let Some(scale) = &declared.scale else {
                continue;
            };
            let Some(places) = decimal_places(scale.per) else {
                continue;
            };
            let base = scale_base(&scale.of);
            let of = if base.is_empty() {
                String::new()
            } else {
                format!(" of{base}")
            };
            let scale_literal = format!(
                "{{ of: {}, per: {} }}",
                string_literal(&scale.of),
                scale.per
            );
            body.push('\n');
            write_doc(
                &mut body,
                "",
                &[format!(
                    "`amount`, a decimal number{of} as text (`\"1740.25\"`), as a `{unit}`, exactly: never through a float. Throws a `CalculationError` of kind `amount`, naming the unit and the amount, when it is finer than one `{unit}` (1/{}{base}) or not a decimal number.",
                    scale.per
                )],
            );
            let _ = writeln!(
                body,
                "{name}.fromDecimal = (amount: string): types.{unit} =>\n  unitFromDecimal<types.{unit}>({}, {scale_literal}, amount);\n",
                string_literal(unit)
            );
            write_doc(
                &mut body,
                "",
                &[format!(
                    "`value` as a decimal number{of}, exactly, with {places} decimal{}: how a figure in `{unit}` reads{}.",
                    if places == 1 { "" } else { "s" },
                    if base.is_empty() { String::new() } else { format!(" in{base}") }
                )],
            );
            let _ = writeln!(
                body,
                "{name}.toDecimal = (value: types.{unit}): string => unitToDecimal({}, {scale_literal}, value);",
                string_literal(unit)
            );
        }
        for scalar in self.scalars.values() {
            let name = &scalar.name;
            let wire = scalar_wire_type(scalar);
            let _ = writeln!(
                body,
                "\n/** Validates a `{name}` ({}): a {wire}, typed `{name}` once parsed. The engine runs its checks. */\nexport const {name}Schema: z.ZodType<types.{name}> = scalarSchema<types.{name}>({}, {});\n",
                escape_doc(&scalar.description),
                string_literal(name),
                string_literal(&scalar.of)
            );
            let mut doc = scalar_doc(scalar);
            doc.push(String::new());
            doc.push(format!(
                "Throws a `RangeError` unless `value` is a {wire}; the engine runs the checks and reports a value that fails one as an input error at its path."
            ));
            write_doc(&mut body, "", &doc);
            let _ = writeln!(
                body,
                "export function {}(value: {wire}): types.{name} {{\n  return scalarValue<types.{name}>({}, {}, value);\n}}",
                value_identifier(name),
                string_literal(name),
                string_literal(&scalar.of)
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
                        "{{@link types.{name}}} in a partial case: a field left out is unknown, `none` answers that an optional field is absent, and `{{}}` is a record whose fields are all unknown."
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
        let name = &contract.entry;
        let types = &entry.type_name;
        let values = &entry.value_name;
        let input = self.ts_type(&contract.input, Side::Partial);
        let output = self.ts_type(&contract.output, Side::Qualified);
        let output_schema = self.zod(&contract.output, Side::Qualified);
        let _ = writeln!(out, "// `{name}`\n");

        write_doc(
            out,
            "",
            &[
                format!("The input path patterns of `{name}` a need can name, with the value an answer writes there."),
                "A list index is `[]` and a map key `*`.".to_string(),
            ],
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
        let _ = writeln!(out, "/** An input path pattern of `{name}`. */");
        let _ = writeln!(
            out,
            "export type {types}InputPattern = Extract<keyof {types}InputPatterns, string>;\n"
        );
        let _ = writeln!(
            out,
            "/** The input path patterns of `{name}`, which tag its needs. */"
        );
        let _ = writeln!(
            out,
            "export const {types}InputPatternList: readonly {types}InputPattern[] = ["
        );
        for pattern in entry.patterns.keys() {
            let _ = writeln!(out, "  {},", string_literal(pattern));
        }
        out.push_str("];\n\n");

        // A recursive input type has paths of any depth: they cannot be listed.
        let listed = if entry.open_paths {
            "undefined".to_string()
        } else {
            format!("{types}InputPatternList")
        };
        let _ = writeln!(
            out,
            "/** A concrete input path of `{name}`, with list indices and map keys filled in. */"
        );
        let _ = writeln!(
            out,
            "export type {types}InputPath = InputPath<{types}InputPattern>;"
        );
        let _ = writeln!(
            out,
            "export const {types}InputPathSchema = inputPathSchema<{types}InputPattern>({listed});\n"
        );

        let element_key = if entry.keys.is_empty() {
            "never".to_string()
        } else {
            let _ = writeln!(
                out,
                "/** The key of a list element of `{name}` a need lies in, where its type declares one. */"
            );
            let members = entry
                .keys
                .iter()
                .map(|(element, key)| {
                    format!(
                        "  | ElementKey<{}, {}, {}>",
                        string_literal(element),
                        string_literal(&key.field),
                        key.ty
                    )
                })
                .collect::<Vec<_>>();
            let _ = writeln!(
                out,
                "export type {types}ElementKey =\n{};\n",
                members.join("\n")
            );
            format!("{types}ElementKey")
        };
        write_doc(
            out,
            "",
            &[format!(
                "A need of `{name}`: its pattern, its concrete path and the field's metadata. Narrowing `pattern` narrows the type of its answer."
            )],
        );
        let _ = writeln!(
            out,
            "export type {types}Need = Need<{types}InputPattern, {element_key}>;"
        );
        let key_schemas = entry
            .keys
            .iter()
            .map(|(element, key)| {
                format!(
                    "  {{ element: {}, field: {}, value: {} }},\n",
                    string_literal(element),
                    string_literal(&key.field),
                    key.zod
                )
            })
            .collect::<String>();
        let key_schemas = if key_schemas.is_empty() {
            "[]".to_string()
        } else {
            format!("[\n{key_schemas}]")
        };
        let _ = writeln!(
            out,
            "export const {types}NeedSchema = needSchema<{types}InputPattern, {element_key}>({listed}, {key_schemas});\n"
        );
        let _ = writeln!(out, "/** The result of a partial call of `{name}`. */");
        let _ = writeln!(
            out,
            "export type {types}QueryResult = QueryResult<{output}, {types}Need>;"
        );
        let _ = writeln!(
            out,
            "export const {types}QueryResultSchema = queryResultSchema({output_schema}, {types}NeedSchema, {types}InputPathSchema);\n"
        );
        write_doc(
            out,
            "",
            &[
                format!("{{@link setAt}} for a `{name}` case: writes `value` at the need's path, typed by the need's pattern."),
                "A need of several patterns takes no value until `need.pattern` is narrowed.".to_string(),
            ],
        );
        let _ = writeln!(
            out,
            "export function {values}SetAt<N extends {types}Need>(\n  partialCase: {input},\n  need: N,\n  value: AnswerTo<{types}InputPatterns, N>,\n): {input} {{\n  return setAt(partialCase, need.path, value);\n}}"
        );
        self.write_guide(out, entry);
    }

    /// `<entry>Guide`: the entry's answer guide ([`answer_guide`]), keyed by
    /// the TypeScript need pattern, each pattern a need can name once.
    fn write_guide(&self, out: &mut String, entry: &Entry<'_>) {
        let contract = entry.contract;
        let types = &entry.type_name;
        out.push('\n');
        write_doc(
            out,
            "",
            &[
                format!("The answer guide of `{}`, from its law: every input path pattern at which a need can arise, with the type of the value there and the law's metadata.", contract.entry),
                "Data for documentation and forms; a pattern no need can name (a record, a list of records' element) is not in it.".to_string(),
            ],
        );
        let needs: Vec<(String, GuideNeed)> = answer_guide(contract)
            .into_iter()
            .map(|need| (camel_case_path(&need.pattern), need))
            .filter(|(pattern, _)| entry.open_paths || entry.patterns.contains_key(pattern))
            .collect();
        if needs.is_empty() {
            let _ = writeln!(
                out,
                "export const {}Guide: AnswerGuide<{types}Need[\"pattern\"]> = {{}};",
                entry.value_name
            );
            return;
        }
        let _ = writeln!(
            out,
            "export const {}Guide: AnswerGuide<{types}Need[\"pattern\"]> = {{",
            entry.value_name
        );
        for (pattern, need) in needs {
            let _ = writeln!(out, "  {}: {{", string_literal(&pattern));
            let kind = match need.kind {
                CalculationNeedKind::Fact => "fact",
                CalculationNeedKind::Entry => "entry",
            };
            let _ = writeln!(out, "    kind: \"{kind}\",");
            let _ = writeln!(out, "    type: {},", string_literal(&need.ty));
            for (member, value) in [
                ("label", &need.label),
                ("question", &need.question),
                ("help", &need.help),
                ("unit", &need.unit),
                ("entry", &need.entry),
            ] {
                if let Some(value) = value {
                    let _ = writeln!(out, "    {member}: {},", string_literal(value));
                }
            }
            if let Some(subject) = &need.subject {
                let _ = writeln!(
                    out,
                    "    subject: {},",
                    string_literal(&camel_case_path(subject))
                );
            }
            let keys = need
                .keys
                .iter()
                .map(|key| string_literal(&camel_case_path(key)))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "    keys: [{keys}],");
            let sources = need
                .sources
                .iter()
                .map(|source| serde_json::to_string(source).expect("a source reference serializes"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "    sources: [{sources}],");
            out.push_str("  },\n");
        }
        out.push_str("};\n");
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
            let answer_title = match &contract.label {
                Some(label) => format!("`{}` ({})", contract.entry, escape_doc(label)),
                None => format!("`{}`", contract.entry),
            };
            if !first {
                body.push('\n');
            }
            first = false;
            let _ = writeln!(body, "// `{}`\n", contract.entry);
            let _ = writeln!(
                body,
                "/** The `{}` entry: its name, contract hash, the wire codecs of its input and result, and its input path patterns. */",
                contract.entry
            );
            let _ = writeln!(
                body,
                "export const {values}Entry: CalculationEntry = {{\n  name: {},\n  schemaHash: schema.SCHEMA_HASH{},\n  input: {},\n  output: {},\n  patterns: partial.{types}InputPatternList,\n}};\n",
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
                    "Validates the case, encodes it for the engine, runs it through `transport.call`, checks the envelope's schema hash, and decodes and validates the result; returns it with the assumptions it rests on.".to_string(),
                ],
            );
            let _ = writeln!(
                body,
                "export function {values}(transport: EngineTransport, input: {input}): Promise<Calculated<{output}>> {{\n  return callEntry(transport, {values}Entry, {input_schema}, {output_schema}, input);\n}}\n"
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
                "export function {values}Explained(\n  transport: ExplainingTransport,\n  input: {input},\n  depth: ExplainDepth = \"direct\",\n): Promise<Explained<{output}>> {{\n  return explainEntry(transport, {values}Entry, depth, {input_schema}, {output_schema}, input);\n}}\n"
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
                "export function {values}QueryExplained(\n  transport: ExplainingTransport,\n  input: {partial_input},\n  depth: ExplainDepth = \"direct\",\n): Promise<ExplainedQuery<partial.{types}QueryResult>> {{\n  return explainQueryEntry(transport, {values}Entry, depth, {partial_schema}, partial.{types}QueryResultSchema, input);\n}}\n"
            );
            write_doc(
                &mut body,
                "",
                &[format!(
                    "How the needs of `{}` are answered: called once per round with every need of the round, it returns the reply to each need, typed by the need's pattern (`given(need, value)`, `open`, or another entry's outcome through `context.runEntry`).",
                    contract.entry
                )],
            );
            let _ = writeln!(
                body,
                "export type {types}Resolve = Resolve<{partial_input}, partial.{types}InputPatterns, partial.{types}Need>;\n"
            );
            write_doc(
                &mut body,
                "",
                &[
                    format!("Answer {answer_title}, round by round."),
                    String::new(),
                    "Queries the partial case through `transport.callPartial`, calls `resolve` once per round with every need, writes the values it gives and queries again. Ends `Answered` with the result, or `Unanswered` with the last round's needs left open. A reply for another path, or a path still needed after it was given, throws `AnswerError`. From inside another entry's `resolve`, pass on the `options` that `context.runEntry` gives.".to_string(),
                ],
            );
            let _ = writeln!(
                body,
                "export function {values}Answer(\n  transport: EngineTransport,\n  partialCase: {partial_input},\n  resolve: {types}Resolve,\n  options: AnswerOptions = {{}},\n): Promise<Answering<{output}, partial.{types}Need, {partial_input}>> {{\n  return answerEntry<{partial_input}, {output}, partial.{types}Need, partial.{types}InputPatterns>(\n    (current) => {values}Query(transport, current),\n    partialCase,\n    resolve,\n    options,\n  );\n}}\n"
            );
        }
        let mut out = String::new();
        self.header(
            &mut out,
            "Typed calls of each entry through an engine transport.",
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
        let _ = writeln!(out, "export * from {};", string_literal(RUNTIME_PACKAGE));
        out
    }
}

/// The decimal places of `1 / per` when `per` is built from 2s and 5s only,
/// so every value of the unit is a finite decimal of its base; otherwise
/// `None`.
fn decimal_places(per: i64) -> Option<u32> {
    if per < 1 {
        return None;
    }
    let (mut rest, mut twos, mut fives) = (per, 0, 0);
    while rest % 2 == 0 {
        rest /= 2;
        twos += 1;
    }
    while rest % 5 == 0 {
        rest /= 5;
        fives += 1;
    }
    (rest == 1).then_some(twos.max(fives))
}

/// ` DKK` for a scale of a base, nothing for a plain number (`"1"`).
fn scale_base(of: &str) -> String {
    if of == "1" {
        String::new()
    } else {
        format!(" {of}")
    }
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

/// `line` with the contents of its double-quoted string literals blanked out.
fn without_strings(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let (mut quoted, mut escaped) = (false, false);
    for c in line.chars() {
        if quoted && (escaped || c != '"') {
            escaped = !escaped && c == '\\';
            out.push(' ');
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        out.push(c);
    }
    out
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

/// The runtime's TypeScript source: the same for every model.
const RUNTIME: &str = include_str!("sdk_ts_runtime.ts");

/// The runtime compiled to an ES module, and its declarations
/// (`scripts/sdk-runtime.mjs` writes both from [`RUNTIME`];
/// `tests/sdk_ts.test.mjs` checks they are its compilation).
const RUNTIME_JS: &str = include_str!("sdk_ts_runtime.js");
const RUNTIME_DTS: &str = include_str!("sdk_ts_runtime.d.ts");

/// The TypeScript type of a named scalar's value on the wire.
fn scalar_wire_type(scalar: &CalculationScalar) -> &'static str {
    if scalar.of == "Int" {
        "number"
    } else {
        "string"
    }
}

/// The TSDoc of a named scalar type: its description and its checks.
fn scalar_doc(scalar: &CalculationScalar) -> Vec<String> {
    let mut doc = vec![
        sentence(&scalar.description),
        String::new(),
        format!(
            "A Futuruna `{}`: a `{}` the engine checks at the boundary. Only `{}(value)` and the validators make one; a bare {} or a value of another named type is not one.",
            scalar.name,
            scalar.of,
            value_identifier(&scalar.name),
            scalar_wire_type(scalar)
        ),
    ];
    if !scalar.checks.is_empty() {
        doc.push(String::new());
        doc.push("Checks:".to_string());
        doc.extend(scalar.checks.iter().map(|check| format!("- `{check}`")));
    }
    doc
}

/// Every name the runtime exports, and whether it is a type only, read from
/// its `export` declarations.
fn runtime_exports() -> impl Iterator<Item = (&'static str, bool)> {
    RUNTIME.lines().filter_map(|line| {
        let rest = line.strip_prefix("export ")?;
        let rest = rest.strip_prefix("async ").unwrap_or(rest);
        let (keyword, rest) = rest.split_once(' ')?;
        let is_type = match keyword {
            "type" | "interface" => true,
            "const" | "function" | "class" => false,
            _ => return None,
        };
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
            .unwrap_or(rest.len());
        Some((&rest[..end], is_type))
    })
}

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
        generate_typescript_sdk(&contracts(source), "model.entry.runa")
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
@ meta(household_meta)
@ entry("Household tax")
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
@ meta(ledger_meta)
@ entry
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
@ meta(household_meta)
@ entry
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
        assert_contains(schema, "} from \"@futuruna/runtime\";");
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
        assert_contains(partial, "  deduction?: number | None | undefined;");
        assert_contains(partial, "  children?: PartialChild[] | undefined;");
        assert_contains(
            partial,
            "      deduction: z.union([IntSchema, NoneSchema]).optional(),",
        );
        assert_contains(
            schema,
            "  deduction: [\"deduction\", optionalCodec(valueCodec)],",
        );
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
        assert_contains(types, "import type { IsoDate } from \"@futuruna/runtime\";");
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
            "export const MovementCodec: Codec = recordCodec({\n  bookedOn: [\"booked_on\", isoDateCodec],\n  direction: [\"direction\", enumCodec],\n  amountOre: [\"amount_ore\", valueCodec],\n  settledOn: [\"settled_on\", optionalCodec(isoDateCodec)],\n});",
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
            "export const ledgerBalanceEntry: CalculationEntry = {\n  name: \"ledger_balance\",\n  schemaHash: schema.SCHEMA_HASH.ledger_balance,\n  input: () => schema.LedgerCodec,\n  output: () => schema.SummaryCodec,\n  patterns: partial.LedgerBalanceInputPatternList,\n};",
        );
    }

    #[test]
    fn two_fields_with_one_camel_case_name_are_an_error() {
        let error = generate_typescript_sdk(
            &contracts("# Input(tax_year: Int, taxYear: Int)\n@ entry\n> both(input: Input) -> Int { input.tax_year }\n"),
            "model.entry.runa",
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
@ entry
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
            "export const AreaQueryResultSchema = queryResultSchema(schema.PairSchema(IntSchema, z.string()), AreaNeedSchema, AreaInputPathSchema);",
        );
        assert_contains(
            &files["client.ts"],
            "  output: () => schema.PairCodec(valueCodec, valueCodec),",
        );
    }

    #[test]
    fn paths_use_list_indices_and_map_keys() {
        let partial = &generate(&records_source())["partial.ts"];
        assert_contains(partial, "  \"children[].age\": number;");
        assert_contains(partial, "  \"grants.*\": number;");
        assert_contains(partial, "  \"deduction\": number | None;");
        assert_contains(
            partial,
            "export type HouseholdTaxInputPattern = Extract<keyof HouseholdTaxInputPatterns, string>;",
        );
        assert_contains(
            partial,
            "export type HouseholdTaxInputPath = InputPath<HouseholdTaxInputPattern>;\nexport const HouseholdTaxInputPathSchema = inputPathSchema<HouseholdTaxInputPattern>(HouseholdTaxInputPatternList);",
        );
        assert_contains(
            partial,
            "export type HouseholdTaxNeed = Need<HouseholdTaxInputPattern, never>;\nexport const HouseholdTaxNeedSchema = needSchema<HouseholdTaxInputPattern, never>(HouseholdTaxInputPatternList, []);",
        );
        assert_contains(
            partial,
            "export function householdTaxSetAt<N extends HouseholdTaxNeed>(\n  partialCase: PartialHousehold,\n  need: N,\n  value: AnswerTo<HouseholdTaxInputPatterns, N>,\n): PartialHousehold {\n  return setAt(partialCase, need.path, value);\n}",
        );
        assert!(
            !partial.contains("${number}"),
            "a path is an InputPath, not a template"
        );
        assert_contains(partial, "  type AnswerTo,\n");
    }

    #[test]
    fn several_entries_each_get_calls_hashes_and_paths() {
        let source = r#"# Person(name: String, income: Int)
# Couple(first: Person, second: Person)
@ entry("One person")
> person_tax(input: Person) -> Int { input.income / 4 }
@ entry
> couple_tax(input: Couple) -> Int { input.first.income + input.second.income }
"#;
        let contracts = contracts(source);
        let files = generate(source);
        let client = &files["client.ts"];
        assert_contains(
            client,
            "export function personTax(transport: EngineTransport, input: types.Person): Promise<Calculated<number>> {\n  return callEntry(transport, personTaxEntry, schema.PersonSchema, IntSchema, input);",
        );
        assert_contains(
            client,
            "export function coupleTaxQuery(\n  transport: EngineTransport,\n  input: partial.PartialCouple,\n): Promise<partial.CoupleTaxQueryResult> {",
        );
        assert_contains(client, "/**\n * Calculate `person_tax`: One person.");
        for expected in [
            "export function personTaxExplained(\n  transport: ExplainingTransport,\n  input: types.Person,\n  depth: ExplainDepth = \"direct\",\n): Promise<Explained<number>> {\n  return explainEntry(transport, personTaxEntry, depth, schema.PersonSchema, IntSchema, input);",
            "export function coupleTaxQueryExplained(\n  transport: ExplainingTransport,\n  input: partial.PartialCouple,\n  depth: ExplainDepth = \"direct\",\n): Promise<ExplainedQuery<partial.CoupleTaxQueryResult>> {",
            "export function personTax(transport: EngineTransport, input: types.Person)",
            "  patterns: partial.PersonTaxInputPatternList,\n};",
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
        assert_contains(&files["index.ts"], "export * from \"@futuruna/runtime\";");
        assert!(
            !files.contains_key("answer.ts"),
            "answering is in client.ts, no answer module"
        );
        // Entries are ordered by name, so declaration order does not matter.
        assert!(schema.find("  couple_tax:").unwrap() < schema.find("  person_tax:").unwrap());
        let mut reversed = contracts.clone();
        reversed.reverse();
        assert_eq!(
            generate_typescript_sdk(&contracts, "model.entry.runa").unwrap(),
            generate_typescript_sdk(&reversed, "model.entry.runa").unwrap()
        );
    }

    #[test]
    fn every_sdk_imports_the_runtime_package() {
        let ledger = generate(&ledger_source());
        assert!(
            !ledger.keys().any(|name| name.contains("runtime")),
            "the runtime is a package, not a module of the SDK"
        );
        for (name, contents) in &ledger {
            assert!(
                !contents.contains("export class"),
                "{name} holds runtime code"
            );
            assert!(
                !contents.contains("runtime.js"),
                "{name} imports a local runtime"
            );
            assert_contains(
                contents,
                &format!("// Imports @futuruna/runtime {RUNTIME_VERSION}, the runtime of the runa that generated it."),
            );
        }
        assert_contains(&ledger["index.ts"], "export * from \"@futuruna/runtime\";");

        let package: BTreeMap<String, String> = runtime_package()
            .into_iter()
            .map(|file| (file.name, file.contents))
            .collect();
        assert_eq!(
            package.keys().map(String::as_str).collect::<Vec<_>>(),
            ["index.d.ts", "index.js", "package.json"]
        );
        let manifest: serde_json::Value =
            serde_json::from_str(&package["package.json"]).expect("package.json is JSON");
        assert_eq!(manifest["name"], "@futuruna/runtime");
        assert_eq!(manifest["version"], RUNTIME_VERSION);
        assert_eq!(manifest["type"], "module");
        assert_eq!(manifest["exports"]["."]["types"], "./index.d.ts");
        assert_eq!(manifest["exports"]["."]["default"], "./index.js");
        assert!(manifest["peerDependencies"]["zod"].is_string());
        assert!(manifest.get("dependencies").is_none());
        assert_contains(&package["index.js"], "export async function answerEntry(");
        assert_contains(&package["index.js"], "export function unitFromDecimal(");
        assert_contains(&package["index.d.ts"], "export declare function given<");
        assert_contains(&package["index.d.ts"], "export type Resolve<");
        for text in package.values() {
            assert!(
                !text.contains("{INPUT_SCHEMA}"),
                "the schema ids are filled in"
            );
        }
        assert_contains(&package["index.js"], INPUT_SCHEMA);
        for gone in [
            "function resolve",
            "export type Resolvers<",
            "GroupResolvers",
            "guidePath",
            "guideGroup",
            "leaveOpen",
            "MissingFactError",
            "ResolveError",
            "export const ask",
        ] {
            assert!(!RUNTIME.contains(gone), "the runtime still has `{gone}`");
        }
    }

    const PERSON_MODEL: &str = r#"# Event = Trade(@ field(label = "Amount") amount: Int, fee: Int) | Dividend(gross: Int)
# Spouse(income: Int, cohabiting: Bool)
# Person(year: Int, income: Int, spouse: Spouse?, events: List(Event))
@ entry("Tax")
> tax(p: Person) -> Int { p.income }
"#;

    #[test]
    fn every_entry_is_answered_round_by_round_by_one_resolve() {
        let files = generate(PERSON_MODEL);
        let client = &files["client.ts"];
        for expected in [
            "export type TaxResolve = Resolve<partial.PartialPerson, partial.TaxInputPatterns, partial.TaxNeed>;",
            " * Answer `tax` (Tax), round by round.",
            "export function taxAnswer(\n  transport: EngineTransport,\n  partialCase: partial.PartialPerson,\n  resolve: TaxResolve,\n  options: AnswerOptions = {},\n): Promise<Answering<number, partial.TaxNeed, partial.PartialPerson>> {\n  return answerEntry<partial.PartialPerson, number, partial.TaxNeed, partial.TaxInputPatterns>(\n    (current) => taxQuery(transport, current),\n    partialCase,\n    resolve,\n    options,\n  );\n}",
            "type Resolve,",
            "type AnswerOptions",
            "type Answering",
            "answerEntry",
        ] {
            assert_contains(client, expected);
        }
        assert!(
            !files.contains_key("answer.ts"),
            "answering is in client.ts"
        );
        for gone in [
            "GroupResolvers",
            "GuidedNeed",
            "Facts",
            "Resolvers",
            "leaveOpen",
            "by:",
        ] {
            for (name, contents) in &files {
                assert!(!contents.contains(gone), "{name} still has `{gone}`");
            }
        }
    }

    #[test]
    fn every_entry_gets_its_answer_guide_from_the_law() {
        let files = generate(PERSON_MODEL);
        let partial = &files["partial.ts"];
        assert_contains(
            partial,
            "export const taxGuide: AnswerGuide<TaxNeed[\"pattern\"]> = {\n  \"year\": {\n    kind: \"fact\",\n    type: \"Int\",\n    keys: [],\n    sources: [],\n  },",
        );
        assert_contains(
            partial,
            "  \"spouse\": {\n    kind: \"fact\",\n    type: \"Spouse?\",\n    keys: [],\n    sources: [],\n  },",
        );
        assert_contains(
            partial,
            "  \"events[].amount\": {\n    kind: \"fact\",\n    type: \"Int\",\n    label: \"Amount\",\n    keys: [],\n    sources: [],\n  },",
        );
        assert_contains(
            partial,
            "  \"events[].$variant\": {\n    kind: \"fact\",\n    type: \"Event\",",
        );
        assert_contains(partial, "type AnswerGuide");
        let patterns: Vec<&str> = partial
            .lines()
            .skip_while(|line| !line.starts_with("export const taxGuide"))
            .filter_map(|line| line.strip_prefix("  \"")?.strip_suffix("\": {"))
            .collect();
        assert_eq!(
            patterns,
            [
                "year",
                "income",
                "spouse",
                "spouse.income",
                "spouse.cohabiting",
                "events",
                "events[].$variant",
                "events[].amount",
                "events[].fee",
                "events[].gross",
            ]
        );
    }

    #[test]
    fn a_unit_whose_scale_is_a_finite_decimal_gets_decimal_converters() {
        let files = generate(
            "# unit Øre \"DKK øre\" = DKK / 100\n# unit Micro \"millionths\" = 1 / 1000000\n# unit Third \"thirds\" = DKK / 3\n# unit Plain \"plain\"\n# Holding(value: Øre, quantity: Micro, share: Third, count: Plain)\n@ entry\n> worth(input: Holding) -> Øre { input.value }\n",
        );
        let schema = &files["schema.ts"];
        assert_contains(
            schema,
            "øre.fromDecimal = (amount: string): types.Øre =>\n  unitFromDecimal<types.Øre>(\"Øre\", { of: \"DKK\", per: 100 }, amount);",
        );
        assert_contains(
            schema,
            "øre.toDecimal = (value: types.Øre): string => unitToDecimal(\"Øre\", { of: \"DKK\", per: 100 }, value);",
        );
        assert_contains(
            schema,
            "with 2 decimals: how a figure in `Øre` reads in DKK.",
        );
        assert_contains(
            schema,
            "micro.fromDecimal = (amount: string): types.Micro =>\n  unitFromDecimal<types.Micro>(\"Micro\", { of: \"1\", per: 1000000 }, amount);",
        );
        assert!(
            !schema.contains("third.fromDecimal"),
            "1/3 is no finite decimal"
        );
        assert!(
            !schema.contains("plain.fromDecimal"),
            "a unit without a scale"
        );
        let types = &files["types.ts"];
        assert_contains(types, "One `Øre` is 1/100 DKK. `øre.fromDecimal` and `øre.toDecimal` convert a decimal amount exactly.");
        assert_contains(types, "One `Third` is 1/3 DKK, which is no finite decimal: the SDK has no decimal converters for it.");
        assert_eq!(decimal_places(100), Some(2));
        assert_eq!(decimal_places(1), Some(0));
        assert_eq!(decimal_places(8), Some(3));
        assert_eq!(decimal_places(3), None);
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
    fn map_values_and_list_elements_are_patterns() {
        let partial = &generate(
            "# Item(code: String, amount: Int?)\n# Input(items: Map(String, Item), tags: List(String))\n@ entry\n> count(input: Input) -> Int { 0 }\n",
        )["partial.ts"];
        assert_contains(partial, "  \"items.*\": PartialItem;");
        assert_contains(partial, "  \"items.*.amount\": number | None;");
        assert_contains(partial, "  \"tags[]\": string;");
        assert_contains(
            partial,
            "export const CountInputPatternList: readonly CountInputPattern[] = [\n  \"items\",\n  \"items.*\",\n  \"items.*.amount\",\n  \"items.*.code\",\n  \"tags\",\n  \"tags[]\",\n];",
        );
    }

    #[test]
    fn a_list_element_key_types_the_needs_keys() {
        let mut contracts = contracts(
            "# unit Ore \"DKK øre\"\n# Position(isin: String, value_ore: Ore)\n# Account(positions: List(Position), owner: String)\n@ entry\n> worth(input: Account) -> Int { 0 }\n",
        );
        for contract in &mut contracts {
            for definition in &mut contract.definitions {
                if definition.name == "Position" {
                    definition.key = Some("isin".to_string());
                }
            }
        }
        let files: BTreeMap<String, String> =
            generate_typescript_sdk(&contracts, "model.entry.runa")
                .expect("generate SDK")
                .into_iter()
                .map(|file| (file.name, file.contents))
                .collect();
        let partial = &files["partial.ts"];
        assert_contains(
            partial,
            "export type WorthElementKey =\n  | ElementKey<\"positions[]\", \"isin\", string>;",
        );
        assert_contains(
            partial,
            "export type WorthNeed = Need<WorthInputPattern, WorthElementKey>;",
        );
        assert_contains(
            partial,
            "export const WorthNeedSchema = needSchema<WorthInputPattern, WorthElementKey>(WorthInputPatternList, [\n  { element: \"positions[]\", field: \"isin\", value: z.string() },\n]);",
        );
    }

    #[test]
    fn recursive_input_types_have_open_paths() {
        let files = generate(
            r#"# Node = Leaf(value: Int) | Branch(left: Node, right: Node)
@ entry
> total(input: Node) -> Int { 0 }
"#,
        );
        let partial = &files["partial.ts"];
        assert_contains(partial, "  [pattern: string]: unknown;");
        assert_contains(
            partial,
            "export const TotalInputPathSchema = inputPathSchema<TotalInputPattern>(undefined);",
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
            ("# Need(amount: Int)\n@ entry\n> needed(input: Need) -> Int { input.amount }\n", "`Need`"),
            ("# Unknown(amount: Int)\n@ entry\n> needed(input: Unknown) -> Int { input.amount }\n", "`Unknown`"),
        ] {
            let error = generate_typescript_sdk(&contracts(source), "model.entry.runa")
                .expect_err("the name collides with a runtime export");
            assert!(error.contains(name), "{error}");
        }
    }

    #[test]
    fn units_of_measure_are_branded_number_types() {
        let source = r#"# unit Ore "DKK øre"
# unit Micro "millionths of a share"
# Holding(quantity_micro: Micro, value_ore: Ore, count: Int, history: List(Ore)?)
@ entry
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
        assert_contains(
            &files["partial.ts"],
            "  history?: types.Ore[] | None | undefined;",
        );
        assert_contains(types, "Futuruna type: `List(Ore)?`");
        let schema = &files["schema.ts"];
        assert_contains(
            schema,
            "export const OreSchema: z.ZodType<types.Ore> = unitSchema<types.Ore>(\"Ore\");",
        );
        assert_contains(
            schema,
            "export function ore(value: number): types.Ore {\n  return unitValue<types.Ore>(\"Ore\", value);\n}",
        );
        assert_contains(schema, "      valueOre: OreSchema,");
        assert_contains(schema, "      count: IntSchema,");
        let partial = &files["partial.ts"];
        assert_contains(partial, "  \"valueOre\": types.Ore;");
        assert_contains(partial, "  \"history[]\": types.Ore;");
        assert_contains(partial, "      valueOre: schema.OreSchema.optional(),");
        assert_contains(
            &files["client.ts"],
            "export function holdingValue(transport: EngineTransport, input: types.Holding): Promise<Calculated<types.Ore>> {\n  return callEntry(transport, holdingValueEntry, schema.HoldingSchema, schema.OreSchema, input);",
        );
        let error = generate_typescript_sdk(
            &contracts(
                "# unit Given \"replies\"\n# In(n: Given)\n@ entry\n> f(input: In) -> Int { 0 }\n",
            ),
            "model.entry.runa",
        )
        .expect_err("`given` is a runtime export");
        assert!(error.contains("`given`"), "{error}");
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
