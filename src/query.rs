//! Needs in the language (`docs/rfcs/needs-in-the-language.md`).
//!
//! `unknown` is a value not known yet; `query entry(case)` asks a
//! calculation entry about a case that holds unknowns and returns
//! `Query(o, i)`: `Complete` with the result, or `Needs` with what the case
//! still needs. A query runs the entry as `runa call --partial` does: the case
//! is encoded as a partial JSON case (an unknown record field is left out),
//! evaluated by the entry's prepared calculation, and its result decoded into
//! Futuruna values. Under `runa test` every query also runs the strict-mode
//! oracle and the completion test (`--partial=check`).
//!
//! `Need(i)` is the need the partial result writes (`CalculationNeed`): the
//! member tables here declare its Futuruna type, and the tests hold them to the
//! JSON the calculation writes and to `runa schema --format partial-result`.

use super::*;
use crate::calculate::{
    CalculationEnvelopeMetadata, CalculationInputCase, CalculationInputEnvelope,
    CalculationInputStatus, CalculationNeed, CalculationNeedKind, CalculationPartialResult,
    CalculationWaiting, PartialViolation, PreparedCalculation, INPUT_SCHEMA,
};
use crate::constructor_scope::{
    constructor_runtime_name, current_elaboration, with_constructor_elaboration,
};
use serde_json::Value as JsonValue;
use std::cell::Cell;

// ---------------------------------------------------------------------------
// The types of needs in Futuruna
// ---------------------------------------------------------------------------

/// A member of a record the partial result writes: its JSON name, its type in
/// Futuruna and whether the JSON always has it.
pub(crate) struct Member {
    pub name: &'static str,
    pub futuruna: &'static str,
    pub required: bool,
}

const fn member(name: &'static str, futuruna: &'static str, required: bool) -> Member {
    Member {
        name,
        futuruna,
        required,
    }
}

/// `Need(i)`: a member of `CalculationNeed`, in its order.
pub(crate) const NEED_MEMBERS: &[Member] = &[
    member("kind", "NeedKind", true),
    member("path", "Path(i)", true),
    member("keys", "List(NeedKey(i))", false),
    member("entry", "Option(ProgramReference)", false),
    member("subject", "Option(Path(i))", false),
    member("label", "Option(String)", false),
    member("question", "Option(String)", false),
    member("help", "Option(String)", false),
    member("unit", "Option(String)", false),
    member("sources", "List(NeedSource)", false),
    member("waiting", "List(NeedWaiting)", true),
];

/// `NeedKey(i)`: `CalculationNeedKey`. A key's value is its JSON text: a
/// `String` key as the string, an `Int` key as its digits.
pub(crate) const NEED_KEY_MEMBERS: &[Member] = &[
    member("path", "Path(i)", true),
    member("field", "String", true),
    member("value", "String", true),
];

/// `NeedWaiting`: `CalculationWaiting`.
pub(crate) const WAITING_MEMBERS: &[Member] = &[
    member("rule", "ProgramReference", true),
    member("section", "Option(ProgramReference)", false),
    member("paragraphs", "List(NeedParagraph)", false),
];

/// `NeedParagraph`: `CalculationParagraph`.
pub(crate) const PARAGRAPH_MEMBERS: &[Member] = &[
    member("paragraph", "String", true),
    member("source", "Option(String)", false),
    member("act", "Option(NeedAct)", false),
    member("as_amended_by", "Option(NeedAmendment)", false),
];

/// `NeedAct`: `CalculationAct`.
pub(crate) const ACT_MEMBERS: &[Member] = &[
    member("title", "String", true),
    member("identifier", "Option(String)", false),
    member("url", "Option(String)", false),
];

/// `NeedAmendment`: `CalculationAmendment`.
pub(crate) const AMENDMENT_MEMBERS: &[Member] = &[
    member("act", "String", true),
    member("paragraph", "String", true),
    member("instruction", "String", true),
];

/// `NeedSource`: the label, role and binding of a source reference
/// (`CalculationMetadataReference`); the binding as a reference to it.
pub(crate) const SOURCE_MEMBERS: &[Member] = &[
    member("label", "String", true),
    member("role", "String", true),
    member("binding", "Option(ProgramReference)", false),
];

/// `QueryAssumption`: `CalculationAssumption`, an assumption a `Complete`
/// result rests on: the section it is written in, and the binding it is
/// written as (none for an inline value), each as a reference a test compares
/// with `refof(...)`.
pub(crate) const ASSUMPTION_MEMBERS: &[Member] = &[
    member("section", "ProgramReference", true),
    member("binding", "Option(ProgramReference)", false),
];

fn record_declaration(name: &str, parameters: &str, members: &[Member]) -> String {
    let fields = members
        .iter()
        .map(|member| format!("{}: {}", member.name, member.futuruna))
        .collect::<Vec<_>>()
        .join(", ");
    format!("# {name}{parameters} = {name}({fields})\n")
}

/// The Futuruna source of needs in the language: the types a query returns,
/// `fill` and `interview`. A program that asks a query or places an `unknown`
/// gets it with the prelude.
pub fn needs_prelude_source() -> String {
    let mut source = String::from(
        "-- Needs in the language (docs/rfcs/needs-in-the-language.md).\n\
         # Query(o, i) = Complete(result: o, unused: List(Path(i)), assumptions: List(QueryAssumption)) | Needs(needs: List(Need(i)))\n\
         # NeedKind = FactNeed | EntryNeed\n",
    );
    source.push_str(&record_declaration("Need", "(i)", NEED_MEMBERS));
    source.push_str(&record_declaration("QueryAssumption", "", ASSUMPTION_MEMBERS));
    source.push_str(&record_declaration("NeedKey", "(i)", NEED_KEY_MEMBERS));
    source.push_str(&record_declaration("NeedSource", "", SOURCE_MEMBERS));
    source.push_str(&record_declaration("NeedWaiting", "", WAITING_MEMBERS));
    source.push_str(&record_declaration("NeedParagraph", "", PARAGRAPH_MEMBERS));
    source.push_str(&record_declaration("NeedAct", "", ACT_MEMBERS));
    source.push_str(&record_declaration("NeedAmendment", "", AMENDMENT_MEMBERS));
    source.push_str(
        r#"# Interview(o, i) = Interview(rounds: List(List(Path(i))), result: o)

-- Write into `partial`, at each path, the value of `complete` there: a record
-- as existing with its fields unknown, every other value whole.
> fill(partial: i, complete: i, paths: List(Path(i))) -> i {
    __fill(partial, complete, paths)
}

-- Ask until the case is complete, answering each round's needs from
-- `complete`.
> interview(ask: i -> Query(o, i), partial: i, complete: i) -> Interview(o, i) {
    interview_from(ask, partial, complete, [])
}

> interview_from(ask: i -> Query(o, i), case: i, complete: i, rounds: List(List(Path(i)))) -> Interview(o, i) {
    = q = ask(case)
    match q {
        | Complete -> Interview(rounds = rounds, result = q.result)
        | Needs -> {
            = asked = map(q.needs, |n| n.path)
            interview_from(ask, fill(case, complete, asked), complete, concat(rounds, [asked]))
        }
    }
}
"#,
    );
    source
}

/// The declarations of [`needs_prelude_source`].
pub fn needs_prelude() -> Vec<Stmt> {
    let source = needs_prelude_source();
    let tokens = Lexer::new(&source).tokenize();
    match Parser::new(tokens, &source).parse_program() {
        Ok(statements) => statements
            .into_iter()
            .filter(|statement| matches!(statement, Stmt::Defn(_) | Stmt::TypeDecl(_)))
            .collect(),
        Err(error) => {
            eprintln!("BUG: needs prelude parse error: {error}");
            Vec::new()
        }
    }
}

