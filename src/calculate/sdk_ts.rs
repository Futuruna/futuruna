//! TypeScript SDK generation for calculation contracts (`runa sdk ts`).
//!
//! Everything is derived from the contracts `runa schema` emits. The types and
//! Zod validators follow the canonical JSON encoding that `decode_value` reads
//! and `encode_value` writes; the partial-case types and the `Complete`/`Needs`
//! result follow `docs/rfcs/calculation-needs.md`; the client reaches an engine
//! only through a transport. The output depends only on the contracts and the
//! source label, so regenerating an unchanged model rewrites identical files.

use super::{
    calculation_type_occurrence_paths, definition_substitutions, joined_column_path,
    product_variant, substitute_contract_type, CalculationContract, CalculationField,
    CalculationFieldMetadata, CalculationTypeDefinition, CalculationTypeRef, CalculationVariant,
    CONTRACT_SCHEMA, INPUT_SCHEMA, OUTPUT_SCHEMA,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// The output envelope schema of `runa call --partial`.
pub const PARTIAL_OUTPUT_SCHEMA: &str = "futuruna.calculate.partial-output.v1";

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

/// One generated TypeScript module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    pub name: &'static str,
    pub contents: String,
}

/// Generate the TypeScript SDK for every calculation entry of one source file.
///
/// `source` names the file in the generated headers. A file whose contracts
/// cannot be expressed (an unsupported type, two entries disagreeing on a
/// type, or a contract name that collides with a generated name) is an error.
pub fn generate_typescript_sdk(
    contracts: &[CalculationContract],
    source: &str,
) -> Result<Vec<GeneratedFile>, String> {
    let sdk = Sdk::new(contracts, source)?;
    Ok(vec![
        GeneratedFile {
            name: "types.ts",
            contents: sdk.types_module(),
        },
        GeneratedFile {
            name: "schema.ts",
            contents: sdk.schema_module(),
        },
        GeneratedFile {
            name: "partial.ts",
            contents: sdk.partial_module(),
        },
        GeneratedFile {
            name: "client.ts",
            contents: sdk.client_module(),
        },
        GeneratedFile {
            name: "index.ts",
            contents: sdk.index_module(),
        },
    ])
}

/// Where a type is written: inside `types.ts`, from another module (complete
/// types under the `types` namespace), or as its partial-case form.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Local,
    Qualified,
    Partial,
}

struct Entry<'a> {
    contract: &'a CalculationContract,
    /// `tax_report` → `taxReport`, the per-entry function prefix.
    value_name: String,
    /// `tax_report` → `TaxReport`, the per-entry type prefix.
    type_name: String,
    /// Input path pattern → TypeScript types of the values written there.
    patterns: BTreeMap<String, BTreeSet<String>>,
    /// A recursive input type has paths of any depth.
    open_paths: bool,
}

struct Sdk<'a> {
    source: &'a str,
    entries: Vec<Entry<'a>>,
    definitions: BTreeMap<&'a str, &'a CalculationTypeDefinition>,
    input_definitions: BTreeSet<&'a str>,
    /// (type, variant, field) → the field metadata of every input path the
    /// field occurs at, including the metadata of its `$variant` tag.
    field_metadata: BTreeMap<(&'a str, &'a str, &'a str), Vec<&'a CalculationFieldMetadata>>,
}

