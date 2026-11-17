//! Answering an entry's needs by a guide (`docs/rfcs/answer-guide.md`).
//!
//! An `AnswerGuide` maps input paths to the integrator's groups of
//! resolvers; every path it does not name is required, so a need there is a
//! defect in the integrator's data. `answer(entry, case, guide, resolve)`
//! queries the entry round by round, as `query entry(case)` does, calls each
//! group's resolver once per round with all of that group's needs, and writes
//! the answers with `fill`.
//!
//! A guide is a top-level binding of a ground `AnswerGuide(fields = [...])`,
//! so `runa check`, `runa call --guide` and `runa sdk ts --guide` read it
//! without running the program.

use super::*;
use crate::query::{
    calculation_entries, display_location, list_items, named_fields, program_files, record, string,
    ProgramFile,
};

/// The answer prelude: the guide, the resolver's reply, the rounds and the
/// outcome, `from_case`, `whole_case_guide` and the round loop of `answer`.
/// It joins every program that builds an `AnswerGuide` or calls `answer`.
const ANSWER_PRELUDE: &str = r#"-- Answering an entry's needs by a guide (docs/rfcs/answer-guide.md).
# AnswerGuide(i, g) = AnswerGuide(fields: List(EntryField(i, g)))
# EntryField(i, g) = EntryField(path: Path(i), by: g)
# Answers(i) = Answers(from: i, paths: List(Path(i)))
# Asking(i, g) = Asking(group: g, paths: List(Path(i)))
# Round(i, g) = Round(paths: List(Path(i)), groups: List(Asking(i, g)))
# Outcome(o, i, g) = Answered(result: o) | Unanswered(open: List(Asking(i, g)), case: i)
# Answering(o, i, g) = Answering(rounds: List(Round(i, g)), outcome: Outcome(o, i, g))
# WholeCase = WholeCase
# AnswerStep(i, g) = AnswerStep(groups: List(Asking(i, g)), case: i, answered: Bool)

-- A resolver that answers every need from `complete`.
> from_case(complete: i) -> (g, List(Need(i)), i) -> Answers(i) {
    |group, needs, case| Answers(from = complete, paths = map(needs, |n| n.path))
}

-- A guide whose one group, `WholeCase`, resolves the whole input.
> whole_case_guide() -> AnswerGuide(i, WholeCase) {
    AnswerGuide(fields = [EntryField(path = __whole_input(), by = WholeCase)])
}

-- `answer(entry, case, guide, resolve)`: the parser passes the query of the
-- entry and the form's site.
> answer(ask: i -> Query(o, i), case: i, guide: AnswerGuide(i, g), resolve: (g, List(Need(i)), i) -> Answers(i), site: String) -> Answering(o, i, g) {
    __answer_from(ask, case, guide, resolve, site, [])
}

> __answer_from(ask: i -> Query(o, i), case: i, guide: AnswerGuide(i, g), resolve: (g, List(Need(i)), i) -> Answers(i), site: String, rounds: List(Round(i, g))) -> Answering(o, i, g) {
    = q = ask(case)
    match q {
        | Complete -> Answering(rounds = rounds, outcome = Answered(result = q.result))
        | Needs -> {
            = step = __answer_step(case, q.needs, guide, resolve, site, length(rounds) + 1)
            = asked = concat(rounds, [Round(paths = map(q.needs, |n| n.path), groups = step.groups)])
            if step.answered {
                __answer_from(ask, step.case, guide, resolve, site, asked)
            } else {
                Answering(rounds = asked, outcome = Unanswered(open = step.groups, case = step.case))
            }
        }
    }
}

-- One round: each group's resolver with all of that group's needs.
> __answer_step(case: i, needs: List(Need(i)), guide: AnswerGuide(i, g), resolve: (g, List(Need(i)), i) -> Answers(i), site: String, round: Int) -> AnswerStep(i, g) {
    __answer_round(case, needs, guide, resolve, site, round)
}
"#;

pub fn answer_prelude_source() -> &'static str {
    ANSWER_PRELUDE
}

/// The declarations of [`answer_prelude_source`].
pub fn answer_prelude() -> Vec<Stmt> {
    let tokens = Lexer::new(ANSWER_PRELUDE).tokenize();
    match Parser::new(tokens, ANSWER_PRELUDE).parse_program() {
        Ok(statements) => statements
            .into_iter()
            .filter(|statement| matches!(statement, Stmt::Defn(_) | Stmt::TypeDecl(_)))
            .collect(),
        Err(error) => {
            eprintln!("BUG: answer prelude parse error: {error}");
            Vec::new()
        }
    }
}

/// Names whose use brings the answer prelude, unless the program declares
/// them itself.
const PRELUDE_NAMES: [&str; 4] = ["AnswerGuide", "EntryField", "from_case", "whole_case_guide"];

