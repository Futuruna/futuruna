//! Ordinary function declarations are unique within an authored lexical scope,
//! and a binding that takes the name of a function or rule is reported.

use super::*;

impl TypeChecker {
    pub(super) fn check_function_declarations(&mut self, statements: &[Stmt]) {
        let mut names = BTreeSet::new();
        for statement in statements {
            match statement {
                // Prelude declarations are a separate source scope. Authored
                // declarations may override them, just as they may override
                // names supplied by a prefix import. Imported units receive
                // their own declaration check before their bodies are checked.
                Stmt::PreludeBoundary => names.clear(),
                Stmt::Defn(Defn::Fn { name, body, .. }) => {
                    if !names.insert(name) {
                        self.error_at_expr(
                            body,
                            format!("duplicate function declaration `{name}` in this scope; use one definition or distinct names"),
                        );
                    }
                }
                _ => {}
            }
        }
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
