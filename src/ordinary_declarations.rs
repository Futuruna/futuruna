//! Ordinary function declarations are unique within an authored lexical scope.

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