/// Whether a program calls `answer` or builds a guide.
pub fn program_uses_answer(statements: &[Stmt]) -> bool {
    let user = statements
        .iter()
        .position(|statement| matches!(statement, Stmt::PreludeBoundary))
        .map_or(statements, |boundary| &statements[boundary + 1..]);
    let declared: BTreeSet<&str> = user
        .iter()
        .filter_map(|statement| match statement {
            Stmt::Defn(Defn::Fn { name, .. }) | Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                Some(name.as_str())
            }
            _ => None,
        })
        .collect();
    let mut uses = false;
    for statement in user {
        walk_ast_stmt(statement, &mut |child| {
            if uses {
                return;
            }
            if let AstChild::Expr(expr) = child {
                uses = answer_parts(expr).is_some()
                    || matches!(&expr.kind, ExprKind::Var(name)
                        if PRELUDE_NAMES.contains(&name.as_str()) && !declared.contains(name.as_str()));
            }
        });
        if uses {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Guides, read without running the program
// ---------------------------------------------------------------------------

/// One item of a guide: a path of the input (`""` for the whole input, list
/// elements without an index) and the group that resolves it, by its bare
/// constructor name, with the owner written as `Type.Group`, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideItem {
    pub path: String,
    pub group: String,
    pub owner: Option<String>,
    pub span: Span,
}

/// A guide: its binding's name, the input type its paths are rooted at, its
/// items in the order written, and the group type its binding declares
/// (`AnswerGuide(Input, Group)`), if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guide {
    pub name: String,
    pub root: String,
    pub items: Vec<GuideItem>,
    pub group_type: Option<String>,
    pub span: Span,
}

impl Guide {
    /// The item that resolves `path`, a need's path or an input path: the
    /// most specific item at or above it. `None` when the path is required.
    pub fn item_for(&self, path: &str) -> Option<&GuideItem> {
        item_for(&self.items, path)
    }

    /// `pathof(...)` of an item's path, as the guide writes it.
    pub fn written(&self, path: &str) -> String {
        written_path(&self.root, path)
    }
}

/// `pathof(T::a::b)` for the path `a.b` of `T`, `pathof(T)` for the whole
/// input.
pub fn written_path(root: &str, path: &str) -> String {
    if path.is_empty() {
        format!("pathof({root})")
    } else {
        format!("pathof({root}::{})", path.replace('.', "::"))
    }
}

/// `path` without its list indices: `events[2].Trade.amount` is
/// `events.Trade.amount`.
pub fn element_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut depth = 0usize;
    for character in path.chars() {
        match character {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => result.push(character),
            _ => {}
        }
    }
    result
}

/// Whether the item path `above` covers `path`: it is the path or above it.
fn covers(above: &str, path: &str) -> bool {
    above.is_empty()
        || path == above
        || path
            .strip_prefix(above)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn item_for<'g>(items: &'g [GuideItem], path: &str) -> Option<&'g GuideItem> {
    let path = element_path(path);
    items
        .iter()
        .filter(|item| covers(&item.path, &path))
        .max_by_key(|item| item.path.len())
}

/// Whether a top-level statement binds a guide: its value is
/// `AnswerGuide(...)` or its declared type is an `AnswerGuide`.
pub fn guide_binding(statement: &Stmt) -> Option<(&str, &Expr)> {
    let Stmt::Bind(Pat::Var(name), ty, value) = statement else {
        return None;
    };
    let typed = match ty {
        Some(Ty::App(head, _)) => matches!(head.as_ref(), Ty::Name(head) if head == "AnswerGuide"),
        Some(Ty::Name(head)) => head == "AnswerGuide",
        _ => false,
    };
    let built = matches!(&value.kind, ExprKind::App(function, _)
        if matches!(&function.kind, ExprKind::Var(name) if name == "AnswerGuide"));
    (typed || built).then_some((name.as_str(), value))
}

/// The group type a guide binding declares: `g` of `AnswerGuide(i, g)`.
fn declared_group_type(statement: &Stmt) -> Option<String> {
    let Stmt::Bind(_, Some(Ty::App(head, arguments)), _) = statement else {
        return None;
    };
    match (head.as_ref(), arguments.as_slice()) {
        (Ty::Name(head), [_, Ty::Name(group)]) if head == "AnswerGuide" => Some(group.clone()),
        _ => None,
    }
}

/// The guide a top-level statement binds, read or with its problems.
fn read_statement_guide(statement: &Stmt) -> Option<Result<Guide, Vec<(Span, String)>>> {
    let (name, value) = guide_binding(statement)?;
    Some(read_guide(name, value).map(|mut guide| {
        guide.group_type = declared_group_type(statement);
        guide
    }))
}

/// Every guide a file binds at top level, each read or with its problems.
pub fn read_guides(statements: &[Stmt]) -> Vec<Result<Guide, Vec<(Span, String)>>> {
    statements.iter().filter_map(read_statement_guide).collect()
}

