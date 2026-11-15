//! The assumptions written in a calculation's model, read once when a worker
//! has initialized the program: every `@ meta(...)` of a section, rule
//! clause or member whose value is (or holds) an `Assumption`, its value
//! taken from the initialized program, and its condition (`when ...`)
//! parsed. A case then finds a fired clause's assumptions by its line,
//! without reading or scanning a file. The format is
//! `docs/reference/calculations.md#assumptions`.

use super::*;
use crate::explain::PlacedAssumption;
use std::collections::HashMap;
use std::rc::Rc;

/// The assumptions of one model file.
struct FileAssumptions {
    /// The character offset where each line starts.
    line_starts: Vec<usize>,
    /// `(first code line, last code line, assumptions)` of every section.
    sections: Vec<(usize, usize, Vec<PlacedAssumption>)>,
    /// `(name, first line, last line, assumptions)` of every rule clause,
    /// member or function with `@ meta`.
    items: Vec<(String, usize, usize, Vec<PlacedAssumption>)>,
}

impl FileAssumptions {
    fn line_of(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|start| *start <= offset)
    }

    /// The assumptions of the innermost section around `line` and of the
    /// item `name` there.
    fn at(&self, line: usize, name: &str) -> Vec<PlacedAssumption> {
        let mut found = Vec::new();
        if let Some((_, _, section)) = self
            .sections
            .iter()
            .filter(|(start, end, _)| *start <= line && line <= *end)
            .max_by_key(|(start, _, _)| *start)
        {
            found.extend(section.iter().cloned());
        }
        if let Some((_, _, _, item)) = self
            .items
            .iter()
            .filter(|(item, start, end, _)| item == name && *start <= line && line <= *end)
            .max_by_key(|(_, start, _, _)| *start)
        {
            found.extend(item.iter().cloned());
        }
        found
    }
}

/// The assumptions of a calculation's model.
pub(super) struct AssumptionIndex {
    root: Option<String>,
    files: HashMap<String, FileAssumptions>,
    /// The entry's and its section's.
    entry: Vec<PlacedAssumption>,
}

impl AssumptionIndex {
    /// Read the model's assumptions with the worker's initialized program.
    pub(super) fn read(
        interpreter: &mut Interpreter,
        env: &Env,
        root_file: Option<&Path>,
        stmts: &[Stmt],
        entry: &str,
    ) -> Self {
        let root = root_file.map(|file| file.to_string_lossy().into_owned());
        let mut paths: Vec<String> = root.iter().cloned().collect();
        paths.extend(interpreter.loaded_runtime_sources());
        // Every file's source, and every binding's defining expression: a
        // binding only metadata reads is not initialized, and an imported
        // file's bindings are not among the entry's statements.
        let mut sources = Vec::new();
        let mut definitions: HashMap<String, Expr> = HashMap::new();
        let mut define = |statements: &[Stmt]| {
            for statement in statements {
                if let Stmt::Bind(Pat::Var(name), _, defining) = statement {
                    definitions
                        .entry(name.clone())
                        .or_insert_with(|| defining.clone());
                }
            }
        };
        define(stmts);
        for path in paths {
            if sources.iter().any(|(known, _)| *known == path) {
                continue;
            }
            let Ok(source) = crate::read_source_file(Path::new(&path)) else {
                continue;
            };
            if source.contains("Assumption") {
                if let Ok(module) = parse_source_module_cached(Path::new(&path), &source) {
                    define(module.statements());
                }
            }
            sources.push((path, source));
        }
        let scope = Scope {
            env,
            definitions: &definitions,
        };
        let mut files = HashMap::new();
        for (path, source) in sources {
            // A file without an assumption needs no more than its line
            // starts, for the lines of its clauses.
            let mut file = FileAssumptions {
                line_starts: line_starts(&source),
                sections: Vec::new(),
                items: Vec::new(),
            };
            if source.contains("Assumption") || source.contains("@ meta(") {
                read_file(interpreter, &scope, &path, &source, &mut file);
            }
            files.insert(path, file);
        }
        let mut index = Self {
            root,
            files,
            entry: Vec::new(),
        };
        index.entry = index.entry_assumptions(stmts, entry);
        index
    }

