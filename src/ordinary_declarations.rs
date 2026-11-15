//! Ordinary function declarations are unique within an authored lexical scope,
//! a rule family and a function never share a name, and a top-level binding
//! is written once: a reader finds one declaration for each name.

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

    /// A top-level binding is written once in a file or module: with two,
    /// a reader of the first would read a value the program never uses.
    /// Bindings in a body may be rebound; each statement sees the latest.
    pub(super) fn check_top_level_bindings(&mut self, statements: &[Stmt]) {
        let mut first: BTreeMap<String, Span> = BTreeMap::new();
        for statement in statements {
            let (Stmt::Bind(pattern, _, value) | Stmt::MonadicBind(pattern, _, value)) = statement
            else {
                if matches!(statement, Stmt::PreludeBoundary) {
                    first.clear();
                }
                continue;
            };
            let mut names = BTreeSet::new();
            collect_pattern_names(pattern, &mut names);
            for name in names {
                match first.get(&name) {
                    Some(earlier) => {
                        let message = format!(
                            "`{name}` is bound twice at top level ({} and {}); only the later value would be used, so keep one binding or rename one",
                            self.source_line_label(*earlier),
                            self.source_line_label(value.span)
                        );
                        self.error_at_expr(value, message);
                    }
                    None => {
                        first.insert(name, value.span);
                    }
                }
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
