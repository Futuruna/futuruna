//! Ordinary function declarations are unique within an authored lexical scope,
//! a rule family and a function never share a name, a top-level binding is
//! written once (a second one that reads the name is an update), and a
//! binding that takes the name of a function or rule is reported: a reader
//! finds one declaration for each name.

use super::*;

impl TypeChecker {
    pub(super) fn check_function_declarations(&mut self, statements: &[Stmt]) {
        let mut names = BTreeSet::new();
        // The first function and the first rule clause of each name.
        let mut functions: BTreeMap<&str, Span> = BTreeMap::new();
        let mut rules: BTreeMap<String, Span> = BTreeMap::new();
        let mut reported = BTreeSet::new();
        for statement in statements {
            match statement {
                // Prelude declarations are a separate source scope. Authored
                // declarations may override them, just as they may override
                // names supplied by a prefix import. Imported units receive
                // their own declaration check before their bodies are checked.
                Stmt::PreludeBoundary => {
                    names.clear();
                    functions.clear();
                    rules.clear();
                }
                Stmt::Defn(Defn::Fn { name, body, .. }) => {
                    if !names.insert(name) {
                        self.error_at_expr(
                            body,
                            format!("duplicate function declaration `{name}` in this scope; use one definition or distinct names"),
                        );
                    }
                    functions.entry(name.as_str()).or_insert(body.span);
                    if let Some(rule) = rules.get(name.as_str()) {
                        if reported.insert(name.clone()) {
                            let message = self.rule_and_function_message(name, *rule, body.span);
                            self.error_at_expr(body, message);
                        }
                    }
                }
                Stmt::Rule(rule) => {
                    let (Some(head), Some((name, _))) = (rule.head(), rule.callable_name_arity())
                    else {
                        continue;
                    };
                    if let Some(function) = functions.get(name.as_str()) {
                        if reported.insert(name.clone()) {
                            let message =
                                self.rule_and_function_message(&name, head.span, *function);
                            self.error_at_expr(head, message);
                        }
                    }
                    rules.entry(name).or_insert(head.span);
                }
                _ => {}
            }
        }
    }

    fn rule_and_function_message(&self, name: &str, rule: Span, function: Span) -> String {
        format!(
            "`{name}` is declared both as a rule ({}) and as a function ({}); a call cannot show which one it means, so rename one of them",
            self.source_line_label(rule),
            self.source_line_label(function)
        )
    }

    /// A top-level name is bound once in a file or module, or rebound by an
    /// update that reads it (`= m = map_insert(m, "k", 1)`): each statement
    /// reads the latest binding. A second binding that does not read the
    /// name replaces the earlier value unseen, so it is an error naming both
    /// lines. A name bound once is initialized before every statement that
    /// reads it, wherever it is written; a rebound name is read in statement
    /// order, so reading it before its first binding is an error. Bindings
    /// in a body may be rebound freely.
    pub(super) fn check_top_level_bindings(&mut self, statements: &[Stmt]) {
        let mut latest: BTreeMap<String, Span> = BTreeMap::new();
        for statement in statements {
            let (Stmt::Bind(pattern, _, value) | Stmt::MonadicBind(pattern, _, value)) = statement
            else {
                if matches!(statement, Stmt::PreludeBoundary) {
                    latest.clear();
                }
                continue;
            };
            let mut names = BTreeSet::new();
            collect_pattern_names(pattern, &mut names);
            let mut read = BTreeSet::new();
            collect_true_free_vars(value, &mut read, &BTreeSet::new());
            for name in names {
                if let Some(earlier) = latest.get(&name) {
                    if !read.contains(&name) {
                        let message = format!(
                            "`{name}` is bound twice at top level ({} and {}), and the second binding does not read the first: only the later value would be used, so keep one binding or rename one",
                            self.source_line_label(*earlier),
                            self.source_line_label(value.span)
                        );
                        self.error_at_expr(value, message);
                    }
                }
                latest.insert(name, value.span);
            }
        }
        self.check_reads_before_first_binding(statements);
    }