/// The guide `name` binds to `value`, or why it is not a ground guide.
pub fn read_guide(name: &str, value: &Expr) -> Result<Guide, Vec<(Span, String)>> {
    let ground = || {
        vec![(
            value.span,
            format!("the AnswerGuide `{name}` is written as `AnswerGuide(fields = [EntryField(path = pathof(...), by = Group), ...])`: a ground value, so that `runa check` reads its paths and groups without running the program"),
        )]
    };
    let ExprKind::App(function, arguments) = &value.kind else {
        return Err(ground());
    };
    if !matches!(&function.kind, ExprKind::Var(constructor) if constructor == "AnswerGuide") {
        return Err(ground());
    }
    let fields = match arguments.as_slice() {
        [argument] => match named_arg_parts(argument) {
            Some(("fields", fields)) => fields,
            Some(_) => return Err(ground()),
            None => argument,
        },
        _ => return Err(ground()),
    };
    let ExprKind::List(items) = &fields.kind else {
        return Err(ground());
    };
    let mut problems = Vec::new();
    let mut read = Vec::new();
    let mut roots: Vec<(String, Span)> = Vec::new();
    for item in items {
        match read_item(name, item) {
            Ok((root, item)) => {
                if !roots.iter().any(|(known, _)| *known == root) {
                    roots.push((root, item.span));
                }
                read.push(item);
            }
            Err(problem) => problems.push(problem),
        }
    }
    if roots.len() > 1 {
        problems.push((
            roots[1].1,
            format!(
                "the AnswerGuide `{name}` names paths into {}: a guide's paths are into one input type, the entry's",
                roots
                    .iter()
                    .map(|(root, _)| format!("`{root}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let Some((root, _)) = roots.into_iter().next() else {
        return Err(vec![(
            value.span,
            format!("the AnswerGuide `{name}` names no path; a guide that resolves nothing holds every path, which no guide says: name at least one"),
        )]);
    };
    Ok(Guide {
        name: name.to_string(),
        root,
        items: read,
        group_type: None,
        span: value.span,
    })
}

/// One `EntryField(path = pathof(...), by = Group)`: its root and the item.
fn read_item(guide: &str, item: &Expr) -> Result<(String, GuideItem), (Span, String)> {
    let shape = || {
        (
            item.span,
            format!("an item of the AnswerGuide `{guide}` is `EntryField(path = pathof(...), by = Group)`"),
        )
    };
    let ExprKind::App(function, arguments) = &item.kind else {
        return Err(shape());
    };
    if !matches!(&function.kind, ExprKind::Var(constructor) if constructor == "EntryField") {
        return Err(shape());
    }
    let (path, by) = match arguments.as_slice() {
        [first, second] => match (named_arg_parts(first), named_arg_parts(second)) {
            (Some(("path", path)), Some(("by", by))) | (Some(("by", by)), Some(("path", path))) => {
                (path, by)
            }
            (None, None) => (first, second),
            _ => return Err(shape()),
        },
        _ => return Err(shape()),
    };
    let Some(root) = pathof_root(path) else {
        return Err((
            path.span,
            format!("the path of an item of the AnswerGuide `{guide}` is a `pathof(...)` path of the entry's input"),
        ));
    };
    if let ExprKind::App(_, segments) = &path.kind {
        for segment in segments.iter().skip(1) {
            if let ExprKind::Lit(Literal::Str(text)) = &segment.kind {
                if text.starts_with('[') {
                    return Err((
                        segment.span,
                        format!("the AnswerGuide `{guide}` names the element `{text}`: who answers does not depend on which element, so a guide names no index; name the list, or a field of its elements"),
                    ));
                }
            }
        }
    }
    let canonical = pathof_canonical_path(path).ok_or_else(shape)?;
    let upper = |name: &str| name.starts_with(char::is_uppercase);
    let (group, owner) = match &by.kind {
        ExprKind::Var(group) if upper(group) => (group.clone(), None),
        ExprKind::Field(base, group) if upper(group) => match &base.kind {
            ExprKind::Var(owner) if upper(owner) => (group.clone(), Some(owner.clone())),
            _ => return Err(not_a_group(guide, by)),
        },
        _ => return Err(not_a_group(guide, by)),
    };
    Ok((
        root.to_string(),
        GuideItem {
            path: canonical,
            group,
            owner,
            span: item.span,
        },
    ))
}

fn not_a_group(guide: &str, by: &Expr) -> (Span, String) {
    (
        by.span,
        format!("`by` in the AnswerGuide `{guide}` names a group: a constructor without fields of the integrator's group type, written `Group` or `GroupType.Group`"),
    )
}

/// The type that owns an item's group, as the type checker resolves the
/// constructor: `Type.Group` names it; a bare name declared by one type is
/// that type's, and one shared by several is the guide's declared group
/// type's. `None` when it does not resolve; the type checker reports why
/// (an unknown constructor, or a bare shared name with nothing to type it).
fn group_owner(guide: &Guide, item: &GuideItem, checker: &TypeChecker) -> Option<String> {
    let owners = checker.constructor_owners(&item.group);
    let chosen = match (&item.owner, owners.as_slice()) {
        (Some(owner), _) => checker.constructor_identity_owner(owner, &item.group),
        (None, [owner]) => owner.clone(),
        (None, _) => guide.group_type.clone()?,
    };
    owners.into_iter().find(|owner| *owner == chosen)
}

/// The number of fields `owner`'s constructor `constructor` has.
fn constructor_arity(checker: &TypeChecker, owner: &str, constructor: &str) -> usize {
    checker
        .constructor_signatures
        .get(constructor)
        .into_iter()
        .flatten()
        .find(|signature| {
            checker.constructor_identity_owner(&signature.parent, constructor) == owner
        })
        .map_or(0, |signature| signature.fields.len())
}

// ---------------------------------------------------------------------------
// Need-able paths
// ---------------------------------------------------------------------------

/// The data types a program declares: parameters and variants.
pub(crate) struct Declarations<'a> {
    types: BTreeMap<&'a str, (&'a [Param], &'a [Variant])>,
    /// Units and named scalars: one fact each.
    scalars: BTreeSet<&'a str>,
}

impl<'a> Declarations<'a> {
    pub(crate) fn of<'s>(files: impl IntoIterator<Item = &'a [Stmt]>) -> Self {
        let mut types = BTreeMap::new();
        let mut scalars = BTreeSet::new();
        for statements in files {
            for statement in statements {
                match statement {
                    Stmt::TypeDecl(TypeDecl::ADT {
                        name,
                        params,
                        variants,
                        ..
                    }) => {
                        types
                            .entry(name.as_str())
                            .or_insert((params.as_slice(), variants.as_slice()));
                    }
                    Stmt::TypeDecl(TypeDecl::Unit { name, .. })
                    | Stmt::TypeDecl(TypeDecl::Scalar { name, .. }) => {
                        scalars.insert(name.as_str());
                    }
                    _ => {}
                }
            }
        }
        Self { types, scalars }
    }

    /// The paths of `root` at which a need can arise, without list indices,
    /// as the table of the RFC's "Which paths can become a need" says. A
    /// type that repeats on a path is not entered again.
    pub(crate) fn needable(&self, root: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.value(
            &Ty::Name(root.to_string()),
            "",
            &BTreeMap::new(),
            &mut Vec::new(),
            &mut out,
        );
        out
    }

    fn substitute(ty: &Ty, substitution: &BTreeMap<String, Ty>) -> Ty {
        match ty {
            Ty::Name(name) | Ty::Var(name) => substitution
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            Ty::App(head, arguments) => Ty::App(
                Box::new(Self::substitute(head, substitution)),
                arguments
                    .iter()
                    .map(|argument| Self::substitute(argument, substitution))
                    .collect(),
            ),
            Ty::Optional(inner) => Ty::Optional(Box::new(Self::substitute(inner, substitution))),
            other => other.clone(),
        }
    }

    /// A value at `path`.
    fn value(
        &self,
        ty: &Ty,
        path: &str,
        substitution: &BTreeMap<String, Ty>,
        stack: &mut Vec<String>,
        out: &mut Vec<String>,
    ) {
        let ty = Self::substitute(ty, substitution);
        match &ty {
            Ty::Optional(inner) => {
                out.push(path.to_string());
                self.below(inner, path, stack, out);
            }
            Ty::App(head, arguments) => match (head.as_ref(), arguments.as_slice()) {
                (Ty::Name(head), [inner]) if head == "Option" => {
                    out.push(path.to_string());
                    self.below(inner, path, stack, out);
                }
                (Ty::Name(head), [item]) if head == "List" || head == "Set" => {
                    out.push(path.to_string());
                    self.element(item, path, stack, out);
                }
                (Ty::Name(head), [_, value]) if head == "Map" => {
                    out.push(path.to_string());
                    self.element(value, path, stack, out);
                }
                (Ty::Name(name), arguments) => self.named(name, arguments, path, false, stack, out),
                _ => out.push(path.to_string()),
            },
            Ty::Name(name) => self.named(name, &[], path, false, stack, out),
            _ => out.push(path.to_string()),
        }
    }

    /// What is below an optional value at `path`: a record's fields, a sum's
    /// tag and fields; nothing more for a single fact.
    fn below(&self, ty: &Ty, path: &str, stack: &mut Vec<String>, out: &mut Vec<String>) {
        let mut inner = Vec::new();
        self.value(ty, path, &BTreeMap::new(), stack, &mut inner);
        out.extend(inner.into_iter().filter(|candidate| candidate != path));
    }

    /// An element of a collection at `path`.
    fn element(&self, ty: &Ty, path: &str, stack: &mut Vec<String>, out: &mut Vec<String>) {
        let (name, arguments): (&str, &[Ty]) = match ty {
            Ty::Name(name) => (name, &[]),
            Ty::App(head, arguments) => match head.as_ref() {
                Ty::Name(name) => (name, arguments),
                _ => return,
            },
            Ty::Optional(inner) => return self.element(inner, path, stack, out),
            _ => return,
        };
        if self.types.contains_key(name) {
            self.named(name, arguments, path, true, stack, out);
        }
    }

    fn named(
        &self,
        name: &str,
        arguments: &[Ty],
        path: &str,
        element: bool,
        stack: &mut Vec<String>,
        out: &mut Vec<String>,
    ) {
        let Some((parameters, variants)) = self.types.get(name).copied() else {
            // A primitive, a unit, a named scalar: one fact.
            if !element || self.scalars.contains(name) {
                if !element {
                    out.push(path.to_string());
                }
            }
            return;
        };
        if name == "Date" || stack.iter().any(|seen| seen == name) {
            if !element {
                out.push(path.to_string());
            }
            return;
        }
        let substitution: BTreeMap<String, Ty> = parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .zip(arguments.iter().cloned())
            .collect();
        let join = |base: &str, field: &str| {
            if base.is_empty() {
                field.to_string()
            } else {
                format!("{base}.{field}")
            }
        };
        stack.push(name.to_string());
        if variants.len() == 1 && !variants[0].fields.is_empty() {
            for field in variants[0]
                .fields
                .iter()
                .filter(|_| !variants[0].positional)
            {
                self.value(
                    &field.ty,
                    &join(path, &field.name),
                    &substitution,
                    stack,
                    out,
                );
            }
        } else if variants.iter().all(|variant| variant.fields.is_empty()) {
            if element {
                out.push(join(path, "$variant"));
            } else {
                out.push(path.to_string());
            }
        } else {
            out.push(join(path, "$variant"));
            for variant in variants.iter().filter(|variant| !variant.positional) {
                let base = join(path, &variant.name);
                for field in &variant.fields {
                    self.value(
                        &field.ty,
                        &join(&base, &field.name),
                        &substitution,
                        stack,
                        out,
                    );
                }
            }
        }
        stack.pop();
    }
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// The checks of the guides a program binds and of the `answer`s it asks:
/// paths, groups, where a guide lives, and what an `answer` names.
pub(crate) fn check_program(
    statements: &[Stmt],
    source: &str,
    checker: &TypeChecker,
) -> Vec<Diagnostic> {
    let files = program_files(statements, source, checker);
    let mentions = files.iter().any(|file| {
        file.statements.iter().any(|statement| {
            if guide_binding(statement).is_some() {
                return true;
            }
            let mut found = false;
            walk_ast_stmt(statement, &mut |child| {
                if let AstChild::Expr(expr) = child {
                    found |= answer_parts(expr).is_some();
                }
            });
            found
        })
    });
    if !mentions {
        return Vec::new();
    }
    let declarations = Declarations::of(files.iter().map(|file| file.statements));
    let mut diagnostics = Vec::new();
    let mut guides: BTreeMap<&str, Guide> = BTreeMap::new();
    for file in &files {
        for statement in file.statements {
            let Some((name, _)) = guide_binding(statement) else {
                continue;
            };
            let Some(read) = read_statement_guide(statement) else {
                continue;
            };
            match read {
                Ok(guide) => {
                    check_guide(file, &guide, checker, &declarations, &mut diagnostics);
                    guides.insert(name, guide);
                }
                Err(problems) => diagnostics.extend(
                    problems
                        .into_iter()
                        .map(|(span, message)| file.diagnostic(span, message)),
                ),
            }
        }
    }
    diagnostics.extend(guides_in_law(&files, checker));
    for file in &files {
        for statement in file.statements {
            walk_ast_stmt(statement, &mut |child| {
                let AstChild::Expr(expr) = child else {
                    return;
                };
                if let Some(parts) = answer_parts(expr) {
                    check_answer(file, &files, &parts, &guides, &mut diagnostics);
                }
            });
        }
    }
    diagnostics
}

fn check_guide(
    file: &ProgramFile,
    guide: &Guide,
    checker: &TypeChecker,
    declarations: &Declarations,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = &guide.name;
    // One group per path.
    let mut seen: BTreeMap<&str, &GuideItem> = BTreeMap::new();
    for item in &guide.items {
        if let Some(first) = seen.insert(item.path.as_str(), item) {
            diagnostics.push(file.diagnostic(
                item.span,
                format!(
                    "the AnswerGuide `{name}` names `{}` twice (`{}` and `{}`); a path has one group",
                    guide.written(&item.path),
                    first.group,
                    item.group
                ),
            ));
        }
    }
    // The groups: the constructors without fields of one plain sum type,
    // each resolved as the type checker resolves a constructor.
    let mut group_type: Option<(String, Span)> = None;
    let mut plain = true;
    for item in &guide.items {
        let Some(owner) = group_owner(guide, item, checker) else {
            continue;
        };
        let arity = constructor_arity(checker, &owner, &item.group);
        if arity > 0 {
            diagnostics.push(file.diagnostic(
                item.span,
                format!(
                    "`by = {}` in the AnswerGuide `{name}`: a group is a constructor without fields, and `{}` has {arity}",
                    item.group, item.group
                ),
            ));
            plain = false;
            continue;
        }
        match &group_type {
            None => group_type = Some((owner, item.span)),
            Some((first, _)) if *first != owner => {
                diagnostics.push(file.diagnostic(
                    item.span,
                    format!(
                        "the AnswerGuide `{name}` names groups of two types, `{first}` and `{owner}`: its groups are the constructors of one type"
                    ),
                ));
                plain = false;
            }
            Some(_) => {}
        }
    }
    if let (Some((owner, span)), true) = (group_type, plain) {
        let owner = owner.as_str();
        let constructors = checker
            .type_variants
            .get(owner)
            .cloned()
            .unwrap_or_default();
        let with_fields: Vec<&String> = constructors
            .iter()
            .filter(|constructor| constructor_arity(checker, owner, constructor) > 0)
            .collect();
        if let Some(constructor) = with_fields.first() {
            diagnostics.push(file.diagnostic(
                span,
                format!(
                    "the group type `{owner}` of the AnswerGuide `{name}` is not a plain sum of constructors: `{constructor}` has fields"
                ),
            ));
        } else {
            for constructor in &constructors {
                if !guide.items.iter().any(|item| item.group == *constructor) {
                    diagnostics.push(file.diagnostic(
                        guide.span,
                        format!(
                            "the AnswerGuide `{name}` names no path for `{constructor}` of `{owner}`: each group needs a resolver, and one without paths would resolve nothing; name its paths or remove `{constructor}`"
                        ),
                    ));
                }
            }
        }
    }
    // An item every one of whose paths is named more specifically.
    let needable = declarations.needable(&guide.root);
    for item in &guide.items {
        let resolves = needable.iter().any(|path| {
            covers(&item.path, path)
                && guide
                    .item_for(path)
                    .is_some_and(|owner| std::ptr::eq(owner, item))
        });
        if !resolves {
            let mut warning = file.diagnostic(
                item.span,
                format!(
                    "the item `{}` of the AnswerGuide `{name}` resolves nothing: every path below it that can become a need is named more specifically",
                    guide.written(&item.path)
                ),
            );
            warning.severity = Severity::Warning;
            diagnostics.push(warning);
        }
    }
}

/// The file key of a program file: its canonical path, or `None` for the
/// root.
fn file_key(file: &ProgramFile) -> Option<PathBuf> {
    file.path
        .map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)))
}

/// A guide in a file that declares an entry or that such a file imports.
fn guides_in_law(files: &[ProgramFile], checker: &TypeChecker) -> Vec<Diagnostic> {
    let keys: Vec<Option<PathBuf>> = files.iter().map(file_key).collect();
    let index_of = |path: &Path| {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        keys.iter()
            .position(|key| key.as_deref() == Some(canonical.as_path()))
    };
    let imports: Vec<Vec<usize>> = files
        .iter()
        .map(|file| {
            let directory = match file.path {
                Some(path) => Path::new(path)
                    .parent()
                    .map(|parent| parent.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string()),
                None => checker
                    .source_dir
                    .clone()
                    .unwrap_or_else(|| ".".to_string()),
            };
            file.statements
                .iter()
                .filter_map(|statement| match statement {
                    Stmt::Import(path) | Stmt::QualifiedImport(_, path) => {
                        TypeChecker::resolve_tc_import(path, &directory)
                    }
                    _ => None,
                })
                .filter_map(|path| index_of(Path::new(&path)))
                .collect()
        })
        .collect();
    let mut diagnostics = Vec::new();
    let mut reported = BTreeSet::new();
    for (index, file) in files.iter().enumerate() {
        let entries = calculation_entries(file.statements);
        let Some(entry) = entries.first() else {
            continue;
        };
        let mut reached = BTreeSet::from([index]);
        let mut queue = vec![index];
        while let Some(current) = queue.pop() {
            for next in &imports[current] {
                if reached.insert(*next) {
                    queue.push(*next);
                }
            }
        }
        for reached in reached {
            let law = &files[reached];
            for statement in law.statements {
                let Some((name, value)) = guide_binding(statement) else {
                    continue;
                };
                if !reported.insert((reached, value.span.start)) {
                    continue;
                }
                let place = match (reached == index, file.path) {
                    (true, _) => "declares it".to_string(),
                    (false, Some(path)) => format!("in {path} imports it"),
                    (false, None) => "in this file imports it".to_string(),
                };
                diagnostics.push(law.diagnostic(
                    value.span,
                    format!(
                        "the AnswerGuide `{name}` is in the law: the entry `{entry}` {place}. A guide is the integrator's: put it in a file of its own that imports the law (by convention `*.guide.runa`)"
                    ),
                ));
            }
        }
    }
    diagnostics
}

/// The input type of the entry `entry`, as its declaration writes it.
fn entry_input_type(files: &[ProgramFile], entry: &str) -> Option<String> {
    for file in files {
        for statement in file.statements {
            match statement {
                Stmt::Defn(Defn::Fn { name, params, .. }) if name == entry => {
                    return params
                        .first()
                        .and_then(|param| param.ty.as_ref())
                        .map(Ty::to_string);
                }
                Stmt::Rule(rule) => {
                    let Some((name, arguments)) = crate::calculate::rule_head(rule) else {
                        continue;
                    };
                    if name != entry {
                        continue;
                    }
                    if let Some((_, ty)) = arguments
                        .first()
                        .and_then(crate::calculate::typed_rule_argument)
                    {
                        return Some(ty.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn check_answer(
    file: &ProgramFile,
    files: &[ProgramFile],
    parts: &AnswerParts,
    guides: &BTreeMap<&str, Guide>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let whole = matches!(&parts.guide.kind, ExprKind::App(function, values)
        if values.is_empty() && matches!(&function.kind, ExprKind::Var(name) if name == "whole_case_guide"));
    if whole {
        return;
    }
    let guide = match &parts.guide.kind {
        ExprKind::Var(name) => guides.get(name.as_str()),
        _ => None,
    };
    let Some(guide) = guide else {
        let named = match &parts.guide.kind {
            ExprKind::Var(name) => format!("; `{name}` is not one"),
            _ => String::new(),
        };
        diagnostics.push(file.diagnostic(
            parts.guide.span,
            format!(
                "the guide of `answer({}, …)` is a top-level binding of an `AnswerGuide(...)`, or `whole_case_guide()`{named}",
                parts.entry
            ),
        ));
        return;
    };
    if let Some(input) = entry_input_type(files, parts.entry) {
        if input != guide.root {
            diagnostics.push(file.diagnostic(
                parts.guide.span,
                format!(
                    "the AnswerGuide `{}` names paths into `{}`, but `{}` takes a `{input}`",
                    guide.name, guide.root, parts.entry
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Running `answer`
// ---------------------------------------------------------------------------

/// The value of the field `name` of a record value.
fn field<'v>(value: &'v Value, name: &str) -> Option<&'v Value> {
    named_fields(value)?
        .1
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
}

/// A constructor's name as written, for messages.
fn constructor_name(value: &Value) -> String {
    match value {
        Value::Constructor(name, _) | Value::NamedConstructor(name, _) => {
            constructor_display_name(name).into_owned()
        }
        Value::NamespacedConstructor { name, .. }
        | Value::NamespacedNamedConstructor { name, .. } => {
            constructor_display_name(name).into_owned()
        }
        other => other.to_string(),
    }
}

/// One group's needs in a round.
struct Batch {
    group: Value,
    needs: Vec<Value>,
    paths: Vec<String>,
}

impl Interpreter {
    /// `__whole_input()`: the path of the whole input.
    pub(crate) fn eval_whole_input(&self) -> Value {
        Value::Str(String::new())
    }

    /// `__answer_round(case, needs, guide, resolve, site, round)`: one round
    /// of `answer`. Each need goes to the group of the guide's most specific
    /// item at or above its path; a need at a required path is an error. Each
    /// group's resolver is called once, with all of its needs and the
    /// round's case, and its answers are written with `fill`.
    pub(crate) fn eval_answer_round(
        &mut self,
        namespace: &RuntimeNamespace,
        args: Vec<Value>,
        env: &Env,
    ) -> Value {
        let Ok([case, needs, guide, resolve, site, round]) = <[Value; 6]>::try_from(args) else {
            return self
                .panic_or_ground_fail("`answer` takes an entry, a case, a guide and a resolver");
        };
        let (Value::Str(site), Value::Int(round)) = (&site, &round) else {
            return self
                .panic_or_ground_fail("`answer` is written `answer(entry, case, guide, resolve)`");
        };
        let mut parts = site.split('\u{1f}');
        let entry = parts.next().unwrap_or_default().to_string();
        let guide_name = parts.next().unwrap_or_default().to_string();
        let offset = parts.next().and_then(|offset| offset.parse::<usize>().ok());
        let at = self
            .query_current_file()
            .map(|file| {
                let line = offset.and_then(|offset| self.query.line(&file, offset));
                format!("{}: ", display_location(&file, line))
            })
            .unwrap_or_default();
        let prefix = format!("{at}`answer({entry}, …)`, round {round}");
        let items: Vec<(String, Value)> = field(&guide, "fields")
            .and_then(list_items)
            .unwrap_or_default()
            .iter()
            .filter_map(|item| {
                let Some(Value::Str(path)) = field(item, "path") else {
                    return None;
                };
                Some((path.clone(), field(item, "by")?.clone()))
            })
            .collect();
        let guide_items: Vec<GuideItem> = items
            .iter()
            .map(|(path, group)| GuideItem {
                path: path.clone(),
                group: group.to_string(),
                owner: None,
                span: Span::dummy(),
            })
            .collect();
        let mut batches: Vec<Batch> = Vec::new();
        for need in list_items(&needs).unwrap_or_default() {
            let Some(Value::Str(path)) = field(&need, "path") else {
                return self.panic_or_ground_fail(format!("{prefix}: a need without a path"));
            };
            let Some(index) = item_for(&guide_items, path).and_then(|item| {
                guide_items
                    .iter()
                    .position(|other| std::ptr::eq(other, item))
            }) else {
                return self.panic_or_ground_fail(format!(
                    "{prefix}: `{path}` is required (the AnswerGuide `{guide_name}` names no group for it), but the case leaves it unknown"
                ));
            };
            let group = &items[index].1;
            let key = group.to_string();
            let path = path.clone();
            match batches
                .iter_mut()
                .find(|batch| batch.group.to_string() == key)
            {
                Some(batch) => {
                    batch.needs.push(need);
                    batch.paths.push(path);
                }
                None => batches.push(Batch {
                    group: group.clone(),
                    needs: vec![need],
                    paths: vec![path],
                }),
            }
        }
        let mut filled = case.clone();
        let mut answered = false;
        let mut askings = Vec::with_capacity(batches.len());
        for batch in batches {
            let reply = self.apply(
                resolve.clone(),
                vec![batch.group.clone(), Value::List(batch.needs), case.clone()],
                env,
            );
            if self.calculation_failed() {
                return Value::Unit;
            }
            let group = constructor_name(&batch.group);
            let (Some(from), Some(paths)) = (
                field(&reply, "from").cloned(),
                field(&reply, "paths").and_then(list_items),
            ) else {
                return self.panic_or_ground_fail(format!(
                    "{prefix}: the resolver for `{group}` returns no `Answers`"
                ));
            };
            for path in &paths {
                let Value::Str(path) = path else {
                    return self.panic_or_ground_fail(format!(
                        "{prefix}: the resolver for `{group}` answers a path that is not a `Path`"
                    ));
                };
                if !batch.paths.contains(path) {
                    return self.panic_or_ground_fail(format!(
                        "{prefix}: the resolver for `{group}` answers `{path}`, which it was not asked; a resolver answers only the needs it is given"
                    ));
                }
            }
            if !paths.is_empty() {
                answered = true;
                filled = self.eval_fill(filled, from, Value::List(paths));
                if self.calculation_failed() {
                    return Value::Unit;
                }
            }
            askings.push(record(
                "Asking",
                "Asking",
                vec![
                    ("group", batch.group),
                    (
                        "paths",
                        Value::List(batch.paths.iter().map(|path| string(path)).collect()),
                    ),
                ],
            ));
        }
        let step = record(
            "AnswerStep",
            "AnswerStep",
            vec![
                ("groups", Value::List(askings)),
                ("case", filled),
                ("answered", Value::Bool(answered)),
            ],
        );
        if namespace.same_instance(&self.runtime_root) {
            step
        } else {
            self.constructors_in(namespace, step)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guide(items: &[(&str, &str)]) -> Vec<GuideItem> {
        items
            .iter()
            .map(|(path, group)| GuideItem {
                path: path.to_string(),
                group: group.to_string(),
                owner: None,
                span: Span::dummy(),
            })
            .collect()
    }

    #[test]
    fn the_most_specific_item_resolves_a_path() {
        let items = guide(&[
            ("events", "Bookings"),
            ("events.$variant", "Backoffice"),
            ("account.ending.Closed.prices", "Snapshots"),
        ]);
        let group = |path: &str| item_for(&items, path).map(|item| item.group.as_str());
        assert_eq!(group("events"), Some("Bookings"));
        assert_eq!(group("events[3].$variant"), Some("Backoffice"));
        assert_eq!(group("events[3].Trade.amount"), Some("Bookings"));
        assert_eq!(
            group("account.ending.Closed.prices[0].isin"),
            Some("Snapshots")
        );
        assert_eq!(group("account.opening"), None);
        assert_eq!(group("eventsx"), None);
    }

    #[test]
    fn the_whole_input_covers_every_path() {
        let items = guide(&[("", "WholeCase")]);
        assert!(item_for(&items, "a.b[1].c").is_some());
    }

    #[test]
    fn list_indices_are_not_part_of_a_guide_path() {
        assert_eq!(element_path("a[1].b[22].c"), "a.b.c");
        assert_eq!(written_path("T", "a.b"), "pathof(T::a::b)");
        assert_eq!(written_path("T", ""), "pathof(T)");
    }

    #[test]
    fn a_group_is_read_by_its_bare_name_with_a_written_owner() {
        let source = "= g: AnswerGuide(T, Origin) = AnswerGuide(fields = [\n    EntryField(path = pathof(T::a), by = Customer),\n    EntryField(path = pathof(T::b), by = AskOrigin.Customer)\n])\n";
        let statements = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .unwrap();
        let guides = read_guides(&statements);
        let [Ok(guide)] = guides.as_slice() else {
            panic!("{guides:?}");
        };
        assert_eq!(guide.group_type.as_deref(), Some("Origin"));
        let read: Vec<(&str, Option<&str>)> = guide
            .items
            .iter()
            .map(|item| (item.group.as_str(), item.owner.as_deref()))
            .collect();
        assert_eq!(read, [("Customer", None), ("Customer", Some("AskOrigin"))]);
    }
}