    /// The assumptions written on a rule clause and its section.
    pub(super) fn of_clause(
        &self,
        interpreter: &Interpreter,
        rule: &Rc<Rule>,
        name: &str,
    ) -> Vec<PlacedAssumption> {
        let file = interpreter
            .rule_source_file(rule)
            .map(|file| file.to_string())
            .or_else(|| self.root.clone());
        let (Some(file), Some(head)) = (file, rule.head()) else {
            return Vec::new();
        };
        let Some(assumptions) = self.files.get(&file) else {
            return Vec::new();
        };
        if assumptions.sections.is_empty() && assumptions.items.is_empty() {
            return Vec::new();
        }
        assumptions.at(assumptions.line_of(head.span.start), name)
    }

    pub(super) fn entry(&self) -> &[PlacedAssumption] {
        &self.entry
    }

    /// The entry's own `@ meta` assumptions and its section's: the entry is
    /// a function or rule of the root file.
    fn entry_assumptions(&self, stmts: &[Stmt], entry: &str) -> Vec<PlacedAssumption> {
        let Some(root) = self.root.as_ref().and_then(|root| self.files.get(root)) else {
            return Vec::new();
        };
        let offset = stmts.iter().find_map(|statement| match statement {
            Stmt::Defn(Defn::Fn { name, body, .. }) if name == entry && !body.span.is_dummy() => {
                Some(body.span.start)
            }
            Stmt::Rule(rule) => rule
                .head()
                .filter(|head| {
                    matches!(&head.kind, ExprKind::App(function, _)
                        if matches!(&function.kind, ExprKind::Var(name) if name == entry))
                })
                .map(|head| head.span.start),
            _ => None,
        });
        offset.map_or_else(Vec::new, |offset| root.at(root.line_of(offset), entry))
    }
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (offset, c) in source.chars().enumerate() {
        if c == '\n' {
            starts.push(offset + 1);
        }
    }
    starts
}

/// What a metadata value is evaluated with: the initialized program's
/// environment, and the defining expression of every binding of the model.
struct Scope<'a> {
    env: &'a Env,
    definitions: &'a HashMap<String, Expr>,
}

impl Scope<'_> {
    /// The value of `expr`; a binding's name is its defining expression's
    /// value, since a binding only metadata reads is not initialized.
    fn eval(&self, interpreter: &mut Interpreter, expr: &Expr) -> Value {
        if let ExprKind::Var(name) = &expr.kind {
            if let Some(defining) = self.definitions.get(name) {
                return interpreter.eval(defining, self.env);
            }
        }
        interpreter.eval(expr, self.env)
    }
}

/// The sections and items of one file with their assumptions.
fn read_file(
    interpreter: &mut Interpreter,
    env: &Scope<'_>,
    path: &str,
    source: &str,
    file: &mut FileAssumptions,
) {
    let annotations = crate::annotations::read(source);
    let path: Rc<str> = Rc::from(path);
    for section in &annotations.sections {
        let placed = place(
            interpreter,
            env,
            &path,
            source,
            Some(&section.name),
            None,
            &section.metas,
            section.line,
        );
        if !placed.is_empty() {
            file.sections
                .push((section.line + 1, section.end_line.saturating_sub(1), placed));
        }
    }
    for item_meta in &annotations.item_metas {
        let item = &item_meta.item;
        if !matches!(item.kind, "rule" | "member" | "function") {
            continue;
        }
        let line = item_meta.metas.first().map_or(item.line, |meta| meta.line);
        let placed = place(
            interpreter,
            env,
            &path,
            source,
            item_meta.section.as_deref(),
            Some(&item.name),
            &item_meta.metas,
            line,
        );
        if !placed.is_empty() {
            file.items
                .push((item.name.clone(), item.line, item.end_line, placed));
        }
    }
}

/// The assumptions among `metas`, written at `line`, in `section` (if any)
/// and on `rule` (if written on a rule, member or function).
#[allow(clippy::too_many_arguments)]
fn place(
    interpreter: &mut Interpreter,
    env: &Scope<'_>,
    path: &Rc<str>,
    source: &str,
    section: Option<&str>,
    rule: Option<&str>,
    metas: &[crate::annotations::MetaAnnotation],
    line: usize,
) -> Vec<PlacedAssumption> {
    let mut placed = Vec::new();
    for meta in metas {
        let Ok(expr) = Parser::new(Lexer::new(&meta.value).tokenize(), &meta.value).parse_expr()
        else {
            continue;
        };
        let condition = meta.condition.as_ref().and_then(|text| {
            crate::annotations::parse_meta_condition(source, meta.line, text).ok()
        });
        for (binding, value) in assumptions_in(interpreter, env, &expr) {
            placed.push(PlacedAssumption {
                place: (path.clone(), line, placed.len()),
                assumption: Rc::new(CalculationAssumption {
                    section: section.map(str::to_string),
                    rule: rule.map(str::to_string),
                    binding,
                    value,
                }),
                condition: condition.clone(),
            });
        }
    }
    placed
}

