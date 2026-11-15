//! Declared rule result types (docs/rfcs/named-scalar-types-and-rule-result-types.md,
//! Part 2).
//!
//! A rule family's **declaring line** is its first line in source order, in
//! the statement sequence that holds its ordinary clauses: `| rate(year: Int):
//! Ore -> 0`, or the bodiless declaration `| fib(n: Int): Int`. It alone writes
//! the parameter types and the result type; every later clause, and every
//! exception, writes a name or a pattern at each position.
//!
//! The parser reads `: R` after a rule head into a parse-time signature
//! statement and then elaborates each statement sequence:
//!
//! - a later clause's name takes the declaring line's type, so every consumer
//!   after the parser (checker, interpreter, Explore, solver, generated Rust)
//!   reads the same typed heads it read when every clause wrote its types;
//! - `: R` is kept as one inert annotation statement, [`RULE_RESULT_TYPE_ANNOTATION`],
//!   that the checker reads as the family's result type and the runtime reads
//!   for the miss of a family that is not `: Bool`;
//! - a type on a later clause, a name that differs from the declaring line's,
//!   a second declaration and a declaration without a clause are parse
//!   errors that point to the declaring line.

use super::*;

/// Parse-time form of `: R` on a rule head: `[head, Lit(Str(R)), Lit(Bool(has_clause))]`.
/// Elaboration replaces it; it never leaves the parser.
const RULE_SIGNATURE_MARKER: &str = "__rule_signature";

/// The elaborated declaration of a family's result type:
/// `[Lit(Str(name)) spanned at the declaring head, Lit(Int(arity)), Lit(Str(R)),
/// Lit(Str(parameter type or ""))...]`. Every argument is a literal, so the
/// statement is inert for every walker that visits annotation arguments.
pub const RULE_RESULT_TYPE_ANNOTATION: &str = "__rule_result_type";

/// One family's declared result, read back from [`RULE_RESULT_TYPE_ANNOTATION`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredRuleResult {
    pub name: String,
    pub arity: usize,
    pub result: String,
    pub parameter_types: Vec<Option<String>>,
    /// The declaring head, for diagnostics that name the declaration.
    pub span: Span,
}

/// The declared result of `statement` when it is a family's result-type
/// annotation.
pub fn declared_rule_result(statement: &Stmt) -> Option<DeclaredRuleResult> {
    let Stmt::Annot(name, arguments) = statement else {
        return None;
    };
    if name != RULE_RESULT_TYPE_ANNOTATION || arguments.len() < 3 {
        return None;
    }
    let ExprKind::Lit(Literal::Str(family)) = &arguments[0].kind else {
        return None;
    };
    let ExprKind::Lit(Literal::Int(arity)) = &arguments[1].kind else {
        return None;
    };
    let ExprKind::Lit(Literal::Str(result)) = &arguments[2].kind else {
        return None;
    };
    let parameter_types = arguments[3..]
        .iter()
        .map(|argument| match &argument.kind {
            ExprKind::Lit(Literal::Str(ty)) if !ty.is_empty() => Some(ty.clone()),
            _ => None,
        })
        .collect();
    Some(DeclaredRuleResult {
        name: family.clone(),
        arity: usize::try_from(*arity).ok()?,
        result: result.clone(),
        parameter_types,
        span: arguments[0].span,
    })
}

/// The declared results of the families in one statement sequence (not its
/// nested scopes), by name and arity.
pub fn declared_rule_results(statements: &[Stmt]) -> BTreeMap<(String, usize), DeclaredRuleResult> {
    statements
        .iter()
        .filter_map(declared_rule_result)
        .map(|declared| ((declared.name.clone(), declared.arity), declared))
        .collect()
}

fn is_parameter_name(name: &str) -> bool {
    name != "_" && !name.chars().next().is_some_and(char::is_uppercase)
}

/// A head argument as the declaring line reads it: its name when it is a
/// name, and its written type.
fn head_parameter(argument: &Expr) -> (Option<String>, Option<String>) {
    let (inner, ty) = match typed_rule_head_argument(argument) {
        Some((inner, ty)) => (inner, Some(ty.to_string())),
        None => (argument, None),
    };
    let name = match &inner.kind {
        ExprKind::Var(name) if is_parameter_name(name) => Some(name.clone()),
        _ => None,
    };
    (name, ty)
}

