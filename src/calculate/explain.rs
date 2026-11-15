//! Explanations of calculation results (`runa call --explain`).
//!
//! An explained case tags every input leaf with its input path, runs with the
//! interpreter's provenance graph on (`crate::explain`), and reads the graph
//! back for every leaf of the result: the rule clause that produced it, the
//! law section the clause sits in, the paragraphs it cites, and the rules and
//! inputs it was computed from. The format is `docs/rfcs/explanations.md`.

use super::*;
use crate::citations::{CitedParagraph, FileCitations};
use crate::explain::{traced, ClauseRecord, ExplainState, FiredClauses, Leaf, Node, NodeId};
use std::collections::HashMap;
use std::rc::Rc;

/// How an explained call reports.
#[derive(Debug, Clone, Default)]
pub struct ExplainOptions {
    /// Report the whole derivation (`derivation`), not only each value's
    /// direct contributors.
    pub full: bool,
    /// The model's root file: rules declared in it have no recorded file.
    pub root_file: Option<PathBuf>,
    /// Source locations are shown relative to this directory.
    pub display_root: Option<PathBuf>,
}

/// Direct contributors reported per value when the derivation is not full.
pub const DIRECT_CONTRIBUTOR_LIMIT: usize = 32;

/// Tag every leaf of a decoded case input with its input path.
pub(super) fn tag_input(
    interpreter: &Interpreter,
    value: &Value,
    contract: &CalculationContract,
) -> Value {
    tag_value(
        interpreter,
        value,
        &contract.input,
        contract,
        &BTreeMap::new(),
        "",
    )
}

fn input_leaf(interpreter: &Interpreter, value: &Value, path: &str) -> Value {
    let Some(leaf) = Leaf::of(value) else {
        return value.clone();
    };
    match interpreter.explain_intern(Node::Input {
        path: Rc::from(path),
        value: leaf,
    }) {
        Some(node) => traced(value.clone(), node),
        None => value.clone(),
    }
}

/// A shared constructor runs under its internal name; the contract keeps the
/// declared one.
fn find_variant<'a>(
    definition: &'a CalculationTypeDefinition,
    name: &str,
) -> Option<&'a CalculationVariant> {
    let name = crate::constructor_display_name(name);
    product_variant(definition)
        .filter(|variant| variant.name == *name || definition.kind == "rule_scope")
        .or_else(|| {
            definition
                .variants
                .iter()
                .find(|variant| variant.name == *name)
        })
}

fn tag_value(
    interpreter: &Interpreter,
    value: &Value,
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
) -> Value {
    if matches!(value, Value::Unknown(_)) {
        return value.clone();
    }
    let ty = substitute_contract_type(ty, substitutions);
    let tag = |value: &Value, ty: &CalculationTypeRef, path: &str| {
        tag_value(interpreter, value, ty, contract, substitutions, path)
    };
    match &ty {
        CalculationTypeRef::Primitive { .. } => input_leaf(interpreter, value, path),
        CalculationTypeRef::Optional { item } => match value {
            Value::Constructor(name, items) if name == "Some" && items.len() == 1 => {
                Value::Constructor(name.clone(), Rc::new(vec![tag(&items[0], item, path)]))
            }
            _ => value.clone(),
        },
        CalculationTypeRef::List { item } => match value {
            Value::List(items) => Value::List(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item_value)| tag(item_value, item, &format!("{path}[{index}]")))
                    .collect(),
            ),
            _ => value.clone(),
        },
        CalculationTypeRef::Map { value: item, .. } => match value {
            Value::Map(entries) => Value::Map(
                entries
                    .iter()
                    .map(|(stored_key, stored)| {
                        let tagged = match runtime_map_entry_parts(stored_key, stored) {
                            Some((Value::Str(key), entry)) => runtime_map_entry(
                                Value::Str(key.clone()),
                                tag(entry, item, &partial::join_path(path, key)),
                            ),
                            _ => stored.clone(),
                        };
                        (stored_key.clone(), tagged)
                    })
                    .collect(),
            ),
            _ => value.clone(),
        },
        CalculationTypeRef::Named { name, arguments } => {
            let Some(definition) = contract.definition(name) else {
                return value.clone();
            };
            let local = definition_substitutions(definition, arguments);
            let tag_fields = |variant: &CalculationVariant, fields: &[(String, Value)]| {
                fields
                    .iter()
                    .map(|(field, field_value)| {
                        let tagged = match variant.fields.iter().find(|f| &f.name == field) {
                            Some(declared) => tag_value(
                                interpreter,
                                field_value,
                                &declared.ty,
                                contract,
                                &local,
                                &partial::join_path(path, field),
                            ),
                            None => field_value.clone(),
                        };
                        (field.clone(), tagged)
                    })
                    .collect::<Vec<_>>()
            };
            match value {
                Value::NamedConstructor(variant_name, fields) => {
                    match find_variant(definition, variant_name) {
                        Some(variant) => Value::NamedConstructor(
                            variant_name.clone(),
                            Rc::new(tag_fields(variant, fields)),
                        ),
                        None => value.clone(),
                    }
                }
                Value::RuleScopeInstance { name, bindings } => match product_variant(definition) {
                    Some(variant) => {
                        let fields: Vec<(String, Value)> = bindings
                            .iter()
                            .map(|(field, item)| (field.clone(), item.clone()))
                            .collect();
                        Value::RuleScopeInstance {
                            name: name.clone(),
                            bindings: Rc::new(tag_fields(variant, &fields).into_iter().collect()),
                        }
                    }
                    None => value.clone(),
                },
                Value::Constructor(_, items) if items.is_empty() => {
                    input_leaf(interpreter, value, &partial::join_path(path, "$variant"))
                }
                Value::Constructor(variant_name, items) => {
                    match find_variant(definition, variant_name) {
                        Some(variant) if variant.fields.len() == items.len() => Value::Constructor(
                            variant_name.clone(),
                            Rc::new(
                                items
                                    .iter()
                                    .zip(&variant.fields)
                                    .enumerate()
                                    .map(|(index, (item, field))| {
                                        tag_value(
                                            interpreter,
                                            item,
                                            &field.ty,
                                            contract,
                                            &local,
                                            &format!("{path}.$values[{index}]"),
                                        )
                                    })
                                    .collect(),
                            ),
                        ),
                        _ => value.clone(),
                    }
                }
                _ => value.clone(),
            }
        }
        _ => value.clone(),
    }
}

/// One leaf of a result: its output path, value and provenance.
struct OutputLeaf {
    path: String,
    value: JsonValue,
    node: Option<NodeId>,
}

