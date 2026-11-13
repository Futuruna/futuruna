//! Typed calculation contracts and canonical value interchange.
//!
//! `@ calculate` does not alter evaluation. It selects an ordinary typed rule
//! or function as a discoverable input/output boundary for external adapters
//! and may carry one human-readable title for that boundary.

use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

mod json;
pub use json::parse_calculation_json;

pub mod compact;
pub mod sdk_ts;

#[cfg(test)]
thread_local! {
    static CONTRACT_EXTRACTION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub const CONTRACT_SCHEMA: &str = "futuruna.calculate.v1";
pub const INPUT_SCHEMA: &str = "futuruna.calculate.input.v2";
pub const LEGACY_INPUT_SCHEMA: &str = "futuruna.calculate.input.v1";
pub const OUTPUT_SCHEMA: &str = "futuruna.calculate.output.v1";
pub const PARTIAL_OUTPUT_SCHEMA: &str = "futuruna.calculate.partial-output.v1";
const CALCULATION_WORKER_STACK_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_MAX_CALCULATION_WORKERS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalculationTypeRef {
    Primitive {
        name: String,
    },
    Named {
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        arguments: Vec<CalculationTypeRef>,
    },
    TypeParameter {
        name: String,
    },
    Optional {
        item: Box<CalculationTypeRef>,
    },
    List {
        item: Box<CalculationTypeRef>,
    },
    Map {
        key: Box<CalculationTypeRef>,
        value: Box<CalculationTypeRef>,
    },
    Set {
        item: Box<CalculationTypeRef>,
    },
    Unit,
}

impl CalculationTypeRef {
    pub fn display_name(&self) -> String {
        match self {
            Self::Primitive { name } | Self::TypeParameter { name } => name.clone(),
            Self::Named { name, arguments } => display_type_application(name, arguments),
            Self::Optional { item } => format!("{}?", item.display_name()),
            Self::List { item } => format!("List({})", item.display_name()),
            Self::Map { key, value } => {
                format!("Map({}, {})", key.display_name(), value.display_name())
            }
            Self::Set { item } => format!("Set({})", item.display_name()),
            Self::Unit => "()".to_string(),
        }
    }
}

fn display_type_application(name: &str, arguments: &[CalculationTypeRef]) -> String {
    if arguments.is_empty() {
        name.to_string()
    } else {
        format!(
            "{}({})",
            name,
            arguments
                .iter()
                .map(CalculationTypeRef::display_name)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: CalculationTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationVariant {
    pub name: String,
    pub positional: bool,
    pub fields: Vec<CalculationField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationTypeDefinition {
    pub name: String,
    pub parameters: Vec<String>,
    pub kind: String,
    pub variants: Vec<CalculationVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationMetadataReference {
    pub label: String,
    pub role: String,
    pub binding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_path: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub qualified_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationFieldMetadata {
    pub path: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub anchor: String,
    pub binding: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<CalculationMetadataReference>,
    /// Another calculation entry provides this field's value, for a subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provided_by: Option<CalculationFieldProvider>,
}

/// The calculation entry that provides a field, and the input path of the
/// subject it runs for (`taxpayer.spouse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationFieldProvider {
    pub entry: String,
    pub subject: String,
}

/// Field metadata record members a calculation reads.
const CALCULATION_FIELD_MEMBERS: [&str; 7] = [
    "path", "label", "question", "help", "unit", "entry", "subject",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationContract {
    pub schema: String,
    pub schema_version: u32,
    pub entry: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub parameter: String,
    pub input: CalculationTypeRef,
    pub output: CalculationTypeRef,
    pub definitions: Vec<CalculationTypeDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata: Vec<CalculationMetadataReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_metadata: Vec<CalculationFieldMetadata>,
    pub schema_hash: String,
}

impl CalculationContract {
    fn finish_hash(mut self) -> Self {
        self.schema_hash.clear();
        let canonical = serde_json::to_vec(&self).expect("calculation contract serializes");
        let mut hasher = Sha256::new();
        hasher.update(canonical);
        self.schema_hash = format!("{:x}", hasher.finalize());
        self
    }

    pub fn definition(&self, name: &str) -> Option<&CalculationTypeDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.name == name)
    }

    pub fn template_envelope(&self) -> CalculationInputEnvelope {
        CalculationInputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: INPUT_SCHEMA.to_string(),
                schema_hash: self.schema_hash.clone(),
                entry: self.entry.clone(),
            },
            cases: vec![CalculationInputCase {
                case_id: "case-1".to_string(),
                input_status: Some(CalculationInputStatus::Draft),
                input: template_value(&self.input, self, &BTreeMap::new(), &mut BTreeSet::new()),
            }],
        }
    }

    pub fn decode_input(&self, value: &JsonValue) -> Result<Value, CalculationValueError> {
        self.decode_input_diagnostics(value).map_err(|errors| {
            errors
                .into_iter()
                .next()
                .expect("invalid input has a diagnostic")
        })
    }

    /// Decode a case while collecting independent input errors. An invalid
    /// parent shape stops that subtree; invalid cases never yield a value.
    pub fn decode_input_diagnostics(
        &self,
        value: &JsonValue,
    ) -> Result<Value, Vec<CalculationValueError>> {
        decode_value(value, &self.input, self, &BTreeMap::new(), "$", None)
    }

    /// Decode a partial case: left-out fields are unknown values carrying
    /// their input paths. Also returns the provided input leaves and the
    /// input containers, for read tracking.
    pub(crate) fn decode_partial_input(
        &self,
        value: &JsonValue,
    ) -> Result<(Value, PartialDecoder), Vec<CalculationValueError>> {
        let decoder = RefCell::new(PartialDecoder::default());
        let decoded = decode_value(
            value,
            &self.input,
            self,
            &BTreeMap::new(),
            "$",
            Some(&decoder),
        )?;
        Ok((decoded, decoder.into_inner()))
    }

    pub fn encode_output(&self, value: &Value) -> Result<JsonValue, CalculationValueError> {
        encode_value(value, &self.output, self, &BTreeMap::new(), "$")
    }

    pub fn input_layout(&self) -> CalculationInputLayout {
        let mut builder = CalculationLayoutBuilder::new(self);
        let mut root_columns = Vec::new();
        builder.flatten_value(
            "",
            "",
            "",
            &self.input,
            &BTreeMap::new(),
            true,
            None,
            &[],
            &mut BTreeSet::new(),
            &mut root_columns,
        );
        if root_columns.is_empty() && builder.tables.is_empty() {
            root_columns.push(CalculationColumn {
                path: "input".to_string(),
                input_path: "input".to_string(),
                value_path: "input".to_string(),
                ty: self.input.clone(),
                encoding: CalculationColumnEncoding::Json,
                required: true,
                choices: Vec::new(),
                variant_guards: Vec::new(),
                metadata: None,
                member_metadata: Vec::new(),
            });
        }
        for column in &mut root_columns {
            self.attach_column_metadata(column);
        }
        for table in &mut builder.tables {
            table.metadata = self.field_metadata_for_path(&table.path).cloned();
            for column in &mut table.columns {
                self.attach_column_metadata(column);
            }
        }
        builder.tables.sort_by(|left, right| {
            left.path
                .split('.')
                .count()
                .cmp(&right.path.split('.').count())
                .then_with(|| left.path.cmp(&right.path))
        });
        CalculationInputLayout {
            root_columns,
            collection_tables: builder.tables,
        }
    }

    pub fn input_columns(&self) -> Vec<CalculationColumn> {
        self.input_layout().root_columns
    }

    pub fn field_metadata_for_path(&self, path: &str) -> Option<&CalculationFieldMetadata> {
        self.field_metadata
            .iter()
            .find(|metadata| metadata.path == path)
    }

    fn attach_column_metadata(&self, column: &mut CalculationColumn) {
        column.metadata = self.field_metadata_for_path(&column.input_path).cloned();
        let members = optional_member_paths(self, column);
        column.member_metadata = self
            .field_metadata
            .iter()
            .filter(|metadata| members.contains(&metadata.path))
            .cloned()
            .collect();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationEnvelopeMetadata {
    pub schema: String,
    pub schema_hash: String,
    pub entry: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalculationInputCase {
    pub case_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_status: Option<CalculationInputStatus>,
    pub input: JsonValue,
}

/// An explicit input-review acknowledgement, not document authentication
/// or a statement about the correctness of the model or its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculationInputStatus {
    Draft,
    Ready,
}

/// Key of an unfilled value in a generated template. No input type accepts
/// an object with this key, so an unedited template never decodes as a case.
pub const TEMPLATE_PLACEHOLDER_KEY: &str = "$fill";

pub fn is_template_placeholder(value: &JsonValue) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.contains_key(TEMPLATE_PLACEHOLDER_KEY))
}

fn template_placeholder(expected: String) -> JsonValue {
    let mut object = JsonMap::new();
    object.insert(
        TEMPLATE_PLACEHOLDER_KEY.to_string(),
        JsonValue::String(expected),
    );
    JsonValue::Object(object)
}

/// The placeholder for an unfilled value of `ty`, as generated templates
/// write it; spreadsheet adapters use it for a blank required cell.
pub fn template_placeholder_for(ty: &CalculationTypeRef) -> JsonValue {
    template_placeholder(ty.display_name())
}

/// Input-relative paths (`$.field`) of unfilled placeholders in one case.
pub fn template_placeholder_paths(input: &JsonValue) -> Vec<String> {
    let mut found = Vec::new();
    collect_template_placeholders(input, "$", &mut found);
    found.into_iter().map(|(path, _)| path).collect()
}

fn collect_template_placeholders(value: &JsonValue, path: &str, found: &mut Vec<(String, String)>) {
    if is_template_placeholder(value) {
        let expected = value[TEMPLATE_PLACEHOLDER_KEY]
            .as_str()
            .unwrap_or("value")
            .to_string();
        found.push((path.to_string(), expected));
    } else if let Some(object) = value.as_object() {
        for (key, item) in object {
            collect_template_placeholders(item, &format!("{path}.{key}"), found);
        }
    } else if let Some(items) = value.as_array() {
        for (index, item) in items.iter().enumerate() {
            collect_template_placeholders(item, &format!("{path}[{index}]"), found);
        }
    }
}

impl CalculationInputCase {
    /// Unfilled placeholders and a missing readiness acknowledgement each
    /// keep a case from running; every unfilled field is listed.
    fn unfinished_diagnostics(&self, index: usize) -> Vec<CalculationCaseDiagnostic> {
        let mut placeholders = Vec::new();
        collect_template_placeholders(
            &self.input,
            &format!("$.cases[{index}].input"),
            &mut placeholders,
        );
        let mut diagnostics: Vec<_> = placeholders
            .into_iter()
            .map(|(path, expected)| CalculationCaseDiagnostic {
                case_id: self.case_id.clone(),
                path,
                message: format!(
                    "template placeholder is not filled; replace it with a `{expected}` value"
                ),
            })
            .collect();
        if self.input_status != Some(CalculationInputStatus::Ready) {
            diagnostics.push(CalculationCaseDiagnostic {
                case_id: self.case_id.clone(),
                path: format!("$.cases[{index}].input_status"),
                message: "case inputs are not marked ready; review all supplied values, including zeros, empty collections and optional facts, then set input_status to `ready`. Readiness is not verification of documents or model correctness".to_string(),
            });
        }
        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalculationInputEnvelope {
    #[serde(rename = "$futuruna")]
    pub futuruna: CalculationEnvelopeMetadata,
    pub cases: Vec<CalculationInputCase>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalculationResultCase {
    pub case_id: String,
    pub result: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationCaseDiagnostic {
    pub case_id: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalculationOutputEnvelope {
    #[serde(rename = "$futuruna")]
    pub futuruna: CalculationEnvelopeMetadata,
    pub results: Vec<CalculationResultCase>,
    pub diagnostics: Vec<CalculationCaseDiagnostic>,
}

/// One case's answer to a partial call: its result, or what it still needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "$variant")]
pub enum CalculationPartialResult {
    Complete {
        result: JsonValue,
        /// Provided input paths evaluation never read.
        unused_paths: Vec<String>,
    },
    Needs {
        needs: Vec<CalculationNeed>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculationNeedKind {
    /// The client answers the path itself.
    Fact,
    /// Another calculation entry provides the value.
    Entry,
}

/// A fact or entry result that blocks a partial case's result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalculationNeed {
    pub kind: CalculationNeedKind,
    /// The concrete input path to answer, with list indices.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<CalculationMetadataReference>,
    /// The innermost rule family evaluating when the path was first read.
    pub waiting_rule: String,
}

enum InputPathToken<'a> {
    Field(&'a str),
    Index(usize),
}

fn input_path_tokens(path: &str) -> Option<Vec<InputPathToken<'_>>> {
    let mut tokens = Vec::new();
    for segment in path.split('.') {
        let (name, mut rest) = match segment.find('[') {
            Some(open) => segment.split_at(open),
            None => (segment, ""),
        };
        if !name.is_empty() {
            tokens.push(InputPathToken::Field(name));
        }
        while let Some(stripped) = rest.strip_prefix('[') {
            let close = stripped.find(']')?;
            tokens.push(InputPathToken::Index(stripped[..close].parse().ok()?));
            rest = &stripped[close + 1..];
        }
    }
    Some(tokens)
}

impl CalculationContract {
    /// The canonical metadata path of a concrete input path: list indices
    /// and map keys removed, a sum type's payload under its variant name.
    /// `input` resolves which variant a payload belongs to.
    pub fn canonical_input_path(&self, input: &JsonValue, path: &str) -> Option<String> {
        let mut ty = self.input.clone();
        let mut substitutions = BTreeMap::new();
        let mut json = Some(input);
        let mut canonical: Vec<String> = Vec::new();
        for token in input_path_tokens(path)? {
            loop {
                ty = substitute_contract_type(&ty, &substitutions);
                match (&ty, &token) {
                    (CalculationTypeRef::Optional { item }, _) => ty = (**item).clone(),
                    (
                        CalculationTypeRef::List { item } | CalculationTypeRef::Set { item },
                        InputPathToken::Index(index),
                    ) => {
                        json = json.and_then(|json| json.get(*index));
                        ty = (**item).clone();
                        break;
                    }
                    _ => break,
                }
            }
            let InputPathToken::Field(name) = token else {
                continue;
            };
            match &ty {
                CalculationTypeRef::Map { value, .. } => {
                    json = json.and_then(|json| json.get(name));
                    ty = (**value).clone();
                    continue;
                }
                CalculationTypeRef::Named {
                    name: type_name,
                    arguments,
                } => {
                    let definition = self.definition(type_name)?;
                    substitutions = definition_substitutions(definition, arguments);
                    if let Some(variant) = product_variant(definition) {
                        ty = variant.fields.iter().find(|f| f.name == name)?.ty.clone();
                        canonical.push(name.to_string());
                    } else if name == "$variant" {
                        canonical.push(name.to_string());
                        return Some(canonical.join("."));
                    } else {
                        let chosen = json
                            .and_then(|json| json.get("$variant"))
                            .and_then(JsonValue::as_str);
                        let variant = definition.variants.iter().find(|variant| {
                            chosen.map_or(true, |chosen| variant.name == chosen)
                                && variant.fields.iter().any(|f| f.name == name)
                        })?;
                        ty = variant.fields.iter().find(|f| f.name == name)?.ty.clone();
                        canonical.push(variant.name.clone());
                        canonical.push(name.to_string());
                    }
                    json = json.and_then(|json| json.get(name));
                }
                _ => return None,
            }
        }
        Some(canonical.join("."))
    }

    /// The need for a concrete input path, with the field's metadata.
    pub(crate) fn need_for_path(
        &self,
        input: &JsonValue,
        path: String,
        waiting_rule: String,
    ) -> CalculationNeed {
        let metadata = self
            .canonical_input_path(input, &path)
            .and_then(|canonical| {
                self.field_metadata_for_path(&canonical).or_else(|| {
                    canonical
                        .strip_suffix(".$variant")
                        .and_then(|owner| self.field_metadata_for_path(owner))
                })
            });
        let provider = metadata.and_then(|metadata| metadata.provided_by.as_ref());
        CalculationNeed {
            kind: if provider.is_some() {
                CalculationNeedKind::Entry
            } else {
                CalculationNeedKind::Fact
            },
            path,
            entry: provider.map(|provider| provider.entry.clone()),
            subject: provider.map(|provider| provider.subject.clone()),
            label: metadata.map(|metadata| metadata.label.clone()),
            question: metadata.and_then(|metadata| metadata.question.clone()),
            help: metadata.and_then(|metadata| metadata.help.clone()),
            unit: metadata.and_then(|metadata| metadata.unit.clone()),
            sources: metadata
                .map(|metadata| metadata.sources.clone())
                .unwrap_or_default(),
            waiting_rule,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculationValueError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for CalculationValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for CalculationValueError {}

fn value_error(path: impl Into<String>, message: impl Into<String>) -> CalculationValueError {
    CalculationValueError {
        path: path.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculationColumnEncoding {
    Integer,
    Float,
    Boolean,
    String,
    Character,
    Enum,
    Variant,
    Json,
}

impl CalculationColumnEncoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::String => "string",
            Self::Character => "character",
            Self::Enum => "enum",
            Self::Variant => "variant",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationVariantGuard {
    pub path: String,
    pub variant: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationColumn {
    pub path: String,
    pub input_path: String,
    pub value_path: String,
    #[serde(rename = "type")]
    pub ty: CalculationTypeRef,
    pub encoding: CalculationColumnEncoding,
    pub required: bool,
    pub choices: Vec<String>,
    pub variant_guards: Vec<CalculationVariantGuard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<CalculationFieldMetadata>,
    /// Field metadata of the members inside an optional composite JSON
    /// column, which stays one cell.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub member_metadata: Vec<CalculationFieldMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculationCollectionKind {
    List,
    Map,
    Set,
}

impl CalculationCollectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Map => "map",
            Self::Set => "set",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationCollectionTable {
    pub path: String,
    pub sheet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_path: Option<String>,
    pub attach_path: String,
    pub kind: CalculationCollectionKind,
    pub item_type: CalculationTypeRef,
    pub item_value_column: bool,
    pub variant_guards: Vec<CalculationVariantGuard>,
    pub columns: Vec<CalculationColumn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<CalculationFieldMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationInputLayout {
    pub root_columns: Vec<CalculationColumn>,
    pub collection_tables: Vec<CalculationCollectionTable>,
}

impl CalculationInputLayout {
    pub fn table(&self, path: &str) -> Option<&CalculationCollectionTable> {
        self.collection_tables
            .iter()
            .find(|table| table.path == path)
    }
}

#[derive(Debug, Clone)]
enum CatalogType {
    Adt {
        parameters: Vec<String>,
        variants: Vec<Variant>,
        except_from: Option<(String, Vec<String>)>,
    },
    RuleScope {
        parameters: Vec<Param>,
    },
}

const EXACT_TYPE_VARIANT_EXPANSION_LIMIT: usize = 1_000_000;
const EXACT_TYPE_RESOLUTION_DEPTH_LIMIT: usize = 64;

#[derive(Debug, Default)]
pub(crate) struct TypeCatalog {
    types: BTreeMap<String, CatalogType>,
    conditional_types: BTreeSet<String>,
    duplicate_types: BTreeSet<String>,
    type_origins: BTreeMap<String, String>,
    type_order: BTreeMap<String, usize>,
    next_type_order: usize,
    exact_mode: bool,
}

impl TypeCatalog {
    /// Rebuild the exact runtime type catalog from the immutable declaration
    /// graph retained by type checking.
    ///
    /// Unlike [`Self::collect_checked`], this path performs no filesystem I/O:
    /// flat/hash imports have already been resolved into `program`, while
    /// qualified imports deliberately remain opaque. Equal declaration IDs
    /// reached through repeated flat imports are consumed once, matching the
    /// import-visited behavior of the source collector without trusting paths.
    pub(crate) fn collect_checked_analysis_program(
        program: &CheckedAnalysisProgram,
    ) -> Result<Self, Vec<String>> {
        let root_modules = program
            .declarations
            .iter()
            .filter(|declaration| matches!(declaration.import_kind, SourcedImportKind::Root))
            .map(|declaration| declaration.id.module.clone())
            .collect::<BTreeSet<_>>();
        if root_modules.len() != 1 {
            return Err(vec![
                "checked analysis does not identify exactly one root module for exact type replay"
                    .to_string(),
            ]);
        }
        let root_module = root_modules
            .iter()
            .next()
            .expect("one checked analysis root module");

        let mut catalog = Self::default();
        catalog.exact_mode = true;
        let mut consumed = BTreeSet::new();
        let mut imported = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for declaration in program.declarations.iter() {
            if !matches!(declaration.statement.as_ref(), Stmt::TypeDecl(_))
                || !consumed.insert(declaration.id.clone())
            {
                continue;
            }
            let origin = if &declaration.id.module == root_module {
                "<root>".to_string()
            } else {
                let import_kind = match &declaration.import_kind {
                    SourcedImportKind::Root => "root",
                    SourcedImportKind::PlainImport => "plain",
                    SourcedImportKind::HashImport { selected_hash } => selected_hash.as_ref(),
                    SourcedImportKind::QualifiedImport { module_name } => module_name.as_ref(),
                };
                format!(
                    "<checked:{import_kind}:{}:{}>",
                    declaration.id.module.content_hash,
                    declaration.id.module.internal_path.join("/")
                )
            };
            catalog.collect_inner(
                std::slice::from_ref(declaration.statement.as_ref()),
                None,
                &origin,
                &mut imported,
                &mut diagnostics,
                true,
            );
        }
        if diagnostics.is_empty() {
            Ok(catalog)
        } else {
            Err(diagnostics)
        }
    }

    /// Collect the named types visible through this program's flat imports.
    ///
    /// Unlike the calculation contract collector's compatibility path, this
    /// constructor fails closed when an import cannot be located, read, or
    /// parsed. Consumers that use the catalog to claim a complete finite type
    /// universe must not silently continue with a partial declaration graph.
    pub(crate) fn collect_checked(
        stmts: &[Stmt],
        source_dir: Option<&str>,
    ) -> Result<Self, Vec<String>> {
        let mut catalog = Self::default();
        catalog.exact_mode = true;
        let mut imported = BTreeSet::new();
        let mut diagnostics = Vec::new();
        catalog.collect_inner(
            stmts,
            source_dir,
            "<root>",
            &mut imported,
            &mut diagnostics,
            true,
        );
        if diagnostics.is_empty() {
            Ok(catalog)
        } else {
            Err(diagnostics)
        }
    }

    fn collect(&mut self, stmts: &[Stmt], source_dir: Option<&str>) {
        let mut imported = BTreeSet::new();
        let mut ignored_diagnostics = Vec::new();
        self.collect_inner(
            stmts,
            source_dir,
            "<root>",
            &mut imported,
            &mut ignored_diagnostics,
            false,
        );
    }

    fn collect_inner(
        &mut self,
        stmts: &[Stmt],
        source_dir: Option<&str>,
        origin: &str,
        imported: &mut BTreeSet<String>,
        diagnostics: &mut Vec<String>,
        checked: bool,
    ) {
        for stmt in stmts {
            match stmt {
                Stmt::TypeDecl(
                    declaration @ TypeDecl::ADT {
                        name,
                        params,
                        variants,
                        except_from,
                        ..
                    },
                ) => {
                    if checked
                        && is_exact_runtime_type_name(name)
                        && !is_canonical_exact_runtime_declaration(declaration)
                    {
                        diagnostics.push(format!(
                            "declared type `{}` shadows a built-in primitive or structural type and cannot define an exact exploration universe",
                            name
                        ));
                    }
                    if checked && self.types.contains_key(name) {
                        self.duplicate_types.insert(name.clone());
                    }
                    if !self.type_order.contains_key(name) {
                        self.type_order.insert(name.clone(), self.next_type_order);
                        self.type_origins.insert(name.clone(), origin.to_string());
                        self.next_type_order += 1;
                    }
                    self.types.insert(
                        name.clone(),
                        CatalogType::Adt {
                            parameters: params.iter().map(|param| param.name.clone()).collect(),
                            variants: variants.clone(),
                            except_from: except_from.clone(),
                        },
                    );
                }
                Stmt::TypeDecl(TypeDecl::RuleScope { name, params, .. }) => {
                    if checked && is_exact_runtime_type_name(name) {
                        diagnostics.push(format!(
                            "declared rule scope `{}` shadows a built-in primitive or structural type and cannot define an exact exploration universe",
                            name
                        ));
                    }
                    if checked && self.types.contains_key(name) {
                        self.duplicate_types.insert(name.clone());
                    }
                    if !self.type_order.contains_key(name) {
                        self.type_order.insert(name.clone(), self.next_type_order);
                        self.type_origins.insert(name.clone(), origin.to_string());
                        self.next_type_order += 1;
                    }
                    self.types.insert(
                        name.clone(),
                        CatalogType::RuleScope {
                            parameters: params.clone(),
                        },
                    );
                }
                Stmt::TypeDecl(TypeDecl::WhenType { name, .. }) => {
                    if checked {
                        if is_exact_runtime_type_name(name) {
                            diagnostics.push(format!(
                                "conditional type evolution for `{}` changes a built-in primitive or structural type and cannot define an exact exploration universe",
                                name
                            ));
                        }
                        self.conditional_types.insert(name.clone());
                    }
                }
                Stmt::Import(path) => {
                    let Some(dir) = source_dir else {
                        if checked {
                            diagnostics.push(format!(
                                "cannot resolve flat import `{}` without a source directory",
                                path
                            ));
                        }
                        continue;
                    };
                    let Some(file_path) = Interpreter::resolve_import_path_for_source(path, dir)
                    else {
                        if checked {
                            diagnostics.push(format!(
                                "cannot resolve flat import `{}` from `{}`",
                                path, dir
                            ));
                        }
                        continue;
                    };
                    let canonical = std::fs::canonicalize(&file_path)
                        .unwrap_or_else(|_| Path::new(&file_path).to_path_buf());
                    let canonical = canonical.to_string_lossy().to_string();
                    if !imported.insert(canonical.clone()) {
                        continue;
                    }
                    let source = match read_source_file(Path::new(&file_path)) {
                        Ok(source) => source,
                        Err(error) => {
                            if checked {
                                diagnostics.push(format!(
                                    "cannot resolve flat import `{}`: {} ({})",
                                    path, file_path, error
                                ));
                            }
                            continue;
                        }
                    };
                    let mut lexer = Lexer::new(&source);
                    let tokens = lexer.tokenize();
                    let mut parser = Parser::new(tokens, &source);
                    let imported_stmts = match parser.parse_program() {
                        Ok(imported_stmts) => imported_stmts,
                        Err(error) => {
                            if checked {
                                diagnostics.push(format!(
                                    "cannot parse flat imported module `{}` at {}: {}",
                                    path, file_path, error
                                ));
                            }
                            continue;
                        }
                    };
                    let nested_dir = Path::new(&file_path)
                        .parent()
                        .map(|parent| parent.to_string_lossy().to_string())
                        .unwrap_or_else(|| ".".to_string());
                    self.collect_inner(
                        &imported_stmts,
                        Some(&nested_dir),
                        &canonical,
                        imported,
                        diagnostics,
                        checked,
                    );
                }
                Stmt::HashImport(hash, path) => {
                    let Some(dir) = source_dir else {
                        if checked {
                            diagnostics.push(format!(
                                "cannot resolve hash import `#{}` from `{}` without a source directory",
                                hash, path
                            ));
                        }
                        continue;
                    };
                    let Some(file_path) = Interpreter::resolve_import_path_for_source(path, dir)
                    else {
                        if checked {
                            diagnostics.push(format!(
                                "cannot resolve hash import `#{}` from `{}`",
                                hash, path
                            ));
                        }
                        continue;
                    };
                    let canonical = std::fs::canonicalize(&file_path)
                        .unwrap_or_else(|_| Path::new(&file_path).to_path_buf())
                        .to_string_lossy()
                        .to_string();
                    let import_key = format!("{}#{}", canonical, hash);
                    if !imported.insert(import_key.clone()) {
                        continue;
                    }
                    let module = match parse_source_module_file_cached(Path::new(&file_path)) {
                        Ok(module) => module,
                        Err(error) => {
                            if checked {
                                diagnostics.push(format!(
                                    "cannot parse hash imported module `{}` at {}: {}",
                                    path, file_path, error
                                ));
                            }
                            continue;
                        }
                    };
                    let matched = module
                        .statements()
                        .iter()
                        .filter(|statement| match statement {
                            Stmt::Defn(definition) => content_hash_defn(definition) == *hash,
                            Stmt::TypeDecl(declaration) => content_hash_type(declaration) == *hash,
                            _ => false,
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if matched.len() != 1 {
                        if checked {
                            diagnostics.push(format!(
                                "cannot resolve hash import `#{}` from `{}`: expected exactly one matching definition, found {}",
                                hash,
                                path,
                                matched.len()
                            ));
                        }
                        continue;
                    }
                    let nested_dir = Path::new(&file_path)
                        .parent()
                        .map(|parent| parent.to_string_lossy().to_string())
                        .unwrap_or_else(|| ".".to_string());
                    self.collect_inner(
                        &matched,
                        Some(&nested_dir),
                        &import_key,
                        imported,
                        diagnostics,
                        checked,
                    );
                }
                _ => {}
            }
        }
    }

    /// Resolve an ADT's variants in source expansion order.
    pub(crate) fn resolved_variants(&self, name: &str) -> Result<Vec<Variant>, String> {
        let mut budget = self
            .exact_mode
            .then_some(EXACT_TYPE_VARIANT_EXPANSION_LIMIT);
        self.resolved_variants_inner(name, &mut BTreeSet::new(), &mut budget)
    }

    /// Return the declared generic parameters for an ADT.
    ///
    /// RuleScope inputs are value fields rather than type parameters, so a
    /// RuleScope has an empty generic-parameter list.
    pub(crate) fn type_parameters(&self, name: &str) -> Result<Vec<String>, String> {
        if self.duplicate_types.contains(name) {
            return Err(format!(
                "type `{}` has multiple declarations and cannot define one exact universe",
                name
            ));
        }
        match self.types.get(name) {
            Some(CatalogType::Adt { parameters, .. }) => Ok(parameters.clone()),
            Some(CatalogType::RuleScope { .. }) => Ok(Vec::new()),
            None => Err(format!("unknown type `{}`", name)),
        }
    }

    pub(crate) fn is_rule_scope(&self, name: &str) -> bool {
        matches!(self.types.get(name), Some(CatalogType::RuleScope { .. }))
    }

    pub(crate) fn contains_type(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    fn ensure_inclusion_source_visible(&self, owner: &str, source: &str) -> Result<(), String> {
        if !self.exact_mode {
            return Ok(());
        }
        if matches!(self.types.get(source), Some(CatalogType::RuleScope { .. })) {
            return Err(format!(
                "type `{}` includes open rule scope `{}`, which cannot define a finite value universe",
                owner, source
            ));
        }
        self.type_origins
            .get(owner)
            .ok_or_else(|| format!("unknown type `{}`", owner))?;
        self.type_origins
            .get(source)
            .ok_or_else(|| format!("unknown included type `{}`", source))?;
        let owner_order = self.type_order.get(owner).copied().unwrap_or(usize::MAX);
        let source_order = self.type_order.get(source).copied().unwrap_or(usize::MAX);
        if source_order >= owner_order {
            return Err(format!(
                "type `{}` includes `{}` outside its already initialized declaration prefix; exact finite universes require an earlier visible type declaration",
                owner, source
            ));
        }
        Ok(())
    }

    fn resolved_variants_inner(
        &self,
        name: &str,
        active: &mut BTreeSet<String>,
        budget: &mut Option<usize>,
    ) -> Result<Vec<Variant>, String> {
        if self.exact_mode && active.len() >= EXACT_TYPE_RESOLUTION_DEPTH_LIMIT {
            return Err(format!(
                "finite type `{}` exceeds the exact resolution depth limit {}",
                name, EXACT_TYPE_RESOLUTION_DEPTH_LIMIT
            ));
        }
        if self.duplicate_types.contains(name) {
            return Err(format!(
                "type `{}` has multiple declarations and cannot define one exact universe",
                name
            ));
        }
        if self.conditional_types.contains(name) {
            return Err(format!(
                "type `{}` has conditional variants and cannot define a complete static universe",
                name
            ));
        }
        if !active.insert(name.to_string()) {
            return Err(format!("cyclic type inclusion involving `{}`", name));
        }
        let result = match self.types.get(name) {
            Some(CatalogType::Adt {
                variants,
                except_from,
                ..
            }) => {
                let mut resolved = Vec::new();
                if let Some((source, excluded)) = except_from {
                    self.ensure_inclusion_source_visible(name, source)?;
                    for variant in self.resolved_variants_inner(source, active, budget)? {
                        if !excluded.contains(&variant.name) {
                            charge_exact_variant_expansion(budget, 1, name)?;
                            resolved.push(variant);
                        }
                    }
                }
                for variant in variants {
                    match variant.from_type.as_deref() {
                        Some("__maybe_include") if self.types.contains_key(&variant.name) => {
                            self.ensure_inclusion_source_visible(name, &variant.name)?;
                            let included =
                                self.resolved_variants_inner(&variant.name, active, budget)?;
                            charge_exact_variant_expansion(budget, included.len(), name)?;
                            resolved.extend(included);
                        }
                        Some(source) if source != "__maybe_include" => {
                            self.ensure_inclusion_source_visible(name, source)?;
                            let source_variant = self
                                .resolved_variants_inner(source, active, budget)?
                                .into_iter()
                                .find(|candidate| candidate.name == variant.name)
                                .ok_or_else(|| {
                                    format!(
                                        "type `{}` has no variant `{}` included by `{}`",
                                        source, variant.name, name
                                    )
                                })?;
                            charge_exact_variant_expansion(budget, 1, name)?;
                            resolved.push(source_variant);
                        }
                        _ => {
                            charge_exact_variant_expansion(budget, 1, name)?;
                            resolved.push(variant.clone());
                        }
                    }
                }
                Ok(resolved)
            }
            Some(CatalogType::RuleScope { parameters }) => {
                charge_exact_variant_expansion(budget, 1, name)?;
                Ok(vec![Variant {
                    name: name.to_string(),
                    fields: parameters
                        .iter()
                        .map(|parameter| Field {
                            name: parameter.name.clone(),
                            ty: parameter.ty.clone().unwrap_or(Ty::Hole),
                        })
                        .collect(),
                    positional: false,
                    from_type: None,
                }])
            }
            None => Err(format!("unknown type `{}`", name)),
        };
        active.remove(name);
        result
    }
}

fn charge_exact_variant_expansion(
    budget: &mut Option<usize>,
    amount: usize,
    owner: &str,
) -> Result<(), String> {
    let Some(remaining) = budget else {
        return Ok(());
    };
    let Some(next) = remaining.checked_sub(amount) else {
        return Err(format!(
            "finite type `{}` exceeds the exact variant-expansion work limit {}",
            owner, EXACT_TYPE_VARIANT_EXPANSION_LIMIT
        ));
    };
    *remaining = next;
    Ok(())
}

#[cfg(test)]
mod type_catalog_tests {
    use super::*;

    fn parse(source: &str) -> Vec<Stmt> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        parser.parse_program().expect("parse type catalog fixture")
    }

    #[test]
    fn checked_catalog_exposes_exact_type_shape_and_variant_order() {
        let statements = parse(
            r#"
# Base = First | Second
# Wrapped(a) = Empty | Present(value: a)
# Combined = Base | Third
# Evolving = Initial
# Evolving WHEN True -> Later

# Case(flag: Base) {
    | current() -> flag
}
"#,
        );

        let catalog = TypeCatalog::collect_checked(&statements, None)
            .expect("collect complete local catalog");
        assert_eq!(catalog.type_parameters("Wrapped").unwrap(), vec!["a"]);
        assert_eq!(
            catalog.type_parameters("Case").unwrap(),
            Vec::<String>::new()
        );
        assert!(catalog.is_rule_scope("Case"));
        assert!(!catalog.is_rule_scope("Wrapped"));
        assert_eq!(
            catalog
                .resolved_variants("Combined")
                .unwrap()
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            vec!["First", "Second", "Third"]
        );
        assert_eq!(
            catalog.resolved_variants("Evolving").unwrap_err(),
            "type `Evolving` has conditional variants and cannot define a complete static universe"
        );
        assert_eq!(
            catalog.type_parameters("Missing").unwrap_err(),
            "unknown type `Missing`"
        );

        let mut calculation_compatibility_catalog = TypeCatalog::default();
        calculation_compatibility_catalog.collect(&statements, None);
        assert_eq!(
            calculation_compatibility_catalog
                .resolved_variants("Evolving")
                .unwrap()
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Initial"]
        );
    }

    #[test]
    fn exact_variant_resolution_rejects_forward_includes_and_except_sources() {
        let statements = parse(
            r#"
# Combined = Base | Third
# Base = First | Second
# Trimmed = Later EXCEPT Skip
# Later = Keep | Skip
"#,
        );
        let catalog = TypeCatalog::collect_checked(&statements, None)
            .expect("collect forward-reference catalog");
        for name in ["Combined", "Trimmed"] {
            let error = catalog
                .resolved_variants(name)
                .expect_err("forward type composition must fail closed");
            assert!(
                error.contains("outside its already initialized declaration prefix"),
                "{error}"
            );
        }

        let mut compatibility_catalog = TypeCatalog::default();
        compatibility_catalog.collect(&statements, None);
        assert_eq!(
            compatibility_catalog
                .resolved_variants("Combined")
                .expect("legacy calculation catalog keeps completed-map resolution")
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            vec!["First", "Second", "Third"]
        );
    }

    #[test]
    fn exact_variant_resolution_has_a_total_expansion_budget() {
        let statements = parse(
            r#"
# T0 = A | B
# T1Left = T0
# T1Right = T0
# T2 = T1Left | T1Right
"#,
        );
        let catalog = TypeCatalog::collect_checked(&statements, None)
            .expect("collect inclusion-diamond catalog");
        let mut budget = Some(8);
        let error = catalog
            .resolved_variants_inner("T2", &mut BTreeSet::new(), &mut budget)
            .expect_err("diamond expansion must exhaust the test budget");
        assert!(error.contains("variant-expansion work limit"), "{error}");
    }

    #[test]
    fn exact_variant_resolution_has_a_depth_limit() {
        let mut source = "# T0 = Leaf\n".to_string();
        for index in 1..=70 {
            source.push_str(&format!("# T{} = T{}\n", index, index - 1));
        }
        let statements = parse(&source);
        let catalog =
            TypeCatalog::collect_checked(&statements, None).expect("collect deep inclusion chain");
        let error = catalog
            .resolved_variants("T70")
            .expect_err("deep inclusion chains must fail before stack recursion");
        assert!(error.contains("exact resolution depth limit"), "{error}");

        let mut compatibility_catalog = TypeCatalog::default();
        compatibility_catalog.collect(&statements, None);
        assert_eq!(
            compatibility_catalog
                .resolved_variants("T70")
                .expect("legacy calculation catalog keeps its prior deep-chain behavior")
                .into_iter()
                .map(|variant| variant.name)
                .collect::<Vec<_>>(),
            vec!["Leaf"]
        );
    }

    #[test]
    fn checked_catalog_collects_nested_flat_import_types() {
        let statements = parse("@ import ./import_mesh_shared\n");
        let source_dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/differential/corpus/imports"
        );

        let catalog = TypeCatalog::collect_checked(&statements, Some(source_dir))
            .expect("collect nested flat imports");

        assert_eq!(
            catalog
                .resolved_variants("Lane")
                .unwrap()
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Draft", "Review", "Ship"]
        );
    }

    #[test]
    fn checked_catalog_composes_an_earlier_imported_type() {
        let directory = std::env::temp_dir().join(format!(
            "futuruna_exact_type_import_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("create imported-type directory");
        std::fs::write(directory.join("domain.runa"), "# Base = A | B\n")
            .expect("write imported type");
        let statements = parse("@ import ./domain\n# Combined = Base | C\n");
        let catalog =
            TypeCatalog::collect_checked(&statements, Some(directory.to_string_lossy().as_ref()))
                .expect("collect imported-prefix composition");
        std::fs::remove_dir_all(&directory).ok();
        assert_eq!(
            catalog
                .resolved_variants("Combined")
                .expect("resolve imported-prefix composition")
                .into_iter()
                .map(|variant| variant.name)
                .collect::<Vec<_>>(),
            vec!["A", "B", "C"]
        );
    }

    #[test]
    fn checked_catalog_reports_unavailable_and_malformed_flat_imports() {
        let missing = parse("@ import ./type_catalog_fixture_that_does_not_exist\n");
        let errors = match TypeCatalog::collect_checked(&missing, None) {
            Err(errors) => errors,
            Ok(_) => panic!("missing import must fail closed"),
        };
        assert_eq!(
            errors,
            vec!["cannot resolve flat import `./type_catalog_fixture_that_does_not_exist` without a source directory"]
        );

        let source_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/errors");
        let malformed = parse("@ import ./multi_parse_error\n");
        let errors = match TypeCatalog::collect_checked(&malformed, Some(source_dir)) {
            Err(errors) => errors,
            Ok(_) => panic!("malformed import must fail closed"),
        };
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("cannot parse flat imported module `./multi_parse_error`"),
            "unexpected diagnostic: {}",
            errors[0]
        );
    }

    #[test]
    fn type_substitution_rewrites_nested_type_parameters() {
        let ty = Ty::App(
            Box::new(Ty::Name("Result".to_string())),
            vec![
                Ty::Optional(Box::new(Ty::Var("value".to_string()))),
                Ty::Var("error".to_string()),
            ],
        );
        let substitutions = BTreeMap::from([
            ("value".to_string(), Ty::Name("Bool".to_string())),
            ("error".to_string(), Ty::Name("UnitError".to_string())),
        ]);

        assert_eq!(
            substitute_type(&ty, &substitutions).to_string(),
            "Result(Bool?, UnitError)"
        );
    }
}

#[derive(Clone)]
struct EndpointCandidate {
    name: String,
    label: Option<String>,
    parameter: String,
    input: Ty,
    output: Ty,
}

fn calculation_label_from_args(args: &[Expr]) -> Result<Option<String>, String> {
    match args {
        [] => Ok(None),
        [arg] => match &arg.kind {
            ExprKind::Lit(Literal::Str(label)) if !label.trim().is_empty() => {
                Ok(Some(label.clone()))
            }
            ExprKind::Lit(Literal::Str(_)) => {
                Err("`@ calculate` label must not be empty".to_string())
            }
            _ => Err("`@ calculate` accepts one human-readable string label".to_string()),
        },
        _ => Err("`@ calculate` accepts at most one human-readable string label".to_string()),
    }
}

/// Extract and validate every `@ calculate` contract in a parsed program.
///
/// A program with no marker is valid and returns an empty list. CLI selection
/// turns that empty list into a user-facing "no calculation" diagnostic.
pub fn extract_calculation_contracts(
    stmts: &[Stmt],
    source: &str,
    source_dir: Option<String>,
) -> Result<Vec<CalculationContract>, Vec<Diagnostic>> {
    if !stmts
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Annot(name, _) if name == "calculate"))
    {
        return Ok(Vec::new());
    }
    let mut checker = TypeChecker::new();
    checker.source_dir = source_dir.clone();
    checker.source_text = source.to_string();
    checker.collect_declarations(stmts);
    checker.infer_rule_return_types(stmts);
    checker.infer_top_level_binding_types(stmts);

    extract_calculation_artifacts_with_checker(stmts, source, source_dir, &checker, false)
        .map(|artifacts| artifacts.contracts)
}

#[derive(Default)]
pub(crate) struct CalculationExtractionArtifacts {
    pub contracts: Vec<CalculationContract>,
    pub compile_time_metadata_bindings: BTreeSet<String>,
}

pub(crate) fn extract_calculation_contracts_with_checker(
    stmts: &[Stmt],
    source: &str,
    source_dir: Option<String>,
    checker: &TypeChecker,
) -> Result<Vec<CalculationContract>, Vec<Diagnostic>> {
    extract_calculation_artifacts_with_checker(stmts, source, source_dir, checker, false)
        .map(|artifacts| artifacts.contracts)
}

pub(crate) fn extract_calculation_artifacts_with_checker(
    stmts: &[Stmt],
    source: &str,
    source_dir: Option<String>,
    checker: &TypeChecker,
    collect_all_imported_metadata_bindings: bool,
) -> Result<CalculationExtractionArtifacts, Vec<Diagnostic>> {
    if !stmts
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Annot(name, _) if name == "calculate"))
    {
        return Ok(CalculationExtractionArtifacts::default());
    }
    #[cfg(test)]
    CONTRACT_EXTRACTION_COUNT.with(|count| count.set(count.get() + 1));

    let mut catalog = TypeCatalog::default();
    catalog.collect(stmts, source_dir.as_deref());

    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();
    let mut pending_markers = Vec::new();
    let callables = host_effects::LocalCallables::new(stmts);

    for stmt in stmts {
        if let Stmt::Annot(name, args) = stmt {
            if name == "calculate" {
                match calculation_label_from_args(args) {
                    Ok(label) => pending_markers.push(label),
                    Err(message) => {
                        diagnostics.push(Diagnostic::error(message));
                        pending_markers.push(None);
                    }
                }
            }
            continue;
        }

        if pending_markers.is_empty() {
            continue;
        }
        if pending_markers.len() > 1 {
            diagnostics.push(Diagnostic::error(
                "duplicate `@ calculate` marker before the same callable",
            ));
        }
        let label = pending_markers.first().cloned().flatten();
        pending_markers.clear();

        match endpoint_from_stmt(stmt, stmts, checker) {
            Ok(mut candidate) => {
                if let Some(effect) =
                    callables.first_reachable_effect(&[AstChild::Stmt(stmt)], true)
                {
                    diagnostics.push(Diagnostic::error_at(
                        effect.span,
                        format!(
                            "calculation `{}` performs the effect {}; calculations must be pure, so pass external data in through the input",
                            candidate.name,
                            host_effects::reached_effect_location(&effect)
                        ),
                    ));
                }
                candidate.label = label;
                candidates.push(candidate);
            }
            Err(message) => diagnostics.push(Diagnostic::error(&message)),
        }
    }

    if !pending_markers.is_empty() {
        diagnostics.push(Diagnostic::error(
            "`@ calculate` must be followed by a top-level typed rule or function",
        ));
    }

    let mut seen_entries = BTreeSet::new();
    for candidate in &candidates {
        if !seen_entries.insert(candidate.name.clone()) {
            diagnostics.push(Diagnostic::error(format!(
                "calculation entry `{}` is marked more than once",
                candidate.name
            )));
        }
        validate_endpoint_type(&candidate.input, &catalog, true, &mut diagnostics);
        validate_endpoint_type(&candidate.output, &catalog, false, &mut diagnostics);
        if !input_is_domain_object(&candidate.input, &catalog) {
            diagnostics.push(Diagnostic::error(format!(
                "calculation `{}` input must be one named Futuruna domain type, got `{}`",
                candidate.name, candidate.input
            )));
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let calculation_labels = candidates
        .iter()
        .map(|candidate| candidate.name.clone())
        .collect::<BTreeSet<_>>();
    let mut metadata_labels = calculation_labels.clone();
    for candidate in &candidates {
        collect_reachable_type_names(&candidate.input, &catalog, &mut metadata_labels);
    }
    let meta_index = scan_meta_comments_with_imported_labels_and_checker(
        source,
        stmts,
        source_dir.clone(),
        &metadata_labels,
        checker,
    );
    let mut compile_time_metadata_bindings = meta_index.resolved_binding_names.clone();
    if collect_all_imported_metadata_bindings {
        compile_time_metadata_bindings.extend(
            collect_imported_resolved_meta_binding_names_with_checker(
                stmts,
                source_dir.as_deref(),
                checker,
            ),
        );
    }
    let mut contracts = Vec::new();
    let mut metadata_diagnostics = Vec::new();
    for candidate in candidates {
        let input = ty_to_contract_ref(&candidate.input, false).expect("validated input type");
        let output = ty_to_contract_ref(&candidate.output, false).expect("validated output type");
        let mut reachable = BTreeSet::new();
        collect_reachable_type_names(&candidate.input, &catalog, &mut reachable);
        collect_reachable_type_names(&candidate.output, &catalog, &mut reachable);
        let mut definitions = Vec::new();
        for name in &reachable {
            if let Some(definition) = contract_definition(name, &catalog) {
                definitions.push(definition);
            }
        }
        definitions.sort_by(|left, right| left.name.cmp(&right.name));

        let mut relevant = reachable;
        relevant.insert(candidate.name.clone());
        let metadata = contract_metadata(&meta_index, &relevant);
        let mut contract = CalculationContract {
            schema: CONTRACT_SCHEMA.to_string(),
            schema_version: 1,
            entry: candidate.name,
            label: candidate.label,
            parameter: candidate.parameter,
            input,
            output,
            definitions,
            metadata,
            field_metadata: Vec::new(),
            schema_hash: String::new(),
        };
        match contract_field_metadata(&meta_index, &contract) {
            Ok(field_metadata) => contract.field_metadata = field_metadata,
            Err(mut diagnostics) => metadata_diagnostics.append(&mut diagnostics),
        }
        contracts.push(contract.finish_hash());
    }
    metadata_diagnostics.extend(entry_provider_diagnostics(&contracts));
    if !metadata_diagnostics.is_empty() {
        return Err(metadata_diagnostics);
    }
    contracts.sort_by(|left, right| left.entry.cmp(&right.entry));
    Ok(CalculationExtractionArtifacts {
        contracts,
        compile_time_metadata_bindings,
    })
}

#[cfg(test)]
pub(crate) fn reset_contract_extraction_count() {
    CONTRACT_EXTRACTION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn contract_extraction_count() -> usize {
    CONTRACT_EXTRACTION_COUNT.with(std::cell::Cell::get)
}

fn endpoint_from_stmt(
    stmt: &Stmt,
    all_stmts: &[Stmt],
    checker: &TypeChecker,
) -> Result<EndpointCandidate, String> {
    match stmt {
        Stmt::Defn(Defn::Fn {
            name,
            params,
            ret_ty,
            effects,
            body,
        }) => {
            if params.len() != 1 {
                return Err(format!(
                    "calculation function `{}` must have exactly one input parameter",
                    name
                ));
            }
            let input = params[0].ty.clone().ok_or_else(|| {
                format!(
                    "calculation function `{}` parameter `{}` needs an explicit type",
                    name, params[0].name
                )
            })?;
            let output = ret_ty.clone().ok_or_else(|| {
                format!(
                    "calculation function `{}` needs an explicit result type",
                    name
                )
            })?;
            if params[0].inout {
                return Err(format!(
                    "calculation function `{}` input cannot be `inout`",
                    name
                ));
            }
            if !effects.is_empty() {
                return Err(format!(
                    "calculation function `{}` declares effects ({}); calculation boundaries must be pure",
                    name,
                    effects.join(", ")
                ));
            }
            Ok(EndpointCandidate {
                name: name.clone(),
                label: None,
                parameter: params[0].name.clone(),
                input,
                output,
            })
        }
        Stmt::Rule(marked_rule) => {
            let (name, args) = rule_head(marked_rule).ok_or_else(|| {
                "`@ calculate` cannot mark a reactive scope; mark a typed value rule".to_string()
            })?;
            if args.len() != 1 {
                return Err(format!(
                    "calculation rule `{}` must have exactly one input parameter",
                    name
                ));
            }
            let (inner, type_name) = typed_rule_argument(&args[0]).ok_or_else(|| {
                format!(
                    "calculation rule `{}` input needs an explicit type, for example `| {}(input: Input) -> ...`",
                    name, name
                )
            })?;
            let ExprKind::Var(parameter) = &inner.kind else {
                return Err(format!(
                    "calculation rule `{}` input must be one named variable",
                    name
                ));
            };
            let input = parse_type_annotation(type_name).map_err(|error| {
                format!(
                    "calculation rule `{}` has invalid input type: {}",
                    name, error
                )
            })?;

            let mut output_names = BTreeSet::new();
            for rule in all_stmts.iter().filter_map(|stmt| match stmt {
                Stmt::Rule(rule)
                    if rule_head(rule).is_some_and(|(rule_name, _)| rule_name == name) =>
                {
                    Some(rule)
                }
                _ => None,
            }) {
                if let Some(output_name) = inferred_rule_output(rule, checker) {
                    output_names.insert(output_name);
                }
            }
            if output_names.is_empty() {
                return Err(format!(
                    "calculation rule `{}` result type could not be inferred; return a concrete typed value",
                    name
                ));
            }
            if output_names.len() != 1 {
                return Err(format!(
                    "calculation rule `{}` has conflicting result types: {}",
                    name,
                    output_names.into_iter().collect::<Vec<_>>().join(", ")
                ));
            }
            let output_name = output_names.into_iter().next().expect("one output type");
            let output = parse_type_annotation(&output_name).map_err(|error| {
                format!(
                    "calculation rule `{}` inferred unsupported result type `{}`: {}",
                    name, output_name, error
                )
            })?;
            Ok(EndpointCandidate {
                name,
                label: None,
                parameter: parameter.clone(),
                input,
                output,
            })
        }
        _ => {
            Err("`@ calculate` must be followed by a top-level typed rule or function".to_string())
        }
    }
}

fn rule_head(rule: &Rule) -> Option<(String, Vec<Expr>)> {
    let head = match rule {
        Rule::Clause { head, .. } | Rule::Default { head, .. } | Rule::Exception { head, .. } => {
            head
        }
        Rule::ReactiveScope { .. } => return None,
    };
    match &head.kind {
        ExprKind::App(function, args) => match &function.kind {
            ExprKind::Var(name) => Some((name.clone(), args.clone())),
            _ => None,
        },
        ExprKind::Var(name) => Some((name.clone(), Vec::new())),
        _ => None,
    }
}

fn typed_rule_argument(argument: &Expr) -> Option<(&Expr, &str)> {
    let ExprKind::App(function, arguments) = &argument.kind else {
        return None;
    };
    if !matches!(&function.kind, ExprKind::Var(name) if name == "__typed") || arguments.len() != 2 {
        return None;
    }
    let ExprKind::Var(type_name) = &arguments[1].kind else {
        return None;
    };
    Some((&arguments[0], type_name))
}

fn inferred_rule_output(rule: &Rule, checker: &TypeChecker) -> Option<String> {
    match rule {
        Rule::Clause { head, body: None } => Some("Bool".to_string()),
        Rule::Clause {
            head,
            body: Some(body),
        }
        | Rule::Default {
            head, value: body, ..
        }
        | Rule::Exception {
            head, value: body, ..
        } => {
            let locals = checker.rule_head_local_types(head);
            checker.infer_expr_type_name_with_locals(body, &locals)
        }
        Rule::ReactiveScope { .. } => None,
    }
}

fn primitive_name(name: &str) -> Option<&'static str> {
    match name {
        "Int" => Some("Int"),
        "Float" => Some("Float"),
        "Bool" => Some("Bool"),
        "String" => Some("String"),
        "Char" => Some("Char"),
        _ => None,
    }
}

fn is_builtin_runtime_type_name(name: &str) -> bool {
    primitive_name(name).is_some() || matches!(name, "Nat" | "Unit" | "Any" | "_")
}

fn is_exact_runtime_type_name(name: &str) -> bool {
    is_builtin_runtime_type_name(name) || matches!(name, "Tuple" | "Option")
}

/// `Option` is source-shadowable, but the interpreter always seeds `Some` and
/// `None` before loading declarations. Exact exploration can therefore trust
/// only the canonical prelude declaration; a different user/imported shape
/// would omit runtime inhabitants from `values(Option(T))`.
fn is_canonical_exact_runtime_declaration(declaration: &TypeDecl) -> bool {
    let TypeDecl::ADT {
        name, except_from, ..
    } = declaration
    else {
        return false;
    };
    if name != "Option" || except_from.is_some() {
        return false;
    }
    let declaration_hash = content_hash_type(declaration);
    parse_prelude().iter().any(|statement| {
        matches!(
            statement,
            Stmt::TypeDecl(canonical @ TypeDecl::ADT { name, .. })
                if name == "Option" && content_hash_type(canonical) == declaration_hash
        )
    })
}

fn ty_to_contract_ref(ty: &Ty, allow_parameters: bool) -> Result<CalculationTypeRef, String> {
    match ty {
        Ty::Name(name) => {
            if let Some(name) = primitive_name(name) {
                Ok(CalculationTypeRef::Primitive {
                    name: name.to_string(),
                })
            } else {
                Ok(CalculationTypeRef::Named {
                    name: name.clone(),
                    arguments: Vec::new(),
                })
            }
        }
        Ty::App(base, arguments) => {
            let Ty::Name(name) = base.as_ref() else {
                return Err(format!("unsupported applied type `{}`", ty));
            };
            let arguments = arguments
                .iter()
                .map(|argument| ty_to_contract_ref(argument, allow_parameters))
                .collect::<Result<Vec<_>, _>>()?;
            match (name.as_str(), arguments.as_slice()) {
                ("List", [item]) => Ok(CalculationTypeRef::List {
                    item: Box::new(item.clone()),
                }),
                ("Option", [item]) => Ok(CalculationTypeRef::Optional {
                    item: Box::new(item.clone()),
                }),
                ("Map", [key, value]) => Ok(CalculationTypeRef::Map {
                    key: Box::new(key.clone()),
                    value: Box::new(value.clone()),
                }),
                ("Set", [item]) => Ok(CalculationTypeRef::Set {
                    item: Box::new(item.clone()),
                }),
                _ => Ok(CalculationTypeRef::Named {
                    name: name.clone(),
                    arguments,
                }),
            }
        }
        Ty::Optional(item) => Ok(CalculationTypeRef::Optional {
            item: Box::new(ty_to_contract_ref(item, allow_parameters)?),
        }),
        Ty::Var(name) if allow_parameters => {
            Ok(CalculationTypeRef::TypeParameter { name: name.clone() })
        }
        Ty::Unit => Ok(CalculationTypeRef::Unit),
        Ty::Var(name) => Err(format!("open type parameter `{}`", name)),
        Ty::Arrow(_, _) => Err(format!("function type `{}`", ty)),
        Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) => Err(format!("reference type `{}`", ty)),
        Ty::Hole => Err("type hole `_`".to_string()),
    }
}

fn validate_endpoint_type(
    ty: &Ty,
    catalog: &TypeCatalog,
    is_input: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Err(reason) = validate_type_inner(ty, catalog, &mut BTreeSet::new()) {
        diagnostics.push(Diagnostic::error(format!(
            "calculation {} type `{}` is not serializable: {}",
            if is_input { "input" } else { "result" },
            ty,
            reason
        )));
    }
}

fn validate_type_inner(
    ty: &Ty,
    catalog: &TypeCatalog,
    active: &mut BTreeSet<String>,
) -> Result<(), String> {
    match ty {
        Ty::Name(name) if primitive_name(name).is_some() => Ok(()),
        Ty::Name(name) => validate_named_type(name, &[], catalog, active),
        Ty::App(base, arguments) => {
            let Ty::Name(name) = base.as_ref() else {
                return Err("applied type constructor is not named".to_string());
            };
            match name.as_str() {
                "List" | "Option" | "Set" if arguments.len() == 1 => {
                    validate_type_inner(&arguments[0], catalog, active)
                }
                "Map" if arguments.len() == 2 => {
                    if !matches!(&arguments[0], Ty::Name(name) if name == "String") {
                        return Err(
                            "Map keys must be `String` in calculation contracts".to_string()
                        );
                    }
                    validate_type_inner(&arguments[1], catalog, active)
                }
                "List" | "Option" | "Set" | "Map" => {
                    Err(format!("`{}` has the wrong number of type arguments", name))
                }
                _ => validate_named_type(name, arguments, catalog, active),
            }
        }
        Ty::Optional(inner) => validate_type_inner(inner, catalog, active),
        Ty::Unit => Ok(()),
        Ty::Var(name) => Err(format!("open type parameter `{}`", name)),
        Ty::Arrow(_, _) => Err("function values cannot cross a calculation boundary".to_string()),
        Ty::Ref(_) | Ty::MutRef(_) | Ty::Shared(_) => {
            Err("references cannot cross a calculation boundary".to_string())
        }
        Ty::Hole => Err("type holes cannot cross a calculation boundary".to_string()),
    }
}

fn validate_named_type(
    name: &str,
    arguments: &[Ty],
    catalog: &TypeCatalog,
    active: &mut BTreeSet<String>,
) -> Result<(), String> {
    let definition = catalog
        .types
        .get(name)
        .ok_or_else(|| format!("unknown named type `{}`", name))?;
    let parameters = match definition {
        CatalogType::Adt { parameters, .. } => parameters.clone(),
        CatalogType::RuleScope { .. } => {
            if !arguments.is_empty() {
                return Err(format!("rule scope `{}` is not generic", name));
            }
            Vec::new()
        }
    };
    if parameters.len() != arguments.len() {
        return Err(format!(
            "type `{}` expects {} type argument{} but got {}",
            name,
            parameters.len(),
            if parameters.len() == 1 { "" } else { "s" },
            arguments.len()
        ));
    }
    for argument in arguments {
        validate_type_inner(argument, catalog, active)?;
    }
    if !active.insert(name.to_string()) {
        return Ok(());
    }
    let substitutions: BTreeMap<String, Ty> = parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect();
    for variant in catalog.resolved_variants(name)? {
        for field in variant.fields {
            let field_ty = substitute_type(&field.ty, &substitutions);
            validate_type_inner(&field_ty, catalog, active)?;
        }
    }
    active.remove(name);
    Ok(())
}

fn input_is_domain_object(ty: &Ty, catalog: &TypeCatalog) -> bool {
    match ty {
        Ty::Name(name) => catalog.types.contains_key(name),
        Ty::App(base, _) => {
            matches!(base.as_ref(), Ty::Name(name) if catalog.types.contains_key(name))
        }
        _ => false,
    }
}

pub(crate) fn substitute_type(ty: &Ty, substitutions: &BTreeMap<String, Ty>) -> Ty {
    match ty {
        Ty::Var(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Ty::App(base, arguments) => Ty::App(
            Box::new(substitute_type(base, substitutions)),
            arguments
                .iter()
                .map(|argument| substitute_type(argument, substitutions))
                .collect(),
        ),
        Ty::Arrow(input, output) => Ty::Arrow(
            Box::new(substitute_type(input, substitutions)),
            Box::new(substitute_type(output, substitutions)),
        ),
        Ty::Ref(inner) => Ty::Ref(Box::new(substitute_type(inner, substitutions))),
        Ty::MutRef(inner) => Ty::MutRef(Box::new(substitute_type(inner, substitutions))),
        Ty::Shared(inner) => Ty::Shared(Box::new(substitute_type(inner, substitutions))),
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_type(inner, substitutions))),
        _ => ty.clone(),
    }
}

fn collect_reachable_type_names(ty: &Ty, catalog: &TypeCatalog, reachable: &mut BTreeSet<String>) {
    match ty {
        Ty::Name(name) => collect_named_reachable(name, &[], catalog, reachable),
        Ty::App(base, arguments) => {
            for argument in arguments {
                collect_reachable_type_names(argument, catalog, reachable);
            }
            if let Ty::Name(name) = base.as_ref() {
                if !matches!(name.as_str(), "List" | "Option" | "Map" | "Set") {
                    collect_named_reachable(name, arguments, catalog, reachable);
                }
            }
        }
        Ty::Optional(inner) | Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
            collect_reachable_type_names(inner, catalog, reachable)
        }
        Ty::Arrow(input, output) => {
            collect_reachable_type_names(input, catalog, reachable);
            collect_reachable_type_names(output, catalog, reachable);
        }
        Ty::Var(_) | Ty::Unit | Ty::Hole => {}
    }
}

fn collect_named_reachable(
    name: &str,
    arguments: &[Ty],
    catalog: &TypeCatalog,
    reachable: &mut BTreeSet<String>,
) {
    if !catalog.types.contains_key(name) || !reachable.insert(name.to_string()) {
        return;
    }
    let parameters = match catalog.types.get(name) {
        Some(CatalogType::Adt { parameters, .. }) => parameters.clone(),
        _ => Vec::new(),
    };
    let substitutions: BTreeMap<String, Ty> = parameters
        .into_iter()
        .zip(arguments.iter().cloned())
        .collect();
    if let Ok(variants) = catalog.resolved_variants(name) {
        for variant in variants {
            for field in variant.fields {
                collect_reachable_type_names(
                    &substitute_type(&field.ty, &substitutions),
                    catalog,
                    reachable,
                );
            }
        }
    }
}

fn contract_definition(name: &str, catalog: &TypeCatalog) -> Option<CalculationTypeDefinition> {
    let catalog_type = catalog.types.get(name)?;
    let (parameters, kind) = match catalog_type {
        CatalogType::Adt { parameters, .. } => (parameters.clone(), "adt".to_string()),
        CatalogType::RuleScope { .. } => (Vec::new(), "rule_scope".to_string()),
    };
    let variants = catalog
        .resolved_variants(name)
        .ok()?
        .into_iter()
        .map(|variant| CalculationVariant {
            name: variant.name,
            positional: variant.positional,
            fields: variant
                .fields
                .into_iter()
                .filter_map(|field| {
                    ty_to_contract_ref(&field.ty, true)
                        .ok()
                        .map(|ty| CalculationField {
                            name: field.name,
                            ty,
                        })
                })
                .collect(),
        })
        .collect();
    Some(CalculationTypeDefinition {
        name: name.to_string(),
        parameters,
        kind,
        variants,
    })
}

fn contract_metadata(
    index: &MetaIndex,
    relevant_names: &BTreeSet<String>,
) -> Vec<CalculationMetadataReference> {
    let relevant_labels: BTreeSet<String> = index
        .spans
        .iter()
        .filter(|span| {
            span.symbols
                .iter()
                .any(|symbol| relevant_names.contains(&symbol.name))
        })
        .map(|span| span.label.clone())
        .collect();

    let mut result = Vec::new();
    for anchor in &index.anchors {
        let label_relevant = relevant_labels.contains(&anchor.label);
        for reference in &anchor.references {
            let type_relevant = reference
                .qualified_type
                .as_ref()
                .is_some_and(|name| relevant_names.contains(name))
                || reference
                    .typed_values()
                    .iter()
                    .any(|value| relevant_names.contains(value.qualified_type));
            if !label_relevant && !type_relevant {
                continue;
            }
            let symbols = index
                .spans_for_label(&anchor.label)
                .into_iter()
                .flat_map(|span| span.symbols.iter().map(|symbol| symbol.name.clone()))
                .collect();
            result.extend(calculation_metadata_references(anchor, reference, symbols));
        }
    }
    result
}

fn calculation_metadata_references(
    anchor: &MetaAnchor,
    reference: &MetaReference,
    symbols: Vec<String>,
) -> Vec<CalculationMetadataReference> {
    let attachments = reference.attachments();
    if reference.role == "meta" && !attachments.is_empty() {
        return attachments
            .into_iter()
            .filter(|attachment| attachment.role != "field")
            .map(|attachment| {
                calculation_metadata_attachment_reference(
                    anchor,
                    reference,
                    symbols.clone(),
                    attachment,
                )
            })
            .collect();
    }
    vec![calculation_metadata_reference(anchor, reference, symbols)]
}

fn calculation_metadata_attachment_reference(
    anchor: &MetaAnchor,
    reference: &MetaReference,
    symbols: Vec<String>,
    attachment: MetaRoleValue<'_>,
) -> CalculationMetadataReference {
    CalculationMetadataReference {
        label: anchor.label.clone(),
        role: attachment.role,
        binding: attachment
            .binding_name
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "{}{}",
                    reference.binding_name,
                    attachment.value_path.trim_start_matches('$')
                )
            }),
        attachment_path: Some(format!(
            "{}{}",
            reference.binding_name,
            attachment.attachment_path.trim_start_matches('$')
        )),
        qualified_type: attachment.value.qualified_type().map(str::to_string),
        value: Some(attachment.value.to_source()),
        data: Some(calculation_meta_value_json(attachment.value)),
        text: anchor.text.clone(),
        symbols,
    }
}

fn calculation_metadata_reference(
    anchor: &MetaAnchor,
    reference: &MetaReference,
    symbols: Vec<String>,
) -> CalculationMetadataReference {
    CalculationMetadataReference {
        label: anchor.label.clone(),
        role: reference.role.clone(),
        binding: reference.binding_name.clone(),
        attachment_path: None,
        qualified_type: reference.qualified_type.clone(),
        value: reference.static_value.clone(),
        data: reference
            .static_data
            .as_ref()
            .map(calculation_meta_value_json),
        text: anchor.text.clone(),
        symbols,
    }
}

fn calculation_meta_value_json(value: &MetaValue) -> JsonValue {
    match value {
        MetaValue::Integer(value) => serde_json::json!({
            "kind": "integer",
            "value": value,
        }),
        MetaValue::Float(value) => serde_json::json!({
            "kind": "float",
            "value": value.parse::<f64>().ok(),
            "source": value,
        }),
        MetaValue::String(value) => serde_json::json!({
            "kind": "string",
            "value": value,
        }),
        MetaValue::Character(value) => serde_json::json!({
            "kind": "character",
            "value": value.to_string(),
        }),
        MetaValue::Boolean(value) => serde_json::json!({
            "kind": "boolean",
            "value": value,
        }),
        MetaValue::Unit => serde_json::json!({
            "kind": "unit",
        }),
        MetaValue::Constructor {
            name,
            qualified_type,
            applied,
            arguments,
        } => serde_json::json!({
            "kind": "constructor",
            "name": name,
            "type": qualified_type,
            "applied": applied,
            "arguments": arguments
                .iter()
                .map(|argument| serde_json::json!({
                    "field": argument.field,
                    "binding": argument.binding_name,
                    "value": calculation_meta_value_json(&argument.value),
                }))
                .collect::<Vec<_>>(),
        }),
        MetaValue::List(items) => serde_json::json!({
            "kind": "list",
            "items": items
                .iter()
                .map(calculation_meta_value_json)
                .collect::<Vec<_>>(),
        }),
        MetaValue::Tuple(items) => serde_json::json!({
            "kind": "tuple",
            "items": items
                .iter()
                .map(calculation_meta_value_json)
                .collect::<Vec<_>>(),
        }),
        MetaValue::Unary { operator, value } => serde_json::json!({
            "kind": "unary",
            "operator": operator,
            "value": calculation_meta_value_json(value),
        }),
    }
}

/// A field another entry provides must have that entry's result type, so a
/// client can write the entry's result at the field's path unchanged.
fn entry_provider_diagnostics(contracts: &[CalculationContract]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for contract in contracts {
        for field in &contract.field_metadata {
            let Some(provider) = &field.provided_by else {
                continue;
            };
            let Some(providing) = contracts
                .iter()
                .find(|candidate| candidate.entry == provider.entry)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "calculation `{}` field `{}` is provided by `{}`, which is not a calculation entry of this program",
                    contract.entry, field.path, provider.entry
                )));
                continue;
            };
            match contract_type_at_path(contract, &field.path) {
                Some(ty) if ty == providing.output => {}
                Some(ty) => diagnostics.push(Diagnostic::error(format!(
                    "calculation `{}` field `{}` has type `{}`, but its provider `{}` returns `{}`",
                    contract.entry,
                    field.path,
                    ty.display_name(),
                    provider.entry,
                    providing.output.display_name()
                ))),
                None => diagnostics.push(Diagnostic::error(format!(
                    "calculation `{}` field `{}` provided by `{}` is not a single input field",
                    contract.entry, field.path, provider.entry
                ))),
            }
        }
    }
    diagnostics
}

/// The type of the input field at a canonical path (list, set, optional and
/// map values are transparent; a sum type's payload is reached through its
/// variant name).
fn contract_type_at_path(contract: &CalculationContract, path: &str) -> Option<CalculationTypeRef> {
    let mut ty = contract.input.clone();
    let mut substitutions = BTreeMap::new();
    let mut segments = path.split('.').peekable();
    while let Some(segment) = segments.next() {
        loop {
            ty = substitute_contract_type(&ty, &substitutions);
            match ty {
                CalculationTypeRef::List { item }
                | CalculationTypeRef::Set { item }
                | CalculationTypeRef::Optional { item }
                | CalculationTypeRef::Map { value: item, .. } => ty = *item,
                _ => break,
            }
        }
        let CalculationTypeRef::Named { name, arguments } = &ty else {
            return None;
        };
        let definition = contract.definition(name)?;
        substitutions = definition_substitutions(definition, arguments);
        let variant = match product_variant(definition) {
            Some(variant) => variant,
            None => {
                let variant = definition.variants.iter().find(|v| v.name == segment)?;
                let field = segments.next()?;
                ty = variant.fields.iter().find(|f| f.name == field)?.ty.clone();
                continue;
            }
        };
        ty = variant
            .fields
            .iter()
            .find(|f| f.name == segment)?
            .ty
            .clone();
    }
    Some(substitute_contract_type(&ty, &substitutions))
}

fn contract_field_metadata(
    index: &MetaIndex,
    contract: &CalculationContract,
) -> Result<Vec<CalculationFieldMetadata>, Vec<Diagnostic>> {
    let layout = contract.input_layout();
    let mut valid_paths = BTreeSet::new();
    let columns = layout.root_columns.iter().chain(
        layout
            .collection_tables
            .iter()
            .flat_map(|table| &table.columns),
    );
    for column in columns {
        valid_paths.insert(column.input_path.clone());
        valid_paths.extend(optional_member_paths(contract, column));
    }
    valid_paths.extend(
        layout
            .collection_tables
            .iter()
            .map(|table| table.path.clone()),
    );

    let type_occurrence_paths = calculation_type_occurrence_paths(contract);
    let input_type_names = type_occurrence_paths
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut relevant_symbols = input_type_names.clone();
    relevant_symbols.insert(contract.entry.clone());
    let span_labels: BTreeSet<String> = index
        .spans
        .iter()
        .filter(|span| {
            span.symbols
                .iter()
                .any(|symbol| relevant_symbols.contains(&symbol.name))
        })
        .map(|span| span.label.clone())
        .collect();
    let mut candidates_by_path: BTreeMap<String, Vec<CalculationFieldMetadataCandidate>> =
        BTreeMap::new();
    let mut diagnostics = Vec::new();

    for anchor in &index.anchors {
        if anchor.label != contract.entry
            && !input_type_names.contains(&anchor.label)
            && !span_labels.contains(&anchor.label)
        {
            continue;
        }
        let field_references = anchor
            .references
            .iter()
            .enumerate()
            .filter(|(_, reference)| reference.role == "field" || reference.role == "meta")
            .map(|(index, reference)| (index, reference, calculation_field_values(reference)))
            .collect::<Vec<_>>();
        let field_reference_indices = field_references
            .iter()
            .filter_map(|(index, _, values)| (!values.is_empty()).then_some(*index))
            .collect::<BTreeSet<_>>();
        let sources = anchor
            .references
            .iter()
            .enumerate()
            .flat_map(|(index, reference)| {
                let nested = calculation_metadata_references(anchor, reference, Vec::new());
                if reference.role == "field" || field_reference_indices.contains(&index) {
                    Vec::new()
                } else {
                    nested
                }
            })
            .collect::<Vec<_>>();
        for (_, reference, field_values) in field_references {
            if field_values.is_empty() {
                if reference.role == "field" {
                    diagnostics.push(Diagnostic::error(format!(
                        "calculation field metadata binding `{}` must be a pure ground named record or contain one or more such records",
                        reference.binding_name
                    )));
                }
                continue;
            }
            let scoped_sources = (reference.role == "meta")
                .then(|| calculation_scoped_metadata_references(anchor, reference))
                .unwrap_or_default();
            for (binding, value_path, value) in field_values {
                let mut field_sources = sources.clone();
                field_sources.extend(
                    scoped_sources
                        .iter()
                        .filter(|(scope_path, _)| meta_value_path_contains(scope_path, &value_path))
                        .map(|(_, source)| source.clone()),
                );
                match calculation_field_metadata_from_value(
                    contract,
                    &binding,
                    value,
                    anchor,
                    &valid_paths,
                    &input_type_names,
                    &type_occurrence_paths,
                    &field_sources,
                ) {
                    Ok(candidates) => {
                        for candidate in candidates {
                            candidates_by_path
                                .entry(candidate.metadata.path.clone())
                                .or_default()
                                .push(candidate);
                        }
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
        }
    }

    let mut result = Vec::new();
    for (path, candidates) in candidates_by_path {
        let highest_specificity = candidates
            .iter()
            .map(|candidate| candidate.specificity)
            .max()
            .expect("metadata path has at least one candidate");
        let mut selected = candidates
            .into_iter()
            .filter(|candidate| candidate.specificity == highest_specificity);
        let first = selected
            .next()
            .expect("highest metadata specificity has a candidate");
        if let Some(second) = selected.next() {
            diagnostics.push(Diagnostic::error(format!(
                "calculation `{}` has duplicate field metadata for `{}` in bindings `{}` and `{}`",
                contract.entry, path, first.metadata.binding, second.metadata.binding
            )));
        } else {
            result.push(first.metadata);
        }
    }

    if diagnostics.is_empty() {
        Ok(result)
    } else {
        Err(diagnostics)
    }
}

#[derive(Debug, Clone)]
struct CalculationFieldMetadataCandidate {
    metadata: CalculationFieldMetadata,
    specificity: u8,
}

fn calculation_field_values(reference: &MetaReference) -> Vec<(String, String, &MetaValue)> {
    reference
        .typed_values()
        .into_iter()
        .filter(|typed_value| calculation_field_value(typed_value.value))
        .map(|typed_value| {
            let binding = if typed_value.value_path == "$" {
                reference.binding_name.clone()
            } else {
                format!(
                    "{}{}",
                    reference.binding_name,
                    typed_value.value_path.trim_start_matches('$')
                )
            };
            (binding, typed_value.value_path, typed_value.value)
        })
        .collect()
}

fn calculation_scoped_metadata_references(
    anchor: &MetaAnchor,
    reference: &MetaReference,
) -> Vec<(String, CalculationMetadataReference)> {
    let typed_values = reference.typed_values();
    reference
        .attachments()
        .into_iter()
        .filter(|attachment| attachment.role != "field")
        .map(|attachment| {
            let scope_path = typed_values
                .iter()
                .filter(|typed_value| {
                    !is_meta_attachment(typed_value.value)
                        && typed_value.value_path != attachment.attachment_path
                        && meta_value_path_contains(
                            &typed_value.value_path,
                            &attachment.attachment_path,
                        )
                })
                .max_by_key(|typed_value| typed_value.value_path.len())
                .map(|typed_value| typed_value.value_path.clone())
                .unwrap_or_else(|| "$".to_string());
            let source = calculation_metadata_attachment_reference(
                anchor,
                reference,
                Vec::new(),
                attachment,
            );
            (scope_path, source)
        })
        .collect()
}

fn is_meta_attachment(value: &MetaValue) -> bool {
    matches!(value, MetaValue::Constructor { name, .. } if name == "MetaAttachment")
}

fn meta_value_path_contains(parent: &str, child: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
}

fn calculation_field_value(value: &MetaValue) -> bool {
    let MetaValue::Constructor {
        applied: true,
        arguments,
        ..
    } = value
    else {
        return false;
    };
    let supported = CALCULATION_FIELD_MEMBERS;
    let fields = arguments
        .iter()
        .filter_map(|argument| argument.field.as_deref())
        .collect::<BTreeSet<_>>();
    arguments.len() == fields.len()
        && fields.contains("path")
        && fields.contains("label")
        && fields.iter().all(|field| supported.contains(field))
}

fn calculation_field_metadata_from_value(
    contract: &CalculationContract,
    binding: &str,
    value: &MetaValue,
    anchor: &MetaAnchor,
    valid_paths: &BTreeSet<String>,
    input_type_names: &BTreeSet<String>,
    type_occurrence_paths: &BTreeMap<String, BTreeSet<String>>,
    sources: &[CalculationMetadataReference],
) -> Result<Vec<CalculationFieldMetadataCandidate>, Diagnostic> {
    let MetaValue::Constructor { arguments, .. } = value else {
        return Err(Diagnostic::error(format!(
            "calculation field metadata binding `{}` must be a pure ground named record",
            binding
        )));
    };
    let mut fields = BTreeMap::new();
    for argument in arguments {
        let Some(name) = argument.field.as_deref() else {
            return Err(Diagnostic::error(format!(
                "calculation field metadata binding `{}` must use named fields",
                binding
            )));
        };
        if fields.insert(name, &argument.value).is_some() {
            return Err(Diagnostic::error(format!(
                "calculation field metadata binding `{}` repeats field `{}`",
                binding, name
            )));
        }
    }
    let supported = CALCULATION_FIELD_MEMBERS;
    if let Some(unknown) = fields
        .keys()
        .find(|field| !supported.contains(field))
        .copied()
    {
        return Err(Diagnostic::error(format!(
            "calculation field metadata binding `{}` has unsupported field `{}`",
            binding, unknown
        )));
    }

    let label = required_meta_string(&fields, "label", binding)?;
    if label.trim().is_empty() {
        return Err(Diagnostic::error(format!(
            "calculation field metadata binding `{}` has an empty label",
            binding
        )));
    }

    let (specificity, paths) = calculation_field_paths_from_value(
        contract,
        binding,
        fields.get("path").ok_or_else(|| {
            Diagnostic::error(format!(
                "calculation field metadata binding `{}` is missing `path`",
                binding
            ))
        })?,
        anchor,
        valid_paths,
        input_type_names,
        type_occurrence_paths,
    )?;
    let question = optional_meta_string(&fields, "question", binding)?;
    let help = optional_meta_string(&fields, "help", binding)?;
    let unit = optional_meta_string(&fields, "unit", binding)?;
    let provided_by = match (
        optional_meta_string(&fields, "entry", binding)?,
        optional_meta_string(&fields, "subject", binding)?,
    ) {
        (Some(entry), Some(subject)) => Some(CalculationFieldProvider { entry, subject }),
        (None, None) => None,
        _ => {
            return Err(Diagnostic::error(format!(
                "calculation field metadata binding `{}` must give both `entry` and `subject`, or neither",
                binding
            )))
        }
    };
    Ok(paths
        .into_iter()
        .map(|path| CalculationFieldMetadataCandidate {
            metadata: CalculationFieldMetadata {
                path,
                label: label.trim().to_string(),
                question: question.clone(),
                help: help.clone(),
                unit: unit.clone(),
                anchor: anchor.label.clone(),
                binding: binding.to_string(),
                sources: sources.to_vec(),
                provided_by: provided_by.clone(),
            },
            specificity,
        })
        .collect())
}

fn calculation_field_paths_from_value(
    contract: &CalculationContract,
    binding: &str,
    value: &MetaValue,
    anchor: &MetaAnchor,
    valid_paths: &BTreeSet<String>,
    input_type_names: &BTreeSet<String>,
    type_occurrence_paths: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(u8, Vec<String>), Diagnostic> {
    if let MetaValue::String(declared_path) = value {
        let path = normalize_calculation_field_path(contract, declared_path, valid_paths)
            .ok_or_else(|| {
                unknown_calculation_field_path(contract, binding, declared_path, valid_paths)
            })?;
        return Ok((2, vec![path]));
    }

    let (root_type, relative_path) = calculation_program_member_path(value).ok_or_else(|| {
        Diagnostic::error(format!(
            "calculation field metadata binding `{}` field `path` must be a string or a structural `refof(Type::member)` reference",
            binding
        ))
    })?;
    if input_type_names.contains(&anchor.label)
        && anchor.label != contract.entry
        && anchor.label != root_type
    {
        return Err(Diagnostic::error(format!(
            "calculation field metadata binding `{}` is anchored to type `{}` but references `{}`",
            binding, anchor.label, root_type
        )));
    }

    let root_input_type = match &contract.input {
        CalculationTypeRef::Named { name, .. } => Some(name.as_str()),
        _ => None,
    };
    let specificity = if root_input_type == Some(root_type.as_str()) {
        2
    } else {
        1
    };
    let paths = type_occurrence_paths
        .get(&root_type)
        .into_iter()
        .flat_map(|paths| paths.iter())
        .map(|prefix| joined_column_path(&prefix, &relative_path))
        .filter_map(|path| canonical_calculation_field_path(&path, valid_paths))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err(unknown_calculation_field_path(
            contract,
            binding,
            &format!("{}::{}", root_type, relative_path.replace('.', "::")),
            valid_paths,
        ));
    }
    Ok((specificity, paths))
}

fn calculation_program_member_path(value: &MetaValue) -> Option<(String, String)> {
    let MetaValue::Constructor {
        name, arguments, ..
    } = value
    else {
        return None;
    };
    if name != "ProgramMemberReference" {
        return None;
    }
    let root_type = meta_named_string_argument(arguments, "root_type")?;
    let path = meta_named_string_argument(arguments, "path")?;
    Some((root_type, path))
}

fn meta_named_string_argument(arguments: &[MetaValueArgument], field: &str) -> Option<String> {
    arguments.iter().find_map(|argument| {
        (argument.field.as_deref() == Some(field))
            .then_some(&argument.value)
            .and_then(|value| match value {
                MetaValue::String(value) => Some(value.clone()),
                _ => None,
            })
    })
}

fn unknown_calculation_field_path(
    contract: &CalculationContract,
    binding: &str,
    declared_path: &str,
    valid_paths: &BTreeSet<String>,
) -> Diagnostic {
    let examples = valid_paths.iter().take(8).cloned().collect::<Vec<_>>();
    Diagnostic::error(format!(
        "calculation field metadata binding `{}` targets unknown input path `{}` for `{}`",
        binding, declared_path, contract.entry
    ))
    .with_note(format!(
        "use an exact canonical path such as {}",
        examples.join(", ")
    ))
}

fn normalize_calculation_field_path(
    contract: &CalculationContract,
    declared: &str,
    valid_paths: &BTreeSet<String>,
) -> Option<String> {
    let declared = declared.trim();
    let mut candidates = vec![declared];
    if let Some(path) = declared.strip_prefix("$.") {
        candidates.push(path);
    }
    if let Some(path) = declared.strip_prefix(&format!("{}.", contract.parameter)) {
        candidates.push(path);
    }
    if let CalculationTypeRef::Named { name, .. } = &contract.input {
        if let Some(path) = declared.strip_prefix(&format!("{}.", name)) {
            candidates.push(path);
        }
    }
    candidates
        .into_iter()
        .find_map(|path| canonical_calculation_field_path(path, valid_paths))
}

fn canonical_calculation_field_path(path: &str, valid_paths: &BTreeSet<String>) -> Option<String> {
    if valid_paths.contains(path) {
        return Some(path.to_string());
    }
    path.strip_suffix(".$variant")
        .filter(|enum_path| valid_paths.contains(*enum_path))
        .map(str::to_string)
}

fn calculation_type_occurrence_paths(
    contract: &CalculationContract,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut paths = BTreeMap::new();
    collect_calculation_type_occurrence_paths(
        &contract.input,
        contract,
        &BTreeMap::new(),
        "",
        &mut BTreeSet::new(),
        &mut paths,
    );
    paths
}

#[allow(clippy::too_many_arguments)]
fn collect_calculation_type_occurrence_paths(
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    active: &mut BTreeSet<String>,
    paths: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let ty = substitute_contract_type(ty, substitutions);
    match ty {
        CalculationTypeRef::Optional { item }
        | CalculationTypeRef::List { item }
        | CalculationTypeRef::Set { item } => collect_calculation_type_occurrence_paths(
            &item,
            contract,
            substitutions,
            path,
            active,
            paths,
        ),
        CalculationTypeRef::Map { value, .. } => collect_calculation_type_occurrence_paths(
            &value,
            contract,
            substitutions,
            path,
            active,
            paths,
        ),
        CalculationTypeRef::Named { name, arguments } => {
            let Some(definition) = contract.definition(&name) else {
                return;
            };
            paths
                .entry(name.clone())
                .or_default()
                .insert(path.to_string());
            if !active.insert(name.clone()) {
                return;
            }
            let local = definition_substitutions(definition, &arguments);
            if let Some(variant) = product_variant(definition) {
                for field in &variant.fields {
                    collect_calculation_type_occurrence_paths(
                        &field.ty,
                        contract,
                        &local,
                        &joined_column_path(path, &field.name),
                        active,
                        paths,
                    );
                }
            } else {
                for variant in &definition.variants {
                    let variant_path = joined_column_path(path, &variant.name);
                    for field in &variant.fields {
                        collect_calculation_type_occurrence_paths(
                            &field.ty,
                            contract,
                            &local,
                            &joined_column_path(&variant_path, &field.name),
                            active,
                            paths,
                        );
                    }
                }
            }
            active.remove(&name);
        }
        CalculationTypeRef::Primitive { .. }
        | CalculationTypeRef::TypeParameter { .. }
        | CalculationTypeRef::Unit => {}
    }
}

/// The canonical member paths inside an optional composite JSON column, as
/// `canonical_input_path` writes them: record fields join with `.`, a sum
/// type's payload sits under its variant name and its tag is `.$variant`,
/// and collection elements and map values keep their container's path.
fn optional_member_paths(
    contract: &CalculationContract,
    column: &CalculationColumn,
) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    if column.encoding != CalculationColumnEncoding::Json
        || !matches!(column.ty, CalculationTypeRef::Optional { .. })
    {
        return paths;
    }
    let base = if column.input_path == "input" {
        ""
    } else {
        column.input_path.as_str()
    };
    collect_member_paths(
        &column.ty,
        contract,
        &BTreeMap::new(),
        base,
        &mut BTreeSet::new(),
        &mut paths,
    );
    paths
}

fn collect_member_paths(
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    active: &mut BTreeSet<String>,
    paths: &mut BTreeSet<String>,
) {
    match substitute_contract_type(ty, substitutions) {
        CalculationTypeRef::Optional { item }
        | CalculationTypeRef::List { item }
        | CalculationTypeRef::Set { item }
        | CalculationTypeRef::Map { value: item, .. } => {
            collect_member_paths(&item, contract, substitutions, path, active, paths)
        }
        CalculationTypeRef::Named { name, arguments } => {
            let Some(definition) = contract.definition(&name) else {
                return;
            };
            if !active.insert(name.clone()) {
                return;
            }
            let local = definition_substitutions(definition, &arguments);
            let product = product_variant(definition).is_some();
            if is_payload_sum(definition) {
                paths.insert(joined_column_path(path, "$variant"));
            }
            for variant in &definition.variants {
                let base = if product {
                    path.to_string()
                } else {
                    joined_column_path(path, &variant.name)
                };
                for field in &variant.fields {
                    let field_path = joined_column_path(&base, &field.name);
                    // As in the layout, a required payload sum is addressed by
                    // its `$variant` tag; an optional one also as a whole.
                    if !payload_sum_type(&substitute_contract_type(&field.ty, &local), contract) {
                        paths.insert(field_path.clone());
                    }
                    collect_member_paths(&field.ty, contract, &local, &field_path, active, paths);
                }
            }
            active.remove(&name);
        }
        CalculationTypeRef::Primitive { .. }
        | CalculationTypeRef::TypeParameter { .. }
        | CalculationTypeRef::Unit => {}
    }
}

fn is_payload_sum(definition: &CalculationTypeDefinition) -> bool {
    product_variant(definition).is_none()
        && definition
            .variants
            .iter()
            .any(|variant| !variant.fields.is_empty())
}

fn payload_sum_type(ty: &CalculationTypeRef, contract: &CalculationContract) -> bool {
    matches!(ty, CalculationTypeRef::Named { name, .. }
        if contract.definition(name).is_some_and(is_payload_sum))
}

fn required_meta_string(
    fields: &BTreeMap<&str, &MetaValue>,
    field: &str,
    binding: &str,
) -> Result<String, Diagnostic> {
    let value = fields.get(field).ok_or_else(|| {
        Diagnostic::error(format!(
            "calculation field metadata binding `{}` is missing `{}`",
            binding, field
        ))
    })?;
    match value {
        MetaValue::String(value) => Ok(value.clone()),
        _ => Err(Diagnostic::error(format!(
            "calculation field metadata binding `{}` field `{}` must be a string",
            binding, field
        ))),
    }
}

fn optional_meta_string(
    fields: &BTreeMap<&str, &MetaValue>,
    field: &str,
    binding: &str,
) -> Result<Option<String>, Diagnostic> {
    let Some(value) = fields.get(field) else {
        return Ok(None);
    };
    let value = match value {
        MetaValue::String(value) => Some(value.clone()),
        MetaValue::Constructor {
            name,
            arguments,
            ..
        } if name == "None" && arguments.is_empty() => None,
        MetaValue::Constructor {
            name,
            arguments,
            ..
        } if name == "Some" && arguments.len() == 1 => match &arguments[0].value {
            MetaValue::String(value) => Some(value.clone()),
            _ => {
                return Err(Diagnostic::error(format!(
                    "calculation field metadata binding `{}` field `{}` must contain a string",
                    binding, field
                )))
            }
        },
        _ => {
            return Err(Diagnostic::error(format!(
                "calculation field metadata binding `{}` field `{}` must be a string or optional string",
                binding, field
            )))
        }
    };
    Ok(value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }))
}

fn definition_substitutions(
    definition: &CalculationTypeDefinition,
    arguments: &[CalculationTypeRef],
) -> BTreeMap<String, CalculationTypeRef> {
    definition
        .parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect()
}

fn substitute_contract_type(
    ty: &CalculationTypeRef,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
) -> CalculationTypeRef {
    match ty {
        CalculationTypeRef::TypeParameter { name } => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        CalculationTypeRef::Named { name, arguments } => CalculationTypeRef::Named {
            name: name.clone(),
            arguments: arguments
                .iter()
                .map(|argument| substitute_contract_type(argument, substitutions))
                .collect(),
        },
        CalculationTypeRef::Optional { item } => CalculationTypeRef::Optional {
            item: Box::new(substitute_contract_type(item, substitutions)),
        },
        CalculationTypeRef::List { item } => CalculationTypeRef::List {
            item: Box::new(substitute_contract_type(item, substitutions)),
        },
        CalculationTypeRef::Map { key, value } => CalculationTypeRef::Map {
            key: Box::new(substitute_contract_type(key, substitutions)),
            value: Box::new(substitute_contract_type(value, substitutions)),
        },
        CalculationTypeRef::Set { item } => CalculationTypeRef::Set {
            item: Box::new(substitute_contract_type(item, substitutions)),
        },
        _ => ty.clone(),
    }
}

/// Every value the user must supply is a placeholder object; only the
/// structure fixed by the contract (product fields, a sole variant) is
/// spelled out. A choice between variants is itself a placeholder.
fn template_value(
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    active: &mut BTreeSet<String>,
) -> JsonValue {
    let ty = substitute_contract_type(ty, substitutions);
    match &ty {
        CalculationTypeRef::Unit => JsonValue::Null,
        CalculationTypeRef::Named { name, arguments } => {
            let Some(definition) = contract.definition(name) else {
                return template_placeholder(ty.display_name());
            };
            if !active.insert(name.clone()) {
                return template_placeholder(ty.display_name());
            }
            let local = definition_substitutions(definition, arguments);
            let value = if let Some(variant) = product_variant(definition) {
                let mut object = JsonMap::new();
                for field in &variant.fields {
                    object.insert(
                        field.name.clone(),
                        template_value(&field.ty, contract, &local, active),
                    );
                }
                JsonValue::Object(object)
            } else if let [variant] = definition.variants.as_slice() {
                variant_template(variant, contract, &local, active)
            } else {
                template_placeholder(format!(
                    "{}: one of {}",
                    ty.display_name(),
                    definition
                        .variants
                        .iter()
                        .map(|variant| variant.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            };
            active.remove(name);
            value
        }
        _ => template_placeholder(ty.display_name()),
    }
}

fn variant_template(
    variant: &CalculationVariant,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    active: &mut BTreeSet<String>,
) -> JsonValue {
    let mut object = JsonMap::new();
    object.insert(
        "$variant".to_string(),
        JsonValue::String(variant.name.clone()),
    );
    if variant.positional {
        object.insert(
            "$values".to_string(),
            JsonValue::Array(
                variant
                    .fields
                    .iter()
                    .map(|field| template_value(&field.ty, contract, substitutions, active))
                    .collect(),
            ),
        );
    } else {
        for field in &variant.fields {
            object.insert(
                field.name.clone(),
                template_value(&field.ty, contract, substitutions, active),
            );
        }
    }
    JsonValue::Object(object)
}

fn product_variant(definition: &CalculationTypeDefinition) -> Option<&CalculationVariant> {
    if definition.variants.len() == 1
        && definition.variants[0].name == definition.name
        && !definition.variants[0].positional
    {
        definition.variants.first()
    } else {
        None
    }
}

fn collect_decoded<T>(
    results: impl IntoIterator<Item = Result<T, Vec<CalculationValueError>>>,
) -> Result<Vec<T>, Vec<CalculationValueError>> {
    let mut values = Vec::new();
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(value) => values.push(value),
            Err(mut diagnostics) => errors.append(&mut diagnostics),
        }
    }
    if errors.is_empty() {
        Ok(values)
    } else {
        Err(errors)
    }
}

/// Bookkeeping while decoding a partial case: the input leaves the case
/// provides, and the input containers whose field reads are tracked.
#[derive(Debug, Default)]
pub(crate) struct PartialDecoder {
    pub(crate) provided: BTreeSet<String>,
    pub(crate) containers: Vec<(usize, String)>,
    expanding: BTreeSet<String>,
}

type PartialDecode<'a> = Option<&'a RefCell<PartialDecoder>>;

/// A decoder path (`$.taxpayer.married`) relative to the case input.
pub(crate) fn input_relative_path(path: &str) -> String {
    match path.strip_prefix("$.") {
        Some(relative) => relative.to_string(),
        None if path == "$" => String::new(),
        None => path.trim_start_matches('$').to_string(),
    }
}

fn partial_provided(partial: PartialDecode, path: &str) {
    if let Some(partial) = partial {
        partial
            .borrow_mut()
            .provided
            .insert(input_relative_path(path));
    }
}

fn partial_container(partial: PartialDecode, value: Value, path: &str) -> Value {
    if let Some(partial) = partial {
        if let Some(id) = partial::container_id(&value) {
            partial
                .borrow_mut()
                .containers
                .push((id, input_relative_path(path)));
        }
    }
    value
}

fn unknown_at(path: &str) -> Value {
    partial::unknown(BTreeSet::from([input_relative_path(path)]))
}

/// The value of a field a partial case leaves out. A record must exist, so
/// it decodes like `{}` with its own fields unknown; any other value is
/// unknown at its path, and a sum type at its `$variant`.
fn decode_missing(
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    partial: &RefCell<PartialDecoder>,
) -> Value {
    let ty = substitute_contract_type(ty, substitutions);
    let CalculationTypeRef::Named { name, arguments } = &ty else {
        return unknown_at(path);
    };
    let Some(definition) = contract.definition(name) else {
        return unknown_at(path);
    };
    let Some(variant) = product_variant(definition) else {
        return unknown_at(&field_path(path, "$variant"));
    };
    if definition.parameters.len() != arguments.len()
        || !partial.borrow_mut().expanding.insert(name.clone())
    {
        return unknown_at(path);
    }
    let local = definition_substitutions(definition, arguments);
    let fields: Vec<(String, Value)> = variant
        .fields
        .iter()
        .map(|field| {
            (
                field.name.clone(),
                decode_missing(
                    &field.ty,
                    contract,
                    &local,
                    &field_path(path, &field.name),
                    partial,
                ),
            )
        })
        .collect();
    partial.borrow_mut().expanding.remove(name);
    let value = if definition.kind == "rule_scope" {
        Value::RuleScopeInstance {
            name: name.clone(),
            bindings: Rc::new(fields.into_iter().collect()),
        }
    } else {
        Value::NamedConstructor(runtime_variant_name(definition, variant), fields.into())
    };
    partial_container(Some(partial), value, path)
}

fn decode_value(
    value: &JsonValue,
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    partial: PartialDecode,
) -> Result<Value, Vec<CalculationValueError>> {
    let ty = substitute_contract_type(ty, substitutions);
    match ty {
        CalculationTypeRef::Primitive { name } => {
            let decoded = decode_primitive(value, &name, path).map_err(|error| vec![error])?;
            partial_provided(partial, path);
            Ok(decoded)
        }
        CalculationTypeRef::Unit => {
            if value.is_null() {
                partial_provided(partial, path);
                Ok(Value::Unit)
            } else {
                Err(vec![value_error(path, "expected null for unit")])
            }
        }
        CalculationTypeRef::Optional { item } => {
            if value.is_null() {
                partial_provided(partial, path);
                Ok(partial_container(
                    partial,
                    Value::Constructor("None".to_string(), Vec::new().into()),
                    path,
                ))
            } else {
                Ok(partial_container(
                    partial,
                    Value::Constructor(
                        "Some".to_string(),
                        vec![decode_value(
                            value,
                            &item,
                            contract,
                            substitutions,
                            path,
                            partial,
                        )?]
                        .into(),
                    ),
                    path,
                ))
            }
        }
        CalculationTypeRef::List { item } => {
            let values = value
                .as_array()
                .ok_or_else(|| vec![value_error(path, "expected an array")])?;
            if values.is_empty() {
                partial_provided(partial, path);
            }
            collect_decoded(values.iter().enumerate().map(|(index, value)| {
                decode_value(
                    value,
                    &item,
                    contract,
                    substitutions,
                    &format!("{}[{}]", path, index),
                    partial,
                )
            }))
            .map(Value::List)
        }
        CalculationTypeRef::Map { key, value: item } => {
            if !matches!(*key, CalculationTypeRef::Primitive { ref name } if name == "String") {
                return Err(vec![value_error(path, "only Map(String, T) is supported")]);
            }
            let object = value
                .as_object()
                .ok_or_else(|| vec![value_error(path, "expected an object")])?;
            if object.is_empty() {
                partial_provided(partial, path);
            }
            let decoded = collect_decoded(object.iter().map(|(name, value)| {
                decode_value(
                    value,
                    &item,
                    contract,
                    substitutions,
                    &field_path(path, name),
                    partial,
                )
                .map(|value| (name, value))
            }))?;
            let mut values = BTreeMap::new();
            for (name, value) in decoded {
                runtime_map_insert_value(&mut values, Value::Str(name.clone()), value);
            }
            Ok(Value::Map(values))
        }
        CalculationTypeRef::Set { item } => {
            let array = value
                .as_array()
                .ok_or_else(|| vec![value_error(path, "expected an array")])?;
            if array.is_empty() {
                partial_provided(partial, path);
            }
            let mut values = BTreeMap::new();
            let mut errors = Vec::new();
            for (index, item_value) in array.iter().enumerate() {
                let decoded = match decode_value(
                    item_value,
                    &item,
                    contract,
                    substitutions,
                    &format!("{}[{}]", path, index),
                    partial,
                ) {
                    Ok(value) => value,
                    Err(mut diagnostics) => {
                        errors.append(&mut diagnostics);
                        continue;
                    }
                };
                // A set orders and deduplicates its elements, so each must
                // be known when the case is decoded.
                if partial::has_unknown(&decoded) {
                    errors.push(value_error(
                        format!("{}[{}]", path, index),
                        "a set element must be given completely",
                    ));
                    continue;
                }
                let rendered = decoded.to_string();
                if !runtime_set_insert_value(&mut values, decoded) {
                    errors.push(value_error(
                        format!("{}[{}]", path, index),
                        format!("duplicate set value `{}`", rendered),
                    ));
                }
            }
            if errors.is_empty() {
                Ok(Value::Set(values))
            } else {
                Err(errors)
            }
        }
        CalculationTypeRef::TypeParameter { name } => Err(vec![value_error(
            path,
            format!("unresolved type parameter `{}`", name),
        )]),
        CalculationTypeRef::Named { name, arguments } => {
            let definition = contract.definition(&name).ok_or_else(|| {
                vec![value_error(
                    path,
                    format!("contract has no definition for `{}`", name),
                )]
            })?;
            if definition.parameters.len() != arguments.len() {
                return Err(vec![value_error(
                    path,
                    format!(
                        "type `{}` expects {} arguments but got {}",
                        name,
                        definition.parameters.len(),
                        arguments.len()
                    ),
                )]);
            }
            let local = definition_substitutions(definition, &arguments);
            if let Some(variant) = product_variant(definition) {
                let fields =
                    decode_named_fields(value, variant, contract, &local, path, false, partial)?;
                let decoded = if definition.kind == "rule_scope" {
                    Value::RuleScopeInstance {
                        name,
                        bindings: Rc::new(fields.into_iter().collect()),
                    }
                } else {
                    Value::NamedConstructor(
                        runtime_variant_name(definition, variant),
                        fields.into(),
                    )
                };
                Ok(partial_container(partial, decoded, path))
            } else {
                decode_variant(value, definition, contract, &local, path, partial)
            }
        }
    }
}

fn decode_primitive(
    value: &JsonValue,
    name: &str,
    path: &str,
) -> Result<Value, CalculationValueError> {
    match name {
        "Int" => value
            .as_i64()
            .map(Value::Int)
            .ok_or_else(|| value_error(path, "expected an exact signed integer")),
        "Float" => value
            .as_f64()
            .filter(|number| number.is_finite())
            .map(Value::Float)
            .ok_or_else(|| value_error(path, "expected a finite number")),
        "Bool" => value
            .as_bool()
            .map(Value::Bool)
            .ok_or_else(|| value_error(path, "expected a boolean")),
        "String" => value
            .as_str()
            .map(|value| Value::Str(value.to_string()))
            .ok_or_else(|| value_error(path, "expected a string")),
        "Char" => {
            let text = value
                .as_str()
                .ok_or_else(|| value_error(path, "expected a one-character string"))?;
            let mut chars = text.chars();
            let character = chars
                .next()
                .ok_or_else(|| value_error(path, "expected one character, got an empty string"))?;
            if chars.next().is_some() {
                return Err(value_error(path, "expected a one-character string"));
            }
            Ok(Value::Char(character))
        }
        _ => Err(value_error(
            path,
            format!("unknown primitive type `{}`", name),
        )),
    }
}

fn decode_named_fields(
    value: &JsonValue,
    variant: &CalculationVariant,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    allow_variant_key: bool,
    partial: PartialDecode,
) -> Result<Vec<(String, Value)>, Vec<CalculationValueError>> {
    let object = value
        .as_object()
        .ok_or_else(|| vec![value_error(path, "expected an object")])?;
    let allowed: BTreeSet<&str> = variant
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let mut errors = Vec::new();
    for key in object.keys() {
        if !(allowed.contains(key.as_str()) || allow_variant_key && key == "$variant") {
            errors.push(value_error(
                field_path(path, key),
                format!("unknown field for variant `{}`", variant.name),
            ));
        }
    }
    let mut result = Vec::new();
    for field in &variant.fields {
        let field_ty = substitute_contract_type(&field.ty, substitutions);
        let field_value = match (object.get(&field.name), partial) {
            (Some(value), _) => value,
            // Left out of a partial case: not asked yet.
            (None, Some(partial)) => {
                result.push((
                    field.name.clone(),
                    decode_missing(
                        &field_ty,
                        contract,
                        substitutions,
                        &field_path(path, &field.name),
                        partial,
                    ),
                ));
                continue;
            }
            (None, None) if matches!(field_ty, CalculationTypeRef::Optional { .. }) => {
                &JsonValue::Null
            }
            (None, None) => {
                errors.push(value_error(
                    field_path(path, &field.name),
                    "missing required field",
                ));
                continue;
            }
        };
        match decode_value(
            field_value,
            &field_ty,
            contract,
            substitutions,
            &field_path(path, &field.name),
            partial,
        ) {
            Ok(value) => result.push((field.name.clone(), value)),
            Err(mut diagnostics) => errors.append(&mut diagnostics),
        }
    }
    if errors.is_empty() {
        Ok(result)
    } else {
        Err(errors)
    }
}

fn decode_variant(
    value: &JsonValue,
    definition: &CalculationTypeDefinition,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    partial: PartialDecode,
) -> Result<Value, Vec<CalculationValueError>> {
    let object = value.as_object().ok_or_else(|| {
        vec![value_error(
            path,
            format!(
                "expected an object with `$variant` for `{}`",
                definition.name
            ),
        )]
    })?;
    if partial.is_some() && object.is_empty() {
        // `{}` for a sum type: which alternative applies is not known yet.
        return Ok(unknown_at(&field_path(path, "$variant")));
    }
    let variant_name = object
        .get("$variant")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            vec![value_error(
                field_path(path, "$variant"),
                "missing variant name",
            )]
        })?;
    let variant = definition
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)
        .ok_or_else(|| {
            vec![value_error(
                field_path(path, "$variant"),
                format!(
                    "unknown `{}` variant `{}`; expected one of {}",
                    definition.name,
                    variant_name,
                    definition
                        .variants
                        .iter()
                        .map(|variant| variant.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )]
        })?;
    partial_provided(partial, &field_path(path, "$variant"));
    if variant.positional {
        let mut errors = Vec::new();
        for key in object.keys() {
            if key != "$variant" && key != "$values" {
                errors.push(value_error(
                    field_path(path, key),
                    "positional variants only accept `$variant` and `$values`",
                ));
            }
        }
        let Some(values) = object.get("$values").and_then(JsonValue::as_array) else {
            errors.push(value_error(
                field_path(path, "$values"),
                "expected an array",
            ));
            return Err(errors);
        };
        if values.len() != variant.fields.len() {
            errors.push(value_error(
                field_path(path, "$values"),
                format!(
                    "variant `{}` expects {} values but got {}",
                    variant.name,
                    variant.fields.len(),
                    values.len()
                ),
            ));
        }
        let decoded = collect_decoded(values.iter().zip(&variant.fields).enumerate().map(
            |(index, (value, field))| {
                decode_value(
                    value,
                    &field.ty,
                    contract,
                    substitutions,
                    &format!("{}.$values[{}]", path, index),
                    partial,
                )
            },
        ));
        match decoded {
            Ok(values) if errors.is_empty() => Ok(partial_container(
                partial,
                Value::Constructor(runtime_variant_name(definition, variant), values.into()),
                path,
            )),
            Ok(_) => Err(errors),
            Err(mut diagnostics) => {
                errors.append(&mut diagnostics);
                Err(errors)
            }
        }
    } else {
        let fields =
            decode_named_fields(value, variant, contract, substitutions, path, true, partial)?;
        let decoded = if fields.is_empty() {
            Value::Constructor(runtime_variant_name(definition, variant), Vec::new().into())
        } else {
            Value::NamedConstructor(runtime_variant_name(definition, variant), fields.into())
        };
        Ok(partial_container(partial, decoded, path))
    }
}

fn encode_value(
    value: &Value,
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
) -> Result<JsonValue, CalculationValueError> {
    if matches!(value, Value::Unknown(_)) {
        return Err(value_error(
            path,
            partial::unknown_leak_message("JSON encoding"),
        ));
    }
    let ty = substitute_contract_type(ty, substitutions);
    match ty {
        CalculationTypeRef::Primitive { name } => encode_primitive(value, &name, path),
        CalculationTypeRef::Unit => match value {
            Value::Unit => Ok(JsonValue::Null),
            _ => Err(runtime_type_error(path, "()", value)),
        },
        CalculationTypeRef::Optional { item } => match value {
            Value::Constructor(name, values) if name == "None" && values.is_empty() => {
                Ok(JsonValue::Null)
            }
            Value::Constructor(name, values) if name == "Some" && values.len() == 1 => {
                encode_value(&values[0], &item, contract, substitutions, path)
            }
            _ => Err(runtime_type_error(
                path,
                &format!("{}?", item.display_name()),
                value,
            )),
        },
        CalculationTypeRef::List { item } => {
            let Some(values) = runtime_list_values(value) else {
                return Err(runtime_type_error(
                    path,
                    &format!("List({})", item.display_name()),
                    value,
                ));
            };
            Ok(JsonValue::Array(
                values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        encode_value(
                            value,
                            &item,
                            contract,
                            substitutions,
                            &format!("{}[{}]", path, index),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        CalculationTypeRef::Map { key, value: item } => {
            if !matches!(*key, CalculationTypeRef::Primitive { ref name } if name == "String") {
                return Err(value_error(path, "only Map(String, T) is supported"));
            }
            let Value::Map(values) = value else {
                return Err(runtime_type_error(path, "Map", value));
            };
            let mut object = JsonMap::new();
            for (stored_key, stored_value) in values {
                let Some((name, value)) = runtime_map_string_entry(stored_key, stored_value) else {
                    return Err(value_error(
                        path,
                        "Map(String, T) contains a non-String key",
                    ));
                };
                let encoded = encode_value(
                    value,
                    &item,
                    contract,
                    substitutions,
                    &field_path(path, name),
                )?;
                if object.insert(name.to_string(), encoded).is_some() {
                    return Err(value_error(
                        field_path(path, name),
                        format!("duplicate map key `{}`", name),
                    ));
                }
            }
            Ok(JsonValue::Object(object))
        }
        CalculationTypeRef::Set { item } => {
            let Value::Set(values) = value else {
                return Err(runtime_type_error(path, "Set", value));
            };
            Ok(JsonValue::Array(
                values
                    .values()
                    .enumerate()
                    .map(|(index, value)| {
                        encode_value(
                            value,
                            &item,
                            contract,
                            substitutions,
                            &format!("{}[{}]", path, index),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        CalculationTypeRef::TypeParameter { name } => Err(value_error(
            path,
            format!("unresolved type parameter `{}`", name),
        )),
        CalculationTypeRef::Named { name, arguments } => {
            let definition = contract.definition(&name).ok_or_else(|| {
                value_error(path, format!("contract has no definition for `{}`", name))
            })?;
            let local = definition_substitutions(definition, &arguments);
            if let Some(variant) = product_variant(definition) {
                let fields = runtime_named_fields(value, definition, variant, path)?;
                encode_named_fields(fields, variant, contract, &local, path, false)
            } else {
                encode_variant(value, definition, contract, &local, path)
            }
        }
    }
}

fn runtime_list_values(value: &Value) -> Option<Vec<&Value>> {
    let mut values = Vec::new();
    let mut current = value;
    loop {
        match current {
            Value::List(items) => {
                values.extend(items.iter());
                return Some(values);
            }
            Value::Constructor(name, fields) if name == "Nil" && fields.is_empty() => {
                return Some(values);
            }
            Value::Constructor(name, fields) if name == "Cons" && fields.len() == 2 => {
                values.push(&fields[0]);
                current = &fields[1];
            }
            _ => return None,
        }
    }
}

fn encode_primitive(
    value: &Value,
    name: &str,
    path: &str,
) -> Result<JsonValue, CalculationValueError> {
    match (name, value) {
        ("Int", Value::Int(value)) => Ok(JsonValue::Number((*value).into())),
        ("Float", Value::Float(value)) if value.is_finite() => JsonNumber::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| value_error(path, "runtime produced a non-finite float")),
        ("Bool", Value::Bool(value)) => Ok(JsonValue::Bool(*value)),
        ("String", Value::Str(value)) => Ok(JsonValue::String(value.clone())),
        ("Char", Value::Char(value)) => Ok(JsonValue::String(value.to_string())),
        _ => Err(runtime_type_error(path, name, value)),
    }
}

fn runtime_type_error(path: &str, expected: &str, value: &Value) -> CalculationValueError {
    value_error(
        path,
        format!("expected runtime `{}`, got `{}`", expected, value),
    )
}

fn runtime_named_fields<'a>(
    value: &'a Value,
    definition: &CalculationTypeDefinition,
    variant: &CalculationVariant,
    path: &str,
) -> Result<&'a [(String, Value)], CalculationValueError> {
    match value {
        Value::NamedConstructor(name, fields) if constructor_display_name(name) == variant.name => {
            Ok(fields)
        }
        Value::RuleScopeInstance { name, bindings } if name == &definition.name => {
            // Rule-scope results are intentionally not a v1 output surface. Keeping
            // this branch explicit produces a useful diagnostic instead of a panic.
            let _ = bindings;
            Err(value_error(
                path,
                "rule-scope instances cannot be emitted as calculation results",
            ))
        }
        _ => Err(runtime_type_error(path, &definition.name, value)),
    }
}

fn encode_named_fields(
    values: &[(String, Value)],
    variant: &CalculationVariant,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    include_variant: bool,
) -> Result<JsonValue, CalculationValueError> {
    let by_name: BTreeMap<&str, &Value> = values
        .iter()
        .map(|(name, value)| (name.as_str(), value))
        .collect();
    let mut object = JsonMap::new();
    if include_variant {
        object.insert(
            "$variant".to_string(),
            JsonValue::String(variant.name.clone()),
        );
    }
    for field in &variant.fields {
        let value = by_name.get(field.name.as_str()).ok_or_else(|| {
            value_error(
                field_path(path, &field.name),
                "runtime value is missing a declared field",
            )
        })?;
        object.insert(
            field.name.clone(),
            encode_value(
                value,
                &field.ty,
                contract,
                substitutions,
                &field_path(path, &field.name),
            )?,
        );
    }
    for name in by_name.keys() {
        if !variant.fields.iter().any(|field| field.name == *name) {
            return Err(value_error(
                field_path(path, name),
                "runtime value contains an undeclared field",
            ));
        }
    }
    Ok(JsonValue::Object(object))
}

fn encode_variant(
    value: &Value,
    definition: &CalculationTypeDefinition,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
) -> Result<JsonValue, CalculationValueError> {
    match value {
        Value::Constructor(name, values) => {
            let variant = definition
                .variants
                .iter()
                .find(|variant| variant.name == constructor_display_name(name))
                .ok_or_else(|| runtime_type_error(path, &definition.name, value))?;
            if variant.positional {
                if variant.fields.len() != values.len() {
                    return Err(value_error(
                        path,
                        format!(
                            "runtime variant `{}` has {} values, expected {}",
                            name,
                            values.len(),
                            variant.fields.len()
                        ),
                    ));
                }
                let mut object = JsonMap::new();
                object.insert(
                    "$variant".to_string(),
                    JsonValue::String(variant.name.clone()),
                );
                object.insert(
                    "$values".to_string(),
                    JsonValue::Array(
                        values
                            .iter()
                            .zip(&variant.fields)
                            .enumerate()
                            .map(|(index, (value, field))| {
                                encode_value(
                                    value,
                                    &field.ty,
                                    contract,
                                    substitutions,
                                    &format!("{}.$values[{}]", path, index),
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                );
                Ok(JsonValue::Object(object))
            } else if values.is_empty() && variant.fields.is_empty() {
                let mut object = JsonMap::new();
                object.insert(
                    "$variant".to_string(),
                    JsonValue::String(variant.name.clone()),
                );
                Ok(JsonValue::Object(object))
            } else {
                Err(runtime_type_error(path, &definition.name, value))
            }
        }
        Value::NamedConstructor(name, fields) => {
            let variant = definition
                .variants
                .iter()
                .find(|variant| {
                    variant.name == constructor_display_name(name) && !variant.positional
                })
                .ok_or_else(|| runtime_type_error(path, &definition.name, value))?;
            encode_named_fields(fields, variant, contract, substitutions, path, true)
        }
        _ => Err(runtime_type_error(path, &definition.name, value)),
    }
}

/// The runtime name of a contract variant: a shared constructor carries its
/// internal name while the program runs, and its declared name in data.
fn runtime_variant_name(
    definition: &CalculationTypeDefinition,
    variant: &CalculationVariant,
) -> String {
    constructor_runtime_name(&definition.name, &variant.name).into_owned()
}

fn field_path(parent: &str, field: &str) -> String {
    if parent == "$" {
        format!("$.{}", field)
    } else {
        format!("{}.{}", parent, field)
    }
}

struct CalculationLayoutBuilder<'a> {
    contract: &'a CalculationContract,
    tables: Vec<CalculationCollectionTable>,
    used_sheet_names: BTreeSet<String>,
}

impl<'a> CalculationLayoutBuilder<'a> {
    fn new(contract: &'a CalculationContract) -> Self {
        Self {
            contract,
            tables: Vec::new(),
            used_sheet_names: [
                "_futuruna",
                "_columns",
                "_tables",
                "cases",
                "results",
                "diagnostics",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn flatten_value(
        &mut self,
        column_path: &str,
        absolute_path: &str,
        value_path: &str,
        ty: &CalculationTypeRef,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
        required: bool,
        parent_table: Option<&str>,
        variant_guards: &[CalculationVariantGuard],
        active: &mut BTreeSet<String>,
        columns: &mut Vec<CalculationColumn>,
    ) {
        let resolved = substitute_contract_type(ty, substitutions);
        match &resolved {
            CalculationTypeRef::List { item } => self.register_collection(
                absolute_path,
                value_path,
                CalculationCollectionKind::List,
                item,
                parent_table,
                variant_guards,
                active,
            ),
            CalculationTypeRef::Map { key, value } if is_string_type(key) => self
                .register_collection(
                    absolute_path,
                    value_path,
                    CalculationCollectionKind::Map,
                    value,
                    parent_table,
                    variant_guards,
                    active,
                ),
            CalculationTypeRef::Set { item } => self.register_collection(
                absolute_path,
                value_path,
                CalculationCollectionKind::Set,
                item,
                parent_table,
                variant_guards,
                active,
            ),
            CalculationTypeRef::Primitive { name } => {
                columns.push(scalar_column(
                    layout_column_path(column_path, parent_table),
                    normalized_input_path(absolute_path),
                    normalized_value_path(value_path),
                    resolved.clone(),
                    name,
                    required,
                    variant_guards,
                ));
            }
            CalculationTypeRef::Optional { item } => {
                let item = substitute_contract_type(item, substitutions);
                if is_scalar_column_type(&item, self.contract) {
                    let before = columns.len();
                    self.flatten_value(
                        column_path,
                        absolute_path,
                        value_path,
                        &item,
                        substitutions,
                        false,
                        parent_table,
                        variant_guards,
                        active,
                        columns,
                    );
                    if columns.len() == before + 1 {
                        columns[before].ty = resolved;
                        columns[before].required = false;
                    }
                } else {
                    columns.push(json_column(
                        &layout_column_path(column_path, parent_table),
                        &normalized_input_path(absolute_path),
                        &normalized_value_path(value_path),
                        resolved,
                        false,
                        variant_guards,
                    ));
                }
            }
            CalculationTypeRef::Named { name, arguments } => {
                let Some(definition) = self.contract.definition(name) else {
                    columns.push(json_column(
                        &layout_column_path(column_path, parent_table),
                        &normalized_input_path(absolute_path),
                        &normalized_value_path(value_path),
                        resolved,
                        required,
                        variant_guards,
                    ));
                    return;
                };
                if !active.insert(name.clone()) {
                    columns.push(json_column(
                        &layout_column_path(column_path, parent_table),
                        &normalized_input_path(absolute_path),
                        &normalized_value_path(value_path),
                        resolved,
                        required,
                        variant_guards,
                    ));
                    return;
                }
                let local = definition_substitutions(definition, arguments);
                if let Some(variant) = product_variant(definition) {
                    for field in &variant.fields {
                        let field_ty = substitute_contract_type(&field.ty, &local);
                        self.flatten_value(
                            &joined_column_path(column_path, &field.name),
                            &joined_column_path(absolute_path, &field.name),
                            &joined_column_path(value_path, &field.name),
                            &field.ty,
                            &local,
                            !matches!(field_ty, CalculationTypeRef::Optional { .. }),
                            parent_table,
                            variant_guards,
                            active,
                            columns,
                        );
                    }
                } else if !definition.variants.is_empty()
                    && definition
                        .variants
                        .iter()
                        .all(|variant| variant.fields.is_empty())
                {
                    columns.push(CalculationColumn {
                        path: layout_column_path(column_path, parent_table),
                        input_path: normalized_input_path(absolute_path),
                        value_path: normalized_value_path(value_path),
                        ty: resolved.clone(),
                        encoding: CalculationColumnEncoding::Enum,
                        required,
                        choices: definition
                            .variants
                            .iter()
                            .map(|variant| variant.name.clone())
                            .collect(),
                        variant_guards: variant_guards.to_vec(),
                        metadata: None,
                        member_metadata: Vec::new(),
                    });
                } else if !definition.variants.is_empty() {
                    let discriminator_path = joined_column_path(column_path, "$variant");
                    let discriminator_value_path = joined_column_path(value_path, "$variant");
                    columns.push(CalculationColumn {
                        path: layout_column_path(&discriminator_path, parent_table),
                        input_path: normalized_input_path(&joined_column_path(
                            absolute_path,
                            "$variant",
                        )),
                        value_path: normalized_value_path(&discriminator_value_path),
                        ty: resolved.clone(),
                        encoding: CalculationColumnEncoding::Variant,
                        required,
                        choices: definition
                            .variants
                            .iter()
                            .map(|variant| variant.name.clone())
                            .collect(),
                        variant_guards: variant_guards.to_vec(),
                        metadata: None,
                        member_metadata: Vec::new(),
                    });

                    for variant in &definition.variants {
                        let mut guards = variant_guards.to_vec();
                        guards.push(CalculationVariantGuard {
                            path: normalized_value_path(value_path),
                            variant: variant.name.clone(),
                        });
                        let variant_column_path = joined_column_path(column_path, &variant.name);
                        let variant_absolute_path =
                            joined_column_path(absolute_path, &variant.name);
                        for (index, field) in variant.fields.iter().enumerate() {
                            let field_ty = substitute_contract_type(&field.ty, &local);
                            let field_value_path = if variant.positional {
                                joined_column_path(
                                    &joined_column_path(value_path, "$values"),
                                    &index.to_string(),
                                )
                            } else {
                                joined_column_path(value_path, &field.name)
                            };
                            self.flatten_value(
                                &joined_column_path(&variant_column_path, &field.name),
                                &joined_column_path(&variant_absolute_path, &field.name),
                                &field_value_path,
                                &field.ty,
                                &local,
                                !matches!(field_ty, CalculationTypeRef::Optional { .. }),
                                parent_table,
                                &guards,
                                active,
                                columns,
                            );
                        }
                    }
                } else {
                    columns.push(json_column(
                        &layout_column_path(column_path, parent_table),
                        &normalized_input_path(absolute_path),
                        &normalized_value_path(value_path),
                        resolved.clone(),
                        required,
                        variant_guards,
                    ));
                }
                active.remove(name);
            }
            _ => columns.push(json_column(
                &layout_column_path(column_path, parent_table),
                &normalized_input_path(absolute_path),
                &normalized_value_path(value_path),
                resolved,
                required,
                variant_guards,
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn register_collection(
        &mut self,
        absolute_path: &str,
        value_path: &str,
        kind: CalculationCollectionKind,
        item_type: &CalculationTypeRef,
        parent_table: Option<&str>,
        variant_guards: &[CalculationVariantGuard],
        active: &mut BTreeSet<String>,
    ) {
        let path = if absolute_path.is_empty() {
            match parent_table {
                Some(parent) => joined_column_path(parent, "items"),
                None => "input".to_string(),
            }
        } else {
            absolute_path.to_string()
        };
        if self.tables.iter().any(|table| table.path == path) {
            return;
        }
        let item_type = item_type.clone();
        let item_value_column =
            collection_item_uses_value_column(&item_type, self.contract, active);
        let mut columns = Vec::new();
        let item_absolute_path = if is_normalized_collection_type(&item_type) {
            joined_column_path(&path, "items")
        } else {
            path.clone()
        };
        self.flatten_value(
            "",
            &item_absolute_path,
            "",
            &item_type,
            &BTreeMap::new(),
            true,
            Some(&path),
            &[],
            active,
            &mut columns,
        );
        let sheet = self.next_sheet_name(&path);
        self.tables.push(CalculationCollectionTable {
            path: path.clone(),
            sheet,
            parent_path: parent_table.map(str::to_string),
            attach_path: normalized_value_path(value_path),
            kind,
            item_type,
            item_value_column,
            variant_guards: variant_guards.to_vec(),
            columns,
            metadata: None,
        });
    }

    fn next_sheet_name(&mut self, path: &str) -> String {
        let mut base: String = path
            .chars()
            .map(|character| {
                if character.is_alphanumeric() || character == '_' {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        base = base.trim_matches('_').to_string();
        if base.is_empty() {
            base = "items".to_string();
        }
        let base_key = base.to_lowercase();
        if base.chars().count() <= 31 && !self.used_sheet_names.contains(&base_key) {
            self.used_sheet_names.insert(base_key);
            return base;
        }

        let prefix: String = base.chars().take(22).collect();
        for salt in 0_u64.. {
            let digest = format!(
                "{:x}",
                Sha256::digest(format!("{}:{}", path, salt).as_bytes())
            );
            let candidate = format!("{}_{}", prefix, &digest[..8]);
            if self.used_sheet_names.insert(candidate.to_lowercase()) {
                return candidate;
            }
        }
        unreachable!("worksheet name salt space is exhaustive")
    }
}

fn scalar_column(
    path: String,
    input_path: String,
    value_path: String,
    ty: CalculationTypeRef,
    primitive: &str,
    required: bool,
    variant_guards: &[CalculationVariantGuard],
) -> CalculationColumn {
    CalculationColumn {
        path,
        input_path,
        value_path,
        ty,
        encoding: match primitive {
            "Int" => CalculationColumnEncoding::Integer,
            "Float" => CalculationColumnEncoding::Float,
            "Bool" => CalculationColumnEncoding::Boolean,
            "Char" => CalculationColumnEncoding::Character,
            _ => CalculationColumnEncoding::String,
        },
        required,
        choices: if primitive == "Bool" {
            vec!["true".to_string(), "false".to_string()]
        } else {
            Vec::new()
        },
        variant_guards: variant_guards.to_vec(),
        metadata: None,
        member_metadata: Vec::new(),
    }
}

fn is_string_type(ty: &CalculationTypeRef) -> bool {
    matches!(ty, CalculationTypeRef::Primitive { name } if name == "String")
}

fn is_normalized_collection_type(ty: &CalculationTypeRef) -> bool {
    match ty {
        CalculationTypeRef::List { .. } | CalculationTypeRef::Set { .. } => true,
        CalculationTypeRef::Map { key, .. } => is_string_type(key),
        _ => false,
    }
}

fn collection_item_uses_value_column(
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    active: &BTreeSet<String>,
) -> bool {
    match ty {
        CalculationTypeRef::Primitive { .. } => true,
        CalculationTypeRef::Optional { .. } => true,
        CalculationTypeRef::Named { name, .. } => {
            if active.contains(name) {
                return true;
            }
            contract.definition(name).is_none_or(|definition| {
                product_variant(definition).is_none()
                    && (definition.variants.is_empty()
                        || definition
                            .variants
                            .iter()
                            .all(|variant| variant.fields.is_empty()))
            })
        }
        CalculationTypeRef::List { .. } | CalculationTypeRef::Set { .. } => false,
        CalculationTypeRef::Map { key, .. } => !is_string_type(key),
        _ => true,
    }
}

fn layout_column_path(path: &str, parent_table: Option<&str>) -> String {
    if path.is_empty() {
        if parent_table.is_some() {
            "value".to_string()
        } else {
            "input".to_string()
        }
    } else {
        path.to_string()
    }
}

fn is_scalar_column_type(ty: &CalculationTypeRef, contract: &CalculationContract) -> bool {
    match ty {
        CalculationTypeRef::Primitive { .. } => true,
        CalculationTypeRef::Named { name, .. } => {
            contract.definition(name).is_some_and(|definition| {
                !definition.variants.is_empty()
                    && definition
                        .variants
                        .iter()
                        .all(|variant| variant.fields.is_empty())
            })
        }
        _ => false,
    }
}

fn json_column(
    path: &str,
    input_path: &str,
    value_path: &str,
    ty: CalculationTypeRef,
    required: bool,
    variant_guards: &[CalculationVariantGuard],
) -> CalculationColumn {
    CalculationColumn {
        path: normalized_column_path(path),
        input_path: normalized_input_path(input_path),
        value_path: normalized_value_path(value_path),
        ty,
        encoding: CalculationColumnEncoding::Json,
        required,
        choices: Vec::new(),
        variant_guards: variant_guards.to_vec(),
        metadata: None,
        member_metadata: Vec::new(),
    }
}

fn joined_column_path(parent: &str, field: &str) -> String {
    if parent.is_empty() {
        field.to_string()
    } else {
        format!("{}.{}", parent, field)
    }
}

fn normalized_column_path(path: &str) -> String {
    if path.is_empty() {
        "input".to_string()
    } else {
        path.to_string()
    }
}

fn normalized_input_path(path: &str) -> String {
    if path.is_empty() {
        "input".to_string()
    } else {
        path.to_string()
    }
}

fn normalized_value_path(path: &str) -> String {
    if path.is_empty() {
        "input".to_string()
    } else {
        path.to_string()
    }
}

/// Set a dotted record path in a canonical JSON input tree.
///
/// Numeric segments address array elements. Calculation layouts use those
/// segments for positional ADT payloads such as `$values.0`.
pub fn set_calculation_input_path(
    root: &mut JsonValue,
    path: &str,
    value: JsonValue,
) -> Result<(), CalculationValueError> {
    if path == "input" {
        *root = value;
        return Ok(());
    }
    if !root.is_object() {
        *root = JsonValue::Object(JsonMap::new());
    }
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(value_error(path, "empty input path"));
    }
    set_calculation_json_path(root, &parts, value, path)
}

fn set_calculation_json_path(
    current: &mut JsonValue,
    parts: &[&str],
    value: JsonValue,
    full_path: &str,
) -> Result<(), CalculationValueError> {
    if parts.is_empty() {
        *current = value;
        return Ok(());
    }
    if current.is_null() {
        *current = if parts[0].parse::<usize>().is_ok() {
            JsonValue::Array(Vec::new())
        } else {
            JsonValue::Object(JsonMap::new())
        };
    }
    match current {
        JsonValue::Object(object) => {
            let child = object
                .entry(parts[0].to_string())
                .or_insert(JsonValue::Null);
            set_calculation_json_path(child, &parts[1..], value, full_path)
        }
        JsonValue::Array(items) => {
            let index = parts[0].parse::<usize>().map_err(|_| {
                value_error(
                    full_path,
                    format!("array path segment `{}` is not an index", parts[0]),
                )
            })?;
            if items.len() <= index {
                items.resize(index + 1, JsonValue::Null);
            }
            set_calculation_json_path(&mut items[index], &parts[1..], value, full_path)
        }
        _ => Err(value_error(
            full_path,
            "column path crosses a non-object, non-array value",
        )),
    }
}

/// Validate envelope identity and case identifiers before adapter values run.
pub fn validate_input_envelope(
    contract: &CalculationContract,
    envelope: &CalculationInputEnvelope,
) -> Result<(), Vec<CalculationCaseDiagnostic>> {
    let mut diagnostics = Vec::new();
    if envelope.futuruna.schema != INPUT_SCHEMA && envelope.futuruna.schema != LEGACY_INPUT_SCHEMA {
        diagnostics.push(CalculationCaseDiagnostic {
            case_id: String::new(),
            path: "$.$futuruna.schema".to_string(),
            message: format!(
                "unsupported input schema `{}`; expected `{}` or `{}`",
                envelope.futuruna.schema, INPUT_SCHEMA, LEGACY_INPUT_SCHEMA
            ),
        });
    }
    if envelope.futuruna.entry != contract.entry {
        diagnostics.push(CalculationCaseDiagnostic {
            case_id: String::new(),
            path: "$.$futuruna.entry".to_string(),
            message: format!(
                "template entry `{}` does not match requested `{}`",
                envelope.futuruna.entry, contract.entry
            ),
        });
    }
    if envelope.futuruna.schema_hash != contract.schema_hash {
        diagnostics.push(CalculationCaseDiagnostic {
            case_id: String::new(),
            path: "$.$futuruna.schema_hash".to_string(),
            message: format!(
                "stale calculation template: expected schema hash `{}`, got `{}`",
                contract.schema_hash, envelope.futuruna.schema_hash
            ),
        });
    }
    let mut case_ids = BTreeSet::new();
    for (index, case) in envelope.cases.iter().enumerate() {
        if case.case_id.trim().is_empty() {
            diagnostics.push(CalculationCaseDiagnostic {
                case_id: case.case_id.clone(),
                path: format!("$.cases[{}].case_id", index),
                message: "case_id must not be empty".to_string(),
            });
        } else if !case_ids.insert(case.case_id.clone()) {
            diagnostics.push(CalculationCaseDiagnostic {
                case_id: case.case_id.clone(),
                path: format!("$.cases[{}].case_id", index),
                message: format!("duplicate case_id `{}`", case.case_id),
            });
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

enum CalculationCaseOutcome {
    Result(CalculationResultCase),
    Diagnostic(CalculationCaseDiagnostic),
    Diagnostics(Vec<CalculationCaseDiagnostic>),
}

/// The exact import-aware rule classification runtime dispatch needs.
/// Contract extraction already validated the program; this is recomputed
/// once per prepared calculation.
struct RuleDispatchMetadata {
    return_types: BTreeMap<RuleDispatchKey, String>,
    return_issues: BTreeMap<RuleDispatchKey, String>,
    runtime_boolean_miss_keys: BTreeSet<RuleDispatchKey>,
}

/// What a calculation worker initializes its interpreter from, shared by
/// every worker of one prepared calculation.
#[derive(Clone)]
struct CalculationProgram {
    entry: String,
    stmts: Arc<[Stmt]>,
    source_dir: Option<String>,
    rule_dispatch: Arc<RuleDispatchMetadata>,
}

impl CalculationProgram {
    fn new(entry: &str, stmts: Arc<[Stmt]>, source_dir: Option<String>) -> Self {
        let (return_types, return_issues, runtime_boolean_miss_keys) =
            TypeChecker::rule_dispatch_metadata_for_runtime(&stmts, source_dir.clone());
        Self {
            entry: entry.to_string(),
            stmts,
            source_dir,
            rule_dispatch: Arc::new(RuleDispatchMetadata {
                return_types,
                return_issues,
                runtime_boolean_miss_keys,
            }),
        }
    }
}

/// An interpreter that has initialized a calculation program, and runs
/// cases from that state one at a time.
struct CalculationWorker {
    program: CalculationProgram,
    interpreter: Interpreter,
    base_env: Env,
    initialization_error: Option<String>,
    needs_reinitialization: bool,
    /// Set for partial cases (`runa call --partial`).
    partial: Option<partial::PartialMode>,
}

impl CalculationWorker {
    fn new(program: CalculationProgram) -> Self {
        let mut interpreter = Interpreter::new();
        interpreter.suppress_output = true;
        interpreter.deny_host_effects("a calculation");
        interpreter.source_dir = program.source_dir.clone();
        interpreter.install_rule_dispatch_return_metadata(
            &program.rule_dispatch.return_types,
            &program.rule_dispatch.return_issues,
            &program.rule_dispatch.runtime_boolean_miss_keys,
        );
        let mut base_env = interpreter.default_env();
        let initialization_error = interpreter
            .with_calculation_runtime(|runtime| {
                runtime.initialize_calculation_program(
                    &program.entry,
                    &program.stmts,
                    &mut base_env,
                )
            })
            .err()
            .map(|error| {
                format!(
                    "calculation `{}` initialization failed: {error}",
                    program.entry
                )
            });
        interpreter.seal_calculation_initialization();
        Self {
            program,
            interpreter,
            base_env,
            initialization_error,
            needs_reinitialization: false,
            partial: None,
        }
    }

    fn invoke(
        &mut self,
        contract: &CalculationContract,
        case: &CalculationInputCase,
    ) -> CalculationCaseOutcome {
        let started = trace_clock();
        let decoded = match self.partial {
            Some(_) => contract
                .decode_partial_input(&case.input)
                .map(|(input, decoder)| (input, Some(decoder))),
            None => contract
                .decode_input_diagnostics(&case.input)
                .map(|input| (input, None)),
        };
        let (input, decoder) = match decoded {
            Ok(decoded) => decoded,
            Err(errors) => {
                let outcome = CalculationCaseOutcome::Diagnostics(
                    errors
                        .into_iter()
                        .map(|error| CalculationCaseDiagnostic {
                            case_id: case.case_id.clone(),
                            path: error.path,
                            message: error.message,
                        })
                        .collect(),
                );
                trace_calculation_case(&case.case_id, started);
                return outcome;
            }
        };

        if self.needs_reinitialization {
            let partial = self.partial;
            *self = Self::new(self.program.clone());
            self.partial = partial;
        }
        if let Some(message) = &self.initialization_error {
            trace_calculation_case(&case.case_id, started);
            return CalculationCaseOutcome::Diagnostic(CalculationCaseDiagnostic {
                case_id: case.case_id.clone(),
                path: "$".to_string(),
                message: message.clone(),
            });
        }

        self.interpreter.begin_calculation_case();
        if let Some(decoder) = &decoder {
            self.interpreter
                .begin_partial_case(self.partial, &contract.entry);
            self.interpreter.partial.borrow_mut().containers =
                decoder.containers.iter().cloned().collect();
        }

        let mut runtime_env = self.base_env.clone();
        let input_binding = "__futuruna_calculation_input".to_string();
        runtime_env.set(input_binding.clone(), input);
        let call: Expr = ExprKind::App(
            Box::new(ExprKind::Var(contract.entry.clone()).into()),
            vec![ExprKind::Var(input_binding).into()],
        )
        .into();
        let evaluated = self
            .interpreter
            .with_calculation_runtime(|runtime| runtime.eval(&call, &runtime_env));
        // A panic may leave the interpreter half-changed, and a changed actor
        // or live stream of the initialized program would reach the next
        // case: either way the program is initialized again first.
        self.needs_reinitialization = matches!(evaluated, Err(CalculationRuntimeFailure::Panicked))
            || self.interpreter.prepared_live_cell_changed();
        if let Some(decoder) = decoder {
            let outcome = self.finish_partial(contract, case, evaluated, &decoder);
            self.interpreter.begin_partial_case(None, "");
            trace_calculation_case(&case.case_id, started);
            return outcome;
        }
        let result = match evaluated {
            Ok(result) => result,
            Err(error) => {
                trace_calculation_case(&case.case_id, started);
                return CalculationCaseOutcome::Diagnostic(CalculationCaseDiagnostic {
                    case_id: case.case_id.clone(),
                    path: "$".to_string(),
                    message: format!("calculation `{}` failed: {error}", contract.entry),
                });
            }
        };
        let outcome = match contract.encode_output(&result) {
            Ok(result) => CalculationCaseOutcome::Result(CalculationResultCase {
                case_id: case.case_id.clone(),
                result,
            }),
            Err(error) => CalculationCaseOutcome::Diagnostic(CalculationCaseDiagnostic {
                case_id: case.case_id.clone(),
                path: error.path,
                message: error.message,
            }),
        };
        trace_calculation_case(&case.case_id, started);
        outcome
    }

    /// Turn a partial case's evaluation into `Complete` or `Needs`.
    fn finish_partial(
        &mut self,
        contract: &CalculationContract,
        case: &CalculationInputCase,
        evaluated: Result<Value, CalculationRuntimeFailure>,
        decoder: &PartialDecoder,
    ) -> CalculationCaseOutcome {
        let diagnostic = |path: String, message: String| {
            CalculationCaseOutcome::Diagnostic(CalculationCaseDiagnostic {
                case_id: case.case_id.clone(),
                path,
                message,
            })
        };
        let outcome = match evaluated {
            Ok(result) => self.interpreter.finish_partial_case(result),
            Err(CalculationRuntimeFailure::NeedsInput(paths)) => partial::PartialOutcome::Needs {
                paths,
                waiting: self.interpreter.partial_waiting(),
            },
            Err(error) => {
                return diagnostic(
                    "$".to_string(),
                    format!("calculation `{}` failed: {error}", contract.entry),
                );
            }
        };
        let result = match outcome {
            partial::PartialOutcome::Complete {
                result,
                read_exact,
                read_subtree,
            } => match contract.encode_output(&result) {
                Ok(result) => CalculationPartialResult::Complete {
                    result,
                    unused_paths: decoder
                        .provided
                        .iter()
                        .filter(|path| !partial::path_was_read(path, &read_exact, &read_subtree))
                        .cloned()
                        .collect(),
                },
                Err(error) => return diagnostic(error.path, error.message),
            },
            partial::PartialOutcome::Needs { paths, waiting } => CalculationPartialResult::Needs {
                needs: paths
                    .into_iter()
                    .map(|path| {
                        let rule = waiting
                            .get(&path)
                            .cloned()
                            .unwrap_or_else(|| contract.entry.clone());
                        contract.need_for_path(&case.input, path, rule)
                    })
                    .collect(),
            },
        };
        CalculationCaseOutcome::Result(CalculationResultCase {
            case_id: case.case_id.clone(),
            result: serde_json::to_value(result).expect("partial result is JSON"),
        })
    }
}

fn calculation_trace_enabled() -> bool {
    std::env::var("FUTURUNA_CALCULATION_TRACE")
        .ok()
        .is_some_and(|value| !value.is_empty() && value != "0" && value != "false")
}

/// The start of a traced interval. The clock is read only when tracing is on,
/// since a host without a clock (WebAssembly in Node) cannot read it.
fn trace_clock() -> Option<Instant> {
    calculation_trace_enabled().then(Instant::now)
}

fn trace_calculation_case(case_id: &str, started: Option<Instant>) {
    if let Some(started) = started {
        eprintln!(
            "runa calculation case {:?}: {:.3}s",
            case_id,
            started.elapsed().as_secs_f64()
        );
    }
}

fn requested_calculation_jobs() -> Option<usize> {
    std::env::var("FUTURUNA_CALCULATION_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|jobs| *jobs > 0)
}

fn calculation_worker_count(case_count: usize, requested_jobs: Option<usize>) -> usize {
    if case_count <= 1 {
        return case_count;
    }
    if let Some(jobs) = requested_jobs {
        return jobs.clamp(1, case_count);
    }
    if case_count < 4 {
        return 1;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(DEFAULT_MAX_CALCULATION_WORKERS)
            .clamp(1, case_count)
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

fn invoke_calculation_case_stride(
    worker: &mut CalculationWorker,
    contract: &CalculationContract,
    cases: &[(usize, &CalculationInputCase)],
    start: usize,
    step: usize,
) -> Vec<(usize, CalculationCaseOutcome)> {
    (start..cases.len())
        .step_by(step)
        .map(|position| {
            let (index, case) = cases[position];
            (index, worker.invoke(contract, case))
        })
        .collect()
}

fn invoke_calculation_case_queue(
    worker: &mut CalculationWorker,
    contract: &CalculationContract,
    cases: &[(usize, &CalculationInputCase)],
    next_case: &AtomicUsize,
) -> Vec<(usize, CalculationCaseOutcome)> {
    let mut outcomes = Vec::new();
    loop {
        let index = next_case.fetch_add(1, Ordering::Relaxed);
        let Some((original_index, case)) = cases.get(index) else {
            return outcomes;
        };
        outcomes.push((*original_index, worker.invoke(contract, case)));
    }
}

/// A calculation entry prepared once and invoked any number of times.
///
/// Preparing builds the state every case starts from: the runtime
/// rule-dispatch metadata and an interpreter that has initialized the program
/// (prelude, declarations, the top-level and metadata bindings the entry
/// reaches). It happens on the first batch with a ready case, and later
/// batches reuse it. Each case still starts from exactly that state: a fresh
/// environment over the initialized bindings, with the handler, rule-scope,
/// step-budget and random state initialization left. A case that changes an
/// actor or live stream the initialized program holds, or that panics, makes
/// the program be initialized again before the next case, so no case sees
/// another case's state. Results are the ones a fresh preparation gives.
///
/// A batch with four or more ready cases also runs on further workers, each
/// initializing its own interpreter for that batch (natively; WebAssembly runs
/// one worker).
pub struct PreparedCalculation {
    contract: CalculationContract,
    stmts: Arc<[Stmt]>,
    source_dir: Option<String>,
    worker: Option<CalculationWorker>,
}

impl PreparedCalculation {
    /// Prepare `contract`'s entry of the program `stmts` (with the prelude
    /// already prepended when the program uses it). Nothing runs until the
    /// first batch with a ready case.
    pub fn new(
        contract: CalculationContract,
        stmts: Arc<[Stmt]>,
        source_dir: Option<String>,
    ) -> Self {
        Self {
            contract,
            stmts,
            source_dir,
            worker: None,
        }
    }

    pub fn contract(&self) -> &CalculationContract {
        &self.contract
    }

    /// Decode, invoke, and encode a batch. Cases are isolated and retain input
    /// order. Larger batches use independent workers unless explicitly
    /// serialized.
    pub fn invoke(&mut self, envelope: &CalculationInputEnvelope) -> CalculationOutputEnvelope {
        self.invoke_batch(envelope, requested_calculation_jobs(), None)
    }

    /// Decode and invoke a batch of partial cases: each answers `Complete` or
    /// `Needs` (`docs/rfcs/calculation-needs.md`).
    pub fn invoke_partial(
        &mut self,
        envelope: &CalculationInputEnvelope,
        mode: partial::PartialMode,
    ) -> CalculationOutputEnvelope {
        self.invoke_batch(envelope, requested_calculation_jobs(), Some(mode))
    }

    fn invoke_batch(
        &mut self,
        envelope: &CalculationInputEnvelope,
        requested_jobs: Option<usize>,
        partial: Option<partial::PartialMode>,
    ) -> CalculationOutputEnvelope {
        let Self {
            contract,
            stmts,
            source_dir,
            worker,
        } = self;
        let contract = &*contract;
        let started = trace_clock();
        let mut output = CalculationOutputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: if partial.is_some() {
                    PARTIAL_OUTPUT_SCHEMA
                } else {
                    OUTPUT_SCHEMA
                }
                .to_string(),
                schema_hash: contract.schema_hash.clone(),
                entry: contract.entry.clone(),
            },
            results: Vec::new(),
            diagnostics: Vec::new(),
        };
        if let Err(diagnostics) = validate_input_envelope(contract, envelope) {
            output.diagnostics = diagnostics;
            return output;
        }
        // Do not initialize an interpreter for an entirely unfinished batch. An
        // unfinished case in a mixed batch also never enters a calculation worker.
        let mut readiness_outcomes = Vec::new();
        let cases: Vec<_> = envelope
            .cases
            .iter()
            .enumerate()
            .filter_map(|(index, case)| {
                let diagnostics = case.unfinished_diagnostics(index);
                if diagnostics.is_empty() {
                    Some((index, case))
                } else {
                    readiness_outcomes
                        .push((index, CalculationCaseOutcome::Diagnostics(diagnostics)));
                    None
                }
            })
            .collect();
        let worker_count = calculation_worker_count(cases.len(), requested_jobs);
        if worker_count == 0 {
            for (_, outcome) in readiness_outcomes {
                if let CalculationCaseOutcome::Diagnostics(diagnostics) = outcome {
                    output.diagnostics.extend(diagnostics);
                }
            }
            return output;
        }

        // The primary worker is the prepared state, and also primes the
        // shared parsed-module cache before peers initialize their own
        // isolated interpreters.
        let primary = worker.get_or_insert_with(|| {
            CalculationWorker::new(CalculationProgram::new(
                &contract.entry,
                stmts.clone(),
                source_dir.clone(),
            ))
        });
        primary.partial = partial;
        let initialized_after = started.map(|started| started.elapsed());
        let mut outcomes = if worker_count == 1 {
            invoke_calculation_case_stride(primary, contract, &cases, 0, 1)
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let next_case = AtomicUsize::new(0);
                let program = &primary.program.clone();
                let cases = &cases;
                std::thread::scope(|scope| {
                    let mut handles = Vec::with_capacity(worker_count - 1);
                    for worker_index in 1..worker_count {
                        let next_case = &next_case;
                        handles.push(
                            std::thread::Builder::new()
                                .name(format!("futuruna-calculation-{worker_index}"))
                                .stack_size(CALCULATION_WORKER_STACK_BYTES)
                                .spawn_scoped(scope, move || {
                                    let mut worker = CalculationWorker::new(program.clone());
                                    worker.partial = partial;
                                    invoke_calculation_case_queue(
                                        &mut worker,
                                        contract,
                                        cases,
                                        next_case,
                                    )
                                })
                                .expect("start calculation worker"),
                        );
                    }
                    let mut outcomes =
                        invoke_calculation_case_queue(primary, contract, cases, &next_case);
                    for handle in handles {
                        outcomes.extend(handle.join().expect("calculation worker panicked"));
                    }
                    outcomes
                })
            }
            #[cfg(target_arch = "wasm32")]
            {
                unreachable!("WASM calculation batches always use one worker")
            }
        };

        outcomes.extend(readiness_outcomes);
        outcomes.sort_by_key(|(index, _)| *index);
        for (_, outcome) in outcomes {
            match outcome {
                CalculationCaseOutcome::Result(result) => output.results.push(result),
                CalculationCaseOutcome::Diagnostic(diagnostic) => {
                    output.diagnostics.push(diagnostic)
                }
                CalculationCaseOutcome::Diagnostics(diagnostics) => {
                    output.diagnostics.extend(diagnostics)
                }
            }
        }
        if let (Some(started), Some(initialized_after)) = (started, initialized_after) {
            eprintln!(
                "runa calculation: {} case(s), {} worker(s), initialization {:.3}s, total {:.3}s",
                envelope.cases.len(),
                worker_count,
                initialized_after.as_secs_f64(),
                started.elapsed().as_secs_f64()
            );
        }
        output
    }
}

/// Deterministic generator of values for the completion test.
struct CompletionRng(u64);

impl CompletionRng {
    fn next(&mut self) -> u64 {
        // xorshift64, as `runa stress-gen` uses.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}

/// Integers a completion takes: boundaries of amounts and signs, and
/// pseudo-random values within ±10^12.
const COMPLETION_INTEGERS: [i64; 10] = [
    0,
    1,
    -1,
    2,
    100,
    -100,
    50_000,
    1_000_000,
    1_000_000_000_000,
    -1_000_000_000_000,
];

const COMPLETION_DEPTH_LIMIT: usize = 6;

impl CalculationContract {
    fn generate_completion_value(
        &self,
        ty: &CalculationTypeRef,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
        rng: &mut CompletionRng,
        depth: usize,
    ) -> JsonValue {
        let ty = substitute_contract_type(ty, substitutions);
        let shallow = depth >= COMPLETION_DEPTH_LIMIT;
        match ty {
            CalculationTypeRef::Primitive { name } => match name.as_str() {
                "Int" => {
                    if rng.below(3) == 0 {
                        let magnitude = (rng.next() % 1_000_000_000_000) as i64;
                        JsonValue::from(if rng.below(2) == 0 {
                            magnitude
                        } else {
                            -magnitude
                        })
                    } else {
                        JsonValue::from(COMPLETION_INTEGERS[rng.below(COMPLETION_INTEGERS.len())])
                    }
                }
                "Float" => JsonValue::from([0.0, 1.5, -2.25, 1e6, 0.001][rng.below(5)]),
                "Bool" => JsonValue::Bool(rng.below(2) == 0),
                "String" => JsonValue::from(["", "a", "DK0000000001", "x y"][rng.below(4)]),
                "Char" => JsonValue::from(["a", "Z", "0"][rng.below(3)]),
                _ => JsonValue::Null,
            },
            CalculationTypeRef::Unit => JsonValue::Null,
            CalculationTypeRef::Optional { item } => {
                if shallow || rng.below(3) == 0 {
                    JsonValue::Null
                } else {
                    self.generate_completion_value(&item, substitutions, rng, depth + 1)
                }
            }
            CalculationTypeRef::List { item } | CalculationTypeRef::Set { item } => {
                let length = if shallow { 0 } else { rng.below(3) };
                let mut items = Vec::new();
                for _ in 0..length {
                    let value =
                        self.generate_completion_value(&item, substitutions, rng, depth + 1);
                    if !items.contains(&value) {
                        items.push(value);
                    }
                }
                JsonValue::Array(items)
            }
            CalculationTypeRef::Map { value, .. } => {
                let mut object = JsonMap::new();
                if !shallow && rng.below(2) == 0 {
                    object.insert(
                        "k".to_string(),
                        self.generate_completion_value(&value, substitutions, rng, depth + 1),
                    );
                }
                JsonValue::Object(object)
            }
            CalculationTypeRef::TypeParameter { .. } => JsonValue::Null,
            CalculationTypeRef::Named { name, arguments } => {
                let Some(definition) = self.definition(&name) else {
                    return JsonValue::Null;
                };
                let local = definition_substitutions(definition, &arguments);
                if let Some(variant) = product_variant(definition) {
                    let mut object = JsonMap::new();
                    for field in &variant.fields {
                        object.insert(
                            field.name.clone(),
                            self.generate_completion_value(&field.ty, &local, rng, depth + 1),
                        );
                    }
                    return JsonValue::Object(object);
                }
                let candidates: Vec<&CalculationVariant> = if shallow {
                    let nullary: Vec<_> = definition
                        .variants
                        .iter()
                        .filter(|variant| variant.fields.is_empty())
                        .collect();
                    if nullary.is_empty() {
                        definition.variants.iter().collect()
                    } else {
                        nullary
                    }
                } else {
                    definition.variants.iter().collect()
                };
                let variant = candidates[rng.below(candidates.len())];
                let mut object = JsonMap::new();
                object.insert(
                    "$variant".to_string(),
                    JsonValue::from(variant.name.clone()),
                );
                if variant.positional {
                    object.insert(
                        "$values".to_string(),
                        JsonValue::Array(
                            variant
                                .fields
                                .iter()
                                .map(|field| {
                                    self.generate_completion_value(
                                        &field.ty,
                                        &local,
                                        rng,
                                        depth + 1,
                                    )
                                })
                                .collect(),
                        ),
                    );
                } else {
                    for field in &variant.fields {
                        object.insert(
                            field.name.clone(),
                            self.generate_completion_value(&field.ty, &local, rng, depth + 1),
                        );
                    }
                }
                JsonValue::Object(object)
            }
        }
    }

    /// Fill the left-out parts of a partial value of type `ty`.
    fn complete_value(
        &self,
        value: Option<&JsonValue>,
        ty: &CalculationTypeRef,
        substitutions: &BTreeMap<String, CalculationTypeRef>,
        rng: &mut CompletionRng,
        depth: usize,
    ) -> JsonValue {
        let Some(value) = value else {
            return self.generate_completion_value(ty, substitutions, rng, depth);
        };
        let ty = substitute_contract_type(ty, substitutions);
        match ty {
            CalculationTypeRef::Optional { item } if !value.is_null() => {
                self.complete_value(Some(value), &item, substitutions, rng, depth + 1)
            }
            CalculationTypeRef::List { item } | CalculationTypeRef::Set { item } => {
                match value.as_array() {
                    Some(items) => JsonValue::Array(
                        items
                            .iter()
                            .map(|element| {
                                self.complete_value(
                                    Some(element),
                                    &item,
                                    substitutions,
                                    rng,
                                    depth + 1,
                                )
                            })
                            .collect(),
                    ),
                    None => value.clone(),
                }
            }
            CalculationTypeRef::Map { value: item, .. } => match value.as_object() {
                Some(entries) => JsonValue::Object(
                    entries
                        .iter()
                        .map(|(key, element)| {
                            (
                                key.clone(),
                                self.complete_value(
                                    Some(element),
                                    &item,
                                    substitutions,
                                    rng,
                                    depth + 1,
                                ),
                            )
                        })
                        .collect(),
                ),
                None => value.clone(),
            },
            CalculationTypeRef::Named {
                ref name,
                ref arguments,
            } => {
                let (Some(definition), Some(object)) = (self.definition(name), value.as_object())
                else {
                    return value.clone();
                };
                let local = definition_substitutions(definition, arguments);
                let variant = match product_variant(definition) {
                    Some(variant) => variant,
                    None => {
                        let chosen = object.get("$variant").and_then(JsonValue::as_str);
                        let Some(variant) = chosen.and_then(|chosen| {
                            definition
                                .variants
                                .iter()
                                .find(|variant| variant.name == chosen)
                        }) else {
                            // `{}`: which alternative applies is not known yet.
                            return self.generate_completion_value(&ty, substitutions, rng, depth);
                        };
                        if variant.positional {
                            return value.clone();
                        }
                        variant
                    }
                };
                let mut completed = object.clone();
                for field in &variant.fields {
                    let field_value = self.complete_value(
                        object.get(&field.name),
                        &field.ty,
                        &local,
                        rng,
                        depth + 1,
                    );
                    completed.insert(field.name.clone(), field_value);
                }
                JsonValue::Object(completed)
            }
            _ => value.clone(),
        }
    }

    /// Completions of a partial case: every left-out part filled with
    /// generated values of its type. Deterministic for a seed.
    pub fn partial_case_completions(
        &self,
        input: &JsonValue,
        count: usize,
        seed: u64,
    ) -> Vec<JsonValue> {
        (0..count)
            .map(|index| {
                let mut rng = CompletionRng(
                    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        .wrapping_add(index as u64 + 1)
                        | 1,
                );
                self.complete_value(Some(input), &self.input, &BTreeMap::new(), &mut rng, 0)
            })
            .collect()
    }
}

/// JSON Schema of one case's `result` in a partial output envelope
/// (`futuruna.calculate.partial-output.v1`). The calculation's own output
/// type is the contract's `output`.
pub fn partial_result_json_schema(contract: &CalculationContract) -> JsonValue {
    let string = serde_json::json!({ "type": "string" });
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("Partial result of `{}`", contract.entry),
        "description": "A partial case answers Complete with its result, or Needs with the facts and entries that block it. See docs/rfcs/calculation-needs.md.",
        "x-futuruna-output-schema": PARTIAL_OUTPUT_SCHEMA,
        "x-futuruna-schema-hash": contract.schema_hash,
        "oneOf": [
            {
                "type": "object",
                "required": ["$variant", "result", "unused_paths"],
                "additionalProperties": false,
                "properties": {
                    "$variant": { "const": "Complete" },
                    "result": { "description": format!("The canonical JSON value of `{}`.", contract.output.display_name()) },
                    "unused_paths": { "type": "array", "items": string }
                }
            },
            {
                "type": "object",
                "required": ["$variant", "needs"],
                "additionalProperties": false,
                "properties": {
                    "$variant": { "const": "Needs" },
                    "needs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["kind", "path", "waiting_rule"],
                            "additionalProperties": false,
                            "properties": {
                                "kind": { "enum": ["fact", "entry"] },
                                "path": string,
                                "entry": string,
                                "subject": string,
                                "label": string,
                                "question": string,
                                "help": string,
                                "unit": string,
                                "sources": { "type": "array", "items": { "type": "object" } },
                                "waiting_rule": string
                            }
                        }
                    }
                }
            }
        ]
    })
}

/// Completions the completion test runs per `Complete` partial case.
pub const COMPLETION_TEST_COUNT: usize = 32;

impl PreparedCalculation {
    /// Run partial cases and check the guarantee: the strict-mode oracle
    /// (strict `Complete(r)` implies propagating `Complete(r)`) and the
    /// completion test (every completion of a `Complete(r)` case returns `r`
    /// or stops with an arithmetic-overflow error). Returns the propagating
    /// results; every violation is a diagnostic. Every run shares this
    /// prepared state.
    pub fn check_partial(
        &mut self,
        envelope: &CalculationInputEnvelope,
    ) -> CalculationOutputEnvelope {
        check_partial_calculation_cases(self, envelope)
    }
}

fn check_partial_calculation_cases(
    prepared: &mut PreparedCalculation,
    envelope: &CalculationInputEnvelope,
) -> CalculationOutputEnvelope {
    let jobs = requested_calculation_jobs();
    let mut propagated =
        prepared.invoke_batch(envelope, jobs, Some(partial::PartialMode::Propagate));
    let strict = prepared.invoke_batch(envelope, jobs, Some(partial::PartialMode::Strict));
    let parse = |case: &CalculationResultCase| {
        serde_json::from_value::<CalculationPartialResult>(case.result.clone()).ok()
    };
    let mut violations = Vec::new();
    for strict_case in &strict.results {
        let Some(CalculationPartialResult::Complete { result, .. }) = parse(strict_case) else {
            continue;
        };
        let agrees = propagated.results.iter().any(|case| {
            case.case_id == strict_case.case_id
                && matches!(parse(case), Some(CalculationPartialResult::Complete { result: ref propagated, .. }) if *propagated == result)
        });
        if !agrees {
            violations.push(CalculationCaseDiagnostic {
                case_id: strict_case.case_id.clone(),
                path: "$".to_string(),
                message: "strict-mode oracle: strict evaluation is Complete, propagation does not return the same result".to_string(),
            });
        }
    }
    for case in &propagated.results {
        let Some(CalculationPartialResult::Complete { result, .. }) = parse(case) else {
            continue;
        };
        let Some(input) = envelope
            .cases
            .iter()
            .find(|input| input.case_id == case.case_id)
        else {
            continue;
        };
        let completions =
            prepared
                .contract
                .partial_case_completions(&input.input, COMPLETION_TEST_COUNT, 0x5EED);
        let completion_envelope = CalculationInputEnvelope {
            futuruna: envelope.futuruna.clone(),
            cases: completions
                .into_iter()
                .enumerate()
                .map(|(index, completion)| CalculationInputCase {
                    case_id: format!("completion-{index}"),
                    input_status: Some(CalculationInputStatus::Ready),
                    input: completion,
                })
                .collect(),
        };
        let completed = prepared.invoke_batch(&completion_envelope, jobs, None);
        for completion in &completed.results {
            if completion.result != result {
                let index = completion
                    .case_id
                    .trim_start_matches("completion-")
                    .parse::<usize>()
                    .unwrap_or_default();
                violations.push(CalculationCaseDiagnostic {
                    case_id: case.case_id.clone(),
                    path: "$".to_string(),
                    message: format!(
                        "completion test: completion {} returns {} instead of {}",
                        serde_json::to_string(&completion_envelope.cases[index].input)
                            .unwrap_or_default(),
                        completion.result,
                        result
                    ),
                });
            }
        }
        for diagnostic in &completed.diagnostics {
            if !diagnostic.message.contains("overflow") {
                violations.push(CalculationCaseDiagnostic {
                    case_id: case.case_id.clone(),
                    path: diagnostic.path.clone(),
                    message: format!(
                        "completion test: {} fails: {}",
                        diagnostic.case_id, diagnostic.message
                    ),
                });
            }
        }
    }
    propagated.diagnostics.extend(violations);
    propagated
}

#[cfg(test)]
mod calculation_execution_tests {
    use super::*;

    /// One batch through a fresh preparation of `contract`'s entry.
    fn invoke_calculation_cases_with_jobs(
        contract: &CalculationContract,
        stmts: &[Stmt],
        source_dir: Option<String>,
        envelope: &CalculationInputEnvelope,
        requested_jobs: Option<usize>,
    ) -> CalculationOutputEnvelope {
        PreparedCalculation::new(contract.clone(), stmts.into(), source_dir).invoke_batch(
            envelope,
            requested_jobs,
            None,
        )
    }

    fn needs_fixture() -> (Vec<Stmt>, CalculationContract) {
        let source = r#"
# Filing = Single | Joint(partner_income: Int)
# Household(income: Int, married: Bool, filing: Filing)
# Result(tax: Int)

| base_tax(h: Household) -> h.income / 4
| exception relief base_tax(h: Household) -> h.income / 5 under h.income > 100 && h.married

@ calculate
| household_tax(h: Household) -> Result(tax = base_tax(h))
"#;
        let stmts = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect("fixture parses");
        let contract = extract_calculation_contracts(&stmts, source, None)
            .expect("fixture has a contract")
            .remove(0);
        (stmts, contract)
    }

    fn partial_envelope(
        contract: &CalculationContract,
        input: JsonValue,
    ) -> CalculationInputEnvelope {
        CalculationInputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: INPUT_SCHEMA.to_string(),
                schema_hash: contract.schema_hash.clone(),
                entry: contract.entry.clone(),
            },
            cases: vec![CalculationInputCase {
                case_id: "case".to_string(),
                input_status: Some(CalculationInputStatus::Ready),
                input,
            }],
        }
    }

    fn contract_from_source(source: &str) -> CalculationContract {
        let stmts = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect("fixture parses");
        extract_calculation_contracts(&stmts, source, None)
            .expect("fixture has a contract")
            .remove(0)
    }

    const NEEDS_FACT_SOURCE: &str = include_str!("../tests/expect/run/needs/needs_fact.runa");

    #[test]
    fn optional_composites_without_member_metadata_keep_their_schema_hash() {
        let input: JsonValue =
            serde_json::from_str(include_str!("../tests/expect/run/needs/needs_fact.json"))
                .expect("fixture input is JSON");
        let contract = contract_from_source(NEEDS_FACT_SOURCE);
        assert!(matches!(
            contract_type_at_path(&contract, "spouse"),
            Some(CalculationTypeRef::Optional { .. })
        ));
        assert_eq!(
            contract.schema_hash,
            input["$futuruna"]["schema_hash"].as_str().unwrap()
        );
        assert!(contract
            .input_columns()
            .iter()
            .all(|column| column.member_metadata.is_empty()));
    }

    #[test]
    fn optional_composite_member_metadata_is_a_contract_entry_and_a_need() {
        let plain = contract_from_source(NEEDS_FACT_SOURCE);
        let source = NEEDS_FACT_SOURCE.replace(
            "pathof(Household::married)",
            "pathof(Household::spouse::income)",
        );
        let contract = contract_from_source(&source);
        assert_ne!(contract.schema_hash, plain.schema_hash);
        assert_eq!(
            contract
                .field_metadata
                .iter()
                .map(|metadata| metadata.path.as_str())
                .collect::<Vec<_>>(),
            ["spouse.income"]
        );

        let columns = contract.input_columns();
        assert_eq!(columns.len(), plain.input_columns().len());
        let spouse = columns
            .iter()
            .find(|column| column.input_path == "spouse")
            .expect("the optional spouse stays one column");
        assert_eq!(spouse.encoding, CalculationColumnEncoding::Json);
        assert!(spouse.metadata.is_none());
        assert_eq!(spouse.member_metadata.len(), 1);
        assert_eq!(spouse.member_metadata[0].path, "spouse.income");

        let need = contract.need_for_path(
            &serde_json::json!({ "spouse": {} }),
            "spouse.income".to_string(),
            "rate".to_string(),
        );
        assert_eq!(
            need.question.as_deref(),
            Some("Were you married at the end of the year?")
        );
        assert_eq!(need.help.as_deref(), Some("Cohabiting spouses only."));
    }

    #[test]
    fn oracle_and_completion_test_pass_on_a_decided_case() {
        let (stmts, contract) = needs_fixture();
        let envelope = partial_envelope(&contract, serde_json::json!({ "income": 50 }));
        let checked =
            PreparedCalculation::new(contract, stmts.into(), None).check_partial(&envelope);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        assert_eq!(
            checked.results[0].result,
            serde_json::json!({ "$variant": "Complete", "result": { "tax": 12 }, "unused_paths": [] })
        );
    }

    #[test]
    fn completions_fill_every_left_out_part_and_keep_given_facts() {
        let (_, contract) = needs_fixture();
        let completions = contract.partial_case_completions(
            &serde_json::json!({ "income": 50, "filing": {} }),
            32,
            1,
        );
        let mut variants = BTreeSet::new();
        for completion in &completions {
            assert_eq!(completion["income"], 50);
            assert!(completion["married"].is_boolean());
            variants.insert(
                completion["filing"]["$variant"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
            assert!(contract.decode_input(completion).is_ok(), "{completion}");
        }
        assert_eq!(variants.len(), 2, "every variant is generated");
    }

    #[test]
    fn the_completion_test_rejects_a_result_some_completion_contradicts() {
        let (stmts, contract) = needs_fixture();
        let input = serde_json::json!({ "income": 1000 });
        let completions = contract.partial_case_completions(&input, 32, 0x5EED);
        let envelope = CalculationInputEnvelope {
            cases: completions
                .into_iter()
                .enumerate()
                .map(|(index, input)| CalculationInputCase {
                    case_id: format!("completion-{index}"),
                    input_status: Some(CalculationInputStatus::Ready),
                    input,
                })
                .collect(),
            ..partial_envelope(&contract, input)
        };
        let results: BTreeSet<String> =
            invoke_calculation_cases_with_jobs(&contract, &stmts, None, &envelope, Some(1))
                .results
                .iter()
                .map(|case| case.result.to_string())
                .collect();
        // `married` decides the result here, so a claimed Complete would be
        // contradicted by some completion.
        assert!(results.len() > 1, "{results:?}");
    }

    fn runtime_failure_fixture(
        source: &str,
        inputs: Vec<serde_json::Value>,
    ) -> (Vec<Stmt>, CalculationContract, CalculationInputEnvelope) {
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse runtime-failure calculation");
        let contract = extract_calculation_contracts(&statements, source, None)
            .expect("extract runtime-failure calculation")
            .pop()
            .expect("calculation contract");
        let envelope = CalculationInputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: INPUT_SCHEMA.to_string(),
                schema_hash: contract.schema_hash.clone(),
                entry: contract.entry.clone(),
            },
            cases: inputs
                .into_iter()
                .enumerate()
                .map(|(index, input)| CalculationInputCase {
                    case_id: format!("case-{index}"),
                    input_status: Some(CalculationInputStatus::Ready),
                    input,
                })
                .collect(),
        };
        (statements, contract, envelope)
    }

    #[test]
    fn calculation_imports_resolve_against_provided_source_files() {
        let _provided = crate::PROVIDED_SOURCE_FILES_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // No such directory exists: every import is served from the
        // host-provided files, as in the WebAssembly engine.
        let root = Path::new("/futuruna-provided-sources-test/model/src");
        crate::provide_source_files([
            (
                root.join("rates.runa"),
                "= rate_bp = 2700\n> taxed(amount: Int) -> Int { amount * rate_bp / 10000 }\n"
                    .to_string(),
            ),
            (
                root.join("money.runa"),
                "@ import ./rates\n# Input(amount: Int)\n".to_string(),
            ),
        ]);
        let source =
            "@ import ./money\n@ calculate\n> tax(input: Input) -> Int { taxed(input.amount) }\n";
        let source_dir = Some(root.to_string_lossy().into_owned());
        let statements = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect("parse calculation");
        let contract = extract_calculation_contracts(&statements, source, source_dir.clone())
            .expect("contract from provided imports")
            .pop()
            .expect("calculation contract");
        let envelope = CalculationInputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: INPUT_SCHEMA.to_string(),
                schema_hash: contract.schema_hash.clone(),
                entry: contract.entry.clone(),
            },
            cases: vec![CalculationInputCase {
                case_id: "provided".to_string(),
                input_status: Some(CalculationInputStatus::Ready),
                input: serde_json::json!({"amount": 100000}),
            }],
        };
        let output =
            PreparedCalculation::new(contract, statements.into(), source_dir).invoke(&envelope);
        crate::provide_source_files([]);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        assert_eq!(output.results[0].result, serde_json::json!(27000));
    }

    #[test]
    fn calculation_arithmetic_failures_are_case_scoped() {
        for (operator, left, right, expected) in [
            ("/", 7, 0, 4),
            ("%", 7, 0, 0),
            ("/", i64::MIN, -1, 4),
            ("%", i64::MIN, -1, 0),
            ("+", i64::MAX, 1, 10),
            ("-", i64::MIN, 1, 6),
            ("*", i64::MAX, 2, 16),
        ] {
            let source = format!(
                "# Input(left: Int, right: Int)\n@ calculate\n> calculate(input: Input) -> Int {{ input.left {operator} input.right }}\n"
            );
            let (statements, contract, envelope) = runtime_failure_fixture(
                &source,
                vec![
                    serde_json::json!({"left": 8, "right": 2}),
                    serde_json::json!({"left": left, "right": right}),
                    serde_json::json!({"left": 8, "right": 2}),
                ],
            );
            let serial = invoke_calculation_cases_with_jobs(
                &contract,
                &statements,
                None,
                &envelope,
                Some(1),
            );
            let parallel = invoke_calculation_cases_with_jobs(
                &contract,
                &statements,
                None,
                &envelope,
                Some(3),
            );
            assert_eq!(
                serial, parallel,
                "{operator} must isolate failures identically"
            );
            assert_eq!(serial.results.len(), 2, "{operator}: {serial:?}");
            assert_eq!(serial.results[0].case_id, "case-0");
            assert_eq!(serial.results[1].case_id, "case-2");
            assert!(serial.results.iter().all(|case| case.result == expected));
            assert_eq!(serial.diagnostics.len(), 1);
            assert_eq!(serial.diagnostics[0].case_id, "case-1");
            assert!(serial.diagnostics[0].message.contains("integer"));
        }
    }

    #[test]
    fn calculation_partial_value_misses_are_not_empty_successes() {
        let source = "# Input(year: Int)\n| parameter(year: Int) -> \"known\" under year == 2026\n@ calculate\n> calculate(input: Input) -> String { parameter(input.year) }\n";
        let (statements, contract, envelope) = runtime_failure_fixture(
            source,
            vec![
                serde_json::json!({"year": 2026}),
                serde_json::json!({"year": 2027}),
                serde_json::json!({"year": 2026}),
            ],
        );
        for jobs in [1, 3] {
            let output = invoke_calculation_cases_with_jobs(
                &contract,
                &statements,
                None,
                &envelope,
                Some(jobs),
            );
            assert_eq!(output.results.len(), 2, "{output:?}");
            assert!(output.results.iter().all(|case| case.result == "known"));
            assert_eq!(output.diagnostics.len(), 1);
            assert_eq!(output.diagnostics[0].case_id, "case-1");
            assert!(output.diagnostics[0].message.contains("parameter"));
        }
    }

    #[test]
    fn calculation_unsupported_arithmetic_cannot_be_consumed_as_empty_list() {
        // Exercise the runtime backstop independently of frontend rejection.
        // A future unchecked/indirect caller must not observe successful zero.
        let source = r#"
# Input(bad: Bool, left: List(Int), right: List(Int))
@ calculate
> calculate(input: Input) -> Int {
    if input.bad { length(input.left + input.right) }
    else { length(concat(input.left, input.right)) }
}
"#;
        let (statements, contract, envelope) = runtime_failure_fixture(
            source,
            [false, true, false]
                .into_iter()
                .map(|bad| serde_json::json!({"bad":bad,"left":[1,2],"right":[3]}))
                .collect(),
        );
        let serial =
            invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(1));
        let parallel =
            invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(3));
        assert_eq!(serial, parallel);
        assert_eq!(serial.results.len(), 2, "{serial:?}");
        assert_eq!(serial.results[0].case_id, "case-0");
        assert_eq!(serial.results[1].case_id, "case-2");
        assert!(serial.results.iter().all(|case| case.result == 3));
        assert_eq!(serial.diagnostics.len(), 1);
        assert_eq!(serial.diagnostics[0].case_id, "case-1");
        assert!(serial.diagnostics[0]
            .message
            .contains("unsupported operands"));
    }

    #[test]
    fn calculation_runtime_faults_do_not_abort_other_cases() {
        for body in [
            "head(filter([input.value], |value: Int| value != 0))",
            "first(filter([input.value], |value: Int| value != 0))",
            "last(filter([input.value], |value: Int| value != 0))",
            "nth([7], input.value)",
            "[7][input.value]",
            "{ assert(input.value == 0)\n7 }",
        ] {
            let source = format!(
                "# Input(value: Int)\n@ calculate\n> calculate(input: Input) -> Int {{ {body} }}\n"
            );
            let (good, bad) = if body.contains("filter") {
                (7, 0)
            } else {
                (0, 1)
            };
            let (statements, contract, envelope) = runtime_failure_fixture(
                &source,
                vec![
                    serde_json::json!({"value": good}),
                    serde_json::json!({"value": bad}),
                    serde_json::json!({"value": good}),
                ],
            );
            for jobs in [1, 3] {
                let output = invoke_calculation_cases_with_jobs(
                    &contract,
                    &statements,
                    None,
                    &envelope,
                    Some(jobs),
                );
                assert_eq!(output.results.len(), 2, "{body}: {output:?}");
                assert!(output.results.iter().all(|case| case.result == 7));
                assert_eq!(output.results[1].case_id, "case-2");
                assert_eq!(output.diagnostics.len(), 1);
                assert_eq!(output.diagnostics[0].case_id, "case-1");
            }
        }
    }

    #[test]
    fn calculation_assert_with_message_is_case_local_and_precedes_bad_math() {
        let source = r#"
# Input(year: Int)
@ calculate
> calculate(input: Input) -> Int {
    assert_with_message(input.year == 2026, "year {" + show(input.year) + "}: use a supported model")
    100 / (2027 - input.year)
}
"#;
        let (statements, contract, envelope) = runtime_failure_fixture(
            source,
            [2026, 2027, 2026]
                .into_iter()
                .map(|year| serde_json::json!({"year":year}))
                .collect(),
        );
        for jobs in [1, 3] {
            let output = invoke_calculation_cases_with_jobs(
                &contract,
                &statements,
                None,
                &envelope,
                Some(jobs),
            );
            assert_eq!(output.results.len(), 2, "{output:?}");
            assert!(output.results.iter().all(|case| case.result == 100));
            assert_eq!(output.results[1].case_id, "case-2");
            assert_eq!(output.diagnostics.len(), 1);
            assert_eq!(output.diagnostics[0].case_id, "case-1");
            assert!(
                output.diagnostics[0]
                    .message
                    .contains("Assertion failed: year {2027}: use a supported model"),
                "{output:?}"
            );
        }
    }

    #[test]
    fn calculation_builtin_arithmetic_is_checked() {
        for (body, bad, expected) in [
            ("-input.value", i64::MIN, -7),
            ("abs(input.value)", i64::MIN, 7),
            ("sum([input.value, 1])", i64::MAX, 8),
            ("sum_list([input.value, 1])", i64::MAX, 8),
            ("time_diff(input.value, 1)", i64::MIN, 6),
        ] {
            let source = format!(
                "# Input(value: Int)\n@ calculate\n> calculate(input: Input) -> Int {{ {body} }}\n"
            );
            let (statements, contract, envelope) = runtime_failure_fixture(
                &source,
                vec![
                    serde_json::json!({"value": 7}),
                    serde_json::json!({"value": bad}),
                    serde_json::json!({"value": 7}),
                ],
            );
            for jobs in [1, 3] {
                let output = invoke_calculation_cases_with_jobs(
                    &contract,
                    &statements,
                    None,
                    &envelope,
                    Some(jobs),
                );
                assert_eq!(output.results.len(), 2, "{body}: {output:?}");
                assert!(output.results.iter().all(|case| case.result == expected));
                assert_eq!(output.diagnostics.len(), 1);
                assert_eq!(output.diagnostics[0].case_id, "case-1");
                assert!(output.diagnostics[0].message.contains("integer"));
            }
        }
    }

    #[test]
    fn calculation_rejects_non_finite_intermediates_even_with_boolean_output() {
        for (body, bad, expected) in [
            (
                "input.left / input.right > 0.0",
                serde_json::json!({"left": 1.0, "right": 0.0}),
                "division by zero",
            ),
            (
                "input.left / input.right > 0.0",
                serde_json::json!({"left": 0.0, "right": 0.0}),
                "division by zero",
            ),
            (
                "input.left * input.right > 0.0",
                serde_json::json!({"left": 1e308, "right": 1e308}),
                "non-finite",
            ),
            (
                "sqrt(input.left) > 0.0",
                serde_json::json!({"left": -1.0, "right": 1.0}),
                "non-finite",
            ),
        ] {
            let source = format!("# Input(left: Float, right: Float)\n@ calculate\n> calculate(input: Input) -> Bool {{ {body} }}\n");
            let (statements, contract, envelope) = runtime_failure_fixture(
                &source,
                vec![
                    serde_json::json!({"left": 7.0, "right": 1.0}),
                    bad,
                    serde_json::json!({"left": 7.0, "right": 1.0}),
                ],
            );
            for jobs in [1, 3] {
                let output = invoke_calculation_cases_with_jobs(
                    &contract,
                    &statements,
                    None,
                    &envelope,
                    Some(jobs),
                );
                assert_eq!(output.results.len(), 2, "{body}: {output:?}");
                assert!(output.results.iter().all(|case| case.result == true));
                assert_eq!(output.diagnostics.len(), 1);
                assert!(
                    output.diagnostics[0].message.contains(expected),
                    "{body}: {:?}",
                    output.diagnostics[0].message
                );
            }
        }
    }

    #[test]
    fn calculation_required_initialization_fault_is_not_a_successful_zero() {
        let (statements, contract, envelope) = runtime_failure_fixture(
            "# Input(value: Int)\n= parameter = 7 / 0\n@ calculate\n> calculate(input: Input) -> Int { parameter + input.value }\n",
            vec![serde_json::json!({"value": 1}), serde_json::json!({"value": 2})],
        );
        for jobs in [1, 2] {
            let output = invoke_calculation_cases_with_jobs(
                &contract,
                &statements,
                None,
                &envelope,
                Some(jobs),
            );
            assert!(output.results.is_empty(), "{output:?}");
            assert_eq!(output.diagnostics.len(), 2);
            assert!(output
                .diagnostics
                .iter()
                .all(|case| case.message.contains("initialization failed")
                    && case.message.contains("integer division")));
        }
    }

    #[test]
    fn calculation_scoped_and_first_class_value_misses_are_diagnostic() {
        for (declarations, body, rule) in [
            ("| parameter(year: Int) -> \"known\" under year == 2026", "{ = callback = parameter\ncallback(input.year) }", "parameter/1"),
            ("# Policy(year: Int) {\n| parameter() -> \"known\" under year == 2026\n| summary() -> parameter()\n}", "Policy(input.year).parameter()", "Policy.parameter/0"),
            ("# Policy(year: Int) {\n| parameter() -> \"known\" under year == 2026\n| summary() -> parameter()\n}", "Policy(input.year).summary()", "Policy.parameter/0"),
        ] {
            let source = format!("# Input(year: Int)\n{declarations}\n@ calculate\n> calculate(input: Input) -> String {{ {body} }}\n");
            let (statements, contract, envelope) = runtime_failure_fixture(&source, vec![
                serde_json::json!({"year": 2026}), serde_json::json!({"year": 2027}), serde_json::json!({"year": 2026}),
            ]);
            for jobs in [1, 3] {
                let output = invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(jobs));
                assert_eq!(output.results.len(), 2, "{body}: {output:?}");
                assert!(output.results.iter().all(|case| case.result == "known"));
                assert_eq!(output.diagnostics.len(), 1);
                assert!(output.diagnostics[0].message.contains(rule), "{output:?}");
            }
        }
    }

    #[test]
    fn calculation_preserves_captured_boolean_misses_without_proof_certificates() {
        let source = r#"
# Input(value: Int, divisor: Int)
# Output(direct: Bool, delegated: Bool)
# Policy(value: Int, divisor: Int) {
    | predicate() -> True under value / divisor > 0
    | delegated() -> predicate()
}
@ calculate
> calculate(input: Input) -> Output {
    = policy = Policy(input.value, input.divisor)
    Output(policy.predicate(), policy.delegated())
}
"#;
        let (statements, contract, envelope) = runtime_failure_fixture(
            source,
            vec![
                serde_json::json!({"value": 0, "divisor": 1}),
                serde_json::json!({"value": 0, "divisor": 0}),
                serde_json::json!({"value": 1, "divisor": 1}),
            ],
        );
        let (_, _, miss_safe) = TypeChecker::rule_dispatch_metadata_for_runtime(&statements, None);
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        for name in ["predicate", "delegated"] {
            let key = RuleDispatchKey {
                scope: Some("Policy".into()),
                name: name.into(),
                arity: 0,
            };
            assert!(miss_safe.contains(&key));
            assert!(!artifacts
                .rule_dispatch_boolean_miss_safe_keys
                .contains(&key));
        }
        for jobs in [1, 3] {
            let output = invoke_calculation_cases_with_jobs(
                &contract,
                &statements,
                None,
                &envelope,
                Some(jobs),
            );
            assert_eq!(output.results.len(), 2, "{output:?}");
            assert_eq!(
                output.results[0].result,
                serde_json::json!({"direct": false, "delegated": false})
            );
            assert_eq!(
                output.results[1].result,
                serde_json::json!({"direct": true, "delegated": true})
            );
            assert_eq!(output.diagnostics.len(), 1);
            assert_eq!(output.diagnostics[0].case_id, "case-1");
            assert!(output.diagnostics[0].message.contains("integer division"));
        }
    }

    #[test]
    fn calculation_inline_module_rules_are_callable_and_namespace_owned() {
        let source = r#"
# Input(value: Int)
# Output(first: Int, second: Int, count: Int, eligible: Bool)
| amount(value: Int) -> value + 1000
> module First {
    | amount(value: Int) -> value + 10
    | amounts(value: Int) -> [value, value + 1]
    | eligible(value: Int) -> True under value > 0
}
> module Second {
    | amount(value: Int) -> value + 20
}
> module Partial {
    | amount(value: Int) -> value + 30 under value >= 0
}
@ calculate
> calculate(input: Input) -> Output {
    = callback = First.amount
    if input.value < 0 {
        Output(Partial.amount(input.value), Second.amount(input.value), length(First.amounts(input.value)), First.eligible(input.value))
    } else {
        Output(callback(input.value), Second.amount(input.value), length(First.amounts(input.value)), First.eligible(input.value))
    }
}
"#;
        let (statements, contract, envelope) = runtime_failure_fixture(
            source,
            vec![
                serde_json::json!({"value":0}),
                serde_json::json!({"value":-1}),
                serde_json::json!({"value":2}),
            ],
        );
        let serial =
            invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(1));
        let parallel =
            invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(3));
        assert_eq!(serial, parallel);
        assert_eq!(serial.results.len(), 2, "{serial:?}");
        assert_eq!(serial.results[0].case_id, "case-0");
        assert_eq!(
            serial.results[0].result,
            serde_json::json!({"first":10,"second":20,"count":2,"eligible":false})
        );
        assert_eq!(serial.results[1].case_id, "case-2");
        assert_eq!(
            serial.results[1].result,
            serde_json::json!({"first":12,"second":22,"count":2,"eligible":true})
        );
        assert_eq!(serial.diagnostics.len(), 1);
        assert_eq!(serial.diagnostics[0].case_id, "case-1");
        assert!(serial.diagnostics[0].message.contains("amount/1"));
    }

    #[test]
    fn calculation_non_callable_values_fail_without_rejecting_valid_unit_results() {
        let mut interpreter = Interpreter::new();
        let env = interpreter.default_env();
        for callee in [Value::Unit, Value::Int(7)] {
            let failure =
                interpreter.with_calculation_runtime(|runtime| runtime.apply(callee, vec![], &env));
            assert!(
                matches!(failure, Err(CalculationRuntimeFailure::InvalidValue(message)) if message.contains("non-callable value"))
            );
        }
        let valid = interpreter.with_calculation_runtime(|runtime| {
            runtime.eval_builtin("assert", vec![Value::Bool(true)], &env)
        });
        assert!(matches!(valid, Ok(Value::Unit)));
    }

    #[test]
    fn calculation_inline_module_misses_use_their_own_return_types() {
        let source = r#"
# Input(value: Int)
# Output(eligible: Bool, amount: Int)
| parameter(value: Int) -> True under value > 0
> module Policy {
    | parameter(value: Int) -> 7 under value > 0
    | predicate(value: Int) -> True under value > 0
    > eligible(value: Int) -> Bool { predicate(value) }
    > amount(value: Int) -> Int { parameter(value) }
}
@ calculate
> calculate(input: Input) -> Output {
    if input.value == 0 {
        Output(Policy.eligible(input.value), 0)
    } else {
        Output(Policy.eligible(input.value), Policy.amount(input.value))
    }
}
"#;
        let (statements, contract, envelope) = runtime_failure_fixture(
            source,
            vec![
                serde_json::json!({"value": 0}),
                serde_json::json!({"value": -1}),
                serde_json::json!({"value": 1}),
            ],
        );
        for jobs in [1, 3] {
            let output = invoke_calculation_cases_with_jobs(
                &contract,
                &statements,
                None,
                &envelope,
                Some(jobs),
            );
            assert_eq!(output.results.len(), 2, "{output:?}");
            assert_eq!(
                output.results[0].result,
                serde_json::json!({"eligible":false,"amount":0})
            );
            assert_eq!(
                output.results[1].result,
                serde_json::json!({"eligible":true,"amount":7})
            );
            assert_eq!(output.diagnostics.len(), 1);
            assert_eq!(output.diagnostics[0].case_id, "case-1");
            assert!(output.diagnostics[0].message.contains("parameter/1"));
        }
    }

    #[test]
    fn calculation_preserves_valid_empty_zero_false_and_large_collection_values() {
        // Equality traverses the linked-list representation. Use the normal
        // calculation-worker stack, not libtest's small default thread stack.
        std::thread::Builder::new()
            .stack_size(CALCULATION_WORKER_STACK_BYTES)
            .spawn(|| {
        let source = r#"
# Input(value: Int)
# Output(zero: Int, empty: String, predicate: Bool, descending: List(Int), same: Bool)
| empty() -> ""
| predicate(value: Int) -> value > 0
@ calculate
> calculate(input: Input) -> Output {
    Output(input.value / 2, empty(), predicate(input.value), range(3, 1), range(0, 600) == range(0, 600))
}
"#;
        let (statements, contract, envelope) =
            runtime_failure_fixture(source, vec![serde_json::json!({"value": 0})]);
        let output =
            invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(1));
        assert!(output.diagnostics.is_empty(), "{output:?}");
        assert_eq!(
            output.results[0].result,
            serde_json::json!({"zero": 0, "empty": "", "predicate": false, "descending": [], "same": true})
        );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn calculation_guard_restores_state_after_unexpected_panic() {
        let mut interpreter = Interpreter::new();
        let failure: Result<(), _> = interpreter.with_calculation_runtime(|runtime| {
            runtime.calculation_fail("first error");
            panic!("synthetic internal runtime failure");
        });
        assert_eq!(failure, Err(CalculationRuntimeFailure::Panicked));
        assert!(!interpreter.checking_calculation);
        assert!(interpreter.calculation_error.borrow().is_none());
        assert_eq!(interpreter.with_calculation_runtime(|_| 7), Ok(7));

        // Mixed sums use Float arithmetic, not an unused overflowing Int
        // accumulator. The ordinary mode is deliberately not changed here.
        let env = interpreter.default_env();
        let sum = interpreter
            .with_calculation_runtime(|runtime| {
                runtime.eval_builtin(
                    "sum",
                    vec![Value::List(vec![
                        Value::Int(i64::MAX),
                        Value::Int(i64::MAX),
                        Value::Float(1.0),
                    ])],
                    &env,
                )
            })
            .unwrap();
        assert!(matches!(sum, Value::Float(value) if value.is_finite() && value > i64::MAX as f64));
    }

    fn batch_fixture() -> (Vec<Stmt>, CalculationContract, CalculationInputEnvelope) {
        let source = r#"
# BatchInput(value: Int)
# BatchResult(doubled: Int)

@ calculate("Batch calculation")
| calculate_batch(input: BatchInput) -> BatchResult(doubled = input.value * 2)
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse batch calculation");
        let contract = extract_calculation_contracts(&stmts, source, None)
            .expect("extract batch calculation")
            .pop()
            .expect("batch contract");
        let cases = (0..12)
            .map(|index| CalculationInputCase {
                case_id: format!("case-{index:02}"),
                input_status: Some(CalculationInputStatus::Ready),
                input: if index == 7 {
                    serde_json::json!({"value": "invalid"})
                } else {
                    serde_json::json!({"value": index})
                },
            })
            .collect();
        let envelope = CalculationInputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: INPUT_SCHEMA.to_string(),
                schema_hash: contract.schema_hash.clone(),
                entry: contract.entry.clone(),
            },
            cases,
        };
        (stmts, contract, envelope)
    }

    #[test]
    fn generated_calculation_cases_require_filled_values_and_explicit_readiness() {
        let source = r#"
# Filing = Single | Joint(partner: Int)
# Intake(amount: Int, items: List(Int), optional: Int?, enabled: Bool, filing: Filing)
@ calculate
> echo(input: Intake) -> Intake { input }
"#;
        let facts = serde_json::json!({"amount": 0, "items": [], "optional": null, "enabled": false, "filing": {"$variant": "Single"}});
        let (stmts, contract, _) = runtime_failure_fixture(source, vec![facts.clone()]);
        let mut template = contract.template_envelope();
        assert_eq!(template.futuruna.schema, "futuruna.calculate.input.v2");
        assert_eq!(
            template.cases[0].input_status,
            Some(CalculationInputStatus::Draft)
        );

        // An unedited template, even when marked ready, names every value
        // still to be supplied and runs nothing.
        template.cases[0].input_status = Some(CalculationInputStatus::Ready);
        let unedited =
            invoke_calculation_cases_with_jobs(&contract, &stmts, None, &template, Some(1));
        assert!(unedited.results.is_empty());
        let unfilled: BTreeSet<_> = unedited
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.path.as_str())
            .collect();
        assert_eq!(
            unfilled,
            BTreeSet::from([
                "$.cases[0].input.amount",
                "$.cases[0].input.enabled",
                "$.cases[0].input.filing",
                "$.cases[0].input.items",
                "$.cases[0].input.optional",
            ])
        );
        // A placeholder is never a valid value of its type.
        for field in ["amount", "enabled", "filing", "items", "optional"] {
            let mut input = facts.clone();
            input[field] = template.cases[0].input[field].clone();
            assert!(
                contract.decode_input(&input).is_err(),
                "placeholder for `{field}` decoded as a value"
            );
        }

        template.cases[0].input = facts.clone();
        template.cases[0].input_status = Some(CalculationInputStatus::Draft);
        let draft = invoke_calculation_cases_with_jobs(&contract, &stmts, None, &template, Some(1));
        assert!(draft.results.is_empty());
        assert_eq!(draft.diagnostics.len(), 1);
        assert_eq!(draft.diagnostics[0].path, "$.cases[0].input_status");

        template.cases[0].input_status = Some(CalculationInputStatus::Ready);
        let ready = invoke_calculation_cases_with_jobs(&contract, &stmts, None, &template, Some(1));
        assert!(ready.diagnostics.is_empty(), "{:?}", ready.diagnostics);
        assert_eq!(ready.results[0].result, facts);

        for schema in [INPUT_SCHEMA, LEGACY_INPUT_SCHEMA] {
            template.futuruna.schema = schema.to_string();
            template.cases[0].input_status = None;
            let missing =
                invoke_calculation_cases_with_jobs(&contract, &stmts, None, &template, Some(1));
            assert!(missing.results.is_empty());
            assert_eq!(missing.diagnostics[0].path, "$.cases[0].input_status");
        }
    }

    #[test]
    fn calculation_readiness_precedes_required_runtime_initialization() {
        let (statements, contract, mut envelope) = runtime_failure_fixture(
            "# Input(value: Int)\n= parameter = 7 / 0\n@ calculate\n> calculate(input: Input) -> Int { parameter + input.value }\n",
            vec![serde_json::json!({"value": 1}), serde_json::json!({"value": 2})],
        );
        envelope.cases[0].input_status = Some(CalculationInputStatus::Draft);
        envelope.cases[1].input_status = None;
        let draft =
            invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(2));
        assert!(draft.results.is_empty());
        assert_eq!(draft.diagnostics.len(), 2);
        assert!(draft
            .diagnostics
            .iter()
            .all(|case| case.message.contains("not marked ready")));

        envelope.cases[1].input_status = Some(CalculationInputStatus::Ready);
        let mixed =
            invoke_calculation_cases_with_jobs(&contract, &statements, None, &envelope, Some(2));
        assert!(mixed.results.is_empty());
        assert_eq!(mixed.diagnostics.len(), 2);
        assert!(mixed.diagnostics[0].message.contains("not marked ready"));
        assert!(mixed.diagnostics[1]
            .message
            .contains("initialization failed"));
    }

    #[test]
    fn calculation_readiness_keeps_mixed_batch_order_and_isolation() {
        let (stmts, contract, mut envelope) = batch_fixture();
        envelope.cases[1].input_status = Some(CalculationInputStatus::Draft);
        envelope.cases[3].input_status = None;
        let serial =
            invoke_calculation_cases_with_jobs(&contract, &stmts, None, &envelope, Some(1));
        let parallel =
            invoke_calculation_cases_with_jobs(&contract, &stmts, None, &envelope, Some(3));
        assert_eq!(serial, parallel);
        assert_eq!(serial.results.len(), 9);
        assert_eq!(serial.results[0].result, serde_json::json!({"doubled": 0}));
        assert_eq!(
            serial
                .diagnostics
                .iter()
                .map(|row| row.case_id.as_str())
                .collect::<Vec<_>>(),
            ["case-01", "case-03", "case-07"]
        );
        assert_eq!(serial.diagnostics[0].path, "$.cases[1].input_status");
        assert_eq!(serial.diagnostics[1].path, "$.cases[3].input_status");
    }

    #[test]
    fn parallel_calculation_cases_match_serial_results_and_order() {
        let (stmts, contract, envelope) = batch_fixture();
        let serial =
            invoke_calculation_cases_with_jobs(&contract, &stmts, None, &envelope, Some(1));
        let parallel =
            invoke_calculation_cases_with_jobs(&contract, &stmts, None, &envelope, Some(4));

        assert_eq!(parallel, serial);
        assert_eq!(
            parallel
                .results
                .iter()
                .map(|result| result.case_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "case-00", "case-01", "case-02", "case-03", "case-04", "case-05", "case-06",
                "case-08", "case-09", "case-10", "case-11"
            ]
        );
        assert_eq!(parallel.diagnostics[0].case_id, "case-07");
    }

    #[test]
    fn automatic_calculation_workers_avoid_small_batch_overhead() {
        assert_eq!(calculation_worker_count(0, None), 0);
        assert_eq!(calculation_worker_count(1, None), 1);
        assert_eq!(calculation_worker_count(3, None), 1);
        assert_eq!(calculation_worker_count(8, Some(3)), 3);
        assert_eq!(calculation_worker_count(2, Some(8)), 2);
    }

    #[test]
    fn calculation_initialization_skips_top_level_scenario_checks_and_effects() {
        let source = r#"
# Input(value: Int)

@ calculate("Scenario-backed calculation")
| calculate_scenario(input: Input) -> input.value

| unrelated_scenario_check: 1 -> 1 == 1
? unrelated_scenario_check

@ print("scenario output must not run during a calculation call")
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse scenario calculation");

        let worker = CalculationWorker::new(CalculationProgram::new(
            "calculate_scenario",
            stmts.into(),
            None,
        ));

        assert!(worker.interpreter.output.is_empty());
    }

    #[test]
    fn calculation_runtime_uses_canonical_boolean_rule_misses() {
        let source = r#"
# BoolInput(value: Int)
# BoolResult(conditional: Bool, exception_only: Bool)

| conditional(value: Int) -> True under value > 0
| exception_only(value: Int) -> True under value > 100
| exception positive exception_only(value: Int) -> True under value > 0

@ calculate("Boolean miss calculation")
| calculate_boolean_misses(input: BoolInput) -> BoolResult(
    conditional = conditional(input.value),
    exception_only = exception_only(input.value)
)
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse Boolean calculation");
        let contract = extract_calculation_contracts(&stmts, source, None)
            .expect("extract Boolean calculation")
            .pop()
            .expect("Boolean calculation contract");
        let envelope = CalculationInputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: INPUT_SCHEMA.to_string(),
                schema_hash: contract.schema_hash.clone(),
                entry: contract.entry.clone(),
            },
            cases: vec![CalculationInputCase {
                case_id: "miss".to_string(),
                input_status: Some(CalculationInputStatus::Ready),
                input: serde_json::json!({"value": 0}),
            }],
        };

        let output =
            invoke_calculation_cases_with_jobs(&contract, &stmts, None, &envelope, Some(1));

        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            output.diagnostics
        );
        assert_eq!(
            output.results[0].result,
            serde_json::json!({"conditional": false, "exception_only": false})
        );
    }

    #[test]
    fn calculation_collections_round_trip_through_structural_runtime_keys() {
        let source = r#"
# CollectionInput(values: Map(String, Int), tags: Set(String))

@ calculate("Collection calculation")
| calculate_collections(input: CollectionInput) -> CollectionInput(
    values = input.values,
    tags = input.tags
)
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser
            .parse_program()
            .expect("parse collection calculation");
        let contract = extract_calculation_contracts(&stmts, source, None)
            .expect("extract collection calculation")
            .pop()
            .expect("collection calculation contract");
        let input = serde_json::json!({
            "values": {"alpha": 1, "beta": 2},
            "tags": ["first", "second"]
        });
        let decoded = contract.decode_input(&input).expect("decode collections");
        let Value::NamedConstructor(record_name, fields) = &decoded else {
            panic!("expected decoded collection record");
        };
        let values = fields
            .iter()
            .find(|(name, _)| name == "values")
            .map(|(_, value)| value.clone())
            .expect("decoded map");
        let tags = fields
            .iter()
            .find(|(name, _)| name == "tags")
            .map(|(_, value)| value.clone())
            .expect("decoded set");

        let mut interpreter = Interpreter::new();
        let env = interpreter.default_env();
        assert!(matches!(
            interpreter.eval_builtin(
                "map_get_or",
                vec![values.clone(), Value::Str("alpha".into()), Value::Int(-1)],
                &env,
            ),
            Value::Int(1)
        ));
        assert!(matches!(
            interpreter.eval_builtin(
                "map_contains",
                vec![values.clone(), Value::Str("beta".into())],
                &env,
            ),
            Value::Bool(true)
        ));
        let replaced = interpreter.eval_builtin(
            "map_insert",
            vec![values.clone(), Value::Str("alpha".into()), Value::Int(9)],
            &env,
        );
        assert!(matches!(
            interpreter.eval_builtin("map_len", vec![replaced.clone()], &env),
            Value::Int(2)
        ));
        assert!(matches!(
            interpreter.eval_builtin(
                "map_get_or",
                vec![replaced.clone(), Value::Str("alpha".into()), Value::Int(-1)],
                &env,
            ),
            Value::Int(9)
        ));
        let removed_values = interpreter.eval_builtin(
            "map_remove",
            vec![replaced, Value::Str("beta".into())],
            &env,
        );
        assert!(matches!(
            interpreter.eval_builtin(
                "map_contains",
                vec![removed_values.clone(), Value::Str("beta".into())],
                &env,
            ),
            Value::Bool(false)
        ));

        assert!(matches!(
            interpreter.eval_builtin(
                "set_contains",
                vec![tags.clone(), Value::Str("first".into())],
                &env,
            ),
            Value::Bool(true)
        ));
        let duplicate = interpreter.eval_builtin(
            "set_insert",
            vec![tags.clone(), Value::Str("first".into())],
            &env,
        );
        assert!(matches!(
            interpreter.eval_builtin("set_len", vec![duplicate.clone()], &env),
            Value::Int(2)
        ));
        let removed_tags = interpreter.eval_builtin(
            "set_remove",
            vec![duplicate, Value::Str("second".into())],
            &env,
        );
        assert!(matches!(
            interpreter.eval_builtin(
                "set_contains",
                vec![removed_tags.clone(), Value::Str("second".into())],
                &env,
            ),
            Value::Bool(false)
        ));

        let encoded = contract
            .encode_output(&decoded)
            .expect("encode collections");
        assert_eq!(encoded["values"], input["values"]);
        let mut encoded_tags = encoded["tags"]
            .as_array()
            .expect("encoded set array")
            .iter()
            .map(|value| value.as_str().expect("encoded String set").to_string())
            .collect::<Vec<_>>();
        encoded_tags.sort();
        assert_eq!(encoded_tags, ["first", "second"]);
        let encoded_json = serde_json::to_string(&encoded).expect("serialize encoded collections");
        assert!(!encoded_json.contains(RUNTIME_COLLECTION_KEY_PREFIX));
        assert!(!encoded_json.contains(RUNTIME_MAP_ENTRY_MARKER));

        let mutated = Value::NamedConstructor(
            record_name.clone(),
            fields
                .iter()
                .map(|(name, value)| {
                    let value = match name.as_str() {
                        "values" => removed_values.clone(),
                        "tags" => removed_tags.clone(),
                        _ => value.clone(),
                    };
                    (name.clone(), value)
                })
                .collect::<Vec<_>>()
                .into(),
        );
        let mutated_encoded = contract
            .encode_output(&mutated)
            .expect("encode mutated collections");
        assert_eq!(
            mutated_encoded,
            serde_json::json!({
                "values": {"alpha": 9},
                "tags": ["first"]
            })
        );
        let mutated_json =
            serde_json::to_string(&mutated_encoded).expect("serialize mutated collections");
        assert!(!mutated_json.contains(RUNTIME_COLLECTION_KEY_PREFIX));
        assert!(!mutated_json.contains(RUNTIME_MAP_ENTRY_MARKER));

        let duplicate_error = contract
            .decode_input(&serde_json::json!({
                "values": {},
                "tags": ["first", "first"]
            }))
            .expect_err("duplicate Set members must be rejected");
        assert!(duplicate_error.to_string().contains("duplicate set value"));
    }

    /// Each case sends to a top-level actor, subscribes (in a named scope) to
    /// and emits on a top-level subject, shuffles with the shared random
    /// state, and asks a RuleScope whose zero-argument members are memoized
    /// per evaluation. Without isolation the second case of `[8, 8]` would
    /// see `tally = 32`, `seen = 2` and `total = 16`.
    const ISOLATION_SOURCE: &str = r#"
# Input(value: Int)
# Observed(tally: Int, seen: Int, total: Int, shuffled: List(Int), due: Int)

> actor counter(state: Int) {
    | n -> state + n
}

= tally = spawn(counter, 0)
~ feed = subject()

# Assessment(income: Int) {
    | base() -> income / 4
    | due() -> base() + base()
}

@ calculate
> observe(input: Input) -> Observed {
    | scope Watch {
        ~ feed | x -> { tally <- x }
    }
    feed <- input.value
    Observed(
        tally = ask(tally, 0),
        seen = feed.count,
        total = sum(collect(feed)),
        shuffled = shuffle(range(0, 8)),
        due = Assessment(input.value).due()
    )
}
"#;

    fn document(output: &CalculationOutputEnvelope) -> String {
        serde_json::to_string_pretty(output).expect("output is JSON")
    }

    #[test]
    fn prepared_cases_never_see_an_earlier_case_or_call() {
        let values = [8, 8, 20, 8];
        let (statements, contract, envelope) = runtime_failure_fixture(
            ISOLATION_SOURCE,
            values
                .iter()
                .map(|value| serde_json::json!({ "value": value }))
                .collect(),
        );
        let mut prepared =
            PreparedCalculation::new(contract.clone(), statements.as_slice().into(), None);
        let first = prepared.invoke_batch(&envelope, Some(1), None);
        assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
        let shuffled = &first.results[0].result["shuffled"];
        for (case, value) in first.results.iter().zip(values) {
            // Only this case's message, value and subscription.
            assert_eq!(case.result["tally"], value, "{}", case.case_id);
            assert_eq!(case.result["seen"], 1, "{}", case.case_id);
            assert_eq!(case.result["total"], value, "{}", case.case_id);
            assert_eq!(case.result["due"], 2 * (value / 4), "{}", case.case_id);
            assert_eq!(&case.result["shuffled"], shuffled, "{}", case.case_id);
        }
        // The case changed the initialized actor and subject, so the program
        // is initialized again before the next case.
        assert!(prepared.worker.as_ref().unwrap().needs_reinitialization);

        // The next call through the same prepared state, and a fresh
        // preparation per case, give the same documents byte for byte.
        let second = prepared.invoke_batch(&envelope, Some(1), None);
        assert_eq!(document(&second), document(&first));
        for (index, case) in envelope.cases.iter().enumerate() {
            let alone = CalculationInputEnvelope {
                futuruna: envelope.futuruna.clone(),
                cases: vec![case.clone()],
            };
            let fresh =
                invoke_calculation_cases_with_jobs(&contract, &statements, None, &alone, Some(1));
            assert_eq!(fresh.results, vec![first.results[index].clone()]);
        }
    }

    #[test]
    fn a_prepared_calculation_is_initialized_once_for_every_call() {
        let (stmts, contract, envelope) = batch_fixture();
        let fresh = invoke_calculation_cases_with_jobs(&contract, &stmts, None, &envelope, Some(1));
        let mut prepared = PreparedCalculation::new(contract, stmts.as_slice().into(), None);
        for _ in 0..3 {
            let output = prepared.invoke_batch(&envelope, Some(1), None);
            assert_eq!(document(&output), document(&fresh));
            // Nothing a case did requires initializing the program again.
            assert!(!prepared.worker.as_ref().unwrap().needs_reinitialization);
        }
        // Parallel batches reuse the prepared worker as their primary one.
        let parallel = prepared.invoke_batch(&envelope, Some(4), None);
        assert_eq!(document(&parallel), document(&fresh));
    }

    #[test]
    fn partial_and_complete_calls_share_one_prepared_calculation() {
        let (stmts, contract) = needs_fixture();
        let decided = partial_envelope(&contract, serde_json::json!({ "income": 50 }));
        let open = partial_envelope(&contract, serde_json::json!({ "income": 500 }));
        let complete = partial_envelope(
            &contract,
            serde_json::json!({ "income": 500, "married": true, "filing": { "$variant": "Single" } }),
        );
        let fresh = |envelope: &CalculationInputEnvelope, partial| {
            document(
                &PreparedCalculation::new(contract.clone(), stmts.as_slice().into(), None)
                    .invoke_batch(envelope, Some(1), partial),
            )
        };
        let propagate = Some(partial::PartialMode::Propagate);
        let expected = [
            fresh(&decided, propagate),
            fresh(&open, propagate),
            fresh(&complete, None),
        ];
        assert!(expected[0].contains("Complete"), "{}", expected[0]);
        assert!(expected[1].contains("Needs"), "{}", expected[1]);

        let mut prepared =
            PreparedCalculation::new(contract.clone(), stmts.as_slice().into(), None);
        for _ in 0..2 {
            for (envelope, partial, expected) in [
                (&decided, propagate, &expected[0]),
                (&open, propagate, &expected[1]),
                (&complete, None, &expected[2]),
                (&open, propagate, &expected[1]),
            ] {
                assert_eq!(
                    &document(&prepared.invoke_batch(envelope, Some(1), partial)),
                    expected
                );
            }
        }
    }
}