impl<'a> Sdk<'a> {
    fn new(contracts: &'a [CalculationContract], source: &'a str) -> Result<Self, String> {
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

        let mut sdk = Sdk {
            source,
            entries: Vec::new(),
            definitions,
            input_definitions: BTreeSet::new(),
            field_metadata: BTreeMap::new(),
        };
        for definition in sdk.definitions.values() {
            for variant in &definition.variants {
                for field in &variant.fields {
                    sdk.validate_type(
                        &field.ty,
                        &definition.parameters,
                        &format!("field `{}` of `{}`", field.name, definition.name),
                    )?;
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
            CalculationTypeRef::Primitive { name } => {
                if PRIMITIVES.contains(&name.as_str()) {
                    Ok(())
                } else {
                    Err(format!("{context}: unsupported primitive type `{name}`"))
                }
            }
            CalculationTypeRef::Unit => Ok(()),
            CalculationTypeRef::Optional { item }
            | CalculationTypeRef::List { item }
            | CalculationTypeRef::Set { item } => self.validate_type(item, parameters, context),
            CalculationTypeRef::Map { key, value } => {
                if !matches!(key.as_ref(), CalculationTypeRef::Primitive { name } if name == "String")
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
                        for candidate in [field_path.clone(), format!("{field_path}.$variant")] {
                            let Some(metadata) = contract.field_metadata_for_path(&candidate)
                            else {
                                continue;
                            };
                            let list = self
                                .field_metadata
                                .entry((
                                    definition.name.as_str(),
                                    variant.name.as_str(),
                                    field.name.as_str(),
                                ))
                                .or_default();
                            if !list.contains(&metadata) {
                                list.push(metadata);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Input path patterns as the needs RFC writes concrete paths: record
    /// fields and a sum type's fields join with `.`, list and set elements are
    /// `[]`, map values are `*`, and a sum type's tag is `.$variant`.
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
                self.record_path(entry, &element, item);
                self.collect_paths(item, substitutions, &element, active, entry);
            }
            CalculationTypeRef::Map { value, .. } => {
                let element = joined_column_path(path, "*");
                self.record_path(entry, &element, value);
                self.collect_paths(value, substitutions, &element, active, entry);
            }
            CalculationTypeRef::Named { name, arguments } => {
                let definition = self.definitions[name.as_str()];
                let key = ty.display_name();
                if active.contains(&key) {
                    entry.open_paths = true;
                    return;
                }
                active.push(key);
                let local = definition_substitutions(definition, arguments);
                if let Some(variant) = product_variant(definition) {
                    for field in &variant.fields {
                        let field_ty = substitute_contract_type(&field.ty, &local);
                        let field_path = joined_column_path(path, &field.name);
                        self.record_path(entry, &field_path, &field_ty);
                        self.collect_paths(&field_ty, &local, &field_path, active, entry);
                    }
                } else if !definition.variants.is_empty() {
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
                                self.record_path(entry, &field_path, &field_ty);
                                self.collect_paths(&field_ty, &local, &field_path, active, entry);
                            }
                        } else {
                            for field in &variant.fields {
                                let field_ty = substitute_contract_type(&field.ty, &local);
                                let field_path = joined_column_path(path, &field.name);
                                self.record_path(entry, &field_path, &field_ty);
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

    fn record_path(&self, entry: &mut Entry<'a>, pattern: &str, ty: &CalculationTypeRef) {
        entry
            .patterns
            .entry(pattern.to_string())
            .or_default()
            .insert(self.ts_type(ty, Side::Partial));
    }

    fn check_exported_names(&self) -> Result<(), String> {
        let mut names: Vec<(String, &str)> = Vec::new();
        for name in SCHEMA_RUNTIME_EXPORTS {
            names.push((name.to_string(), "schema.ts"));
        }
        for name in PARTIAL_RUNTIME_EXPORTS {
            names.push((name.to_string(), "partial.ts"));
        }
        for name in CLIENT_RUNTIME_EXPORTS {
            names.push((name.to_string(), "client.ts"));
        }
        for name in self.definitions.keys() {
            names.push((name.to_string(), "types.ts"));
            names.push((format!("{name}Schema"), "schema.ts"));
        }
        for name in &self.input_definitions {
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

    fn ts_type(&self, ty: &CalculationTypeRef, side: Side) -> String {
        match ty {
            CalculationTypeRef::Primitive { name } => match name.as_str() {
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
                let item = self.ts_type(item, side);
                if item.contains(" | ") {
                    format!("({item})[]")
                } else {
                    format!("{item}[]")
                }
            }
            CalculationTypeRef::Map { value, .. } => {
                format!("{{ [key: string]: {} }}", self.ts_type(value, side))
            }
            CalculationTypeRef::TypeParameter { name } => name.clone(),
            CalculationTypeRef::Named { name, arguments } => {
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
                match side {
                    Side::Local => format!("{name}{arguments}"),
                    Side::Qualified => format!("types.{name}{arguments}"),
                    Side::Partial => format!("Partial{name}{arguments}"),
                }
            }
        }
    }

    /// The Zod expression for `ty`; `Local` means inside `schema.ts`.
    fn zod(&self, ty: &CalculationTypeRef, side: Side) -> String {
        let module = if side == Side::Local { "" } else { "schema." };
        match ty {
            CalculationTypeRef::Primitive { name } => match name.as_str() {
                "Int" => format!("{module}IntSchema"),
                "Float" => format!("{module}FloatSchema"),
                "Char" => format!("{module}CharSchema"),
                "Bool" => "z.boolean()".to_string(),
                _ => "z.string()".to_string(),
            },
            CalculationTypeRef::Unit => "z.null()".to_string(),
            CalculationTypeRef::Optional { item } => format!("{}.nullable()", self.zod(item, side)),
            CalculationTypeRef::List { item } => format!("z.array({})", self.zod(item, side)),
            CalculationTypeRef::Set { item } => {
                format!("{module}SetSchema({})", self.zod(item, side))
            }
            CalculationTypeRef::Map { value, .. } => {
                format!("z.record(z.string(), {})", self.zod(value, side))
            }
            CalculationTypeRef::TypeParameter { name } => format!("schema_{name}"),
            CalculationTypeRef::Named { name, arguments } => {
                let schema = match side {
                    Side::Local => format!("{name}Schema"),
                    Side::Qualified => format!("schema.{name}Schema"),
                    Side::Partial => format!("Partial{name}Schema"),
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

    fn types_module(&self) -> String {
        let mut out = String::new();
        self.header(
            &mut out,
            "Types of every calculation input and result, in their canonical JSON encoding.",
        );
        out.push_str(
            "// Int is `number` and holds safe integers only; the validators in schema.ts\n// reject anything else. An optional value is `T | null`.\n\n",
        );
        for definition in self.definitions.values() {
            self.write_type(&mut out, definition);
            out.push('\n');
        }
        out
    }

    fn write_type(&self, out: &mut String, definition: &CalculationTypeDefinition) {
        let parameters = type_parameters(definition);
        if let Some(variant) = product_variant(definition) {
            write_doc(
                out,
                "",
                &[format!(
                    "Futuruna record `{}`.",
                    display_definition(definition)
                )],
            );
            let _ = writeln!(out, "export type {}{parameters} = {{", definition.name);
            for field in &variant.fields {
                self.write_field(out, "  ", definition, variant, field);
            }
            out.push_str("};\n");
            return;
        }
        if definition.variants.is_empty() {
            write_doc(
                out,
                "",
                &[format!(
                    "Futuruna type `{}`, which has no values.",
                    display_definition(definition)
                )],
            );
            let _ = writeln!(out, "export type {}{parameters} = never;", definition.name);
            return;
        }
        write_doc(
            out,
            "",
            &[format!(
                "Futuruna sum type `{}`, tagged by `$variant`: {}.",
                display_definition(definition),
                variant_list(definition)
            )],
        );
        let _ = writeln!(out, "export type {}{parameters} =", definition.name);
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
            property_name(&field.name),
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
        let mut groups: Vec<(Vec<String>, Vec<&str>)> = Vec::new();
        if let Some(list) = self.field_metadata.get(&(
            definition.name.as_str(),
            variant.name.as_str(),
            field.name.as_str(),
        )) {
            for metadata in list {
                let block = metadata_block(metadata);
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
                        .map(|path| format!("`{path}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                lines.extend(block);
                lines.push(String::new());
            }
        }
        let mut type_line = format!("Futuruna type: `{}`", field.ty.display_name());
        if mentions_int(&field.ty) {
            type_line.push_str("; Int values are safe integers");
        }
        type_line.push('.');
        lines.push(type_line);
        lines
    }

    fn schema_module(&self) -> String {
        let mut out = String::new();
        self.header(
            &mut out,
            "Zod validators for every calculation input and result type.",
        );
        out.push_str("import { z } from \"zod\";\nimport type * as types from \"./types.js\";\n\n");
        out.push_str(SCHEMA_RUNTIME);
        out.push_str("\n/** The contract fingerprint of each calculation entry. */\nexport const SCHEMA_HASH = {\n");
        for entry in &self.entries {
            let _ = writeln!(
                out,
                "  {}: {},",
                property_name(&entry.contract.entry),
                string_literal(&entry.contract.schema_hash)
            );
        }
        out.push_str("} as const;\n");
        for definition in self.definitions.values() {
            out.push('\n');
            self.write_schema(&mut out, definition, Side::Local);
        }
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
                    property_name(&field.name),
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
                            property_name(&field.name),
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
            format!("  z.union([\n    z.object({{}}).strict(),\n    {union},\n  ])")
        } else {
            format!("  {union}")
        }
    }

    fn partial_module(&self) -> String {
        let mut out = String::new();
        self.header(
            &mut out,
            "Partial cases, the Complete/Needs result of a partial call, typed needs and setAt.",
        );
        out.push_str(
            "import { z } from \"zod\";\nimport type * as types from \"./types.js\";\nimport * as schema from \"./schema.js\";\n\n",
        );
        out.push_str(PARTIAL_RUNTIME);
        for name in &self.input_definitions {
            let definition = self.definitions[name];
            out.push('\n');
            self.write_partial_type(&mut out, definition);
            out.push('\n');
            self.write_schema(&mut out, definition, Side::Partial);
        }
        for entry in &self.entries {
            out.push('\n');
            self.write_entry_paths(&mut out, entry);
        }
        out
    }

    fn write_partial_type(&self, out: &mut String, definition: &CalculationTypeDefinition) {
        let parameters = type_parameters(definition);
        let name = &definition.name;
        if let Some(variant) = product_variant(definition) {
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
                    property_name(&field.name),
                    self.ts_type(&field.ty, Side::Partial)
                );
            }
            out.push_str("};\n");
            return;
        }
        if definition.variants.is_empty() {
            let _ = writeln!(out, "export type Partial{name}{parameters} = never;");
            return;
        }
        write_doc(
            out,
            "",
            &[format!(
                "{{@link types.{name}}} in a partial case: without `$variant` (left out, or `{{}}`) the variant is unknown; once it is given, a field left out is unknown."
            )],
        );
        let _ = writeln!(out, "export type Partial{name}{parameters} =");
        let mut members = vec!["  | { [key: string]: never }".to_string()];
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
                        property_name(&field.name),
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
        let mut out = String::new();
        self.header(
            &mut out,
            "Typed calls of each calculation entry through an engine transport, and the resolver loop.",
        );
        out.push_str("import { z } from \"zod\";\nimport type * as types from \"./types.js\";\nimport * as schema from \"./schema.js\";\nimport * as partial from \"./partial.js\";\nimport type { Need, QueryResult } from \"./partial.js\";\n\n");
        out.push_str(
            &CLIENT_RUNTIME
                .replace("{INPUT_SCHEMA}", INPUT_SCHEMA)
                .replace("{OUTPUT_SCHEMA}", OUTPUT_SCHEMA)
                .replace("{PARTIAL_OUTPUT_SCHEMA}", PARTIAL_OUTPUT_SCHEMA),
        );
        for entry in &self.entries {
            let contract = entry.contract;
            let name = string_literal(&contract.entry);
            let types = &entry.type_name;
            let values = &entry.value_name;
            let input = self.ts_type(&contract.input, Side::Qualified);
            let input_schema = self.zod(&contract.input, Side::Qualified);
            let partial_input = self.ts_type(&contract.input, Side::Partial);
            let partial_input = qualify_partial(&partial_input);
            let partial_schema = qualify_partial(&self.zod(&contract.input, Side::Partial));
            let output = self.ts_type(&contract.output, Side::Qualified);
            let output_schema = self.zod(&contract.output, Side::Qualified);
            let title = match &contract.label {
                Some(label) => format!("`{}`: {}.", contract.entry, escape_doc(label)),
                None => format!("`{}`.", contract.entry),
            };
            out.push('\n');
            let _ = writeln!(out, "// `{}`\n", contract.entry);
            write_doc(
                &mut out,
                "",
                &[
                    format!("Calculate {title}"),
                    String::new(),
                    "Validates the case, runs it through `transport.call`, checks the envelope's schema hash and validates the result.".to_string(),
                ],
            );
            let _ = writeln!(
                out,
                "export function {values}(transport: EngineTransport, input: {input}): Promise<{output}> {{\n  return invoke(transport, {name}, false, {input_schema}, {output_schema}, input);\n}}\n"
            );
            write_doc(
                &mut out,
                "",
                &[
                    format!("Query {title}"),
                    String::new(),
                    "Runs a partial case through `transport.callPartial` and returns `Complete` with the result, or `Needs` with every fact and entry that blocks it.".to_string(),
                ],
            );
            let _ = writeln!(
                out,
                "export function {values}Query(\n  transport: EngineTransport,\n  input: {partial_input},\n): Promise<partial.{types}QueryResult> {{\n  return invoke(transport, {name}, true, {partial_schema}, partial.{types}QueryResultSchema, input);\n}}\n"
            );
            write_doc(
                &mut out,
                "",
                &[
                    format!("Calculate and explain {title}"),
                    String::new(),
                    "As the plain call, through `transport.callExplained`: also returns where every leaf of the result came from (`runa call --explain`).".to_string(),
                ],
            );
            let _ = writeln!(
                out,
                "export function {values}Explained(\n  transport: EngineTransport,\n  input: {input},\n  depth: ExplainDepth = \"direct\",\n): Promise<Explained<{output}>> {{\n  return invokeExplained(transport, {name}, false, depth, {input_schema}, {output_schema}, input);\n}}\n"
            );
            write_doc(
                &mut out,
                "",
                &[
                    format!("Query and explain {title}"),
                    String::new(),
                    "As the query, through `transport.callPartialExplained`: a `Complete` result comes with its explanation.".to_string(),
                ],
            );
            let _ = writeln!(
                out,
                "export function {values}QueryExplained(\n  transport: EngineTransport,\n  input: {partial_input},\n  depth: ExplainDepth = \"direct\",\n): Promise<ExplainedQuery<partial.{types}QueryResult>> {{\n  return invokeExplained(transport, {name}, true, depth, {partial_schema}, partial.{types}QueryResultSchema, input);\n}}\n"
            );
            let _ = writeln!(
                out,
                "/** Resolvers for the needs of `{}`, keyed by path pattern. */",
                contract.entry
            );
            let _ = writeln!(
                out,
                "export type {types}Resolvers = {{\n  readonly [K in keyof partial.{types}InputPatterns]?: Resolver<\n    {partial_input},\n    partial.{types}InputPatterns[K],\n    partial.{types}InputPath\n  >;\n}};"
            );
        }
        out
    }

    fn index_module(&self) -> String {
        let mut out = String::new();
        self.header(&mut out, "The SDK's public surface.");
        out.push_str("export * from \"./types.js\";\nexport * from \"./schema.js\";\nexport * from \"./partial.js\";\nexport * from \"./client.js\";\n");
        out
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

fn metadata_block(metadata: &CalculationFieldMetadata) -> Vec<String> {
    let mut lines = Vec::new();
    let label = if metadata.path.ends_with(".$variant") {
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
        CalculationTypeRef::Primitive { name } => name == "Int",
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

fn property_name(name: &str) -> String {
    let mut chars = name.chars();
    let identifier = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|next| next.is_ascii_alphanumeric() || next == '_' || next == '$');
    if identifier {
        name.to_string()
    } else {
        string_literal(name)
    }
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

const SCHEMA_RUNTIME_EXPORTS: [&str; 5] = [
    "IntSchema",
    "FloatSchema",
    "CharSchema",
    "SetSchema",
    "SCHEMA_HASH",
];

const SCHEMA_RUNTIME: &str = r#"/** `Int`: a safe integer. Values beyond ±(2^53 − 1) are rejected, never rounded. */
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
  return JSON.stringify(value);
}
"#;

const PARTIAL_RUNTIME_EXPORTS: [&str; 17] = [
    "PathSegment",
    "PathError",
    "parsePath",
    "setAt",
    "pathPattern",
    "matchesPathPattern",
    "inputPathSchema",
    "SourceReference",
    "SourceReferenceSchema",
    "FactNeed",
    "EntryNeed",
    "Need",
    "needSchema",
    "Complete",
    "Needs",
    "QueryResult",
    "queryResultSchema",
];

const PARTIAL_RUNTIME: &str = r#"/** One step of a concrete input path: a field (record field, map key, `$variant`, `$values`) or a list index. */
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

/** Split a concrete input path such as `accounts[2].closing_cash_ore` into its steps. */
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
 * Write `value` at a concrete input path, such as `accounts[2].closing_cash_ore`
 * or `taxpayer.outside_endavu.$variant`, and return the new case. The case
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

/** A source reference from the contract's field metadata, unchanged. */
export type SourceReference = {
  label: string;
  role: string;
  binding: string;
  attachment_path?: string | undefined;
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
    attachment_path: z.string().optional(),
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
   * The innermost rule family of a law section evaluating when the path was
   * first read; without one, the innermost rule family (or the entry).
   */
  waiting_rule: string;
  /** The law section (anchor label) of `waiting_rule`. */
  section?: string | undefined;
  /** The paragraphs `waiting_rule` cites, as `§ 16`. */
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
    waiting_rule: z.string(),
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
  unused_paths: P[];
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
    z.object({ $variant: z.literal("Complete"), result, unused_paths: z.array(path) }).strict(),
    z.object({ $variant: z.literal("Needs"), needs: z.array(needSchema(path)) }).strict(),
  ]) as unknown as z.ZodType<QueryResult<R, P>>;
}
"#;

const CLIENT_RUNTIME_EXPORTS: [&str; 33] = [
    "INPUT_SCHEMA",
    "OUTPUT_SCHEMA",
    "PARTIAL_OUTPUT_SCHEMA",
    "EngineTransport",
    "ExplainDepth",
    "CitedParagraph",
    "ExplainedRule",
    "RuleContributor",
    "InputContributor",
    "Contributor",
    "ExplainedBy",
    "ExplainedValue",
    "ExplainedSection",
    "Explanation",
    "Explained",
    "ExplainedQuery",
    "CalculationErrorKind",
    "CalculationDiagnostic",
    "CalculationError",
    "EntryFrame",
    "ResolverAnswer",
    "ask",
    "answer",
    "ResolverContext",
    "Query",
    "Resolver",
    "Resolvers",
    "Resolution",
    "PendingEntry",
    "ResolveOptions",
    "ResolveError",
    "ResolveCycleError",
    "resolve",
];

const CLIENT_RUNTIME: &str = r#"export const INPUT_SCHEMA = "{INPUT_SCHEMA}";
export const OUTPUT_SCHEMA = "{OUTPUT_SCHEMA}";
export const PARTIAL_OUTPUT_SCHEMA = "{PARTIAL_OUTPUT_SCHEMA}";

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

/** `direct`: each value's direct contributors; `full`: also the whole derivation. */
export type ExplainDepth = "direct" | "full";

/** A paragraph a rule clause cites, with its quoted text. */
export type CitedParagraph = { paragraph: string; source?: string; text: string };

/** The rule clause that produced a value: its law section and cited paragraphs. */
export type ExplainedRule = {
  $variant: "Rule";
  rule: string;
  /** The RuleScope of a scoped rule. */
  scope?: string;
  clause: "default" | "exception" | "clause";
  /** The label of the exception that applied. */
  exception?: string;
  /** `file:line` of the clause. */
  location?: string;
  /** The law section around the clause; a key of {@link Explanation.sections}. */
  section?: string;
  cites?: CitedParagraph[];
};

/** A rule a value was computed from, with the value it gave. */
export type RuleContributor = Omit<ExplainedRule, "cites"> & {
  paragraphs?: string[];
  value: unknown;
  /** The node in {@link Explanation.derivation}, with depth `full`. */
  node?: number;
};

/** An input a value was computed from. */
export type InputContributor = { $variant: "Input"; path: string; value: unknown };

export type Contributor = RuleContributor | InputContributor;

/**
 * What produced a value: a rule clause, the input unchanged, a computation
 * outside any rule, or a constant of the model.
 */
export type ExplainedBy =
  | ExplainedRule
  | { $variant: "Input"; path: string }
  | { $variant: "Computed" }
  | { $variant: "Constant" };

/** One leaf of a result and where it came from. */
export type ExplainedValue = {
  value: unknown;
  by: ExplainedBy;
  from?: Contributor[];
  /** Direct contributors beyond the first 32, with depth `direct`. */
  from_omitted?: number;
};

/** A law section: its anchor, quoted documents, attachments and quote. */
export type ExplainedSection = {
  location: string;
  sources: string[];
  attachments: { role: string; binding?: string; value: unknown }[];
  quote: string;
};

/** Where every leaf of a result came from; `docs/rfcs/explanations.md`. */
export type Explanation = {
  /** Every leaf of the result by its output path, in the path syntax of needs. */
  values: { [path: string]: ExplainedValue };
  sections: { [label: string]: ExplainedSection };
  /** With depth `full`: every rule node a value was computed from. */
  derivation?: { [node: string]: { value: unknown; by: ExplainedRule; from: Contributor[] } };
};

/** A result with its explanation. */
export type Explained<R> = { readonly result: R; readonly explanation: Explanation };

/** A partial call's result; `Complete` results carry an explanation. */
export type ExplainedQuery<R> = { readonly result: R; readonly explanation?: Explanation };

/**
 * Why a call failed: the input did not validate, the output did not validate,
 * the envelope is for another contract or entry, or the engine reported
 * diagnostics for the case.
 */
export type CalculationErrorKind = "input" | "output" | "envelope" | "diagnostics";

/** A diagnostic the engine reported for the case. */
export type CalculationDiagnostic = { case_id: string; path: string; message: string };

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

type EntryName = keyof typeof schema.SCHEMA_HASH;

function isObject(value: unknown): value is { readonly [key: string]: unknown } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function issueSummary(issues: readonly { path: readonly PropertyKey[]; message: string }[]): string {
  const first = issues[0];
  if (first === undefined) return "invalid";
  const at = first.path.length === 0 ? "$" : first.path.map(String).join(".");
  return `${issues.length} issue${issues.length === 1 ? "" : "s"}; first at ${at}: ${first.message}`;
}

function caseOutput(
  entry: EntryName,
  outputSchema: string,
  document: unknown,
): { readonly [key: string]: unknown } {
  const parsed: unknown = typeof document === "string" ? JSON.parse(document) : document;
  if (!isObject(parsed) || !("$futuruna" in parsed)) return { result: parsed };
  const envelope = parsed["$futuruna"];
  if (!isObject(envelope)) throw new CalculationError("envelope", entry, "`$futuruna` is not an object");
  const expectedHash = schema.SCHEMA_HASH[entry];
  if (envelope["schema_hash"] !== undefined && envelope["schema_hash"] !== expectedHash) {
    throw new CalculationError(
      "envelope",
      entry,
      `the engine runs contract ${String(envelope["schema_hash"])}, but this SDK was generated for ${expectedHash}`,
    );
  }
  if (envelope["entry"] !== undefined && envelope["entry"] !== entry) {
    throw new CalculationError("envelope", entry, `the output is for entry ${String(envelope["entry"])}`);
  }
  if (envelope["schema"] !== undefined && envelope["schema"] !== outputSchema) {
    throw new CalculationError("envelope", entry, `expected a ${outputSchema} document, got ${String(envelope["schema"])}`);
  }
  const diagnostics = parsed["diagnostics"];
  if (Array.isArray(diagnostics) && diagnostics.length > 0) {
    const list = diagnostics.filter(isObject).map((diagnostic) => ({
      case_id: String(diagnostic["case_id"]),
      path: String(diagnostic["path"]),
      message: String(diagnostic["message"]),
    }));
    const first = list[0];
    const summary = first === undefined ? "" : `; first at ${first.path}: ${first.message}`;
    throw new CalculationError("diagnostics", entry, `${diagnostics.length} diagnostic(s)${summary}`, {
      diagnostics: list,
    });
  }
  const results = parsed["results"];
  const only: unknown = Array.isArray(results) && results.length === 1 ? results[0] : undefined;
  if (!isObject(only) || !("result" in only)) {
    throw new CalculationError("envelope", entry, "the output document does not hold exactly one case result");
  }
  return only;
}

function inputDocument<I>(entry: EntryName, inputSchema: z.ZodType<I>, input: I): unknown {
  const checkedInput = inputSchema.safeParse(input);
  if (!checkedInput.success) {
    const issues = checkedInput.error.issues;
    throw new CalculationError("input", entry, `invalid input: ${issueSummary(issues)}`, { issues });
  }
  return {
    $futuruna: { schema: INPUT_SCHEMA, schema_hash: schema.SCHEMA_HASH[entry], entry },
    cases: [{ case_id: CASE_ID, input_status: "ready", input: checkedInput.data }],
  };
}

function checkedResult<O>(entry: EntryName, outputSchema: z.ZodType<O>, result: unknown): O {
  const checkedOutput = outputSchema.safeParse(result);
  if (!checkedOutput.success) {
    const issues = checkedOutput.error.issues;
    throw new CalculationError("output", entry, `invalid result: ${issueSummary(issues)}`, { issues });
  }
  return checkedOutput.data;
}

async function invokeExplained<I, O>(
  transport: EngineTransport,
  entry: EntryName,
  partialCall: boolean,
  depth: ExplainDepth,
  inputSchema: z.ZodType<I>,
  outputSchema: z.ZodType<O>,
  input: I,
): Promise<{ result: O; explanation: Explanation }> {
  const document = inputDocument(entry, inputSchema, input);
  const call = partialCall ? transport.callPartialExplained : transport.callExplained;
  if (call === undefined) {
    throw new CalculationError(
      "envelope",
      entry,
      `the transport has no ${partialCall ? "callPartialExplained" : "callExplained"}`,
    );
  }
  const output = await call.call(transport, entry, document, depth === "full");
  const only = caseOutput(entry, partialCall ? PARTIAL_OUTPUT_SCHEMA : OUTPUT_SCHEMA, output);
  const result = checkedResult(entry, outputSchema, only["result"]);
  const explanation = only["explanation"];
  if (explanation !== undefined && !isObject(explanation)) {
    throw new CalculationError("output", entry, "the explanation is not an object");
  }
  if (explanation === undefined && !partialCall) {
    throw new CalculationError("output", entry, "the engine returned no explanation");
  }
  return { result, explanation: explanation as unknown as Explanation };
}

async function invoke<I, O>(
  transport: EngineTransport,
  entry: EntryName,
  partialCall: boolean,
  inputSchema: z.ZodType<I>,
  outputSchema: z.ZodType<O>,
  input: I,
): Promise<O> {
  const document = inputDocument(entry, inputSchema, input);
  const output = partialCall ? await transport.callPartial(entry, document) : await transport.call(entry, document);
  const result = caseOutput(entry, partialCall ? PARTIAL_OUTPUT_SCHEMA : OUTPUT_SCHEMA, output)["result"];
  const checkedOutput = outputSchema.safeParse(result);
  if (!checkedOutput.success) {
    const issues = checkedOutput.error.issues;
    throw new CalculationError("output", entry, `invalid result: ${issueSummary(issues)}`, { issues });
  }
  return checkedOutput.data;
}

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
 * pattern with `[]` for any list index (`accounts[].closing_cash_ore`) and `*`
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
      readonly unused_paths: readonly P[];
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
  if (Object.prototype.hasOwnProperty.call(resolvers, path) && resolvers[path] !== undefined) return resolvers[path];
  const pattern = patterns.find((candidate) => resolvers[candidate] !== undefined && partial.matchesPathPattern(path, candidate));
  return pattern === undefined ? undefined : resolvers[pattern];
}

/**
 * Resolve a partial case: query it, let the resolvers answer its needs, write
 * the answers into the case with {@link partial.setAt}, and query again. Stops
 * with `Complete`, or with `Questions` when a round adds no answer. A path that
 * is still needed after it was answered is a {@link ResolveError}; an entry
 * that needs itself for the same subject is a {@link ResolveCycleError}.
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
      return { $variant: "Complete", result: result.result, unused_paths: result.unused_paths, partialCase, rounds };
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
        next = partial.setAt(next, need.path, reply.value);
        answered.add(need.path);
      } else if (reply.resolution.$variant === "Complete") {
        next = partial.setAt(next, need.path, reply.resolution.result);
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

    fn generate(source: &str) -> BTreeMap<&'static str, String> {
        generate_typescript_sdk(&contracts(source), "model.calculate.runa")
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

        let schema = &files["schema.ts"];
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
        assert_contains(schema, "refine(Number.isSafeInteger");
        assert_contains(schema, "  household_tax: \"");

        let partial = &files["partial.ts"];
        assert_contains(partial, "export type PartialHousehold = {");
        assert_contains(partial, "  deduction?: number | null | undefined;");
        assert_contains(partial, "  children?: PartialChild[] | undefined;");
        assert_contains(
            partial,
            "      deduction: schema.IntSchema.nullable().optional(),",
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
    fn sum_types_are_unions_tagged_by_variant() {
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

        let partial = &files["partial.ts"];
        assert_contains(
            partial,
            "export type PartialShape =\n  | { [key: string]: never }",
        );
        assert_contains(
            partial,
            "  | { $variant: \"Circle\"; $values?: [number] | undefined }",
        );
        assert_contains(partial, "    z.object({}).strict(),");
        assert_contains(
            partial,
            "  \"shape.$variant\": \"Circle\" | \"Rect\" | \"Empty\";",
        );
        assert_contains(partial, "  \"shape.w\": number;");
        assert_contains(partial, "  \"shape.$values[0]\": number;");
        assert_contains(partial, "  \"pair.first\": number;");
        assert_contains(
            partial,
            "export const AreaQueryResultSchema = queryResultSchema(schema.PairSchema(schema.IntSchema, z.string()), AreaInputPathSchema);",
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
            "export function personTax(transport: EngineTransport, input: types.Person): Promise<number> {",
        );
        assert_contains(
            client,
            "export function coupleTaxQuery(\n  transport: EngineTransport,\n  input: partial.PartialCouple,\n): Promise<partial.CoupleTaxQueryResult> {",
        );
        for expected in [
            "export function personTaxExplained(\n  transport: EngineTransport,\n  input: types.Person,\n  depth: ExplainDepth = \"direct\",\n): Promise<Explained<number>> {",
            "export function coupleTaxQueryExplained(\n  transport: EngineTransport,\n  input: partial.PartialCouple,\n  depth: ExplainDepth = \"direct\",\n): Promise<ExplainedQuery<partial.CoupleTaxQueryResult>> {",
            "callExplained?(entry: string, input: unknown, full: boolean): Promise<unknown>;",
        ] {
            assert_contains(client, expected);
        }
        assert_contains(client, "/**\n * Calculate `person_tax`: One person.");
        assert_contains(client, "export type PersonTaxResolvers = {");
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
        // Entries are ordered by name, so declaration order does not matter.
        assert!(schema.find("  couple_tax:").unwrap() < schema.find("  person_tax:").unwrap());
        let mut reversed = contracts.clone();
        reversed.reverse();
        assert_eq!(
            generate_typescript_sdk(&contracts, "model.calculate.runa").unwrap(),
            generate_typescript_sdk(&reversed, "model.calculate.runa").unwrap()
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
    }

    #[test]
    fn a_contract_name_that_collides_with_a_generated_name_is_an_error() {
        let error = generate_typescript_sdk(
            &contracts(
                "# Need(amount: Int)\n@ calculate\n> needed(input: Need) -> Int { input.amount }\n",
            ),
            "model.calculate.runa",
        )
        .expect_err("`Need` collides with the generated need type");
        assert!(error.contains("`Need`"), "{error}");
    }

    #[test]
    fn partial_type_names_are_qualified_from_the_client() {
        assert_eq!(
            qualify_partial("PartialPair<PartialChild, number>[]"),
            "partial.PartialPair<partial.PartialChild, number>[]"
        );
        assert_eq!(
            qualify_partial("PartialPairSchema(schema.IntSchema)"),
            "partial.PartialPairSchema(schema.IntSchema)"
        );
    }
}