fn output_leaves(
    value: &Value,
    ty: &CalculationTypeRef,
    contract: &CalculationContract,
    substitutions: &BTreeMap<String, CalculationTypeRef>,
    path: &str,
    leaves: &mut Vec<OutputLeaf>,
) {
    let ty = substitute_contract_type(ty, substitutions);
    let node = crate::explain::node_of(value);
    let plain = crate::explain::untraced(value);
    let leaf = |leaves: &mut Vec<OutputLeaf>, path: String| {
        if let Some(data) = Leaf::of(plain) {
            leaves.push(OutputLeaf {
                path,
                value: data.json(),
                node,
            });
        }
    };
    match &ty {
        CalculationTypeRef::Primitive { .. } => leaf(leaves, path.to_string()),
        CalculationTypeRef::Optional { item } => {
            if let Value::Constructor(name, items) = plain {
                if name == "Some" && items.len() == 1 {
                    output_leaves(&items[0], item, contract, substitutions, path, leaves);
                }
            }
        }
        CalculationTypeRef::List { item } => {
            if let Some(items) = runtime_list_values(plain) {
                for (index, item_value) in items.into_iter().enumerate() {
                    output_leaves(
                        item_value,
                        item,
                        contract,
                        substitutions,
                        &format!("{path}[{index}]"),
                        leaves,
                    );
                }
            }
        }
        CalculationTypeRef::Map { value: item, .. } => {
            if let Value::Map(entries) = plain {
                for (stored_key, stored) in entries {
                    if let Some((key, entry)) = runtime_map_string_entry(stored_key, stored) {
                        output_leaves(
                            entry,
                            item,
                            contract,
                            substitutions,
                            &partial::join_path(path, key),
                            leaves,
                        );
                    }
                }
            }
        }
        CalculationTypeRef::Set { item } => {
            if let Value::Set(entries) = plain {
                for (index, entry) in entries.values().enumerate() {
                    output_leaves(
                        entry,
                        item,
                        contract,
                        substitutions,
                        &format!("{path}[{index}]"),
                        leaves,
                    );
                }
            }
        }
        CalculationTypeRef::Named { name, arguments } => {
            let Some(definition) = contract.definition(name) else {
                return;
            };
            let local = definition_substitutions(definition, arguments);
            match plain {
                Value::NamedConstructor(variant_name, fields) => {
                    let Some(variant) = find_variant(definition, variant_name) else {
                        return;
                    };
                    for field in &variant.fields {
                        if let Some((_, field_value)) =
                            fields.iter().find(|(name, _)| name == &field.name)
                        {
                            output_leaves(
                                field_value,
                                &field.ty,
                                contract,
                                &local,
                                &partial::join_path(path, &field.name),
                                leaves,
                            );
                        }
                    }
                }
                Value::Constructor(_, items) if items.is_empty() => {
                    leaf(leaves, partial::join_path(path, "$variant"));
                }
                Value::Constructor(variant_name, items) => {
                    let Some(variant) = find_variant(definition, variant_name) else {
                        return;
                    };
                    for (index, (item, field)) in items.iter().zip(&variant.fields).enumerate() {
                        output_leaves(
                            item,
                            &field.ty,
                            contract,
                            &local,
                            &format!("{path}.$values[{index}]"),
                            leaves,
                        );
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// A rule clause as an explanation shows it.
struct ClauseInfo {
    rule: String,
    scope: Option<String>,
    clause: &'static str,
    exception: Option<String>,
    location: Option<String>,
    section: Option<String>,
    /// Each cited paragraph, the act it was quoted from, and the
    /// instruction that gives its wording (`as amended`).
    paragraphs: Vec<(CitedParagraph, Option<CalculationAct>, Option<JsonValue>)>,
}

/// A section: `@ section <name> { ... }`, its quotes and metadata.
struct SectionInfo {
    location: String,
    /// The files of the documents it quotes.
    sources: Vec<String>,
    attachments: Vec<JsonValue>,
    quote: String,
    /// Each quoted document: binding, file and role.
    documents: Vec<JsonValue>,
    /// The quoted base passages an instruction quoted in the section amends.
    superseded: Vec<JsonValue>,
}

/// The `@ meta` values written on a section or item, and the line of its
/// first annotation.
struct Attached {
    line: usize,
    attachments: Vec<JsonValue>,
}

/// A rule clause, member or function with `@ meta`: its name, its first and
/// last line, and its metadata.
struct ItemMeta {
    name: String,
    start: usize,
    end: usize,
    attached: Attached,
}

/// What an explanation reads from one source file.
struct FileIndex {
    /// The file's path, as read.
    path: String,
    display: String,
    source: String,
    citations: FileCitations,
    /// `(label, first code line, last code line)` of every section.
    spans: Vec<(String, usize, usize)>,
    /// Every rule clause, member or function with `@ meta`, in source
    /// order; two clauses of one rule are two items.
    items: Vec<ItemMeta>,
    sections: BTreeMap<String, Rc<SectionInfo>>,
    /// The metadata attachments of every section.
    anchor_attachments: BTreeMap<String, Attached>,
    /// The first clause line of every rule family, by `(scope, name)`.
    families: HashMap<(Option<String>, String), usize>,
    /// The line where each top-level function's body starts.
    functions: HashMap<String, usize>,
}

/// An assumption and where it is written: file, line, and its place among
/// the values written there.
type PlacedAssumption = ((Rc<str>, usize, usize), CalculationAssumption);

impl FileIndex {
    /// The assumptions written on the innermost section around `line` and
    /// on the item `name` there (a clause, member or function with `@ meta`).
    /// An assumption on an item names the section it sits in, else the item.
    fn assumptions_at(&self, line: usize, name: &str) -> Vec<PlacedAssumption> {
        let file: Rc<str> = Rc::from(self.path.as_str());
        let section = self
            .spans
            .iter()
            .filter(|(_, start, end)| *start <= line && line <= *end)
            .max_by_key(|(_, start, _)| *start)
            .map(|(label, _, _)| label.as_str());
        let item = self
            .items
            .iter()
            .filter(|item| item.name == name && item.start <= line && line <= item.end)
            .max_by_key(|item| item.start);
        let mut found = Vec::new();
        let mut place = |label: &str, attached: &Attached| {
            for (position, attachment) in attached.attachments.iter().enumerate() {
                if attachment.get("role").and_then(JsonValue::as_str) != Some("Assumption") {
                    continue;
                }
                found.push((
                    (file.clone(), attached.line, position),
                    CalculationAssumption {
                        section: label.to_string(),
                        binding: attachment
                            .get("binding")
                            .and_then(JsonValue::as_str)
                            .map(str::to_string),
                        value: attachment.get("value").cloned().unwrap_or(JsonValue::Null),
                    },
                ));
            }
        };
        if let Some(label) = section {
            if let Some(attached) = self.anchor_attachments.get(label) {
                place(label, attached);
            }
        }
        if let Some(item) = item {
            place(section.unwrap_or(&item.name), &item.attached);
        }
        found
    }
}

/// Renders explanations; caches what it reads from source files.
#[derive(Default)]
pub(super) struct ExplainRenderer {
    options: ExplainOptions,
    files: HashMap<String, Option<Rc<FileIndex>>>,
    /// The act of each quoted document, by `(model file, source)`.
    acts: HashMap<(String, String), Option<CalculationAct>>,
    /// The assumptions written on each rule clause and its section, by the
    /// clause's allocation (the clause is kept so its address is not reused).
    clause_assumptions: HashMap<usize, (Rc<Rule>, Rc<[PlacedAssumption]>)>,
    /// The assumptions written on each entry and its section.
    entry_assumptions: HashMap<String, Rc<[PlacedAssumption]>>,
}

impl ExplainRenderer {
    pub(super) fn new(options: ExplainOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    fn display_path(&self, path: &Path) -> String {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(root) = &self.options.display_root {
            let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
            if let Ok(relative) = canonical.strip_prefix(&root) {
                return relative.to_string_lossy().into_owned();
            }
        }
        canonical.to_string_lossy().into_owned()
    }

    fn file(&mut self, path: &str) -> Option<Rc<FileIndex>> {
        if let Some(cached) = self.files.get(path) {
            return cached.clone();
        }
        let index = self.read_file(path).map(Rc::new);
        self.files.insert(path.to_string(), index.clone());
        index
    }

    fn read_file(&self, path: &str) -> Option<FileIndex> {
        let module = parse_source_module_file_cached(Path::new(path)).ok()?;
        let source = module.source().to_string();
        let directory = Path::new(path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned());
        let display = self.display_path(Path::new(path));
        let meta = scan_meta_comments_with_dir(&source, directory);
        let annotations = crate::annotations::read(&source);
        let documents = crate::annotations::Documents::for_file(&source, module.statements(), path);
        let mut spans = Vec::new();
        let mut item_spans = Vec::new();
        for span in &meta.spans {
            if annotations
                .section(&span.label)
                .is_some_and(|section| section.line == span.begin_marker_line)
            {
                spans.push((span.label.clone(), span.code_start_line, span.code_end_line));
            } else {
                item_spans.push(span);
            }
        }
        let mut sections = BTreeMap::new();
        let mut items = Vec::new();
        let mut anchor_attachments: BTreeMap<String, Attached> = BTreeMap::new();
        for anchor in &meta.anchors {
            let mut attachments = Vec::new();
            for reference in &anchor.references {
                let roles = reference.attachments();
                if roles.is_empty() {
                    attachments.push(serde_json::json!({
                        "role": reference.role,
                        "binding": reference.named_binding(),
                        "value": reference.static_data.as_ref().map(meta_value_data),
                    }));
                }
                for role in roles {
                    let role_name = match role.attachment {
                        MetaValue::Constructor { name, .. } => name.clone(),
                        _ => role.role.clone(),
                    };
                    let mut attachment = JsonMap::new();
                    attachment.insert("role".into(), JsonValue::from(role_name));
                    if let Some(binding) = role.binding_name {
                        attachment.insert("binding".into(), JsonValue::from(binding));
                    }
                    attachment.insert("value".into(), meta_value_data(role.value));
                    attachments.push(JsonValue::Object(attachment));
                }
            }
            // What holds, in one order however it is written: by role,
            // binding and value, each once.
            attachments.sort_by_cached_key(|attachment| {
                (
                    attachment["role"].as_str().unwrap_or_default().to_string(),
                    attachment["binding"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    attachment["value"].to_string(),
                )
            });
            attachments.dedup();
            if anchor.kind != "section" {
                // An item's anchor and code span start on its first `@ meta`.
                if let Some(span) = item_spans.iter().find(|span| {
                    span.label == anchor.label && span.begin_marker_line == anchor.comment_line
                }) {
                    items.push(ItemMeta {
                        name: anchor.label.clone(),
                        start: span.code_start_line,
                        end: span.code_end_line,
                        attached: Attached {
                            line: anchor.comment_line,
                            attachments,
                        },
                    });
                }
                continue;
            }
            anchor_attachments
                .entry(anchor.label.clone())
                .or_insert_with(|| Attached {
                    line: anchor.comment_line,
                    attachments: attachments.clone(),
                });
            if sections.contains_key(&anchor.label) {
                continue;
            }
            let Some(section) = annotations.section(&anchor.label) else {
                continue;
            };
            let mut sources: Vec<String> = Vec::new();
            let mut quoted = Vec::new();
            for quote in &section.quotes {
                let file = documents.file(&quote.document).ok();
                if let Some(file) = &file {
                    if !sources.contains(file) {
                        sources.push(file.clone());
                    }
                }
                let document = serde_json::json!({
                    "binding": quote.document,
                    "file": file,
                    "role": quote.role,
                });
                if !quoted.contains(&document) {
                    quoted.push(document);
                }
            }
            let superseded = crate::citations::section_quote(section, &documents)
                .superseded()
                .into_iter()
                .map(|(paragraph, amendment)| {
                    serde_json::json!({
                        "paragraph": paragraph,
                        "as_amended_by": amendment_json(&amendment, path),
                    })
                })
                .collect();
            sections.insert(
                anchor.label.clone(),
                Rc::new(SectionInfo {
                    location: format!("{display}:{}", anchor.comment_line),
                    sources,
                    attachments,
                    quote: anchor.text.clone(),
                    documents: quoted,
                    superseded,
                }),
            );
        }
        let mut families = HashMap::new();
        collect_rule_families(module.statements(), None, &source, &mut families);
        let mut functions = HashMap::new();
        for statement in module.statements() {
            if let Stmt::Defn(Defn::Fn { name, body, .. }) = statement {
                if !body.span.is_dummy() {
                    let (line, _) = char_offset_to_line_col(&source, body.span.start);
                    functions.entry(name.clone()).or_insert(line);
                }
            }
        }
        Some(FileIndex {
            path: path.to_string(),
            display,
            citations: crate::citations::file_citations_from(&annotations, &documents),
            source,
            spans,
            items,
            sections,
            anchor_attachments,
            families,
            functions,
        })
    }

    fn clause_info(&mut self, interpreter: &Interpreter, record: &ClauseRecord) -> ClauseInfo {
        let (kind, exception) = clause_kind(record.rule.as_ref());
        let mut info = ClauseInfo {
            rule: record.name.clone(),
            scope: record.scope.clone(),
            clause: kind,
            exception,
            location: None,
            section: None,
            paragraphs: Vec::new(),
        };
        let file = interpreter
            .rule_source_file(&record.rule)
            .map(|file| file.to_string())
            .or_else(|| {
                self.options
                    .root_file
                    .as_ref()
                    .map(|file| file.to_string_lossy().into_owned())
            });
        let Some(file) = file else {
            return info;
        };
        let Some(index) = self.file(&file) else {
            return info;
        };
        let Some(head) = record.rule.head() else {
            return info;
        };
        let (line, _) = char_offset_to_line_col(&index.source, head.span.start);
        info.location = Some(format!("{}:{line}", index.display));
        info.section = index
            .spans
            .iter()
            .filter(|(_, start, end)| *start <= line && line <= *end)
            .max_by_key(|(_, start, _)| *start)
            .map(|(label, _, _)| label.clone());
        let own = index.citations.by_clause_line.get(&line);
        let inherited = || {
            index
                .families
                .get(&(record.scope.clone(), record.name.clone()))
                .and_then(|first| index.citations.by_clause_line.get(first))
        };
        if let Some(citation) = own.or_else(inherited) {
            info.paragraphs = citation
                .paragraphs
                .iter()
                .map(|paragraph| {
                    let act = self.paragraph_act(&index.path, paragraph);
                    let amendment = paragraph
                        .as_amended_by
                        .as_ref()
                        .map(|amendment| amendment_json(amendment, &index.path));
                    (paragraph.clone(), act, amendment)
                })
                .collect();
        }
        info
    }

    /// The act of the document a cited paragraph was quoted from.
    fn paragraph_act(&mut self, file: &str, paragraph: &CitedParagraph) -> Option<CalculationAct> {
        let source = paragraph.source.as_ref()?;
        let key = (file.to_string(), source.clone());
        if let Some(act) = self.acts.get(&key) {
            return act.clone();
        }
        let act = read_act(file, source);
        self.acts.insert(key, act.clone());
        act
    }

    fn section(
        &mut self,
        interpreter: &Interpreter,
        record: &ClauseRecord,
    ) -> Option<(String, Rc<SectionInfo>)> {
        let file = interpreter
            .rule_source_file(&record.rule)
            .map(|file| file.to_string())
            .or_else(|| {
                self.options
                    .root_file
                    .as_ref()
                    .map(|file| file.to_string_lossy().into_owned())
            })?;
        let index = self.file(&file)?;
        let head = record.rule.head()?;
        let (line, _) = char_offset_to_line_col(&index.source, head.span.start);
        let label = index
            .spans
            .iter()
            .filter(|(_, start, end)| *start <= line && line <= *end)
            .max_by_key(|(_, start, _)| *start)?
            .0
            .clone();
        let section = index.sections.get(&label)?.clone();
        Some((label, section))
    }

    /// The rules a need waits on, given every call stack (innermost last)
    /// its path was read under. From each stack: the innermost frame of a
    /// law section (one whose quote holds a `[sources/...]` document) that
    /// cites paragraphs; failing that, the innermost frame of a law
    /// section; then of any section with a quote; then the innermost rule
    /// family. Of what the stacks give, the best kind is kept (cited law
    /// first), each rule once, in the order the stacks were first seen;
    /// the entry when no stack gives one.
    pub(super) fn consumers(
        &mut self,
        interpreter: &Interpreter,
        stacks: &[partial::WaitingRules],
        entry: &str,
    ) -> Vec<CalculationWaiting> {
        let mut found: Vec<(u8, CalculationWaiting)> = Vec::new();
        for stack in stacks {
            if let Some(consumer) = self.stack_consumer(interpreter, stack) {
                if !found.contains(&consumer) {
                    found.push(consumer);
                }
            }
        }
        let Some(best) = found.iter().map(|(rank, _)| *rank).min() else {
            return vec![CalculationWaiting {
                rule: entry.to_string(),
                section: None,
                paragraphs: Vec::new(),
            }];
        };
        found
            .into_iter()
            .filter(|(rank, _)| *rank == best)
            .map(|(_, waiting)| waiting)
            .collect()
    }

    /// The consumer one call stack names, ranked: 0 a cited frame of a law
    /// section, 1 an uncited one, 2 a frame of a section of checks, 3 the
    /// innermost rule family.
    fn stack_consumer(
        &mut self,
        interpreter: &Interpreter,
        frames: &[partial::CallFrame],
    ) -> Option<(u8, CalculationWaiting)> {
        let mut law = None;
        let mut any_section = None;
        for frame in frames.iter().rev() {
            let Some((file, label, is_law, cited)) = self.frame_section(interpreter, frame) else {
                continue;
            };
            let paragraphs: Vec<CalculationParagraph> = cited
                .iter()
                .map(|paragraph| CalculationParagraph {
                    paragraph: paragraph.paragraph.clone(),
                    source: paragraph.source.clone(),
                    act: self.paragraph_act(&file, paragraph),
                    as_amended_by: paragraph
                        .as_amended_by
                        .as_ref()
                        .map(|amendment| calculation_amendment(amendment, &file)),
                })
                .collect();
            let waiting = CalculationWaiting {
                rule: frame.name.to_string(),
                section: Some(label),
                paragraphs,
            };
            if is_law && !waiting.paragraphs.is_empty() {
                return Some((0, waiting));
            }
            if is_law {
                law.get_or_insert(waiting);
            } else {
                any_section.get_or_insert(waiting);
            }
        }
        if let Some(waiting) = law {
            return Some((1, waiting));
        }
        if let Some(waiting) = any_section {
            return Some((2, waiting));
        }
        frames
            .iter()
            .rev()
            .find(|frame| frame.is_rule)
            .map(|frame| {
                (
                    3,
                    CalculationWaiting {
                        rule: frame.name.to_string(),
                        section: None,
                        paragraphs: Vec::new(),
                    },
                )
            })
    }

    /// Whether a frame is cited in a law section: its clause (or its rule's
    /// first clause) or its function carries an `@ cite`, and the section
    /// around it quotes a document.
    pub(super) fn frame_cited(
        &mut self,
        interpreter: &Interpreter,
        frame: &partial::CallFrame,
    ) -> bool {
        self.frame_section(interpreter, frame)
            .is_some_and(|(_, _, is_law, cited)| is_law && !cited.is_empty())
    }

    /// A frame's file, the section (with a `----` quote) its clause or
    /// function is declared in, whether that is a law section, and the
    /// paragraphs the clause, else the rule's first clause, or the function
    /// cites.
    fn frame_section(
        &mut self,
        interpreter: &Interpreter,
        frame: &partial::CallFrame,
    ) -> Option<(String, String, bool, Vec<CitedParagraph>)> {
        let (index, line, family_line) = self.frame_location(interpreter, frame)?;
        let label = index
            .spans
            .iter()
            .filter(|(_, start, end)| *start <= line && line <= *end)
            .max_by_key(|(_, start, _)| *start)
            .map(|(label, _, _)| label.clone())?;
        // Only a section with a `----` quote counts: the code span of an
        // anchor that only carries metadata (an entry's input guide) is no
        // section of law or checks.
        let section = index.sections.get(&label)?;
        let is_law = !section.sources.is_empty();
        let cited = index
            .citations
            .by_clause_line
            .get(&line)
            .or_else(|| family_line.and_then(|first| index.citations.by_clause_line.get(&first)))
            .map(|citation| citation.paragraphs.clone())
            .unwrap_or_default();
        Some((index.path.clone(), label, is_law, cited))
    }

    /// The file index, line and family first-clause line of a frame: the
    /// clause being tried, else the family's first clause, else the function
    /// of that name.
    fn frame_location(
        &mut self,
        interpreter: &Interpreter,
        frame: &partial::CallFrame,
    ) -> Option<(Rc<FileIndex>, usize, Option<usize>)> {
        if frame.is_rule {
            let rule = match &frame.clause {
                Some(rule) => rule.clone(),
                None => {
                    interpreter
                        .registered_rules()
                        .into_iter()
                        .find(|(name, _)| **name == *frame.name)?
                        .1
                }
            };
            let file = interpreter
                .rule_source_file(&rule)
                .map(|file| file.to_string())
                .or_else(|| self.root_file_name())?;
            let index = self.file(&file)?;
            let (line, _) = char_offset_to_line_col(&index.source, rule.head()?.span.start);
            // The family of this name (a RuleScope member's too) whose first
            // clause is the nearest at or above this clause.
            let family_line = index
                .families
                .iter()
                .filter(|((_, name), first)| **name == *frame.name && **first <= line)
                .map(|(_, first)| *first)
                .max();
            return Some((index, line, family_line));
        }
        let mut files: Vec<String> = self.root_file_name().into_iter().collect();
        files.extend(interpreter.loaded_runtime_sources());
        for file in files {
            let Some(index) = self.file(&file) else {
                continue;
            };
            if let Some(line) = index.functions.get(&*frame.name).copied() {
                return Some((index, line, None));
            }
        }
        None
    }

    fn root_file_name(&self) -> Option<String> {
        self.options
            .root_file
            .as_ref()
            .map(|file| file.to_string_lossy().into_owned())
    }

    /// The assumptions a case's result rests on: the `Assumption`
    /// attachments of the rule clauses that fired and of the sections they
    /// sit in, then of the entry and its section; each once (by binding and
    /// value), in the order the model writes them (by file, then line).
    pub(super) fn assumptions(
        &mut self,
        interpreter: &Interpreter,
        fired: &FiredClauses,
        entry: &str,
    ) -> Vec<CalculationAssumption> {
        let mut placed: BTreeMap<(Rc<str>, usize, usize), CalculationAssumption> =
            BTreeMap::new();
        let mut add = |found: &[PlacedAssumption]| {
            for (place, assumption) in found {
                placed
                    .entry(place.clone())
                    .or_insert_with(|| assumption.clone());
            }
        };
        for id in 0..fired.len() {
            add(&self.clause_assumptions(interpreter, fired.clause(id as u32)));
        }
        add(&self.entry_assumptions(interpreter, entry));
        let mut found: Vec<CalculationAssumption> = Vec::new();
        for assumption in placed.into_values() {
            if !found
                .iter()
                .any(|seen| seen.binding == assumption.binding && seen.value == assumption.value)
            {
                found.push(assumption);
            }
        }
        found
    }

    /// The assumptions written on a rule clause (with `@ meta`) and on the
    /// section around it.
    fn clause_assumptions(
        &mut self,
        interpreter: &Interpreter,
        record: &ClauseRecord,
    ) -> Rc<[PlacedAssumption]> {
        let key = Rc::as_ptr(&record.rule) as usize;
        if let Some((_, found)) = self.clause_assumptions.get(&key) {
            return found.clone();
        }
        let found: Rc<[PlacedAssumption]> = self
            .locate_clause_assumptions(interpreter, record)
            .unwrap_or_default()
            .into();
        self.clause_assumptions
            .insert(key, (record.rule.clone(), found.clone()));
        found
    }

    fn locate_clause_assumptions(
        &mut self,
        interpreter: &Interpreter,
        record: &ClauseRecord,
    ) -> Option<Vec<PlacedAssumption>> {
        let file = interpreter
            .rule_source_file(&record.rule)
            .map(|file| file.to_string())
            .or_else(|| self.root_file_name())?;
        let index = self.file(&file)?;
        let (line, _) = char_offset_to_line_col(&index.source, record.rule.head()?.span.start);
        Some(index.assumptions_at(line, &record.name))
    }

    /// The assumptions written on the calculation entry and on the section
    /// around it.
    fn entry_assumptions(
        &mut self,
        interpreter: &Interpreter,
        entry: &str,
    ) -> Rc<[PlacedAssumption]> {
        if let Some(found) = self.entry_assumptions.get(entry) {
            return found.clone();
        }
        let located = [false, true].into_iter().find_map(|is_rule| {
            self.frame_location(
                interpreter,
                &partial::CallFrame {
                    name: Rc::from(entry),
                    is_rule,
                    clause: None,
                },
            )
        });
        let found: Rc<[PlacedAssumption]> = located
            .map(|(index, line, _)| index.assumptions_at(line, entry))
            .unwrap_or_default()
            .into();
        self.entry_assumptions
            .insert(entry.to_string(), found.clone());
        found
    }

    /// The explanation of one case's result; `fired` holds the clauses its
    /// rule nodes name.
    pub(super) fn explanation(
        &mut self,
        interpreter: &Interpreter,
        state: &ExplainState,
        fired: &FiredClauses,
        result: &Value,
        contract: &CalculationContract,
    ) -> JsonValue {
        let mut leaves = Vec::new();
        output_leaves(
            result,
            &contract.output,
            contract,
            &BTreeMap::new(),
            "",
            &mut leaves,
        );
        let mut case = CaseRenderer {
            renderer: self,
            interpreter,
            state,
            fired,
            clauses: HashMap::new(),
            sections: BTreeMap::new(),
            reached: Vec::new(),
        };
        let limit = (!case.renderer.options.full).then_some(DIRECT_CONTRIBUTOR_LIMIT);
        let mut values = JsonMap::new();
        for leaf in leaves {
            let mut entry = JsonMap::new();
            entry.insert("value".into(), leaf.value);
            let (from, because) = match leaf.node.map(|node| (node, state.node(node))) {
                None => {
                    entry.insert("by".into(), serde_json::json!({ "$variant": "Constant" }));
                    (None, None)
                }
                Some((_, Node::Input { path, .. })) => {
                    entry.insert(
                        "by".into(),
                        serde_json::json!({ "$variant": "Input", "path": path.as_ref() }),
                    );
                    (None, None)
                }
                Some((
                    _,
                    Node::Rule {
                        clause,
                        from,
                        because,
                        ..
                    },
                )) => {
                    entry.insert("by".into(), case.clause_json(*clause, true));
                    (*from, Some(because))
                }
                Some((node, Node::Computed { .. })) => {
                    entry.insert("by".into(), serde_json::json!({ "$variant": "Computed" }));
                    (Some(node), None)
                }
            };
            if let Some(because) = because.and_then(|because| case.because_json(because, limit)) {
                entry.insert("because".into(), because);
            }
            if let Some(from) = from {
                let (contributors, omitted) = state.contributors(from, limit);
                entry.insert(
                    "from".into(),
                    JsonValue::Array(
                        contributors
                            .into_iter()
                            .map(|node| case.reference_json(node))
                            .collect(),
                    ),
                );
                if omitted > 0 {
                    entry.insert("from_omitted".into(), JsonValue::from(omitted));
                }
            }
            values.insert(leaf.path, JsonValue::Object(entry));
        }
        let mut explanation = JsonMap::new();
        explanation.insert("values".into(), JsonValue::Object(values));
        if case.renderer.options.full {
            let mut derivation = JsonMap::new();
            let mut done = std::collections::HashSet::new();
            while let Some(node) = case.reached.pop() {
                if !done.insert(node) {
                    continue;
                }
                let Node::Rule {
                    clause,
                    from,
                    value,
                    because,
                } = state.node(node)
                else {
                    continue;
                };
                let mut entry = JsonMap::new();
                entry.insert("value".into(), value.json());
                entry.insert("by".into(), case.clause_json(*clause, true));
                let contributors = from
                    .map(|from| state.contributors(from, None).0)
                    .unwrap_or_default();
                entry.insert(
                    "from".into(),
                    JsonValue::Array(
                        contributors
                            .into_iter()
                            .map(|node| case.reference_json(node))
                            .collect(),
                    ),
                );
                if let Some(because) = case.because_json(because, None) {
                    entry.insert("because".into(), because);
                }
                derivation.insert(node.to_string(), JsonValue::Object(entry));
            }
            let mut ordered: Vec<(u64, JsonValue)> = derivation
                .into_iter()
                .map(|(key, value)| (key.parse().unwrap_or_default(), value))
                .collect();
            ordered.sort_by_key(|(key, _)| *key);
            explanation.insert(
                "derivation".into(),
                JsonValue::Object(
                    ordered
                        .into_iter()
                        .map(|(key, value)| (key.to_string(), value))
                        .collect(),
                ),
            );
        }
        let sections = std::mem::take(&mut case.sections);
        explanation.insert(
            "sections".into(),
            JsonValue::Object(
                sections
                    .into_iter()
                    .map(|(label, section)| {
                        let mut object = JsonMap::new();
                        object.insert("location".into(), JsonValue::from(section.location.clone()));
                        object.insert("sources".into(), serde_json::json!(section.sources));
                        object.insert(
                            "documents".into(),
                            JsonValue::Array(section.documents.clone()),
                        );
                        object.insert(
                            "attachments".into(),
                            JsonValue::Array(section.attachments.clone()),
                        );
                        object.insert("quote".into(), JsonValue::from(section.quote.clone()));
                        if !section.superseded.is_empty() {
                            object.insert(
                                "superseded".into(),
                                JsonValue::Array(section.superseded.clone()),
                            );
                        }
                        (label, JsonValue::Object(object))
                    })
                    .collect(),
            ),
        );
        JsonValue::Object(explanation)
    }
}

/// The state of rendering one case.
struct CaseRenderer<'a> {
    renderer: &'a mut ExplainRenderer,
    interpreter: &'a Interpreter,
    state: &'a ExplainState,
    fired: &'a FiredClauses,
    clauses: HashMap<u32, Rc<ClauseInfo>>,
    sections: BTreeMap<String, Rc<SectionInfo>>,
    /// Rule nodes referenced so far, for the full derivation.
    reached: Vec<NodeId>,
}

impl CaseRenderer<'_> {
    fn clause(&mut self, clause: u32) -> Rc<ClauseInfo> {
        if let Some(info) = self.clauses.get(&clause) {
            return info.clone();
        }
        let record = self.fired.clause(clause);
        let info = Rc::new(self.renderer.clause_info(self.interpreter, record));
        if let Some((label, section)) = self.renderer.section(self.interpreter, record) {
            self.sections.entry(label).or_insert(section);
        }
        self.clauses.insert(clause, info.clone());
        info
    }

    /// A clause: its rule, kind, location and section, and the cited
    /// paragraphs with their text when `with_text`, else their labels.
    fn clause_json(&mut self, clause: u32, with_text: bool) -> JsonValue {
        let info = self.clause(clause);
        let mut object = JsonMap::new();
        object.insert("$variant".into(), JsonValue::from("Rule"));
        object.insert("rule".into(), JsonValue::from(info.rule.clone()));
        if let Some(scope) = &info.scope {
            object.insert("scope".into(), JsonValue::from(scope.clone()));
        }
        object.insert("clause".into(), JsonValue::from(info.clause));
        if let Some(exception) = &info.exception {
            object.insert("exception".into(), JsonValue::from(exception.clone()));
        }
        if let Some(location) = &info.location {
            object.insert("location".into(), JsonValue::from(location.clone()));
        }
        if let Some(section) = &info.section {
            object.insert("section".into(), JsonValue::from(section.clone()));
        }
        if !info.paragraphs.is_empty() {
            let paragraphs = info
                .paragraphs
                .iter()
                .map(|(paragraph, act, amendment)| {
                    let mut cited = JsonMap::new();
                    cited.insert(
                        "paragraph".into(),
                        JsonValue::from(paragraph.paragraph.clone()),
                    );
                    if let Some(source) = &paragraph.source {
                        cited.insert("source".into(), JsonValue::from(source.clone()));
                    }
                    if let Some(act) = act {
                        cited.insert(
                            "act".into(),
                            serde_json::to_value(act).expect("an act is JSON"),
                        );
                    }
                    if let Some(amendment) = amendment {
                        cited.insert("as_amended_by".into(), amendment.clone());
                    }
                    if with_text {
                        cited.insert("text".into(), JsonValue::from(paragraph.text.clone()));
                    }
                    JsonValue::Object(cited)
                })
                .collect();
            let key = if with_text { "cites" } else { "paragraphs" };
            object.insert(key.into(), JsonValue::Array(paragraphs));
        }
        JsonValue::Object(object)
    }

    /// What the guards that chose a clause were computed from: the rules
    /// and inputs behind each guard value, each once, at most `limit`.
    fn because_json(&mut self, because: &[NodeId], limit: Option<usize>) -> Option<JsonValue> {
        let mut nodes: Vec<NodeId> = Vec::new();
        for guard in because {
            for node in self.state.contributors(*guard, None).0 {
                if !nodes.contains(&node) {
                    nodes.push(node);
                }
            }
        }
        if let Some(limit) = limit {
            nodes.truncate(limit);
        }
        (!nodes.is_empty()).then(|| {
            JsonValue::Array(
                nodes
                    .into_iter()
                    .map(|node| self.reference_json(node))
                    .collect(),
            )
        })
    }

    /// A contributor: a rule node or an input node, briefly.
    fn reference_json(&mut self, node: NodeId) -> JsonValue {
        match self.state.node(node) {
            Node::Input { path, value } => serde_json::json!({
                "$variant": "Input",
                "path": path.as_ref(),
                "value": value.json(),
            }),
            Node::Rule { clause, value, .. } => {
                let mut reference = match self.clause_json(*clause, false) {
                    JsonValue::Object(object) => object,
                    _ => JsonMap::new(),
                };
                reference.insert("value".into(), value.json());
                if self.renderer.options.full {
                    reference.insert("node".into(), JsonValue::from(node));
                    self.reached.push(node);
                }
                JsonValue::Object(reference)
            }
            Node::Computed { .. } => serde_json::json!({ "$variant": "Computed" }),
        }
    }
}

/// How a clause is reported: an exception with its label; a value rule
/// (`| f(x) -> value`, with or without `under`) as the rule's default; a
/// logic clause (`:-` goals, or a fact) as a clause.
fn clause_kind(rule: &Rule) -> (&'static str, Option<String>) {
    match rule {
        Rule::Exception { label, .. } => ("exception", Some(label.clone())),
        Rule::Clause {
            body: Some(body), ..
        } if !matches!(
            body.kind,
            ExprKind::Conjunction(_) | ExprKind::Disjunction(_)
        ) =>
        {
            ("default", None)
        }
        Rule::Clause { .. } => ("clause", None),
        _ => ("default", None),
    }
}

/// Record the first clause line of every rule family in `statements`.
fn collect_rule_families(
    statements: &[Stmt],
    scope: Option<&str>,
    source: &str,
    families: &mut HashMap<(Option<String>, String), usize>,
) {
    for statement in statements {
        match statement {
            Stmt::Rule(
                rule @ (Rule::Clause { .. } | Rule::Default { .. } | Rule::Exception { .. }),
            ) => {
                let Some((name, _)) = TypeChecker::rule_name_arity(rule) else {
                    continue;
                };
                let Some(head) = rule.head() else {
                    continue;
                };
                let (line, _) = char_offset_to_line_col(source, head.span.start);
                families
                    .entry((scope.map(str::to_string), name))
                    .and_modify(|first| *first = (*first).min(line))
                    .or_insert(line);
            }
            Stmt::TypeDecl(TypeDecl::RuleScope { name, body, .. }) => {
                collect_rule_families(body, Some(name), source, families);
            }
            _ => {}
        }
    }
}

/// The amending act of a passage cited `as amended`, as needs report it:
/// the act's identifier from its saved file's header, else from the
/// document's `identifier`.
fn calculation_amendment(
    amendment: &crate::citations::CitedAmendment,
    model_file: &str,
) -> CalculationAmendment {
    let act = amendment
        .file
        .as_deref()
        .and_then(|file| read_act(model_file, file))
        .and_then(|act| act.identifier)
        .or_else(|| amendment.identifier.clone())
        .unwrap_or_else(|| amendment.document.clone());
    CalculationAmendment {
        act,
        paragraph: amendment.paragraph.clone(),
        instruction: amendment.instruction.clone(),
    }
}

/// [`calculation_amendment`] as explanations write it.
fn amendment_json(amendment: &crate::citations::CitedAmendment, model_file: &str) -> JsonValue {
    serde_json::to_value(calculation_amendment(amendment, model_file))
        .expect("an amendment is JSON")
}

/// The act of the quoted document `source` (`sources/...`) of a model file:
/// the document is looked up beside the file and in each directory above
/// it, the first found counts; its header names the act.
pub(super) fn read_act(file: &str, source: &str) -> Option<CalculationAct> {
    let file = std::fs::canonicalize(file).unwrap_or_else(|_| PathBuf::from(file));
    let mut directory = file.parent();
    while let Some(current) = directory {
        if let Ok(text) = crate::read_source_file(&current.join(source)) {
            return act_from_header(&text);
        }
        directory = current.parent();
    }
    None
}

/// The act a document's header names. The header is the block between a
/// first line `---` and the next `---`, of `Key: value` lines: `Title`
/// (required), `Document identifier`, and `Human-readable URL` or `URL`.
pub(super) fn act_from_header(text: &str) -> Option<CalculationAct> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut fields: HashMap<&str, &str> = HashMap::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            if !value.is_empty() {
                fields.entry(key.trim()).or_insert(value);
            }
        }
    }
    let text = |key: &str| fields.get(key).map(|value| value.to_string());
    Some(CalculationAct {
        title: text("Title")?,
        identifier: text("Document identifier"),
        url: text("Human-readable URL").or_else(|| text("URL")),
    })
}

/// A metadata value as plain JSON: a record's fields as an object, a list
/// as an array, a variant without fields as its name.
fn meta_value_data(value: &MetaValue) -> JsonValue {
    match value {
        MetaValue::Integer(number) => JsonValue::from(*number),
        MetaValue::Float(text) => text
            .parse::<f64>()
            .ok()
            .and_then(JsonNumber::from_f64)
            .map(JsonValue::Number)
            .unwrap_or_else(|| JsonValue::from(text.clone())),
        MetaValue::String(text) => JsonValue::from(text.clone()),
        MetaValue::Character(character) => JsonValue::from(character.to_string()),
        MetaValue::Boolean(boolean) => JsonValue::Bool(*boolean),
        MetaValue::Unit => JsonValue::Null,
        MetaValue::Constructor {
            name,
            applied,
            arguments,
            ..
        } => {
            if !applied {
                return JsonValue::from(name.clone());
            }
            if !arguments.is_empty() && arguments.iter().all(|argument| argument.field.is_some()) {
                JsonValue::Object(
                    arguments
                        .iter()
                        .map(|argument| {
                            (
                                argument.field.clone().unwrap_or_default(),
                                meta_value_data(&argument.value),
                            )
                        })
                        .collect(),
                )
            } else {
                serde_json::json!({
                    "$variant": name,
                    "$values": arguments.iter().map(|argument| meta_value_data(&argument.value)).collect::<Vec<_>>(),
                })
            }
        }
        MetaValue::List(items) | MetaValue::Tuple(items) => {
            JsonValue::Array(items.iter().map(meta_value_data).collect())
        }
        MetaValue::Unary { operator, value } => match (operator.as_str(), meta_value_data(value)) {
            ("-", JsonValue::Number(number)) => number
                .as_i64()
                .map(|number| JsonValue::from(-number))
                .or_else(|| number.as_f64().map(|number| JsonValue::from(-number)))
                .unwrap_or(JsonValue::Null),
            (_, other) => other,
        },
    }
}

/// JSON Schema of a case's `explanation` (`runa call --explain`).
pub fn explanation_json_schema(contract: &CalculationContract) -> JsonValue {
    let string = serde_json::json!({ "type": "string" });
    let leaf = serde_json::json!({
        "description": "The canonical JSON of a scalar, or the name of a variant without fields."
    });
    let amendment = serde_json::json!({
        "type": "object",
        "description": "The amending act's instruction that gives a passage cited `as amended` its wording.",
        "required": ["act", "paragraph", "instruction"],
        "additionalProperties": false,
        "properties": {
            "act": { "type": "string", "description": "The amending act, as `LOV nr. 1691 af 30/12/2024`." },
            "paragraph": { "type": "string", "description": "Its paragraph and instruction, as `§ 4, nr. 1`." },
            "instruction": { "type": "string", "description": "The instruction line, verbatim." }
        }
    });
    let paragraph = |with_text: bool| {
        let mut properties = serde_json::json!({
            "paragraph": { "type": "string", "description": "The cited paragraph, as `§ 9, stk. 3`." },
            "source": { "type": "string", "description": "The file of the quoted document it was found in." },
            "act": act_json_schema(),
            "as_amended_by": amendment
        });
        let mut required = vec!["paragraph"];
        if with_text {
            properties["text"] = serde_json::json!({ "type": "string", "description": "The quoted text of the paragraph, verbatim; for a passage cited `as amended`, the wording in force." });
            required.push("text");
        }
        serde_json::json!({
            "type": "object",
            "required": required,
            "additionalProperties": false,
            "properties": properties
        })
    };
    let rule = |with_text: bool| {
        let mut properties = serde_json::json!({
            "$variant": { "const": "Rule" },
            "rule": string,
            "scope": { "type": "string", "description": "The RuleScope of a scoped rule." },
            "clause": { "enum": ["default", "exception", "clause"] },
            "exception": { "type": "string", "description": "The label of the exception that applied." },
            "location": { "type": "string", "description": "file:line of the clause." },
            "section": { "type": "string", "description": "The `@ section` around the clause; a key of `sections`." }
        });
        if with_text {
            properties["cites"] = serde_json::json!({ "type": "array", "items": paragraph(true) });
        } else {
            properties["paragraphs"] =
                serde_json::json!({ "type": "array", "items": paragraph(false) });
            properties["value"] = leaf.clone();
            properties["node"] = serde_json::json!({ "type": "integer", "description": "The node in `derivation` (`--explain=full`)." });
        }
        serde_json::json!({
            "type": "object",
            "required": ["$variant", "rule", "clause"],
            "additionalProperties": false,
            "properties": properties
        })
    };
    let reference = serde_json::json!({
        "oneOf": [
            rule(false),
            {
                "type": "object",
                "required": ["$variant", "path", "value"],
                "additionalProperties": false,
                "properties": { "$variant": { "const": "Input" }, "path": string, "value": leaf }
            }
        ]
    });
    let by = serde_json::json!({
        "oneOf": [
            rule(true),
            {
                "type": "object",
                "required": ["$variant", "path"],
                "additionalProperties": false,
                "properties": { "$variant": { "const": "Input" }, "path": string }
            },
            {
                "type": "object",
                "required": ["$variant"],
                "additionalProperties": false,
                "properties": { "$variant": { "enum": ["Computed", "Constant"] } }
            }
        ]
    });
    let contributors = serde_json::json!({ "type": "array", "items": reference });
    let because = serde_json::json!({
        "type": "array",
        "items": reference,
        "description": "What the guards that chose the clause were computed from: the guard of the clause that applied and of every clause above it that was tried."
    });
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("Explanation of a `{}` result", contract.entry),
        "description": "Where every leaf of a result came from: the rule clause that produced it, the law section and cited paragraphs of the clause, and the rules and inputs it was computed from. See docs/rfcs/explanations.md.",
        "x-futuruna-schema-hash": contract.schema_hash,
        "type": "object",
        "required": ["values", "sections"],
        "additionalProperties": false,
        "properties": {
            "values": {
                "type": "object",
                "description": "Every leaf of the result by its output path, in the path syntax of needs.",
                "additionalProperties": {
                    "type": "object",
                    "required": ["value", "by"],
                    "additionalProperties": false,
                    "properties": {
                        "value": leaf,
                        "by": by,
                        "from": contributors,
                        "from_omitted": { "type": "integer", "description": "Direct contributors beyond the first 32, with `--explain` (not `full`)." },
                        "because": because
                    }
                }
            },
            "sections": {
                "type": "object",
                "description": "The sections the explained clauses sit in, by name.",
                "additionalProperties": {
                    "type": "object",
                    "required": ["location", "sources", "documents", "attachments", "quote"],
                    "additionalProperties": false,
                    "properties": {
                        "location": string,
                        "sources": { "type": "array", "items": string, "description": "The files of the documents the section quotes." },
                        "documents": {
                            "type": "array",
                            "description": "Each document the section quotes: its binding, file and role.",
                            "items": {
                                "type": "object",
                                "required": ["binding", "file", "role"],
                                "additionalProperties": false,
                                "properties": {
                                    "binding": string,
                                    "file": { "type": ["string", "null"] },
                                    "role": { "type": ["string", "null"] }
                                }
                            }
                        },
                        "attachments": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["role", "value"],
                                "properties": { "role": string, "binding": string, "value": {} }
                            }
                        },
                        "quote": { "type": "string", "description": "The section's quotes in order, each document's text after a `[<file>]` line." },
                        "superseded": {
                            "type": "array",
                            "description": "The quoted passages an instruction quoted in the same section amends.",
                            "items": {
                                "type": "object",
                                "required": ["paragraph", "as_amended_by"],
                                "additionalProperties": false,
                                "properties": { "paragraph": string, "as_amended_by": amendment }
                            }
                        }
                    }
                }
            },
            "derivation": {
                "type": "object",
                "description": "With `--explain=full`: every rule node a value was computed from, by node.",
                "additionalProperties": {
                    "type": "object",
                    "required": ["value", "by", "from"],
                    "additionalProperties": false,
                    "properties": { "value": leaf, "by": rule(true), "from": contributors, "because": because }
                }
            }
        }
    })
}