    /// A statement that reads a rebound top-level name, directly or through
    /// the functions and rules it calls, before the name's first binding.
    fn check_reads_before_first_binding(&mut self, statements: &[Stmt]) {
        let start = statements
            .iter()
            .rposition(|statement| matches!(statement, Stmt::PreludeBoundary))
            .map_or(0, |boundary| boundary + 1);
        let mut first = BTreeMap::<&str, (Span, usize)>::new();
        for statement in &statements[start..] {
            if let Stmt::Bind(Pat::Var(name), _, value) = statement {
                first.entry(name.as_str()).or_insert((value.span, 0)).1 += 1;
            }
        }
        if first.values().all(|(_, count)| *count < 2) {
            return;
        }
        let plan = analyze_top_level_binding_dependencies(statements);
        let mut bound = BTreeSet::new();
        for statement in &statements[start..] {
            let Some(span) = executed_statement_span(statement) else {
                continue;
            };
            for name in plan.statement_reads(statement) {
                let Some(&(binding, count)) = first.get(name.as_str()) else {
                    continue;
                };
                if count < 2 || bound.contains(&name) {
                    continue;
                }
                let message = format!(
                    "`{name}` is read here ({}) before its first binding ({}): a name bound more than once is read in statement order, so bind it before this statement",
                    self.source_line_label(span),
                    self.source_line_label(binding)
                );
                self.error_at_span(span, message);
            }
            if let Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) = statement {
                collect_pattern_names(pattern, &mut bound);
            }
        }
    }

    /// `line N` of a span in the source being checked.
    fn source_line_label(&self, span: Span) -> String {
        if span.is_dummy() {
            return "an unknown line".to_string();
        }
        let line = 1 + self
            .source_text
            .chars()
            .take(span.start)
            .filter(|character| *character == '\n')
            .count();
        format!("line {line}")
    }
}

/// The place of a top-level statement that runs where it is written; `None`
/// for a declaration.
fn executed_statement_span(statement: &Stmt) -> Option<Span> {
    match statement {
        Stmt::Expr(expr)
        | Stmt::Bind(_, _, expr)
        | Stmt::MonadicBind(_, _, expr)
        | Stmt::StreamBind(_, expr)
        | Stmt::For(_, expr, _)
        | Stmt::While(expr, _)
        | Stmt::Send(expr, _)
        | Stmt::StreamSub(expr, _)
        | Stmt::Invariant { subject: expr, .. } => Some(expr.span),
        _ => None,
    }
}

/// A top-level binding (`= closed = …`) that takes the name of a function or
/// rule declared in the program, which shares the program's one namespace of
/// top-level names. Futuruna allows a binding that holds no function (a
/// result is often bound under its rule's name, `= tax = tax(case)`):
/// `closed` reads the binding and `closed(…)` calls the function or rule. A
/// warning names both, so a reader is told which one each use means. A
/// binding that holds a function would make a call `closed(…)` mean either:
/// that is an error naming both. Bindings in a body, parameters and patterns
/// are lexical and keep their scope; the prelude's declarations may be
/// overridden. Returns the warnings and the errors.
pub(crate) fn binding_name_diagnostics(
    statements: &[Stmt],
    source: &str,
    checker: &TypeChecker,
) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    let files = query::program_files(statements, source, checker);
    // The first declaration of each callable name: its kind and place.
    let mut callables: BTreeMap<&str, (&'static str, String)> = BTreeMap::new();
    for file in &files {
        for statement in file.statements {
            let (kind, name, span) = match statement {
                Stmt::Defn(Defn::Fn { name, body, .. }) => ("function", name.as_str(), body.span),
                Stmt::Rule(
                    Rule::Clause { head, .. }
                    | Rule::Default { head, .. }
                    | Rule::Exception { head, .. },
                ) => match query::rule_head_name(head) {
                    Some(name) => ("rule", name, head.span),
                    None => continue,
                },
                _ => continue,
            };
            callables
                .entry(name)
                .or_insert_with(|| (kind, file.place(span)));
        }
    }
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    if callables.is_empty() {
        return (warnings, errors);
    }
    let values = top_level_binding_names(&files);
    for file in &files {
        for statement in file.statements {
            let (Stmt::Bind(Pat::Var(name), _, value)
            | Stmt::MonadicBind(Pat::Var(name), _, value)) = statement
            else {
                continue;
            };
            let Some((kind, place)) = callables.get(name.as_str()) else {
                continue;
            };
            if top_level_binding_holds_function(checker, &values, name, value) {
                errors.push(file.diagnostic(
                    binding_value_span(value),
                    format!(
                        "the binding `{name}` holds a function and takes the name of the {kind} `{name}` declared at {place}: a call `{name}(…)` could mean either. Rename one of them"
                    ),
                ));
                continue;
            }
            let mut warning = file.diagnostic(
                value.span,
                format!(
                    "the binding `{name}` takes the name of the {kind} `{name}` declared at {place}: `{name}` reads this binding, while a call `{name}(…)` calls the {kind}. Rename one of them so the name means one thing"
                ),
            );
            warning.severity = Severity::Warning;
            warnings.push(warning);
        }
    }
    (warnings, errors)
}

