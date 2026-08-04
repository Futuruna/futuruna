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

pub const CONTRACT_SCHEMA: &str = "futuruna.calculate.v1";
pub const INPUT_SCHEMA: &str = "futuruna.calculate.input.v1";
pub const OUTPUT_SCHEMA: &str = "futuruna.calculate.output.v1";

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
}

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
                input: template_value(&self.input, self, &BTreeMap::new(), &mut BTreeSet::new()),
            }],
        }
    }

    pub fn decode_input(&self, value: &JsonValue) -> Result<Value, CalculationValueError> {
        decode_value(value, &self.input, self, &BTreeMap::new(), "$")
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
                metadata: self.field_metadata_for_path("input").cloned(),
            });
        }
        for column in &mut root_columns {
            column.metadata = self.field_metadata_for_path(&column.input_path).cloned();
        }
        for table in &mut builder.tables {
            table.metadata = self.field_metadata_for_path(&table.path).cloned();
            for column in &mut table.columns {
                column.metadata = self.field_metadata_for_path(&column.input_path).cloned();
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
    pub input: JsonValue,
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

#[derive(Clone)]
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

#[derive(Default)]
struct TypeCatalog {
    types: BTreeMap<String, CatalogType>,
}

impl TypeCatalog {
    fn collect(&mut self, stmts: &[Stmt], source_dir: Option<&str>) {
        let mut imported = BTreeSet::new();
        self.collect_inner(stmts, source_dir, &mut imported);
    }

    fn collect_inner(
        &mut self,
        stmts: &[Stmt],
        source_dir: Option<&str>,
        imported: &mut BTreeSet<String>,
    ) {
        for stmt in stmts {
            match stmt {
                Stmt::TypeDecl(TypeDecl::ADT {
                    name,
                    params,
                    variants,
                    except_from,
                    ..
                }) => {
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
                    self.types.insert(
                        name.clone(),
                        CatalogType::RuleScope {
                            parameters: params.clone(),
                        },
                    );
                }
                Stmt::Import(path) => {
                    let Some(dir) = source_dir else {
                        continue;
                    };
                    let Some(file_path) = Interpreter::resolve_import_path_for_source(path, dir)
                    else {
                        continue;
                    };
                    let canonical = std::fs::canonicalize(&file_path)
                        .unwrap_or_else(|_| Path::new(&file_path).to_path_buf());
                    let canonical = canonical.to_string_lossy().to_string();
                    if !imported.insert(canonical) {
                        continue;
                    }
                    let Ok(source) = std::fs::read_to_string(&file_path) else {
                        continue;
                    };
                    let mut lexer = Lexer::new(&source);
                    let tokens = lexer.tokenize();
                    let mut parser = Parser::new(tokens, &source);
                    let Ok(imported_stmts) = parser.parse_program() else {
                        continue;
                    };
                    let nested_dir = Path::new(&file_path)
                        .parent()
                        .map(|parent| parent.to_string_lossy().to_string())
                        .unwrap_or_else(|| ".".to_string());
                    self.collect_inner(&imported_stmts, Some(&nested_dir), imported);
                }
                _ => {}
            }
        }
    }

    fn resolved_variants(&self, name: &str) -> Result<Vec<Variant>, String> {
        self.resolved_variants_inner(name, &mut BTreeSet::new())
    }

    fn resolved_variants_inner(
        &self,
        name: &str,
        active: &mut BTreeSet<String>,
    ) -> Result<Vec<Variant>, String> {
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
                    for variant in self.resolved_variants_inner(source, active)? {
                        if !excluded.contains(&variant.name) {
                            resolved.push(variant);
                        }
                    }
                }
                for variant in variants {
                    match variant.from_type.as_deref() {
                        Some("__maybe_include") if self.types.contains_key(&variant.name) => {
                            resolved.extend(self.resolved_variants_inner(&variant.name, active)?);
                        }
                        Some(source) if source != "__maybe_include" => {
                            let source_variant = self
                                .resolved_variants_inner(source, active)?
                                .into_iter()
                                .find(|candidate| candidate.name == variant.name)
                                .ok_or_else(|| {
                                    format!(
                                        "type `{}` has no variant `{}` included by `{}`",
                                        source, variant.name, name
                                    )
                                })?;
                            resolved.push(source_variant);
                        }
                        _ => resolved.push(variant.clone()),
                    }
                }
                Ok(resolved)
            }
            Some(CatalogType::RuleScope { parameters }) => Ok(vec![Variant {
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
            }]),
            None => Err(format!("unknown type `{}`", name)),
        };
        active.remove(name);
        result
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

    let mut catalog = TypeCatalog::default();
    catalog.collect(stmts, source_dir.as_deref());

    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();
    let mut pending_markers = Vec::new();

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

        match endpoint_from_stmt(stmt, stmts, &checker) {
            Ok(mut candidate) => {
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

    let meta_index = scan_meta_comments_with_dir(source, source_dir.clone());
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
    if !metadata_diagnostics.is_empty() {
        return Err(metadata_diagnostics);
    }
    contracts.sort_by(|left, right| left.entry.cmp(&right.entry));
    Ok(contracts)
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
            reject_direct_effects(name, body)?;
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
                reject_rule_direct_effects(&name, rule)?;
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

fn reject_direct_effects(name: &str, expr: &Expr) -> Result<(), String> {
    let mut effects = BTreeSet::new();
    walk_ast_expr(expr, &mut |child| {
        if let AstChild::Expr(Expr {
            kind: ExprKind::Effect(effect, _),
            ..
        }) = child
        {
            effects.insert(effect.clone());
        }
    });
    if effects.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "calculation `{}` directly performs effects ({}); move external input outside the calculation boundary",
            name,
            effects.into_iter().collect::<Vec<_>>().join(", ")
        ))
    }
}