fn head_arguments(head: &Expr) -> &[Expr] {
    match &head.kind {
        ExprKind::App(_, arguments) => arguments,
        _ => &[],
    }
}

fn head_arguments_mut(head: &mut Expr) -> Option<&mut Vec<Expr>> {
    match &mut head.kind {
        ExprKind::App(_, arguments) => Some(arguments),
        _ => None,
    }
}

fn typed_argument(argument: Expr, ty: &str) -> Expr {
    // The parser's own form for `name: Type` in a rule head.
    ExprKind::App(
        Box::new(ExprKind::Var("__typed".into()).into()),
        vec![argument, ExprKind::Var(ty.to_string()).into()],
    )
    .into()
}

/// The span of a head argument for a diagnostic: the written argument inside
/// a type annotation, which has no span of its own.
fn argument_span(argument: &Expr, head: &Expr) -> Span {
    let inner = typed_rule_head_argument(argument)
        .map(|(inner, _)| inner)
        .unwrap_or(argument);
    if inner.span.is_dummy() {
        head.span
    } else {
        inner.span
    }
}

struct Declaring {
    /// The declaring line's head.
    span: Span,
    /// Name and type at each position.
    parameters: Vec<(Option<String>, Option<String>)>,
    /// Whether the family's first line is an exception, which cannot declare.
    exception: bool,
    /// Whether a signature (`: R`) declared the family.
    signature: bool,
    /// Whether the family has an ordinary clause or default.
    has_clause: bool,
}

pub(crate) struct DeclarationError {
    pub span: Span,
    pub message: String,
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

fn ordinal(position: usize) -> String {
    match position {
        1 => "first".into(),
        2 => "second".into(),
        3 => "third".into(),
        4 => "fourth".into(),
        5 => "fifth".into(),
        n => format!("{n}th"),
    }
}

/// Whether the parser reads rule families written before declarations: a
/// type on every clause and any name. Only `runa fmt --migrate` reads them,
/// to find the declarations it writes.
static LEGACY_RULE_TYPES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Run `read` with the parser accepting the types and names of rule families
/// written before declarations (see [`LEGACY_RULE_TYPES`]).
pub fn with_legacy_rule_types<R>(read: impl FnOnce() -> R) -> R {
    use std::sync::atomic::Ordering;
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            LEGACY_RULE_TYPES.store(self.0, Ordering::SeqCst);
        }
    }
    let _restore = Restore(LEGACY_RULE_TYPES.swap(true, Ordering::SeqCst));
    read()
}