/// Whether a top-level binding holds a function: a closure literal, the name
/// of a function or rule (not of a top-level binding, in `values`), or a
/// value the check typed as a function.
pub(crate) fn top_level_binding_holds_function(
    checker: &TypeChecker,
    values: &BTreeSet<String>,
    name: &str,
    value: &Expr,
) -> bool {
    match &value.kind {
        ExprKind::Lambda(..) => return true,
        ExprKind::Var(target)
            if !values.contains(target)
                && (checker.functions.contains_key(target)
                    || checker.rule_arities.iter().any(|(rule, _)| rule == target)) =>
        {
            return true;
        }
        _ => {}
    }
    checker
        .var_types
        .first()
        .and_then(|types| types.get(name))
        .and_then(|ty| parse_type_annotation(ty).ok())
        .is_some_and(|ty| matches!(ty, Ty::Arrow(..)))
}

/// The names of the program's top-level bindings.
fn top_level_binding_names(files: &[query::ProgramFile<'_>]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for file in files {
        for statement in file.statements {
            if let Stmt::Bind(Pat::Var(name), _, _) | Stmt::MonadicBind(Pat::Var(name), _, _) =
                statement
            {
                names.insert(name.clone());
            }
        }
    }
    names
}

/// Where a binding's value is: a lambda carries no span of its own; its
/// body does.
fn binding_value_span(value: &Expr) -> Span {
    match &value.kind {
        ExprKind::Lambda(_, body) if value.span.is_dummy() => body.span,
        _ => value.span,
    }
}

/// Compiled code keeps a top-level binding as a local of `main` (or a field
/// of the hidden globals a function reads), which cannot hold a closure: a
/// function or rule body that calls or reads a top-level binding holding a
/// function is reported before Rust is generated, instead of a rustc
/// failure. The interpreter runs such a program.
pub(crate) fn native_function_binding_uses(
    statements: &[Stmt],
    source: &str,
    checker: &TypeChecker,
) -> Vec<Diagnostic> {
    let files = query::program_files(statements, source, checker);
    let values = top_level_binding_names(&files);
    // A binding named like a function or rule and holding a function is
    // already an error of its own (`binding_name_diagnostics`).
    let callables = files
        .iter()
        .flat_map(|file| file.statements)
        .filter_map(|statement| match statement {
            Stmt::Defn(Defn::Fn { name, .. }) => Some(name.as_str()),
            Stmt::Rule(
                Rule::Clause { head, .. }
                | Rule::Default { head, .. }
                | Rule::Exception { head, .. },
            ) => query::rule_head_name(head),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut function_bindings = BTreeSet::new();
    for file in &files {
        for statement in file.statements {
            if let Stmt::Bind(Pat::Var(name), _, value)
            | Stmt::MonadicBind(Pat::Var(name), _, value) = statement
            {
                if !callables.contains(name.as_str())
                    && top_level_binding_holds_function(checker, &values, name, value)
                {
                    function_bindings.insert(name.clone());
                }
            }
        }
    }
    let mut errors = Vec::new();
    if function_bindings.is_empty() {
        return errors;
    }
    for file in &files {
        for statement in file.statements {
            let (kind, owner, bodies, bound): (_, _, Vec<&Expr>, BTreeSet<String>) = match statement
            {
                Stmt::Defn(Defn::Fn {
                    name, params, body, ..
                }) => (
                    "function",
                    name.as_str(),
                    vec![body],
                    params.iter().map(|param| param.name.clone()).collect(),
                ),
                Stmt::Rule(rule) => {
                    let (head, bodies) = match rule {
                        Rule::Clause { head, body } => (head, body.iter().collect()),
                        Rule::Default {
                            head,
                            value,
                            condition,
                        }
                        | Rule::Exception {
                            head,
                            value,
                            condition,
                            ..
                        } => (head, std::iter::once(value).chain(condition).collect()),
                        Rule::ReactiveScope { .. } => continue,
                    };
                    let Some(name) = query::rule_head_name(head) else {
                        continue;
                    };
                    // The head's variables are the rule's parameters.
                    let mut bound = BTreeSet::new();
                    collect_true_free_vars(head, &mut bound, &BTreeSet::new());
                    bound.remove(name);
                    ("rule", name, bodies, bound)
                }
                _ => continue,
            };
            for body in bodies {
                let mut free = BTreeSet::new();
                collect_true_free_vars(body, &mut free, &bound);
                for binding in free.intersection(&function_bindings) {
                    let called = collect_scoped_runtime_calls(body, &bound)
                        .into_iter()
                        .find(|call| call.name == *binding && !call.lexically_bound);
                    let (verb, span) = match &called {
                        Some(call) => ("calls", call.span),
                        None => ("reads", body.span),
                    };
                    errors.push(file.diagnostic(
                        span,
                        format!(
                            "the {kind} `{owner}` {verb} the top-level binding `{binding}`, which holds a function: compiled code cannot use a function-valued top-level binding in a function or rule body; pass the function as a parameter. The interpreter runs the program"
                        ),
                    ));
                }
            }
        }
    }
    errors
}
