//! Statically visible effect operations without a handler. An operation of
//! a declared effect runs only inside `| handle Effect { ... } in ...`. Calls
//! the checker can follow (direct calls and calls of top-level functions that
//! perform the operation) are diagnosed here; calls through function values
//! are diagnosed when they run.

use super::*;

/// One call from a function body: the callee and the effects handled around it.
struct EffectfulCall {
    callee: String,
    handled: BTreeSet<String>,
}

#[derive(Default)]
struct FunctionEffects {
    performed: BTreeSet<String>,
    calls: Vec<EffectfulCall>,
}

struct EffectProgram {
    /// Operation name -> declaring effect.
    operations: BTreeMap<String, String>,
    /// Top-level function names; they take precedence over operations.
    functions: BTreeSet<String>,
    /// Top-level value bindings; they also hide operations.
    values: BTreeSet<String>,
}

impl EffectProgram {
    fn operation_effect(&self, name: &str, locals: &BTreeSet<String>) -> Option<&String> {
        if locals.contains(name) || self.functions.contains(name) || self.values.contains(name) {
            return None;
        }
        self.operations.get(name)
    }

    /// Walk `expr`, reporting each operation performed outside a handler and
    /// each call of a top-level function. Lambda bodies run where they are
    /// called, so they are left to runtime.
    fn visit_expr<'a>(
        &self,
        expr: &'a Expr,
        handled: &BTreeSet<String>,
        locals: &BTreeSet<String>,
        report: &mut dyn FnMut(&'a Expr, EffectEvent<'_>),
    ) {
        match &expr.kind {
            ExprKind::Lambda(_, _) => {}
            ExprKind::Handle {
                effect,
                handlers,
                body,
            } => {
                let mut inner = handled.clone();
                inner.insert(effect.clone());
                self.visit_expr(body, &inner, locals, report);
                for handler in handlers {
                    self.visit_expr(&handler.body, handled, locals, report);
                }
            }
            _ => {
                if let ExprKind::App(function, _) = &expr.kind {
                    if let ExprKind::Var(name) = &function.kind {
                        if let Some(effect) = self.operation_effect(name, locals) {
                            if !handled.contains(effect) {
                                report(
                                    expr,
                                    EffectEvent::Performs {
                                        effect,
                                        operation: name,
                                    },
                                );
                            }
                        } else if self.functions.contains(name) && !locals.contains(name) {
                            report(
                                expr,
                                EffectEvent::Calls {
                                    callee: name,
                                    handled,
                                },
                            );
                        }
                    }
                }
                visit_ast_expr_children(expr, &mut |child| match child {
                    AstChild::Expr(child) => self.visit_expr(child, handled, locals, report),
                    AstChild::Stmt(statement) => {
                        self.visit_stmt(statement, handled, locals, report)
                    }
                });
            }
        }
    }

    fn visit_stmt<'a>(
        &self,
        statement: &'a Stmt,
        handled: &BTreeSet<String>,
        locals: &BTreeSet<String>,
        report: &mut dyn FnMut(&'a Expr, EffectEvent<'_>),
    ) {
        // Nested declarations run when called, not where they appear.
        if matches!(statement, Stmt::Defn(_) | Stmt::TypeDecl(_) | Stmt::Rule(_)) {
            return;
        }
        visit_ast_stmt_children(statement, &mut |child| match child {
            AstChild::Expr(child) => self.visit_expr(child, handled, locals, report),
            AstChild::Stmt(child) => self.visit_stmt(child, handled, locals, report),
        });
    }
}

enum EffectEvent<'e> {
    Performs {
        effect: &'e str,
        operation: &'e str,
    },
    Calls {
        callee: &'e str,
        handled: &'e BTreeSet<String>,
    },
}

/// Every name a body can bind locally. Any of them may hide an operation.
fn body_local_names(expr: &Expr, names: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Lambda(params, _) => {
            names.extend(params.iter().map(|param| param.name.clone()));
        }
        ExprKind::Match(_, arms) => {
            for arm in arms {
                collect_pattern_names(&arm.pat, names);
            }
        }
        ExprKind::Handle { handlers, .. } => {
            for handler in handlers {
                names.extend(handler.params.iter().cloned());
            }
        }
        _ => {}
    }
    visit_ast_expr_children(expr, &mut |child| match child {
        AstChild::Expr(child) => body_local_names(child, names),
        AstChild::Stmt(statement) => statement_local_names(statement, names),
    });
}