fn reject_rule_direct_effects(name: &str, rule: &Rule) -> Result<(), String> {
    let expressions: Vec<&Expr> = match rule {
        Rule::Clause { body, .. } => body.iter().collect(),
        Rule::Default {
            value, condition, ..
        }
        | Rule::Exception {
            value, condition, ..
        } => std::iter::once(value).chain(condition.iter()).collect(),
        Rule::ReactiveScope { .. } => Vec::new(),
    };
    for expression in expressions {
        reject_direct_effects(name, expression)?;
    }
    Ok(())
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
            let field_ty = substitute_ty(&field.ty, &substitutions);
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

fn substitute_ty(ty: &Ty, substitutions: &BTreeMap<String, Ty>) -> Ty {
    match ty {
        Ty::Var(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Ty::App(base, arguments) => Ty::App(
            Box::new(substitute_ty(base, substitutions)),
            arguments
                .iter()
                .map(|argument| substitute_ty(argument, substitutions))
                .collect(),
        ),
        Ty::Arrow(input, output) => Ty::Arrow(
            Box::new(substitute_ty(input, substitutions)),
            Box::new(substitute_ty(output, substitutions)),
        ),
        Ty::Ref(inner) => Ty::Ref(Box::new(substitute_ty(inner, substitutions))),
        Ty::MutRef(inner) => Ty::MutRef(Box::new(substitute_ty(inner, substitutions))),
        Ty::Shared(inner) => Ty::Shared(Box::new(substitute_ty(inner, substitutions))),
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_ty(inner, substitutions))),
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
                    &substitute_ty(&field.ty, &substitutions),
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
            .map(|attachment| CalculationMetadataReference {
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
                symbols: symbols.clone(),
            })
            .collect();
    }
    vec![calculation_metadata_reference(anchor, reference, symbols)]
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

fn contract_field_metadata(
    index: &MetaIndex,
    contract: &CalculationContract,
) -> Result<Vec<CalculationFieldMetadata>, Vec<Diagnostic>> {
    let layout = contract.input_layout();
    let mut valid_paths = BTreeSet::new();
    valid_paths.extend(
        layout
            .root_columns
            .iter()
            .map(|column| column.input_path.clone()),
    );
    for table in &layout.collection_tables {
        valid_paths.insert(table.path.clone());
        valid_paths.extend(table.columns.iter().map(|column| column.input_path.clone()));
    }

    let span_labels: BTreeSet<String> = index
        .spans
        .iter()
        .filter(|span| {
            span.symbols
                .iter()
                .any(|symbol| symbol.name == contract.entry)
        })
        .map(|span| span.label.clone())
        .collect();
    let mut result = Vec::new();
    let mut bindings_by_path = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for anchor in &index.anchors {
        if anchor.label != contract.entry && !span_labels.contains(&anchor.label) {
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
                if reference.role == "field" {
                    Vec::new()
                } else if reference.role == "meta" && !reference.attachments().is_empty() {
                    nested
                } else if field_reference_indices.contains(&index) {
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
            for (binding, value) in field_values {
                match calculation_field_metadata_from_value(
                    contract,
                    &binding,
                    value,
                    anchor,
                    &valid_paths,
                    &sources,
                ) {
                    Ok(metadata) => {
                        if let Some(previous) =
                            bindings_by_path.insert(metadata.path.clone(), metadata.binding.clone())
                        {
                            diagnostics.push(Diagnostic::error(format!(
                                "calculation `{}` has duplicate field metadata for `{}` in bindings `{}` and `{}`",
                                contract.entry, metadata.path, previous, metadata.binding
                            )));
                        } else {
                            result.push(metadata);
                        }
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
        }
    }

    if diagnostics.is_empty() {
        result.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(result)
    } else {
        Err(diagnostics)
    }
}

fn calculation_field_values(reference: &MetaReference) -> Vec<(String, &MetaValue)> {
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
            (binding, typed_value.value)
        })
        .collect()
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
    let supported = ["path", "label", "question", "help", "unit"];
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
    sources: &[CalculationMetadataReference],
) -> Result<CalculationFieldMetadata, Diagnostic> {
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
    let supported = ["path", "label", "question", "help", "unit"];
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

    let declared_path = required_meta_string(&fields, "path", binding)?;
    let path = normalize_calculation_field_path(contract, &declared_path, valid_paths).ok_or_else(
        || {
            let examples = valid_paths.iter().take(8).cloned().collect::<Vec<_>>();
            Diagnostic::error(format!(
                "calculation field metadata binding `{}` targets unknown input path `{}` for `{}`",
                binding, declared_path, contract.entry
            ))
            .with_note(format!(
                "use an exact canonical path such as {}",
                examples.join(", ")
            ))
        },
    )?;
    let label = required_meta_string(&fields, "label", binding)?;
    if label.trim().is_empty() {
        return Err(Diagnostic::error(format!(
            "calculation field metadata binding `{}` has an empty label",
            binding
        )));
    }

    Ok(CalculationFieldMetadata {
        path,
        label: label.trim().to_string(),
        question: optional_meta_string(&fields, "question", binding)?,
        help: optional_meta_string(&fields, "help", binding)?,
        unit: optional_meta_string(&fields, "unit", binding)?,
        anchor: anchor.label.clone(),
        binding: binding.to_string(),
        sources: sources.to_vec(),
    })
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
        .find(|path| valid_paths.contains(*path))
        .map(str::to_string)
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

fn template_value(
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    active: &mut BTreeSet<String>,
) -> JsonValue {
    let ty = substitute_contract_type(ty, substitutions);
    match ty {
        CalculationTypeRef::Primitive { name } => match name.as_str() {
            "Int" => JsonValue::Number(0.into()),
            "Float" => JsonValue::Number(JsonNumber::from_f64(0.0).expect("finite zero")),
            "Bool" => JsonValue::Bool(false),
            "Char" => JsonValue::String("x".to_string()),
            _ => JsonValue::String(String::new()),
        },
        CalculationTypeRef::Optional { .. } | CalculationTypeRef::Unit => JsonValue::Null,
        CalculationTypeRef::List { .. } | CalculationTypeRef::Set { .. } => {
            JsonValue::Array(Vec::new())
        }
        CalculationTypeRef::Map { .. } => JsonValue::Object(JsonMap::new()),
        CalculationTypeRef::TypeParameter { .. } => JsonValue::Null,
        CalculationTypeRef::Named { name, arguments } => {
            let Some(definition) = contract.definition(&name) else {
                return JsonValue::Null;
            };
            if !active.insert(name.clone()) {
                return JsonValue::Null;
            }
            let local = definition_substitutions(definition, &arguments);
            let value = if let Some(variant) = product_variant(definition) {
                let mut object = JsonMap::new();
                for field in &variant.fields {
                    object.insert(
                        field.name.clone(),
                        template_value(&field.ty, contract, &local, active),
                    );
                }
                JsonValue::Object(object)
            } else if let Some(variant) = definition.variants.first() {
                variant_template(variant, contract, &local, active)
            } else {
                JsonValue::Null
            };
            active.remove(&name);
            value
        }
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

fn decode_value(
    value: &JsonValue,
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
) -> Result<Value, CalculationValueError> {
    let ty = substitute_contract_type(ty, substitutions);
    match ty {
        CalculationTypeRef::Primitive { name } => decode_primitive(value, &name, path),
        CalculationTypeRef::Unit => {
            if value.is_null() {
                Ok(Value::Unit)
            } else {
                Err(value_error(path, "expected null for unit"))
            }
        }
        CalculationTypeRef::Optional { item } => {
            if value.is_null() {
                Ok(Value::Constructor("None".to_string(), Vec::new()))
            } else {
                Ok(Value::Constructor(
                    "Some".to_string(),
                    vec![decode_value(value, &item, contract, substitutions, path)?],
                ))
            }
        }
        CalculationTypeRef::List { item } => {
            let values = value
                .as_array()
                .ok_or_else(|| value_error(path, "expected an array"))?;
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    decode_value(
                        value,
                        &item,
                        contract,
                        substitutions,
                        &format!("{}[{}]", path, index),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List)
        }
        CalculationTypeRef::Map { key, value: item } => {
            if !matches!(*key, CalculationTypeRef::Primitive { ref name } if name == "String") {
                return Err(value_error(path, "only Map(String, T) is supported"));
            }
            let object = value
                .as_object()
                .ok_or_else(|| value_error(path, "expected an object"))?;
            let mut values = BTreeMap::new();
            for (name, value) in object {
                values.insert(
                    name.clone(),
                    decode_value(
                        value,
                        &item,
                        contract,
                        substitutions,
                        &field_path(path, name),
                    )?,
                );
            }
            Ok(Value::Map(values))
        }
        CalculationTypeRef::Set { item } => {
            let array = value
                .as_array()
                .ok_or_else(|| value_error(path, "expected an array"))?;
            let mut values = BTreeMap::new();
            for (index, item_value) in array.iter().enumerate() {
                let decoded = decode_value(
                    item_value,
                    &item,
                    contract,
                    substitutions,
                    &format!("{}[{}]", path, index),
                )?;
                let key = decoded.to_string();
                if values.insert(key.clone(), decoded).is_some() {
                    return Err(value_error(
                        format!("{}[{}]", path, index),
                        format!("duplicate set value `{}`", key),
                    ));
                }
            }
            Ok(Value::Set(values))
        }
        CalculationTypeRef::TypeParameter { name } => Err(value_error(
            path,
            format!("unresolved type parameter `{}`", name),
        )),
        CalculationTypeRef::Named { name, arguments } => {
            let definition = contract.definition(&name).ok_or_else(|| {
                value_error(path, format!("contract has no definition for `{}`", name))
            })?;
            if definition.parameters.len() != arguments.len() {
                return Err(value_error(
                    path,
                    format!(
                        "type `{}` expects {} arguments but got {}",
                        name,
                        definition.parameters.len(),
                        arguments.len()
                    ),
                ));
            }
            let local = definition_substitutions(definition, &arguments);
            if let Some(variant) = product_variant(definition) {
                let fields = decode_named_fields(value, variant, contract, &local, path, false)?;
                if definition.kind == "rule_scope" {
                    Ok(Value::RuleScopeInstance {
                        name,
                        bindings: fields.into_iter().collect(),
                    })
                } else {
                    Ok(Value::NamedConstructor(variant.name.clone(), fields))
                }
            } else {
                decode_variant(value, definition, contract, &local, path)
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
) -> Result<Vec<(String, Value)>, CalculationValueError> {
    let object = value
        .as_object()
        .ok_or_else(|| value_error(path, "expected an object"))?;
    let allowed: BTreeSet<&str> = variant
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    for key in object.keys() {
        if !(allowed.contains(key.as_str()) || allow_variant_key && key == "$variant") {
            return Err(value_error(
                field_path(path, key),
                format!("unknown field for variant `{}`", variant.name),
            ));
        }
    }
    let mut result = Vec::new();
    for field in &variant.fields {
        let field_ty = substitute_contract_type(&field.ty, substitutions);
        let field_value = match object.get(&field.name) {
            Some(value) => value,
            None if matches!(field_ty, CalculationTypeRef::Optional { .. }) => &JsonValue::Null,
            None => {
                return Err(value_error(
                    field_path(path, &field.name),
                    "missing required field",
                ));
            }
        };
        result.push((
            field.name.clone(),
            decode_value(
                field_value,
                &field_ty,
                contract,
                substitutions,
                &field_path(path, &field.name),
            )?,
        ));
    }
    Ok(result)
}

fn decode_variant(
    value: &JsonValue,
    definition: &CalculationTypeDefinition,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
) -> Result<Value, CalculationValueError> {
    let object = value.as_object().ok_or_else(|| {
        value_error(
            path,
            format!(
                "expected an object with `$variant` for `{}`",
                definition.name
            ),
        )
    })?;
    let variant_name = object
        .get("$variant")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| value_error(field_path(path, "$variant"), "missing variant name"))?;
    let variant = definition
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)
        .ok_or_else(|| {
            value_error(
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
            )
        })?;
    if variant.positional {
        for key in object.keys() {
            if key != "$variant" && key != "$values" {
                return Err(value_error(
                    field_path(path, key),
                    "positional variants only accept `$variant` and `$values`",
                ));
            }
        }
        let values = object
            .get("$values")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| value_error(field_path(path, "$values"), "expected an array"))?;
        if values.len() != variant.fields.len() {
            return Err(value_error(
                field_path(path, "$values"),
                format!(
                    "variant `{}` expects {} values but got {}",
                    variant.name,
                    variant.fields.len(),
                    values.len()
                ),
            ));
        }
        let decoded = values
            .iter()
            .zip(&variant.fields)
            .enumerate()
            .map(|(index, (value, field))| {
                decode_value(
                    value,
                    &field.ty,
                    contract,
                    substitutions,
                    &format!("{}.$values[{}]", path, index),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Constructor(variant.name.clone(), decoded))
    } else {
        let fields = decode_named_fields(value, variant, contract, substitutions, path, true)?;
        if fields.is_empty() {
            Ok(Value::Constructor(variant.name.clone(), Vec::new()))
        } else {
            Ok(Value::NamedConstructor(variant.name.clone(), fields))
        }
    }
}

fn encode_value(
    value: &Value,
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
) -> Result<JsonValue, CalculationValueError> {
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
            for (name, value) in values {
                object.insert(
                    name.clone(),
                    encode_value(
                        value,
                        &item,
                        contract,
                        substitutions,
                        &field_path(path, name),
                    )?,
                );
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
        Value::NamedConstructor(name, fields) if name == &variant.name => Ok(fields),
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
                .find(|variant| &variant.name == name)
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
                object.insert("$variant".to_string(), JsonValue::String(name.clone()));
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
                object.insert("$variant".to_string(), JsonValue::String(name.clone()));
                Ok(JsonValue::Object(object))
            } else {
                Err(runtime_type_error(path, &definition.name, value))
            }
        }
        Value::NamedConstructor(name, fields) => {
            let variant = definition
                .variants
                .iter()
                .find(|variant| &variant.name == name && !variant.positional)
                .ok_or_else(|| runtime_type_error(path, &definition.name, value))?;
            encode_named_fields(fields, variant, contract, substitutions, path, true)
        }
        _ => Err(runtime_type_error(path, &definition.name, value)),
    }
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
    if envelope.futuruna.schema != INPUT_SCHEMA {
        diagnostics.push(CalculationCaseDiagnostic {
            case_id: String::new(),
            path: "$.$futuruna.schema".to_string(),
            message: format!(
                "unsupported input schema `{}`; expected `{}`",
                envelope.futuruna.schema, INPUT_SCHEMA
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

/// Decode, invoke, and encode a batch. The pure program is initialized once,
/// while each case receives an isolated environment and runtime state.
pub fn invoke_calculation_cases(
    contract: &CalculationContract,
    stmts: &[Stmt],
    source_dir: Option<String>,
    envelope: &CalculationInputEnvelope,
) -> CalculationOutputEnvelope {
    let mut output = CalculationOutputEnvelope {
        futuruna: CalculationEnvelopeMetadata {
            schema: OUTPUT_SCHEMA.to_string(),
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

    let mut interpreter = Interpreter::new();
    interpreter.suppress_output = true;
    interpreter.source_dir = source_dir;
    let mut base_env = interpreter.default_env();
    interpreter.run_program(stmts, &mut base_env);
    let base_actor_instances = interpreter.actor_instances.clone();
    let base_rng_state = interpreter.rng_state;

    for case in &envelope.cases {
        let input = match contract.decode_input(&case.input) {
            Ok(input) => input,
            Err(error) => {
                output.diagnostics.push(CalculationCaseDiagnostic {
                    case_id: case.case_id.clone(),
                    path: error.path,
                    message: error.message,
                });
                continue;
            }
        };

        interpreter.active_rule_scopes.clear();
        interpreter.output.clear();
        interpreter.handler_stack.clear();
        interpreter.actor_instances = base_actor_instances.clone();
        interpreter.step_count = 0;
        interpreter.budget_exceeded = false;
        interpreter.rng_state = base_rng_state;

        let mut runtime_env = base_env.clone();
        let input_binding = "__futuruna_calculation_input".to_string();
        runtime_env.set(input_binding.clone(), input);
        let call: Expr = ExprKind::App(
            Box::new(ExprKind::Var(contract.entry.clone()).into()),
            vec![ExprKind::Var(input_binding).into()],
        )
        .into();
        let result = interpreter.eval(&call, &runtime_env);
        match contract.encode_output(&result) {
            Ok(result) => output.results.push(CalculationResultCase {
                case_id: case.case_id.clone(),
                result,
            }),
            Err(error) => output.diagnostics.push(CalculationCaseDiagnostic {
                case_id: case.case_id.clone(),
                path: error.path,
                message: error.message,
            }),
        }
    }
    output
}