/// Whether a program asks a query, places an `unknown`, or calls `fill` or
/// `interview` without declaring its own.
pub fn program_uses_needs(statements: &[Stmt]) -> bool {
    let declares = |wanted: &str| {
        statements.iter().any(
            |statement| matches!(statement, Stmt::Defn(Defn::Fn { name, .. }) if name == wanted),
        )
    };
    let user = statements
        .iter()
        .position(|statement| matches!(statement, Stmt::PreludeBoundary))
        .map_or(statements, |boundary| &statements[boundary + 1..]);
    let mut uses = false;
    let fill = !declares("fill");
    let interview = !declares("interview");
    for statement in user {
        if stmt_mentions_query_or_unknown(statement) {
            return true;
        }
        walk_ast_stmt(statement, &mut |child| {
            if let AstChild::Expr(expr) = child {
                if let ExprKind::Var(name) = &expr.kind {
                    uses |= (fill && name == "fill") || (interview && name == "interview");
                }
            }
        });
        if uses {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Checks: where `unknown` stands, what `query` asks, and that no calculation
// reaches either
// ---------------------------------------------------------------------------

/// Where an expression stands, for `unknown`.
#[derive(Clone, Copy)]
enum Place {
    /// A value is placed with a type from its place.
    Value,
    /// The value is read.
    Read(&'static str),
    /// A value is placed, but the place gives it no type.
    Untyped,
}

/// One file of the program, with what its diagnostics need.
pub(crate) struct ProgramFile<'a> {
    /// `None` for the root file, whose diagnostics carry spans.
    pub(crate) path: Option<&'a str>,
    pub(crate) source: &'a str,
    pub(crate) statements: &'a [Stmt],
}

impl ProgramFile<'_> {
    pub(crate) fn diagnostic(&self, span: Span, message: String) -> Diagnostic {
        match self.path {
            None => Diagnostic::error_at(span, message),
            Some(path) => Diagnostic::error(format!(
                "{path}:{}: {message}",
                line_of_offset(self.source, span.start)
            )),
        }
    }

    /// Where `span` is, for a message about another place: `line N` in the
    /// root file, `path:N` in an imported one.
    pub(crate) fn place(&self, span: Span) -> String {
        let line = line_of_offset(self.source, span.start);
        match self.path {
            None => format!("line {line}"),
            Some(path) => format!("{path}:{line}"),
        }
    }
}

/// The root file's own statements (after the prelude) and each file it
/// imports, as the checker read them.
pub(crate) fn program_files<'a>(
    statements: &'a [Stmt],
    source: &'a str,
    checker: &'a TypeChecker,
) -> Vec<ProgramFile<'a>> {
    let user = statements
        .iter()
        .position(|statement| matches!(statement, Stmt::PreludeBoundary))
        .map_or(statements, |boundary| &statements[boundary + 1..]);
    let mut files = vec![ProgramFile {
        path: None,
        source,
        statements: user,
    }];
    for (path, module) in &checker.imported_modules {
        files.push(ProgramFile {
            path: Some(path),
            source: module.source(),
            statements: module.statements(),
        });
    }
    files
}

/// The 1-based line of a character offset.
pub(crate) fn line_of_offset(source: &str, offset: usize) -> usize {
    source
        .chars()
        .take(offset)
        .filter(|character| *character == '\n')
        .count()
        + 1
}

pub(crate) fn rule_head_name(head: &Expr) -> Option<&str> {
    match &head.kind {
        ExprKind::App(function, _) => rule_head_name(function),
        ExprKind::Var(name) => Some(name.as_str()),
        _ => None,
    }
}

/// The name a top-level statement declares as a callable or value.
fn declared_name(statement: &Stmt) -> Option<&str> {
    match statement {
        Stmt::Defn(Defn::Fn { name, .. }) => Some(name.as_str()),
        Stmt::Rule(
            Rule::Clause { head, .. } | Rule::Default { head, .. } | Rule::Exception { head, .. },
        ) => rule_head_name(head),
        Stmt::Bind(Pat::Var(name), _, _) => Some(name.as_str()),
        _ => None,
    }
}

/// The names of the calculation entries (`@ calculate`) of `statements`.
fn calculation_entries(statements: &[Stmt]) -> Vec<&str> {
    let mut entries = Vec::new();
    let mut marked = false;
    for statement in statements {
        if let Stmt::Annot(name, _) = statement {
            marked |= name == "calculate";
            continue;
        }
        if std::mem::take(&mut marked) {
            if let Some(name) = declared_name(statement) {
                entries.push(name);
            }
        }
    }
    entries
}

/// Check the `query`s and `unknown`s of a program: where each `unknown`
/// stands, that each query asks a calculation entry, and that no calculation
/// entry reaches either.
pub(crate) fn check_program(
    statements: &[Stmt],
    source: &str,
    checker: &TypeChecker,
) -> Vec<Diagnostic> {
    let files = program_files(statements, source, checker);
    let mut diagnostics = path_comparisons(&files);
    let mentions = files
        .iter()
        .any(|file| file.statements.iter().any(stmt_mentions_query_or_unknown));
    if !mentions {
        return diagnostics;
    }
    let entries: BTreeSet<&str> = files
        .iter()
        .flat_map(|file| calculation_entries(file.statements))
        .collect();
    for file in &files {
        let mut placement = Placement {
            file,
            checker,
            entries: &entries,
            diagnostics: &mut diagnostics,
        };
        for statement in file.statements {
            placement.statement(statement);
        }
    }
    diagnostics.extend(reachability(&files, &entries));
    diagnostics
}

/// `pathof(A::...) == pathof(B::...)` with `A` not `B` compares paths into
/// different types, which are never equal: a check error, as a `Path(A)`
/// compared with a `pathof` rooted elsewhere is.
fn path_comparisons(files: &[ProgramFile]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for file in files {
        for statement in file.statements {
            walk_ast_stmt(statement, &mut |child| {
                let AstChild::Expr(expr) = child else {
                    return;
                };
                let ExprKind::BinOp(operator, left, right) = &expr.kind else {
                    return;
                };
                if operator != "==" && operator != "!=" {
                    return;
                }
                if let (Some(left_root), Some(right_root)) = (pathof_root(left), pathof_root(right))
                {
                    if left_root != right_root {
                        diagnostics.push(file.diagnostic(
                            expr.span,
                            format!(
                                "`{operator}` compares a `Path({left_root})` with a `Path({right_root})`: paths into different types are never equal. Compare paths of one root"
                            ),
                        ));
                    }
                }
            });
        }
    }
    diagnostics
}

struct Placement<'a, 'b> {
    file: &'b ProgramFile<'a>,
    checker: &'b TypeChecker,
    entries: &'b BTreeSet<&'a str>,
    diagnostics: &'b mut Vec<Diagnostic>,
}

impl Placement<'_, '_> {
    fn report(&mut self, span: Span, message: String) {
        let diagnostic = self.file.diagnostic(span, message);
        self.diagnostics.push(diagnostic);
    }

    fn is_builtin(&self, name: &str) -> bool {
        self.checker.builtins.contains_key(name)
            && !self.checker.user_functions.contains(name)
            && !self.checker.constructors.contains_key(name)
    }

