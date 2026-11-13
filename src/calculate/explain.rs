//! Explanations of calculation results (`runa call --explain`).
//!
//! An explained case tags every input leaf with its input path, runs with the
//! interpreter's provenance graph on (`crate::explain`), and reads the graph
//! back for every leaf of the result: the rule clause that produced it, the
//! law section the clause sits in, the paragraphs it cites, and the rules and
//! inputs it was computed from. The format is `docs/rfcs/explanations.md`.

use super::*;
use crate::citations::{CitedParagraph, FileCitations};
use crate::explain::{traced, ExplainState, Leaf, Node, NodeId};
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

fn find_variant<'a>(
    definition: &'a CalculationTypeDefinition,
    name: &str,
) -> Option<&'a CalculationVariant> {
    product_variant(definition)
        .filter(|variant| variant.name == name || definition.kind == "rule_scope")
        .or_else(|| {
            definition
                .variants
                .iter()
                .find(|variant| variant.name == name)
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
    paragraphs: Vec<CitedParagraph>,
}

/// A law section: the anchor and quote the rules between its markers
/// implement.
struct SectionInfo {
    location: String,
    sources: Vec<String>,
    attachments: Vec<JsonValue>,
    quote: String,
}

/// What an explanation reads from one source file.
struct FileIndex {
    display: String,
    source: String,
    citations: FileCitations,
    /// `(label, first code line, last code line)` of every section.
    spans: Vec<(String, usize, usize)>,
    sections: BTreeMap<String, Rc<SectionInfo>>,
    /// The first clause line of every rule family, by `(scope, name)`.
    families: HashMap<(Option<String>, String), usize>,
    /// The line where each top-level function's body starts.
    functions: HashMap<String, usize>,
}

/// Renders explanations; caches what it reads from source files.
#[derive(Default)]
pub(super) struct ExplainRenderer {
    options: ExplainOptions,
    files: HashMap<String, Option<Rc<FileIndex>>>,
}

impl ExplainRenderer {
    pub(super) fn new(options: ExplainOptions) -> Self {
        Self {
            options,
            files: HashMap::new(),
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
        let spans = meta
            .spans
            .iter()
            .map(|span| (span.label.clone(), span.code_start_line, span.code_end_line))
            .collect();
        let mut sections = BTreeMap::new();
        for anchor in &meta.anchors {
            if anchor.text_start_line.is_none() || sections.contains_key(&anchor.label) {
                continue;
            }
            let sources = anchor
                .text
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    (line.len() > 2 && line.starts_with('[') && line.ends_with(']'))
                        .then(|| line[1..line.len() - 1].trim().to_string())
                })
                .collect();
            let mut attachments = Vec::new();
            for reference in &anchor.references {
                let roles = reference.attachments();
                if roles.is_empty() {
                    attachments.push(serde_json::json!({
                        "role": reference.role,
                        "binding": reference.binding_name,
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
            sections.insert(
                anchor.label.clone(),
                Rc::new(SectionInfo {
                    location: format!("{display}:{}", anchor.comment_line),
                    sources,
                    attachments,
                    quote: anchor.text.clone(),
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
            display,
            citations: crate::citations::file_citations(&source),
            source,
            spans,
            sections,
            families,
            functions,
        })
    }

    fn clause_info(
        &mut self,
        interpreter: &Interpreter,
        state: &ExplainState,
        clause: u32,
    ) -> ClauseInfo {
        let record = state.clause(clause);
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
            info.paragraphs = citation.paragraphs.clone();
        }
        info
    }

    fn section(
        &mut self,
        interpreter: &Interpreter,
        state: &ExplainState,
        clause: u32,
    ) -> Option<(String, Rc<SectionInfo>)> {
        let record = state.clause(clause);
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

    /// The frame a need names: of the rules and functions evaluating when
    /// the path was first read (innermost last), the innermost one declared
    /// in a law section, one whose quote holds a `[sources/...]` document;
    /// failing that, the innermost one declared in any section with a quote. Returns its
    /// name, section and the paragraphs the frame's clause cites (the rule's
    /// first clause when the clause cites none).
    pub(super) fn waiting_frame(
        &mut self,
        interpreter: &Interpreter,
        frames: &[partial::CallFrame],
    ) -> Option<(String, String, Vec<String>)> {
        let mut any_section = None;
        for frame in frames.iter().rev() {
            let Some((index, line, family_line)) = self.frame_location(interpreter, frame) else {
                continue;
            };
            let Some(label) = index
                .spans
                .iter()
                .filter(|(_, start, end)| *start <= line && line <= *end)
                .max_by_key(|(_, start, _)| *start)
                .map(|(label, _, _)| label.clone())
            else {
                continue;
            };
            let paragraphs = index
                .citations
                .by_clause_line
                .get(&line)
                .or_else(|| {
                    family_line.and_then(|first| index.citations.by_clause_line.get(&first))
                })
                .map(|citation| {
                    citation
                        .paragraphs
                        .iter()
                        .map(|paragraph| paragraph.paragraph.clone())
                        .collect()
                })
                .unwrap_or_default();
            // Only a section with a `----` quote counts: the code span of an
            // anchor that only carries metadata (an entry's input guide) is
            // no section of law or checks.
            let Some(section) = index.sections.get(&label) else {
                continue;
            };
            let law = !section.sources.is_empty();
            let found = (frame.name.clone(), label, paragraphs);
            if law {
                return Some(found);
            }
            any_section.get_or_insert(found);
        }
        any_section
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
                        .find(|(name, _)| *name == frame.name)?
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
                .filter(|((_, name), first)| *name == frame.name && **first <= line)
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
            if let Some(line) = index.functions.get(&frame.name).copied() {
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

    /// The explanation of one case's result.
    pub(super) fn explanation(
        &mut self,
        interpreter: &Interpreter,
        state: &ExplainState,
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
            clauses: HashMap::new(),
            sections: BTreeMap::new(),
            reached: Vec::new(),
        };
        let limit = (!case.renderer.options.full).then_some(DIRECT_CONTRIBUTOR_LIMIT);
        let mut values = JsonMap::new();
        for leaf in leaves {
            let mut entry = JsonMap::new();
            entry.insert("value".into(), leaf.value);
            let from = match leaf.node.map(|node| (node, state.node(node))) {
                None => {
                    entry.insert("by".into(), serde_json::json!({ "$variant": "Constant" }));
                    None
                }
                Some((_, Node::Input { path, .. })) => {
                    entry.insert(
                        "by".into(),
                        serde_json::json!({ "$variant": "Input", "path": path.as_ref() }),
                    );
                    None
                }
                Some((_, Node::Rule { clause, from, .. })) => {
                    entry.insert("by".into(), case.clause_json(*clause, true));
                    *from
                }
                Some((node, Node::Computed { .. })) => {
                    entry.insert("by".into(), serde_json::json!({ "$variant": "Computed" }));
                    Some(node)
                }
            };
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
                        (
                            label,
                            serde_json::json!({
                                "location": section.location,
                                "sources": section.sources,
                                "attachments": section.attachments,
                                "quote": section.quote,
                            }),
                        )
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
        let info = Rc::new(
            self.renderer
                .clause_info(self.interpreter, self.state, clause),
        );
        if let Some((label, section)) = self.renderer.section(self.interpreter, self.state, clause)
        {
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
            if with_text {
                object.insert(
                    "cites".into(),
                    JsonValue::Array(
                        info.paragraphs
                            .iter()
                            .map(|paragraph| {
                                let mut cited = JsonMap::new();
                                cited.insert(
                                    "paragraph".into(),
                                    JsonValue::from(paragraph.paragraph.clone()),
                                );
                                if let Some(source) = &paragraph.source {
                                    cited.insert("source".into(), JsonValue::from(source.clone()));
                                }
                                cited
                                    .insert("text".into(), JsonValue::from(paragraph.text.clone()));
                                JsonValue::Object(cited)
                            })
                            .collect(),
                    ),
                );
            } else {
                object.insert(
                    "paragraphs".into(),
                    JsonValue::Array(
                        info.paragraphs
                            .iter()
                            .map(|paragraph| JsonValue::from(paragraph.paragraph.clone()))
                            .collect(),
                    ),
                );
            }
        }
        JsonValue::Object(object)
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
    let paragraph = serde_json::json!({
        "type": "object",
        "required": ["paragraph", "text"],
        "additionalProperties": false,
        "properties": {
            "paragraph": { "type": "string", "description": "The cited paragraph, as `§ 9, stk. 3`." },
            "source": { "type": "string", "description": "The `[sources/...]` document of the quote it was found in." },
            "text": { "type": "string", "description": "The quoted text of the paragraph, verbatim." }
        }
    });
    let rule = |with_text: bool| {
        let mut properties = serde_json::json!({
            "$variant": { "const": "Rule" },
            "rule": string,
            "scope": { "type": "string", "description": "The RuleScope of a scoped rule." },
            "clause": { "enum": ["default", "exception", "clause"] },
            "exception": { "type": "string", "description": "The label of the exception that applied." },
            "location": { "type": "string", "description": "file:line of the clause." },
            "section": { "type": "string", "description": "The law section (anchor label) around the clause; a key of `sections`." }
        });
        if with_text {
            properties["cites"] = serde_json::json!({ "type": "array", "items": paragraph });
        } else {
            properties["paragraphs"] = serde_json::json!({ "type": "array", "items": string });
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
                        "from_omitted": { "type": "integer", "description": "Direct contributors beyond the first 32, with `--explain` (not `full`)." }
                    }
                }
            },
            "sections": {
                "type": "object",
                "description": "The law sections the explained clauses sit in, by anchor label.",
                "additionalProperties": {
                    "type": "object",
                    "required": ["location", "sources", "attachments", "quote"],
                    "additionalProperties": false,
                    "properties": {
                        "location": string,
                        "sources": { "type": "array", "items": string },
                        "attachments": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["role", "value"],
                                "properties": { "role": string, "binding": string, "value": {} }
                            }
                        },
                        "quote": string
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
                    "properties": { "value": leaf, "by": rule(true), "from": contributors }
                }
            }
        }
    })
}