/// Elaborate the rule declarations of one statement sequence (see the module
/// documentation). `line_of` gives the 1-based line of a span.
pub(crate) fn elaborate_rule_declarations(
    statements: &mut Vec<Stmt>,
    line_of: &dyn Fn(Span) -> usize,
) -> Vec<DeclarationError> {
    let has_rules = statements.iter().any(|statement| {
        matches!(statement, Stmt::Rule(rule) if rule.head().is_some())
            || matches!(statement, Stmt::Annot(name, _) if name == RULE_SIGNATURE_MARKER)
    });
    if !has_rules {
        return Vec::new();
    }
    let legacy = LEGACY_RULE_TYPES.load(std::sync::atomic::Ordering::SeqCst);
    let mut errors = Vec::new();
    let mut families: BTreeMap<(String, usize), Declaring> = BTreeMap::new();
    // The family whose declaring clause directly follows its signature.
    let mut attached: Option<(String, usize)> = None;
    // Final annotations by the index of the signature they replace.
    let mut annotations: BTreeMap<usize, Stmt> = BTreeMap::new();

    for index in 0..statements.len() {
        let next_attached = attached.take();
        match &mut statements[index] {
            Stmt::Annot(name, arguments) if name == RULE_SIGNATURE_MARKER => {
                let [head, result, has_clause] = arguments.as_slice() else {
                    continue;
                };
                let (ExprKind::Lit(Literal::Str(result)), ExprKind::Lit(Literal::Bool(has_clause))) =
                    (&result.kind, &has_clause.kind)
                else {
                    continue;
                };
                let Some((family, arity)) = rule_head_name_arity(head) else {
                    continue;
                };
                let key = (family.clone(), arity);
                if let Some(previous) = families.get(&key) {
                    let first = line_of(previous.span);
                    let message = if previous.signature {
                        format!(
                            "rule `{family}` with {arity} argument{} is declared twice; it is declared at line {first}, and a family has one declaring line",
                            plural(arity)
                        )
                    } else if previous.exception {
                        format!(
                            "the declaration of `{family}` must be the family's first line, but the exception at line {first} comes before it; move the declaration above the exception"
                        )
                    } else {
                        format!(
                            "the declaration of `{family}` must be the family's first line, but its clause at line {first} comes before it; write `: {result}` on that line"
                        )
                    };
                    errors.push(DeclarationError {
                        span: head.span,
                        message,
                    });
                    continue;
                }
                let parameters = head_arguments(head)
                    .iter()
                    .map(head_parameter)
                    .collect::<Vec<_>>();
                let mut final_arguments = vec![
                    Expr::new(ExprKind::Lit(Literal::Str(family.clone())), head.span),
                    ExprKind::Lit(Literal::Int(arity as i64)).into(),
                    ExprKind::Lit(Literal::Str(result.clone())).into(),
                ];
                final_arguments.extend(parameters.iter().map(|(_, ty)| {
                    Expr::from(ExprKind::Lit(Literal::Str(ty.clone().unwrap_or_default())))
                }));
                annotations.insert(
                    index,
                    Stmt::Annot(RULE_RESULT_TYPE_ANNOTATION.to_string(), final_arguments),
                );
                families.insert(
                    key.clone(),
                    Declaring {
                        span: head.span,
                        parameters,
                        exception: false,
                        signature: true,
                        has_clause: *has_clause,
                    },
                );
                if *has_clause {
                    attached = Some(key);
                }
            }
            Stmt::Rule(rule) => {
                let exception = matches!(rule, Rule::Exception { .. });
                let ordinary = matches!(
                    rule,
                    Rule::Clause { body: Some(_), .. } | Rule::Default { .. }
                );
                let fact = matches!(rule, Rule::Clause { body: None, .. });
                let (head, label) = match rule {
                    Rule::Clause { head, .. } | Rule::Default { head, .. } => (head, None),
                    Rule::Exception { head, label, .. } => (head, Some(label.clone())),
                    Rule::ReactiveScope { .. } => continue,
                };
                let Some((family, arity)) = rule_head_name_arity(head) else {
                    continue;
                };
                let key = (family.clone(), arity);
                if next_attached.as_ref() == Some(&key) {
                    // The clause written on the declaring line itself.
                    continue;
                }
                let Some(declaring) = families.get_mut(&key) else {
                    // The family's first line in this sequence.
                    if let Some(label) = &label {
                        if let Some(argument) = head_arguments(head)
                            .iter()
                            .find(|argument| typed_rule_head_argument(argument).is_some())
                        {
                            errors.push(DeclarationError {
                                span: argument_span(argument, head),
                                message: format!(
                                    "exception `{label}` writes a parameter type; an exception overrides a rule and writes only names, and the types of `{family}` are written on its declaring line, the family's first ordinary line"
                                ),
                            });
                        }
                    }
                    families.insert(
                        key,
                        Declaring {
                            span: head.span,
                            parameters: head_arguments(head).iter().map(head_parameter).collect(),
                            exception,
                            signature: false,
                            has_clause: ordinary || fact,
                        },
                    );
                    continue;
                };
                declaring.has_clause |= ordinary || fact;
                let declaring_line = line_of(declaring.span);
                let declaring_exception = declaring.exception;
                // Names are the declaration's once it declares a type: an
                // undeclared relation keeps its logic variables free.
                let declared =
                    declaring.signature || declaring.parameters.iter().any(|(_, ty)| ty.is_some());
                let parameters = declaring.parameters.clone();
                let head_span = head.span;
                let Some(arguments) = head_arguments_mut(head) else {
                    continue;
                };
                for (position, argument) in arguments.iter_mut().enumerate() {
                    let (name, ty) = head_parameter(argument);
                    let (declared_name, declared_type) =
                        parameters.get(position).cloned().unwrap_or((None, None));
                    if let (false, Some(ty)) = (legacy, &ty) {
                        let span = typed_rule_head_argument(argument)
                            .map(|(inner, _)| inner.span)
                            .filter(|span| !span.is_dummy())
                            .unwrap_or(head_span);
                        let message = if declaring_exception {
                            format!(
                                "the {} parameter of `{family}` writes the type `{ty}`, but this is not the family's first line: the exception at line {declaring_line} is, and an exception cannot declare types; declare the family above it with `| {family}(...): <result>`",
                                ordinal(position + 1)
                            )
                        } else {
                            format!(
                                "the {} parameter of `{family}` writes the type `{ty}`; types are written once, on the family's declaring line (line {declaring_line}); write only the name here",
                                ordinal(position + 1)
                            )
                        };
                        errors.push(DeclarationError { span, message });
                    }
                    if let (true, false, Some(name), Some(declared)) =
                        (declared, legacy, &name, &declared_name)
                    {
                        if name != declared {
                            errors.push(DeclarationError {
                                span: argument_span(argument, &Expr::new(ExprKind::Unit, head_span)),
                                message: format!(
                                    "the {} parameter of `{family}` is `{declared}` on its declaring line (line {declaring_line}); call it `{declared}` here too",
                                    ordinal(position + 1)
                                ),
                            });
                        }
                    }
                    if ty.is_none() && name.is_some() {
                        if let Some(declared_type) = declared_type {
                            let written = std::mem::replace(argument, ExprKind::Unit.into());
                            *argument = typed_argument(written, &declared_type);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for (key, declaring) in &families {
        if declaring.signature && !declaring.has_clause {
            errors.push(DeclarationError {
                span: declaring.span,
                message: format!(
                    "rule `{}` is declared but has no clause; write its clauses below the declaration",
                    key.0
                ),
            });
        }
    }

    if !annotations.is_empty()
        || statements
            .iter()
            .any(|s| matches!(s, Stmt::Annot(name, _) if name == RULE_SIGNATURE_MARKER))
    {
        // Each result-type annotation stands before the annotations
        // (`@ calculate`, `@ export`) of the declaring line, so that those
        // still stand directly before what they annotate.
        let mut elaborated = Vec::with_capacity(statements.len());
        for (index, statement) in std::mem::take(statements).into_iter().enumerate() {
            if matches!(&statement, Stmt::Annot(name, _) if name == RULE_SIGNATURE_MARKER) {
                if let Some(annotation) = annotations.remove(&index) {
                    let mut position = elaborated.len();
                    while position > 0
                        && matches!(&elaborated[position - 1], Stmt::Annot(name, _) if name != RULE_RESULT_TYPE_ANNOTATION)
                    {
                        position -= 1;
                    }
                    elaborated.insert(position, annotation);
                }
                continue;
            }
            elaborated.push(statement);
        }
        *statements = elaborated;
    }
    errors
}

impl Parser {
    /// `| head: R -> ...` or the bodiless `| head: R`, after the head. A
    /// declaring clause is returned as its signature, and the parser is
    /// rewound to read the clause itself next, without `: R`.
    pub(super) fn parse_rule_declaration(
        &mut self,
        pipe_position: usize,
        head: Expr,
    ) -> Result<Stmt, String> {
        let colon_position = self.pos;
        let colon = self.advance();
        // `->` after the type starts the clause's value, so the result type
        // is an atom (`Ore`, `List(Int)`, `(Int -> Int)`) with an optional `?`.
        let mut result = self.parse_type_atom()?;
        if self.peek_kind() == TokenKind::Op && self.peek().text == "?" {
            self.advance();
            result = Ty::Optional(Box::new(result));
        }
        let head_is_rule = matches!(&head.kind, ExprKind::App(function, _) if matches!(&function.kind, ExprKind::Var(name) if is_parameter_name(name)));
        if !head_is_rule {
            return Err(format!(
                "{}:{}: `: {result}` declares a rule's result type after a rule head with parentheses: `| rate(year: Int): {result} -> ...`",
                colon.line, colon.col
            ));
        }
        let has_clause = self.peek_kind() == TokenKind::Arrow;
        if has_clause {
            if self.tokens.get(pipe_position).map(|token| token.kind) != Some(TokenKind::Pipe) {
                return Err(format!(
                    "{}:{}: a rule declaration starts a `|` line",
                    colon.line, colon.col
                ));
            }
            // Read the clause again without `: R`.
            self.tokens.drain(colon_position..self.pos);
            self.pos = pipe_position;
        } else {
            for argument in head_arguments(&head) {
                if head_parameter(argument).0.is_none() {
                    let span = argument_span(argument, &head);
                    let (line, col) = self.line_col_of(span.start);
                    return Err(format!(
                        "{line}:{col}: a declaration names each parameter (`n: Int`); write patterns on the clauses below it"
                    ));
                }
            }
        }
        Ok(Stmt::Annot(
            RULE_SIGNATURE_MARKER.to_string(),
            vec![
                head,
                ExprKind::Lit(Literal::Str(result.to_string())).into(),
                ExprKind::Lit(Literal::Bool(has_clause)).into(),
            ],
        ))
    }

    /// Elaborate the rule declarations of a parsed statement sequence; each
    /// error as `line:col: message`.
    pub(super) fn elaborate_rule_declarations(
        &self,
        statements: &mut Vec<Stmt>,
    ) -> Result<(), String> {
        let errors =
            elaborate_rule_declarations(statements, &|span| self.line_col_of(span.start).0);
        if errors.is_empty() {
            return Ok(());
        }
        Err(errors
            .into_iter()
            .map(|error| {
                let (line, col) = self.line_col_of(error.span.start);
                format!("{line}:{col}: {}", error.message)
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// A family's declaration as the clause check reads it.
pub(super) struct DeclaredFamily {
    name: String,
    result: Ty,
    result_text: String,
    /// The line of the declaring head in its own file.
    line: usize,
}

impl TypeChecker {
    fn note_declared(&mut self, scope: Option<&str>, declared: DeclaredRuleResult) {
        self.declared_rule_results.insert(
            RuleDispatchKey {
                scope: scope.map(str::to_string),
                name: declared.name.clone(),
                arity: declared.arity,
            },
            declared,
        );
    }

    /// Record the declarations of `statements` and of its RuleScope bodies,
    /// and give each top-level declared family its declared type in
    /// `rule_return_types`. The names of the top-level declared families.
    pub(super) fn note_declared_rule_results(&mut self, statements: &[Stmt]) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for statement in statements {
            match statement {
                Stmt::Annot(..) => {
                    if let Some(declared) = declared_rule_result(statement) {
                        names.insert(declared.name.clone());
                        self.rule_return_types.insert(
                            declared.name.clone(),
                            Self::canonical_explore_type_name(&declared.result),
                        );
                        self.note_declared(None, declared);
                    }
                }
                Stmt::TypeDecl(TypeDecl::RuleScope { name, body, .. }) => {
                    for declared in body.iter().filter_map(declared_rule_result) {
                        self.note_declared(Some(name), declared);
                    }
                }
                _ => {}
            }
        }
        names
    }

    /// The declared members of one RuleScope: member name to declared type.
    pub(super) fn declared_scope_member_results(
        &mut self,
        scope: &str,
        body: &[Stmt],
    ) -> BTreeMap<String, String> {
        let mut members = BTreeMap::new();
        for declared in body.iter().filter_map(declared_rule_result) {
            members.insert(
                declared.name.clone(),
                Self::canonical_explore_type_name(&declared.result),
            );
            self.note_declared(Some(scope), declared);
        }
        members
    }

    /// The declared result of every top-level family by name and arity.
    pub(super) fn declared_top_level_result_tys(&self) -> Vec<((String, usize), Ty)> {
        self.declared_rule_results
            .iter()
            .filter(|(key, _)| key.scope.is_none())
            .filter_map(|(key, declared)| {
                let ty =
                    parse_type_annotation(&Self::canonical_explore_type_name(&declared.result))
                        .ok()?;
                Some(((key.name.clone(), key.arity), ty))
            })
            .collect()
    }

    /// The declarations of the dispatch program: its top level and RuleScope
    /// bodies, a later RuleScope of one name replacing an earlier one.
    pub(super) fn note_declared_dispatch_results(
        &mut self,
        statements: &[Stmt],
    ) -> BTreeMap<RuleDispatchKey, DeclaredRuleResult> {
        let mut declared = BTreeMap::new();
        for statement in statements {
            match statement {
                Stmt::Annot(..) => {
                    if let Some(result) = declared_rule_result(statement) {
                        declared.insert(
                            RuleDispatchKey {
                                scope: None,
                                name: result.name.clone(),
                                arity: result.arity,
                            },
                            result,
                        );
                    }
                }
                Stmt::TypeDecl(TypeDecl::RuleScope { name, body, .. }) => {
                    declared.retain(|key: &RuleDispatchKey, _| key.scope.as_deref() != Some(name));
                    for result in body.iter().filter_map(declared_rule_result) {
                        declared.insert(
                            RuleDispatchKey {
                                scope: Some(name.clone()),
                                name: result.name.clone(),
                                arity: result.arity,
                            },
                            result,
                        );
                    }
                }
                _ => {}
            }
        }
        for (key, result) in &declared {
            self.declared_rule_results
                .insert(key.clone(), result.clone());
        }
        declared
    }

    pub(super) fn canonical_declared_parameter_type(&self, ty: &str) -> Option<Ty> {
        parse_type_annotation(ty).ok()
    }

    /// The declared result of a family, when it has one.
    pub(crate) fn declared_rule_result_type(&self, key: &RuleDispatchKey) -> Option<Ty> {
        let declared = self.declared_rule_results.get(key)?;
        parse_type_annotation(&Self::canonical_explore_type_name(&declared.result)).ok()
    }

    /// The declarations a sequence's clauses are checked against: its own
    /// and those of its plain imports, by name and arity.
    pub(super) fn collect_declared_families(
        &self,
        statements: &[Stmt],
        source: &str,
        dir: &str,
        families: &mut BTreeMap<(String, usize), DeclaredFamily>,
        visited: &mut BTreeSet<PathBuf>,
    ) {
        for statement in statements {
            match statement {
                Stmt::Annot(..) => {
                    let Some(declared) = declared_rule_result(statement) else {
                        continue;
                    };
                    let Ok(result) = parse_type_annotation(&declared.result) else {
                        continue;
                    };
                    let line = char_offset_to_line_col(source, declared.span.start).0;
                    families.insert(
                        (declared.name.clone(), declared.arity),
                        DeclaredFamily {
                            name: declared.name,
                            result,
                            result_text: declared.result,
                            line,
                        },
                    );
                }
                Stmt::Import(path) => {
                    let Some((file, module)) = self
                        .rule_contract_imports
                        .get(&(dir.to_string(), path.clone()))
                    else {
                        continue;
                    };
                    if !visited.insert(canonical_source_path(Path::new(file))) {
                        continue;
                    }
                    let imported_dir = Interpreter::imported_source_dir(file);
                    self.collect_declared_families(
                        module.statements(),
                        module.source(),
                        &imported_dir,
                        families,
                        visited,
                    );
                }
                _ => {}
            }
        }
    }

    /// Check one clause of a declared family against its declaration.
    pub(super) fn check_declared_rule_clause(&mut self, rule: &Rule, declared: &DeclaredFamily) {
        let (head, value) = match rule {
            Rule::Clause { head, body } => (head, body.as_ref()),
            Rule::Default { head, value, .. } | Rule::Exception { head, value, .. } => {
                (head, Some(value))
            }
            Rule::ReactiveScope { .. } => return,
        };
        let goal = match value {
            None => Some("a fact"),
            Some(value)
                if matches!(
                    value.kind,
                    ExprKind::Conjunction(_) | ExprKind::Disjunction(_)
                ) =>
            {
                Some("a goal body")
            }
            _ => None,
        };
        if let Some(kind) = goal {
            if Self::canonical_explore_ty_name(&declared.result) != "Bool" {
                self.error_at_expr(
                    head,
                    format!(
                        "rule `{}` is declared `: {}` at line {}, but this clause is {kind}, which gives `Bool`",
                        declared.name, declared.result_text, declared.line
                    ),
                );
            }
            return;
        }
        let Some(value) = value else {
            return;
        };
        let subject = format!(
            "rule `{}` (declared at line {})",
            declared.name, declared.line
        );
        let locals = self.rule_contract_local_types(head);
        self.check_rule_clause_result(&subject, &declared.result, value, locals);
    }
}

impl RuleMissFallback {
    /// The miss of a family declared with `result`: `False` for `: Bool`, an
    /// evaluation error for every other type.
    pub(super) fn of_declared_result(result: &str) -> Self {
        if TypeChecker::canonical_explore_type_name(result) == "Bool" {
            Self::PredicateFalse
        } else {
            Self::NoValue
        }
    }
}