    fn statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Defn(Defn::Fn { ret_ty, body, .. }) => {
                let place = if ret_ty.is_some() {
                    Place::Value
                } else {
                    Place::Untyped
                };
                self.expr(body, place);
            }
            Stmt::Defn(Defn::Module { body, .. }) => {
                for statement in body {
                    self.statement(statement);
                }
            }
            Stmt::Rule(Rule::Clause { body, .. }) => {
                if let Some(body) = body {
                    self.expr(body, Place::Value);
                }
            }
            Stmt::Rule(Rule::Default {
                value, condition, ..
            })
            | Stmt::Rule(Rule::Exception {
                value, condition, ..
            }) => {
                self.expr(value, Place::Value);
                if let Some(condition) = condition {
                    self.expr(condition, Place::Read("a rule's condition"));
                }
            }
            Stmt::Rule(Rule::ReactiveScope { body, .. }) => {
                for statement in body {
                    self.statement(statement);
                }
            }
            Stmt::Bind(_, ty, value) | Stmt::MonadicBind(_, ty, value) => {
                let place = if ty.is_some() {
                    Place::Value
                } else {
                    Place::Untyped
                };
                self.expr(value, place);
            }
            Stmt::Invariant {
                subject, predicate, ..
            } => {
                self.expr(subject, Place::Untyped);
                self.expr(predicate, Place::Read("an invariant's predicate"));
            }
            Stmt::Expr(expr) => self.expr(expr, Place::Untyped),
            Stmt::TypeDecl(_)
            | Stmt::Defn(_)
            | Stmt::Explore(_)
            | Stmt::PreludeBoundary
            | Stmt::Use(_)
            | Stmt::Import(_)
            | Stmt::QualifiedImport(_, _)
            | Stmt::HashImport(_, _)
            | Stmt::Depend(_, _)
            | Stmt::RustBlock(_)
            | Stmt::Prove { .. } => {}
            other => {
                let read = Place::Read("read by a statement");
                visit_ast_stmt_children(other, &mut |child| match child {
                    AstChild::Expr(expr) => self.expr(expr, read),
                    AstChild::Stmt(statement) => self.statement(statement),
                });
            }
        }
    }

    fn expr(&mut self, expr: &Expr, place: Place) {
        if is_unknown_expr(expr) {
            match place {
                Place::Value => {}
                Place::Read(what) => self.report(
                    expr.span,
                    format!("`unknown` cannot be {what}: an unknown value is placed, never read. It stands where a value is placed with a type from its place: an argument, a field value, a list element, a typed binding, a declared result, a branch"),
                ),
                Place::Untyped => self.report(
                    expr.span,
                    "`unknown` takes the type of its place, and this place has none; give it one, as in `= x: Int = unknown`".to_string(),
                ),
            }
            return;
        }
        if let Some((entry, call, case)) = query_parts(expr) {
            if !self.entries.contains(entry) {
                self.report(
                    call.span,
                    format!(
                        "`query` asks a calculation entry (`@ calculate`); `{entry}` is not one"
                    ),
                );
            }
            self.expr(case, Place::Value);
            return;
        }
        if pathof_canonical_path(expr).is_some() || refof_literal(expr).is_some() {
            return;
        }
        let read = |what: &'static str| Place::Read(what);
        match &expr.kind {
            ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
            ExprKind::App(function, arguments) => {
                if let Some((_, value)) = named_arg_parts(expr) {
                    self.expr(value, place);
                    return;
                }
                let builtin = match &function.kind {
                    ExprKind::Var(name) if name == "__typed" => {
                        if let Some(value) = arguments.first() {
                            self.expr(value, place);
                        }
                        return;
                    }
                    ExprKind::Var(name) => self.is_builtin(name),
                    _ => false,
                };
                self.expr(function, read("called"));
                let argument = if builtin {
                    read("an argument of a builtin, which accepts any type")
                } else {
                    Place::Value
                };
                for value in arguments {
                    match named_arg_parts(value) {
                        Some((_, value)) => self.expr(value, argument),
                        None => self.expr(value, argument),
                    }
                }
            }
            ExprKind::Lambda(_, body) => self.expr(body, Place::Value),
            ExprKind::BinOp(operator, left, right) => {
                let operand = if operator == "==" || operator == "!=" {
                    read("a side of a comparison")
                } else {
                    read("an operand")
                };
                self.expr(left, operand);
                self.expr(right, operand);
            }
            ExprKind::UnOp(_, operand) => self.expr(operand, read("an operand")),
            ExprKind::If(condition, then, otherwise) => {
                self.expr(condition, read("an `if` condition"));
                self.expr(then, place);
                self.expr(otherwise, place);
            }
            ExprKind::Match(scrutinee, arms) => {
                self.expr(scrutinee, read("matched"));
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.expr(guard, read("a guard"));
                    }
                    self.expr(&arm.body, place);
                }
            }
            ExprKind::Block(statements) => {
                for (index, statement) in statements.iter().enumerate() {
                    match statement {
                        Stmt::Expr(expr) if index + 1 == statements.len() => self.expr(expr, place),
                        statement => self.statement(statement),
                    }
                }
            }
            ExprKind::Field(object, _) => self.expr(object, read("the record of a field access")),
            ExprKind::Index(list, index) => {
                self.expr(list, read("indexed"));
                self.expr(index, read("an index"));
            }
            ExprKind::List(items) => {
                let item = match place {
                    Place::Value => Place::Value,
                    Place::Read(what) => Place::Read(what),
                    Place::Untyped => Place::Untyped,
                };
                for value in items {
                    self.expr(value, item);
                }
            }
            ExprKind::Tuple(items) => {
                for value in items {
                    self.expr(value, Place::Untyped);
                }
            }
            ExprKind::Effect(_, arguments) => {
                for value in arguments {
                    self.expr(value, read("an argument of an effect"));
                }
            }
            ExprKind::Handle { handlers, body, .. } => {
                self.expr(body, place);
                for handler in handlers {
                    self.expr(&handler.body, Place::Value);
                }
            }
            ExprKind::Try(inner) => self.expr(inner, read("tried")),
            ExprKind::Conjunction(goals) | ExprKind::Disjunction(goals) => {
                for goal in goals {
                    self.expr(goal, read("a goal"));
                }
            }
            ExprKind::Pipe(value, function) => {
                self.expr(value, read("piped"));
                self.expr(function, read("called"));
            }
        }
    }
}

/// What a top-level declaration contains: the first `query` or `unknown` in
/// it, and the declarations it refers to.
struct Node<'a> {
    file: usize,
    mention: Option<(&'static str, Span)>,
    callees: BTreeSet<&'a str>,
}

fn first_mention(statement: &Stmt) -> Option<(&'static str, Span)> {
    let mut found = None;
    walk_ast_stmt(statement, &mut |child| {
        if found.is_some() {
            return;
        }
        if let AstChild::Expr(expr) = child {
            if is_unknown_expr(expr) {
                found = Some(("`unknown`", expr.span));
            } else if query_parts(expr).is_some() {
                found = Some(("`query`", expr.span));
            }
        }
    });
    found
}

/// A calculation's result must not depend on which facts are known: no
/// `query` or `unknown` may be reached from an entry.
fn reachability(files: &[ProgramFile], entries: &BTreeSet<&str>) -> Vec<Diagnostic> {
    let mut nodes: BTreeMap<&str, Node> = BTreeMap::new();
    for (index, file) in files.iter().enumerate() {
        for statement in file.statements {
            let Some(name) = declared_name(statement) else {
                continue;
            };
            let node = nodes.entry(name).or_insert_with(|| Node {
                file: index,
                mention: None,
                callees: BTreeSet::new(),
            });
            if node.mention.is_none() {
                if let Some(mention) = first_mention(statement) {
                    node.mention = Some(mention);
                    node.file = index;
                }
            }
        }
    }
    let names: BTreeSet<&str> = nodes.keys().copied().collect();
    for file in files {
        for statement in file.statements {
            let Some(name) = declared_name(statement) else {
                continue;
            };
            let mut callees = BTreeSet::new();
            walk_ast_stmt(statement, &mut |child| {
                if let AstChild::Expr(expr) = child {
                    if let ExprKind::Var(callee) = &expr.kind {
                        if let Some(callee) = names.get(callee.as_str()) {
                            callees.insert(*callee);
                        }
                    }
                }
            });
            if let Some(node) = nodes.get_mut(name) {
                node.callees.extend(callees);
            }
        }
    }
    let mut diagnostics = Vec::new();
    let mut reported = BTreeSet::new();
    for entry in entries {
        if !nodes.contains_key(entry) {
            continue;
        }
        let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::from([*entry]);
        let mut seen = BTreeSet::from([*entry]);
        while let Some(name) = queue.pop_front() {
            let node = &nodes[name];
            if let Some((what, span)) = node.mention {
                if reported.insert(name) {
                    let mut chain = Vec::new();
                    let mut current = name;
                    while let Some(previous) = parent.get(current) {
                        chain.push(*previous);
                        current = previous;
                    }
                    chain.reverse();
                    let message = if name == *entry {
                        format!("{what} in the calculation `{entry}`: a calculation's result must not depend on which facts are known")
                    } else {
                        let through = chain
                            .iter()
                            .skip(1)
                            .map(|name| format!("`{name}`"))
                            .collect::<Vec<_>>();
                        let through = if through.is_empty() {
                            String::new()
                        } else {
                            format!(" through {}", through.join(", "))
                        };
                        format!("{what} in `{name}`, reached from the calculation `{entry}`{through}: a calculation's result must not depend on which facts are known")
                    };
                    diagnostics.push(files[node.file].diagnostic(span, message));
                }
            }
            for callee in &node.callees {
                if seen.insert(*callee) {
                    parent.insert(*callee, name);
                    queue.push_back(*callee);
                }
            }
        }
    }
    diagnostics
}