fn statement_local_names(statement: &Stmt, names: &mut BTreeSet<String>) {
    match statement {
        Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
            collect_pattern_names(pattern, names);
        }
        Stmt::For(name, _, _) | Stmt::StreamBind(name, _) => {
            names.insert(name.clone());
        }
        Stmt::Defn(Defn::Fn { name, .. }) => {
            names.insert(name.clone());
        }
        _ => {}
    }
    visit_ast_stmt_children(statement, &mut |child| match child {
        AstChild::Expr(child) => body_local_names(child, names),
        AstChild::Stmt(child) => statement_local_names(child, names),
    });
}

impl TypeChecker {
    pub(super) fn check_unhandled_effects(&mut self, stmts: &[Stmt]) {
        let mut program = EffectProgram {
            operations: BTreeMap::new(),
            functions: BTreeSet::new(),
            values: BTreeSet::new(),
        };
        for statement in stmts {
            match statement {
                Stmt::TypeDecl(TypeDecl::EffectDecl { name, ops }) => {
                    // A builtin of the same name (such as actor `ask`) wins
                    // when no handler is active.
                    for (operation, _, _) in ops {
                        if !self.builtins.contains_key(builtin_canonical(operation)) {
                            program.operations.insert(operation.clone(), name.clone());
                        }
                    }
                }
                Stmt::Defn(Defn::Fn { name, .. }) => {
                    program.functions.insert(name.clone());
                }
                Stmt::Bind(pattern, _, _) | Stmt::MonadicBind(pattern, _, _) => {
                    collect_pattern_names(pattern, &mut program.values);
                }
                Stmt::StreamBind(name, _) => {
                    program.values.insert(name.clone());
                }
                _ => {}
            }
        }
        if program.operations.is_empty() {
            return;
        }

        // Effects each top-level function performs without handling them.
        let mut effects: BTreeMap<String, FunctionEffects> = BTreeMap::new();
        for statement in stmts {
            let Stmt::Defn(Defn::Fn {
                name, params, body, ..
            }) = statement
            else {
                continue;
            };
            let mut locals: BTreeSet<String> =
                params.iter().map(|param| param.name.clone()).collect();
            body_local_names(body, &mut locals);
            let mut summary = FunctionEffects::default();
            program.visit_expr(
                body,
                &BTreeSet::new(),
                &locals,
                &mut |_, event| match event {
                    EffectEvent::Performs { effect, .. } => {
                        summary.performed.insert(effect.to_string());
                    }
                    EffectEvent::Calls { callee, handled } => summary.calls.push(EffectfulCall {
                        callee: callee.to_string(),
                        handled: handled.clone(),
                    }),
                },
            );
            effects.insert(name.clone(), summary);
        }
        loop {
            let mut changed = false;
            let names: Vec<String> = effects.keys().cloned().collect();
            for name in names {
                let mut added = BTreeSet::new();
                for call in &effects[&name].calls {
                    if let Some(callee) = effects.get(&call.callee) {
                        added.extend(
                            callee
                                .performed
                                .iter()
                                .filter(|effect| !call.handled.contains(*effect))
                                .cloned(),
                        );
                    }
                }
                let summary = effects.get_mut(&name).expect("summarized function");
                for effect in added {
                    changed |= summary.performed.insert(effect);
                }
            }
            if !changed {
                break;
            }
        }

        let mut diagnostics = Vec::new();
        let top_level = BTreeSet::new();
        for statement in stmts {
            program.visit_stmt(
                statement,
                &BTreeSet::new(),
                &top_level,
                &mut |expr, event| match event {
                    EffectEvent::Performs { effect, operation } => {
                        diagnostics.push((expr, unhandled_effect_message(effect, operation)))
                    }
                    EffectEvent::Calls { callee, handled } => {
                        if let Some(effect) = effects.get(callee).and_then(|summary| {
                            summary
                                .performed
                                .iter()
                                .find(|effect| !handled.contains(*effect))
                        }) {
                            diagnostics.push((expr, unhandled_effect_message(effect, callee)));
                        }
                    }
                },
            );
        }
        for (expr, message) in diagnostics {
            self.error_at_expr(expr, message);
        }
    }
}
