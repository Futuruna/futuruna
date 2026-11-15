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
    /// lines. Bindings in a body may be rebound freely.
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

/// A top-level binding (`= closed = …`) that takes the name of a function or
/// rule declared in the program, which shares the program's one namespace of
/// top-level names. Futuruna allows it (a result is often bound under its
/// rule's name, `= tax = tax(case)`): `closed` reads the binding, and
/// `closed(…)` calls the function or rule unless the binding holds a
/// function, which then takes the calls after it. A warning names both, so a
/// reader is told which one each use means. Bindings in a body, parameters
/// and patterns are lexical and keep their scope; the prelude's declarations
/// may be overridden.
pub(crate) fn binding_name_warnings(
    statements: &[Stmt],
    source: &str,
    checker: &TypeChecker,
) -> Vec<Diagnostic> {
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
    if callables.is_empty() {
        return Vec::new();
    }
    let mut warnings = Vec::new();
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
            let (meaning, span) = match &value.kind {
                ExprKind::Lambda(_, body) => (
                    format!("after it, a call `{name}(…)` calls this binding, not the {kind}"),
                    // A lambda carries no span of its own; its body does.
                    if value.span.is_dummy() {
                        body.span
                    } else {
                        value.span
                    },
                ),
                _ => (
                    format!(
                        "`{name}` reads this binding, while a call `{name}(…)` calls the {kind}"
                    ),
                    value.span,
                ),
            };
            let mut warning = file.diagnostic(
                span,
                format!(
                    "the binding `{name}` takes the name of the {kind} `{name}` declared at {place}: {meaning}. Rename one of them so the name means one thing"
                ),
            );
            warning.severity = Severity::Warning;
            warnings.push(warning);
        }
    }
    warnings
}