// ---------------------------------------------------------------------------
// Running queries
// ---------------------------------------------------------------------------

/// Marks the path of an unknown no query has placed yet: the `unknown`'s file
/// and character offset follow it.
const UNPLACED: char = '\u{2}';

/// The prepared entries a program's queries ask, and how they run.
#[derive(Default)]
pub struct QueryRuntime {
    /// Run the strict-mode oracle and the completion test on every query.
    check: bool,
    /// The program's root file: where a query in it stands, and where the
    /// search for the entry it asks begins.
    root_file: Option<PathBuf>,
    prepared: RefCell<BTreeMap<(PathBuf, String), Rc<RefCell<PreparedCalculation>>>>,
    /// How often each query (file, offset) ran: its round.
    rounds: RefCell<BTreeMap<(PathBuf, usize), usize>>,
    sources: RefCell<BTreeMap<PathBuf, Rc<str>>>,
    /// Whether an `unknown` was evaluated: reads of unknowns are checked only
    /// then.
    pub(crate) unknown_built: Cell<bool>,
}

impl QueryRuntime {
    fn source(&self, path: &Path) -> Option<Rc<str>> {
        if let Some(source) = self.sources.borrow().get(path) {
            return Some(source.clone());
        }
        let source: Rc<str> = std::fs::read_to_string(path).ok()?.into();
        self.sources
            .borrow_mut()
            .insert(path.to_path_buf(), source.clone());
        Some(source)
    }

    fn line(&self, path: &Path, offset: usize) -> Option<usize> {
        Some(line_of_offset(&self.source(path)?, offset))
    }
}

/// `path:line` for display, relative to the working directory when below it.
fn display_location(path: &Path, line: Option<usize>) -> String {
    let shown = std::env::current_dir()
        .ok()
        .and_then(|directory| path.strip_prefix(directory).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf());
    match line {
        Some(line) => format!("{}:{line}", shown.display()),
        None => shown.display().to_string(),
    }
}

/// The file that declares the calculation entry `entry`: `start` or a file it
/// imports, directly or through others.
fn entry_file(entry: &str, start: &Path) -> Result<PathBuf, String> {
    let mut queue = std::collections::VecDeque::from([start.to_path_buf()]);
    let mut seen = BTreeSet::new();
    while let Some(file) = queue.pop_front() {
        let canonical = std::fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
        if !seen.insert(canonical.clone()) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&canonical) else {
            continue;
        };
        let Ok(statements) = Parser::new(Lexer::new(&source).tokenize(), &source).parse_program()
        else {
            continue;
        };
        if calculation_entries(&statements).contains(&entry) {
            return Ok(canonical);
        }
        let directory = canonical
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        for statement in &statements {
            if let Stmt::Import(path) = statement {
                if let Some(path) = TypeChecker::resolve_tc_import(path, &directory) {
                    queue.push_back(PathBuf::from(path));
                }
            }
        }
    }
    Err(format!(
        "no calculation entry `{entry}` in {} or the files it imports",
        display_location(start, None)
    ))
}

/// Check the file that declares `entry` and prepare the entry, as
/// `runa call` does.
fn prepare_entry(entry: &str, file: &Path) -> Result<PreparedCalculation, String> {
    let source = std::fs::read_to_string(file)
        .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
    let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
        .parse_program()
        .map_err(|error| format!("{}: {error}", display_location(file, None)))?;
    let statements = prepend_prelude(parse_prelude(), &statements);
    let source_dir = file
        .parent()
        .map(|parent| parent.to_string_lossy().to_string());
    // Checking installs the entry program's constructor names; the asking
    // program keeps its own.
    let current = current_elaboration();
    let checked = with_constructor_elaboration(current.as_ref(), || {
        TypeChecker::check_with_artifacts(&statements, source_dir.clone(), &source)
    });
    if let Some(diagnostic) = checked.diagnostics.first() {
        return Err(format!(
            "{} does not check: {}",
            display_location(file, None),
            diagnostic.message
        ));
    }
    let contract = checked
        .calculation_contracts
        .iter()
        .find(|contract| contract.entry == entry)
        .cloned()
        .ok_or_else(|| {
            format!(
                "{} has no calculation `{entry}`",
                display_location(file, None)
            )
        })?;
    Ok(PreparedCalculation::new(
        contract,
        checked.program().clone(),
        source_dir,
        checked.constructor_elaboration.clone(),
    )
    .with_root_file(file.to_path_buf()))
}

fn some(value: Value) -> Value {
    Value::Constructor("Some".to_string(), Rc::new(vec![value]))
}

fn none() -> Value {
    Value::Constructor("None".to_string(), Rc::new(Vec::new()))
}

fn optional(value: Option<Value>) -> Value {
    value.map_or_else(none, some)
}