/// The `Assumption` values a metadata expression gives, with the binding
/// each is written as: `Assumption(value = <binding or value>)`, or a value
/// that holds `Assumption`s (a `Metadata(attachments = (...))` binding).
fn assumptions_in(
    interpreter: &mut Interpreter,
    env: &Scope<'_>,
    expr: &Expr,
) -> Vec<(Option<String>, JsonValue)> {
    if let ExprKind::App(function, arguments) = &expr.kind {
        if matches!(&function.kind, ExprKind::Var(name) if crate::constructor_display_name(name) == "Assumption")
        {
            let value = arguments
                .iter()
                .find_map(|argument| named_arg_parts(argument).filter(|(name, _)| *name == "value"))
                .map(|(_, value)| value)
                .or_else(|| arguments.first());
            let Some(value) = value else {
                return Vec::new();
            };
            let binding = match &value.kind {
                ExprKind::Var(name) => Some(name.clone()),
                _ => None,
            };
            let evaluated = env.eval(interpreter, value);
            return vec![(binding, value_json(&evaluated))];
        }
    }
    if !matches!(expr.kind, ExprKind::Var(_)) {
        return Vec::new();
    }
    let mut found = Vec::new();
    collect_assumptions(&env.eval(interpreter, expr), &mut found);
    found
}

fn collect_assumptions(value: &Value, found: &mut Vec<(Option<String>, JsonValue)>) {
    let value = crate::explain::untraced(value);
    match value {
        Value::NamedConstructor(name, fields) => {
            if crate::constructor_display_name(name) == "Assumption" {
                if let Some((_, inner)) = fields.iter().find(|(field, _)| field == "value") {
                    found.push((None, value_json(inner)));
                }
                return;
            }
            for (_, field) in fields.iter() {
                collect_assumptions(field, found);
            }
        }
        Value::Constructor(name, items) => {
            if crate::constructor_display_name(name) == "Assumption" {
                if let Some(inner) = items.first() {
                    found.push((None, value_json(inner)));
                }
                return;
            }
            for item in items.iter() {
                collect_assumptions(item, found);
            }
        }
        Value::Tuple(items) => {
            for item in items.iter() {
                collect_assumptions(item, found);
            }
        }
        _ => {
            if let Some(items) = runtime_list_values(value) {
                for item in items {
                    collect_assumptions(item, found);
                }
            }
        }
    }
}

/// A metadata value as plain JSON: a record's fields as an object, a list
/// as an array, a variant without fields as its name.
fn value_json(value: &Value) -> JsonValue {
    let value = crate::explain::untraced(value);
    match value {
        Value::Int(number) => JsonValue::from(*number),
        Value::Float(number) => JsonNumber::from_f64(*number)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Bool(boolean) => JsonValue::Bool(*boolean),
        Value::Str(text) => JsonValue::from(text.to_string()),
        Value::Unit => JsonValue::Null,
        Value::NamedConstructor(_, fields) => JsonValue::Object(
            fields
                .iter()
                .map(|(field, item)| (field.clone(), value_json(item)))
                .collect(),
        ),
        Value::Constructor(name, items) if items.is_empty() => {
            JsonValue::from(crate::constructor_display_name(name).into_owned())
        }
        Value::Constructor(name, items) => serde_json::json!({
            "$variant": crate::constructor_display_name(name),
            "$values": items.iter().map(value_json).collect::<Vec<_>>(),
        }),
        Value::Tuple(items) => JsonValue::Array(items.iter().map(value_json).collect()),
        other => match runtime_list_values(other) {
            Some(items) => JsonValue::Array(items.into_iter().map(value_json).collect()),
            None => JsonValue::from(other.to_string()),
        },
    }
}