fn string(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn record(owner: &str, constructor: &str, fields: Vec<(&str, Value)>) -> Value {
    Value::NamedConstructor(
        constructor_runtime_name(owner, constructor).into_owned(),
        Rc::new(
            fields
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        ),
    )
}

fn reference(constructor: &str, name: &str) -> Value {
    record(
        "ProgramReference",
        constructor,
        vec![("name", string(name))],
    )
}

/// `Need(i)` of a need the partial result writes, its members in the order of
/// [`NEED_MEMBERS`]; `typed` writes a JSON input path as `pathof` does.
fn need_value(need: &CalculationNeed, typed: &dyn Fn(&str) -> String) -> Value {
    let kind = match need.kind {
        CalculationNeedKind::Fact => "FactNeed",
        CalculationNeedKind::Entry => "EntryNeed",
    };
    let text = |value: &Option<String>| optional(value.as_deref().map(string));
    record(
        "Need",
        "Need",
        vec![
            (
                "kind",
                Value::Constructor(
                    constructor_runtime_name("NeedKind", kind).into_owned(),
                    Rc::new(Vec::new()),
                ),
            ),
            ("path", string(&typed(&need.path))),
            (
                "keys",
                Value::List(
                    need.keys
                        .iter()
                        .map(|key| {
                            record(
                                "NeedKey",
                                "NeedKey",
                                vec![
                                    ("path", string(&typed(&key.path))),
                                    ("field", string(&key.field)),
                                    (
                                        "value",
                                        string(&match &key.value {
                                            JsonValue::String(text) => text.clone(),
                                            other => other.to_string(),
                                        }),
                                    ),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "entry",
                optional(
                    need.entry
                        .as_deref()
                        .map(|entry| reference("ProgramSymbolReference", entry)),
                ),
            ),
            (
                "subject",
                optional(
                    need.subject
                        .as_deref()
                        .map(|subject| string(&typed(subject))),
                ),
            ),
            ("label", text(&need.label)),
            ("question", text(&need.question)),
            ("help", text(&need.help)),
            ("unit", text(&need.unit)),
            (
                "sources",
                Value::List(
                    need.sources
                        .iter()
                        .map(|source| {
                            record(
                                "NeedSource",
                                "NeedSource",
                                vec![
                                    ("label", string(&source.label)),
                                    ("role", string(&source.role)),
                                    (
                                        "binding",
                                        optional((!source.binding.is_empty()).then(|| {
                                            reference("ProgramSymbolReference", &source.binding)
                                        })),
                                    ),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "waiting",
                Value::List(need.waiting.iter().map(waiting_value).collect()),
            ),
        ],
    )
}

fn waiting_value(waiting: &CalculationWaiting) -> Value {
    record(
        "NeedWaiting",
        "NeedWaiting",
        vec![
            ("rule", reference("ProgramSymbolReference", &waiting.rule)),
            (
                "section",
                optional(
                    waiting
                        .section
                        .as_deref()
                        .map(|section| reference("ProgramSectionReference", section)),
                ),
            ),
            (
                "paragraphs",
                Value::List(
                    waiting
                        .paragraphs
                        .iter()
                        .map(|paragraph| {
                            record(
                                "NeedParagraph",
                                "NeedParagraph",
                                vec![
                                    ("paragraph", string(&paragraph.paragraph)),
                                    ("source", optional(paragraph.source.as_deref().map(string))),
                                    (
                                        "act",
                                        optional(paragraph.act.as_ref().map(|act| {
                                            record(
                                                "NeedAct",
                                                "NeedAct",
                                                vec![
                                                    ("title", string(&act.title)),
                                                    (
                                                        "identifier",
                                                        optional(
                                                            act.identifier.as_deref().map(string),
                                                        ),
                                                    ),
                                                    (
                                                        "url",
                                                        optional(act.url.as_deref().map(string)),
                                                    ),
                                                ],
                                            )
                                        })),
                                    ),
                                    (
                                        "as_amended_by",
                                        optional(paragraph.as_amended_by.as_ref().map(
                                            |amendment| {
                                                record(
                                                    "NeedAmendment",
                                                    "NeedAmendment",
                                                    vec![
                                                        ("act", string(&amendment.act)),
                                                        ("paragraph", string(&amendment.paragraph)),
                                                        (
                                                            "instruction",
                                                            string(&amendment.instruction),
                                                        ),
                                                    ],
                                                )
                                            },
                                        )),
                                    ),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
        ],
    )
}

/// The leaves a completion fills in that the partial case leaves out, as
/// `path = value`.
fn filled_in(
    partial: Option<&JsonValue>,
    completion: &JsonValue,
    path: &str,
    out: &mut Vec<String>,
) {
    match (partial, completion) {
        (Some(JsonValue::Object(given)), JsonValue::Object(fields)) => {
            for (name, value) in fields {
                filled_in(given.get(name), value, &partial::join_path(path, name), out);
            }
        }
        (Some(JsonValue::Array(given)), JsonValue::Array(items)) => {
            for (index, item) in items.iter().enumerate() {
                filled_in(given.get(index), item, &format!("{path}[{index}]"), out);
            }
        }
        (Some(_), _) => {}
        (None, JsonValue::Object(fields)) if !fields.is_empty() => {
            for (name, value) in fields {
                filled_in(None, value, &partial::join_path(path, name), out);
            }
        }
        (None, value) => out.push(format!("{path} = {value}")),
    }
}

fn completion_text(partial: &JsonValue, completion: &JsonValue) -> String {
    let mut leaves = Vec::new();
    filled_in(Some(partial), completion, "", &mut leaves);
    const SHOWN: usize = 8;
    let more = leaves.len().saturating_sub(SHOWN);
    leaves.truncate(SHOWN);
    let mut text = leaves.join(", ");
    if more > 0 {
        text.push_str(&format!(" and {more} more"));
    }
    text
}

fn violation_message(violation: &PartialViolation, partial: &JsonValue) -> String {
    match violation {
        PartialViolation::Oracle { .. } => "the strict-mode oracle fails: strict evaluation is Complete, but propagation does not return the same result".to_string(),
        PartialViolation::CompletionDiffers {
            completion,
            returns,
            expected,
            ..
        } => format!(
            "the propagated result is Complete, but the completion {} returns a different result: {returns} instead of {expected}",
            completion_text(partial, completion)
        ),
        PartialViolation::CompletionFails {
            completion,
            message,
            ..
        } => format!(
            "the propagated result is Complete, but the completion {} fails: {message}",
            completion_text(partial, completion)
        ),
    }
}

/// A step of a path into a value.
enum Step<'a> {
    Field(&'a str),
    Index(usize),
    Variant,
}

fn path_steps(path: &str) -> Option<Vec<Step<'_>>> {
    let mut steps = Vec::new();
    if path.is_empty() {
        return Some(steps);
    }
    for segment in path.split('.') {
        let (name, mut rest) = segment.split_at(segment.find('[').unwrap_or(segment.len()));
        match name {
            "" => {}
            "$variant" => steps.push(Step::Variant),
            name => steps.push(Step::Field(name)),
        }
        while let Some(stripped) = rest.strip_prefix('[') {
            let close = stripped.find(']')?;
            steps.push(Step::Index(stripped[..close].parse().ok()?));
            rest = &stripped[close + 1..];
        }
        if !rest.is_empty() {
            return None;
        }
    }
    Some(steps)
}

/// The constructor name and fields of a record or a named-field variant.
fn named_fields(value: &Value) -> Option<(&str, &[(String, Value)])> {
    match value {
        Value::NamedConstructor(name, fields) => Some((name, fields)),
        Value::NamespacedNamedConstructor { name, fields, .. } => Some((name, fields)),
        _ => None,
    }
}

fn with_fields(value: &Value, fields: Vec<(String, Value)>) -> Value {
    match value {
        Value::NamespacedNamedConstructor {
            namespace,
            name,
            declaration_env,
            ..
        } => Value::NamespacedNamedConstructor {
            namespace: namespace.clone(),
            name: name.clone(),
            fields: Rc::new(fields),
            declaration_env: declaration_env.clone(),
        },
        Value::NamedConstructor(name, _) => Value::NamedConstructor(name.clone(), Rc::new(fields)),
        other => other.clone(),
    }
}

fn some_inner(value: &Value) -> Option<&Value> {
    match value {
        Value::Constructor(name, values) if name == "Some" && values.len() == 1 => Some(&values[0]),
        _ => None,
    }
}

/// The elements of a list value.
fn list_items(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::List(items) => Some(items.clone()),
        Value::Constructor(name, _) if name == "Cons" || name == "Nil" => Some(list_to_vec(value)),
        _ => None,
    }
}

/// Whether `name` is the alternative `value` is: a step `pathof` writes
/// before a payload field.
fn is_alternative(value: &Value, name: &str) -> bool {
    if !name.starts_with(char::is_uppercase) {
        return false;
    }
    let constructor = match value {
        Value::Constructor(constructor, _) | Value::NamedConstructor(constructor, _) => constructor,
        Value::NamespacedConstructor { name, .. }
        | Value::NamespacedNamedConstructor { name, .. } => name,
        _ => return false,
    };
    constructor_display_name(constructor) == name
}

impl Interpreter {
    /// Run each `query` with the strict-mode oracle and the completion test,
    /// as `runa test` does; `root_file` is the program's file.
    pub fn check_queries(&mut self, root_file: &Path) {
        self.query.check = true;
        self.set_query_root(root_file);
    }

    /// Name the program's root file, where queries in it stand and where the
    /// search for the entries they ask begins.
    pub fn set_query_root(&mut self, root_file: &Path) {
        self.query.root_file =
            Some(std::fs::canonicalize(root_file).unwrap_or_else(|_| root_file.to_path_buf()));
    }

    /// Whether no root file is named for queries yet.
    pub(crate) fn query_root_unset(&self) -> bool {
        self.query.root_file.is_none()
    }

    /// The file the statement being evaluated comes from.
    fn query_current_file(&self) -> Option<PathBuf> {
        self.runtime_source_file
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| self.query.root_file.clone())
    }

    pub(crate) fn eval_unknown(&self, expression: &Expr) -> Value {
        self.query.unknown_built.set(true);
        let file = self
            .query_current_file()
            .map(|file| file.to_string_lossy().to_string())
            .unwrap_or_default();
        partial::unknown(BTreeSet::from([format!(
            "{UNPLACED}{}{UNPLACED}{file}",
            expression.span.start
        )]))
    }

    /// Outside a partial case, a read of an `unknown` no query has placed is
    /// an error naming the `unknown`'s line.
    pub(crate) fn unplaced_unknown_read(&self, paths: &BTreeSet<String>) -> Option<Value> {
        if self.partial_mode.is_some() {
            return None;
        }
        let marker = paths.iter().find(|path| path.starts_with(UNPLACED))?;
        let mut parts = marker[UNPLACED.len_utf8()..].splitn(2, UNPLACED);
        let offset = parts.next().and_then(|offset| offset.parse::<usize>().ok());
        let file = parts
            .next()
            .filter(|file| !file.is_empty())
            .map(PathBuf::from);
        let at = match (file, offset) {
            (Some(file), Some(offset)) => {
                format!(
                    " at {}",
                    display_location(&file, self.query.line(&file, offset))
                )
            }
            _ => String::new(),
        };
        Some(self.panic_or_ground_fail(format!(
            "the `unknown`{at} is read before a query places it in a case: only a query asks an unknown value. Build this part of the case without reading it, or state the value"
        )))
    }

    /// `partial_need` outside a partial case: an unplaced unknown among
    /// `values` is an error.
    pub(crate) fn unplaced_unknown_values(&self, values: &[&Value], deep: bool) -> Option<Value> {
        if !self.query.unknown_built.get() {
            return None;
        }
        let mut paths = BTreeSet::new();
        for value in values {
            partial::collect_unknown_paths(value, deep, &mut paths);
        }
        self.unplaced_unknown_read(&paths)
    }

    pub(crate) fn eval_query(&mut self, expression: &Expr, env: &Env) -> Value {
        let Some((entry, _, case)) = query_parts(expression) else {
            return self
                .panic_or_ground_fail("`query` asks a calculation entry: `query entry(case)`");
        };
        let entry = entry.to_string();
        let case = self.eval(case, env);
        if self.calculation_failed() {
            return Value::Unit;
        }
        let file = self.query_current_file();
        let line = file
            .as_deref()
            .and_then(|file| self.query.line(file, expression.span.start));
        let site = file.as_deref().map(|file| display_location(file, line));
        let round = file.map(|file| {
            let mut rounds = self.query.rounds.borrow_mut();
            let round = rounds.entry((file, expression.span.start)).or_default();
            *round += 1;
            *round
        });
        match self.run_query(&entry, &case) {
            Ok(value) => value,
            Err(message) => {
                let at = site.map(|site| format!("{site}: ")).unwrap_or_default();
                let round = round
                    .map(|round| format!(", round {round}"))
                    .unwrap_or_default();
                self.panic_or_ground_fail(format!("{at}query {entry}{round}: {message}"))
            }
        }
    }

    fn query_prepared(&self, entry: &str) -> Result<Rc<RefCell<PreparedCalculation>>, String> {
        let start = self.query_current_file().ok_or_else(|| {
            "a query needs the file of its program to find the entry it asks".to_string()
        })?;
        let file = entry_file(entry, &start)?;
        let key = (file.clone(), entry.to_string());
        if let Some(prepared) = self.query.prepared.borrow().get(&key) {
            return Ok(prepared.clone());
        }
        let prepared = Rc::new(RefCell::new(prepare_entry(entry, &file)?));
        self.query
            .prepared
            .borrow_mut()
            .insert(key, prepared.clone());
        Ok(prepared)
    }

    fn run_query(&mut self, entry: &str, case: &Value) -> Result<Value, String> {
        let prepared = self.query_prepared(entry)?;
        let mut prepared = prepared.borrow_mut();
        let contract = prepared.contract().clone();
        let input = contract
            .encode_partial_input(case)
            .map_err(|error| format!("the case at {}: {}", error.path, error.message))?;
        let envelope = CalculationInputEnvelope {
            futuruna: CalculationEnvelopeMetadata {
                schema: INPUT_SCHEMA.to_string(),
                schema_hash: contract.schema_hash.clone(),
                entry: contract.entry.clone(),
            },
            cases: vec![CalculationInputCase {
                case_id: "query".to_string(),
                input_status: Some(CalculationInputStatus::Ready),
                input: input.clone(),
            }],
        };
        let (output, violations) = if self.query.check {
            prepared.check_partial_violations(&envelope)
        } else {
            (
                prepared.invoke_partial(&envelope, partial::PartialMode::Propagate),
                Vec::new(),
            )
        };
        drop(prepared);
        if let Some(diagnostic) = output.diagnostics.first() {
            return Err(format!("{}: {}", diagnostic.path, diagnostic.message));
        }
        if let Some(violation) = violations.first() {
            return Err(violation_message(violation, &input));
        }
        let result = output
            .results
            .first()
            .ok_or_else(|| "the calculation returned no result".to_string())?;
        let result: CalculationPartialResult = serde_json::from_value(result.result.clone())
            .map_err(|error| format!("unreadable partial result: {error}"))?;
        // Paths as `pathof` writes them: the case names each alternative.
        let typed = |path: &str| {
            contract
                .typed_input_path(&input, path)
                .unwrap_or_else(|| path.to_string())
        };
        Ok(match result {
            CalculationPartialResult::Complete {
                result,
                assumptions,
                unused_paths,
            } => {
                let result = contract
                    .decode_output(&result)
                    .map_err(|error| format!("the result at {}: {}", error.path, error.message))?;
                record(
                    "Query",
                    "Complete",
                    vec![
                        ("result", result),
                        (
                            "unused",
                            Value::List(
                                unused_paths
                                    .iter()
                                    .map(|path| string(&typed(path)))
                                    .collect(),
                            ),
                        ),
                        (
                            "assumptions",
                            Value::List(
                                assumptions
                                    .iter()
                                    .map(|assumption| {
                                        record(
                                            "QueryAssumption",
                                            "QueryAssumption",
                                            vec![
                                                (
                                                    "section",
                                                    reference(
                                                        "ProgramSectionReference",
                                                        &assumption.section,
                                                    ),
                                                ),
                                                (
                                                    "binding",
                                                    optional(assumption.binding.as_deref().map(
                                                        |binding| {
                                                            reference(
                                                                "ProgramSymbolReference",
                                                                binding,
                                                            )
                                                        },
                                                    )),
                                                ),
                                            ],
                                        )
                                    })
                                    .collect(),
                            ),
                        ),
                    ],
                )
            }
            CalculationPartialResult::Needs { needs, .. } => record(
                "Query",
                "Needs",
                vec![(
                    "needs",
                    Value::List(needs.iter().map(|need| need_value(need, &typed)).collect()),
                )],
            ),
        })
    }

    /// Whether a value is a record: the constructor of a type with one
    /// constructor. A `Date` is one fact and is answered whole.
    fn fill_is_record(&self, value: &Value) -> bool {
        let Some((name, fields)) = named_fields(value) else {
            return false;
        };
        let display = constructor_display_name(name);
        if display == "Date"
            && fields.len() == 3
            && fields
                .iter()
                .all(|(field, _)| matches!(field.as_str(), "year" | "month" | "day"))
        {
            return false;
        }
        let state = self.runtime_root.state.borrow();
        let owner = state
            .ctor_to_type
            .get(name)
            .or_else(|| state.ctor_to_type.get(display.as_ref()));
        match owner {
            Some(owner) => state
                .type_variants
                .get(owner)
                .is_none_or(|variants| variants.len() <= 1),
            None => display == name,
        }
    }

    /// `value` with each field unknown.
    fn fill_fields_unknown(&self, value: &Value) -> Value {
        let Some((_, fields)) = named_fields(value) else {
            return value.clone();
        };
        let unknown = partial::unknown(BTreeSet::from([format!("{UNPLACED}fill")]));
        with_fields(
            value,
            fields
                .iter()
                .map(|(name, _)| (name.clone(), unknown.clone()))
                .collect(),
        )
    }

    /// The answer to a need at `steps`, from the complete case.
    fn fill_answer(&self, complete: &Value, steps: &[Step]) -> Result<Value, String> {
        if let [parent @ .., Step::Variant] = steps {
            // A variant is answered by its tag; its fields are asked next.
            let value = fill_get(complete, parent)?;
            let value = some_inner(&value).cloned().unwrap_or(value);
            return Ok(self.fill_fields_unknown(&value));
        }
        let value = fill_get(complete, steps)?;
        Ok(match some_inner(&value) {
            Some(inner) if self.fill_is_record(inner) => some(self.fill_fields_unknown(inner)),
            _ if self.fill_is_record(&value) => self.fill_fields_unknown(&value),
            _ => value,
        })
    }

    /// `partial` with `answer` at `steps`; `complete` gives the shape of a
    /// record the partial case does not have yet.
    fn fill_set(
        &self,
        partial: &Value,
        complete: &Value,
        steps: &[Step],
        answer: &Value,
    ) -> Result<Value, String> {
        let Some((step, rest)) = steps.split_first() else {
            return Ok(answer.clone());
        };
        if matches!(step, Step::Variant) {
            return Ok(answer.clone());
        }
        // An unknown on the way exists in the complete case: as a record
        // with its fields unknown.
        let partial = match partial {
            Value::Unknown(_) => match some_inner(complete) {
                Some(inner) => some(self.fill_fields_unknown(inner)),
                None => self.fill_fields_unknown(complete),
            },
            other => other.clone(),
        };
        if let (Some(inner), Some(complete_inner)) = (some_inner(&partial), some_inner(complete)) {
            return Ok(some(self.fill_set(inner, complete_inner, steps, answer)?));
        }
        // `pathof` names the alternative a payload field is in.
        if let Step::Field(name) = step {
            if is_alternative(&partial, name) {
                return self.fill_set(&partial, complete, rest, answer);
            }
        }
        match step {
            Step::Field(name) => {
                let (_, fields) = named_fields(&partial)
                    .ok_or_else(|| format!("no record holds the field `{name}`"))?;
                let (_, complete_fields) = named_fields(complete)
                    .ok_or_else(|| format!("the complete case has no record with `{name}`"))?;
                let complete_field = complete_fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, value)| value)
                    .ok_or_else(|| format!("the complete case has no field `{name}`"))?;
                let mut updated = Vec::with_capacity(fields.len());
                for (field, value) in fields.iter() {
                    if field == name {
                        updated.push((
                            field.clone(),
                            self.fill_set(value, complete_field, rest, answer)?,
                        ));
                    } else {
                        updated.push((field.clone(), value.clone()));
                    }
                }
                Ok(with_fields(&partial, updated))
            }
            Step::Index(index) => {
                let Some(items) = list_items(&partial) else {
                    return Err(format!("no list holds the element [{index}]"));
                };
                let Some(complete_items) = list_items(complete) else {
                    return Err(format!(
                        "the complete case has no list with [{index}], but {}",
                        self.runtime_value_type_label(complete)
                    ));
                };
                let (Some(item), Some(complete_item)) =
                    (items.get(*index), complete_items.get(*index))
                else {
                    return Err(format!("no element [{index}]"));
                };
                let mut items = items.clone();
                items[*index] = self.fill_set(item, complete_item, rest, answer)?;
                Ok(Value::List(items))
            }
            Step::Variant => unreachable!("handled above"),
        }
    }

    /// `fill(partial, complete, paths)`.
    pub(crate) fn eval_fill(&mut self, partial: Value, complete: Value, paths: Value) -> Value {
        let Value::List(paths) = paths else {
            return self.panic_or_ground_fail("fill takes a list of paths");
        };
        let mut filled = partial;
        for path in &paths {
            let Value::Str(path) = path else {
                return self.panic_or_ground_fail("fill takes paths written with pathof");
            };
            let result = path_steps(path)
                .ok_or_else(|| "not a path".to_string())
                .and_then(|steps| {
                    let answer = self.fill_answer(&complete, &steps)?;
                    self.fill_set(&filled, &complete, &steps, &answer)
                });
            match result {
                Ok(value) => filled = value,
                Err(message) => {
                    return self.panic_or_ground_fail(format!("fill at `{path}`: {message}"))
                }
            }
        }
        filled
    }
}

/// The value at `steps` of a complete case; `Some` is transparent.
fn fill_get(value: &Value, steps: &[Step]) -> Result<Value, String> {
    let mut current = value.clone();
    for step in steps {
        if let Some(inner) = some_inner(&current) {
            current = inner.clone();
        }
        if let Step::Field(name) = step {
            if is_alternative(&current, name) {
                continue;
            }
        }
        current = match step {
            Step::Field(name) => named_fields(&current)
                .and_then(|(_, fields)| fields.iter().find(|(field, _)| field == name))
                .map(|(_, value)| value.clone())
                .ok_or_else(|| format!("the complete case has no field `{name}` here"))?,
            Step::Index(index) => match list_items(&current) {
                Some(items) => items
                    .get(*index)
                    .cloned()
                    .ok_or_else(|| format!("the complete case has no element [{index}]"))?,
                None => return Err(format!("the complete case has no list with [{index}]")),
            },
            Step::Variant => return Ok(current),
        };
        if matches!(current, Value::Unknown(_)) {
            return Err("the complete case holds an unknown here".to_string());
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(members: &[Member]) -> Vec<&'static str> {
        members.iter().map(|member| member.name).collect()
    }

    fn schema_members(schema: &JsonValue) -> (BTreeSet<String>, BTreeSet<String>) {
        let properties = schema["properties"]
            .as_object()
            .expect("object schema")
            .keys()
            .cloned()
            .collect();
        let required = schema["required"]
            .as_array()
            .map(|required| {
                required
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        (properties, required)
    }

    fn assert_members(members: &[Member], schema: &JsonValue) {
        let (properties, required) = schema_members(schema);
        assert_eq!(
            properties,
            members
                .iter()
                .map(|member| member.name.to_string())
                .collect(),
            "members and schema properties"
        );
        assert_eq!(
            required,
            members
                .iter()
                .filter(|member| member.required)
                .map(|member| member.name.to_string())
                .collect(),
            "required members"
        );
    }

    fn contract() -> crate::calculate::CalculationContract {
        let source = "# Case(income: Int)\n# Out(tax: Int)\n@ calculate\n| tax(c: Case) -> Out(tax = c.income)\n";
        let statements = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect("fixture parses");
        crate::calculate::extract_calculation_contracts(&statements, source, None)
            .expect("fixture has a contract")
            .remove(0)
    }

    #[test]
    fn need_members_are_the_partial_result_schema() {
        let schema = crate::calculate::partial_result_json_schema(&contract());
        let need = &schema["oneOf"][1]["properties"]["needs"]["items"];
        assert_members(NEED_MEMBERS, need);
        assert_members(NEED_KEY_MEMBERS, &need["properties"]["keys"]["items"]);
        let waiting = &need["properties"]["waiting"]["items"];
        assert_members(WAITING_MEMBERS, waiting);
        let paragraph = &waiting["properties"]["paragraphs"]["items"];
        assert_members(PARAGRAPH_MEMBERS, paragraph);
        assert_members(ACT_MEMBERS, &paragraph["properties"]["act"]);
        assert_members(AMENDMENT_MEMBERS, &paragraph["properties"]["as_amended_by"]);
    }

    fn full_need() -> CalculationNeed {
        serde_json::from_value(serde_json::json!({
            "kind": "entry",
            "path": "spouse.income",
            "keys": [{ "path": "accounts[0]", "field": "id", "value": "A" }],
            "entry": "share_income",
            "subject": "spouse",
            "label": "Income",
            "question": "What was it?",
            "help": "Help",
            "unit": "øre",
            "sources": [{ "label": "Law", "role": "Source", "binding": "law" }],
            "waiting": [{
                "rule": "tax",
                "section": "pskl_8a",
                "paragraphs": [{
                    "paragraph": "§ 8 a",
                    "source": "law.txt",
                    "act": { "title": "Act", "identifier": "LBK 1", "url": "https://example.invalid" },
                    "as_amended_by": { "act": "LOV 2", "paragraph": "§ 4, nr. 1", "instruction": "1. ..." }
                }]
            }]
        }))
        .expect("a full need")
    }

    #[test]
    fn need_members_are_what_a_need_writes_and_its_value_holds() {
        let need = full_need();
        let written = serde_json::to_value(&need).expect("need is JSON");
        assert_eq!(
            written
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            names(NEED_MEMBERS)
                .into_iter()
                .map(str::to_string)
                .collect()
        );
        let value = need_value(&need, &|path| path.to_string());
        let (_, fields) = named_fields(&value).expect("a record");
        assert_eq!(
            fields
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            names(NEED_MEMBERS)
        );
        let waiting = &fields.iter().find(|(name, _)| name == "waiting").unwrap().1;
        let Value::List(waiting) = waiting else {
            panic!("waiting is a list")
        };
        let (_, waiting_fields) = named_fields(&waiting[0]).unwrap();
        assert_eq!(
            waiting_fields
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            names(WAITING_MEMBERS)
        );
    }

    #[test]
    fn the_needs_prelude_declares_need_from_the_member_table() {
        let source = needs_prelude_source();
        assert!(source.contains(
            "# Need(i) = Need(kind: NeedKind, path: Path(i), keys: List(NeedKey(i)), entry: Option(ProgramReference), subject: Option(Path(i)), label: Option(String), question: Option(String), help: Option(String), unit: Option(String), sources: List(NeedSource), waiting: List(NeedWaiting))"
        ));
        assert!(needs_prelude()
            .iter()
            .any(|statement| matches!(statement, Stmt::Defn(Defn::Fn { name, .. }) if name == "interview")));
    }

    #[test]
    fn paths_split_into_fields_indices_and_variant_tags() {
        let steps = path_steps("accounts[1].ask_ending.$variant").unwrap();
        assert!(matches!(
            steps.as_slice(),
            [
                Step::Field("accounts"),
                Step::Index(1),
                Step::Field("ask_ending"),
                Step::Variant
            ]
        ));
        assert!(path_steps("a[x]").is_none());
    }

    #[test]
    fn a_completion_shows_only_what_it_fills_in() {
        let partial = serde_json::json!({ "income": 5, "spouse": {} });
        let completion =
            serde_json::json!({ "income": 5, "spouse": { "share": 3, "cohabiting": false } });
        assert_eq!(
            completion_text(&partial, &completion),
            "spouse.cohabiting = false, spouse.share = 3"
        );
    }

    /// A model with a calculation entry, for the checks of `unknown` and
    /// `pathof`.
    const MODEL: &str = "\
# Spouse(income: Int, cohabiting: Bool)
# Case(income: Int, spouse: Option(Spouse), incomes: List(Int))
# Result(tax: Int)

@ calculate
| tax(c: Case) -> Result(tax = c.income)
> case_of(income: Int) -> Case { Case(income = income, spouse = None, incomes = []) }
";

    fn program(body: &str) -> (String, Vec<Stmt>) {
        let source = format!("{MODEL}{body}\n");
        let user = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap_or_else(|error| panic!("{body}: {error}"));
        let statements = prepend_prelude(parse_prelude(), &user);
        (source, statements)
    }

    /// The checker's error messages for `body` after the model.
    fn errors(body: &str) -> Vec<String> {
        let (source, statements) = program(body);
        TypeChecker::check_with_artifacts(&statements, None, &source)
            .diagnostics
            .into_iter()
            .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    fn placed(body: &str) {
        let errors = errors(body);
        assert!(errors.is_empty(), "{body}: {errors:#?}");
    }

    fn rejected(body: &str, message: &str) {
        let errors = errors(body);
        assert!(
            errors.iter().any(|error| error.contains(message)),
            "{body}: expected `{message}` in {errors:#?}"
        );
    }

    #[test]
    fn unknown_is_placed_as_an_argument() {
        placed("= asked = query tax(case_of(unknown))");
    }

    #[test]
    fn unknown_is_placed_as_a_field_value() {
        placed("= asked = query tax(Case(income = unknown, spouse = None, incomes = []))");
    }

    #[test]
    fn unknown_is_placed_as_a_list_element() {
        placed("= asked = query tax(Case(income = 1, spouse = None, incomes = [1, unknown]))");
    }

    #[test]
    fn unknown_is_placed_in_a_typed_binding() {
        placed("= later: Int = unknown");
    }

    #[test]
    fn unknown_is_placed_as_a_declared_result() {
        placed("> later() -> Int { unknown }");
    }

    #[test]
    fn unknown_is_placed_as_a_branch() {
        placed("= later: Int = if True { 1 } else { unknown }");
    }

    #[test]
    fn unknown_is_not_an_operand() {
        rejected("= sum: Int = unknown + 1", "`unknown` cannot be an operand");
    }

    #[test]
    fn unknown_is_not_matched() {
        rejected(
            "= matched: Int = match unknown {\n    | _ -> 1\n}",
            "`unknown` cannot be matched",
        );
    }

    #[test]
    fn unknown_is_not_a_guard() {
        rejected(
            "= guarded: Int = match 1 {\n    | n if unknown -> n\n    | _ -> 0\n}",
            "`unknown` cannot be a guard",
        );
    }

    #[test]
    fn unknown_is_not_shown() {
        rejected(
            "= shown = show(unknown)",
            "`unknown` cannot be an argument of a builtin",
        );
    }

    #[test]
    fn unknown_is_not_a_side_of_a_comparison() {
        rejected(
            "= compared = unknown == 3",
            "`unknown` cannot be a side of a comparison",
        );
    }

    #[test]
    fn unknown_needs_a_typed_binding() {
        rejected(
            "= untyped = unknown",
            "`unknown` takes the type of its place, and this place has none",
        );
    }

    /// The value of the binding `name` after running `body`.
    fn value_of(body: &str, name: &str) -> String {
        let errors = errors(body);
        assert!(errors.is_empty(), "{body}: {errors:#?}");
        let (_, statements) = program(body);
        let mut interpreter = Interpreter::new();
        let mut env = interpreter.default_env();
        interpreter.run_program(&statements, &mut env);
        env.get(name)
            .map(ToString::to_string)
            .unwrap_or_else(|| panic!("{body}: no binding `{name}`"))
    }

    #[test]
    fn paths_of_one_root_are_equal_when_they_name_the_same_place() {
        assert_eq!(
            value_of(
                "= same = pathof(Case::spouse::income) == pathof(Case::spouse::income)",
                "same"
            ),
            "true"
        );
        assert_eq!(
            value_of(
                "= indexed = pathof(Case::incomes[1]) == pathof(Case::incomes[1])",
                "indexed"
            ),
            "true"
        );
    }

    #[test]
    fn paths_of_one_root_differ_when_they_name_different_places() {
        assert_eq!(
            value_of(
                "= other = pathof(Case::income) == pathof(Case::spouse::income)",
                "other"
            ),
            "false"
        );
        assert_eq!(
            value_of(
                "= index = pathof(Case::incomes[0]) == pathof(Case::incomes[1])",
                "index"
            ),
            "false"
        );
    }

    #[test]
    fn paths_of_different_roots_are_not_compared() {
        rejected(
            "= mixed = pathof(Case::income) == pathof(Spouse::income)",
            "Path(Spouse)",
        );
    }
}
