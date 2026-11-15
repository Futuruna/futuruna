//! The names a declaration reads from outside itself.
//!
//! A name is read from outside when no parameter, rule head argument, lambda
//! parameter, match arm pattern or block binding binds it around the read.
//! A binding form this module does not know leaves its names free, so the
//! set only ever over-approximates: a calculation's runtime demand never
//! misses a binding it reads, and `runa check` never misses a `query` a
//! calculation reaches. Both use this one definition, so a parameter named
//! like a top-level binding is the parameter in each.

use super::*;

/// The names `statement` reads from outside itself; a member `m` of a
/// qualified import `module.m` counts as `m`.
pub(crate) fn statement_references(
    statement: &Stmt,
    qualified_modules: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut references = References {
        bound: Vec::new(),
        qualified_modules,
        out: BTreeSet::new(),
    };
    references.statement(statement);
    references.out
}

/// The names `expr` reads from outside itself, as [`statement_references`].
pub(crate) fn expression_references(
    expr: &Expr,
    qualified_modules: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut references = References {
        bound: Vec::new(),
        qualified_modules,
        out: BTreeSet::new(),
    };
    references.expr(expr);
    references.out
}

struct References<'m> {
    /// The names bound around the current read, innermost last.
    bound: Vec<String>,
    qualified_modules: &'m BTreeSet<String>,
    out: BTreeSet<String>,
}

impl References<'_> {
    fn is_bound(&self, name: &str) -> bool {
        self.bound.iter().any(|local| local == name)
    }

    fn statement(&mut self, statement: &Stmt) {
        let mark = self.bound.len();
        match statement {
            Stmt::Defn(Defn::Fn { params, body, .. }) => {
                self.bound
                    .extend(params.iter().map(|param| param.name.clone()));
                self.expr(body);
            }
            Stmt::Rule(
                rule @ (Rule::Clause { head, .. }
                | Rule::Default { head, .. }
                | Rule::Exception { head, .. }),
            ) => {
                if let ExprKind::App(_, arguments) = &head.kind {
                    for argument in arguments {
                        let argument =
                            typed_rule_head_argument(argument).map_or(argument, |(inner, _)| inner);
                        if let ExprKind::Var(name) = &argument.kind {
                            if name != "_" && !name.starts_with(char::is_uppercase) {
                                self.bound.push(name.clone());
                            }
                        }
                    }
                }
                visit_ast_rule_children(rule, &mut |child| self.child(child));
            }
            _ => visit_ast_stmt_children(statement, &mut |child| self.child(child)),
        }
        self.bound.truncate(mark);
    }

    fn child(&mut self, child: AstChild<'_>) {
        match child {
            AstChild::Expr(expr) => self.expr(expr),
            AstChild::Stmt(statement) => self.statement(statement),
        }
    }

    fn expr(&mut self, expr: &Expr) {
        let mark = self.bound.len();
        match &expr.kind {
            ExprKind::Var(name) => {
                if !self.is_bound(name) {
                    self.out.insert(name.clone());
                }
            }
            ExprKind::Field(base, member) => {
                if matches!(&base.kind, ExprKind::Var(module)
                    if self.qualified_modules.contains(module) && !self.is_bound(module))
                {
                    self.out.insert(member.clone());
                }
                self.expr(base);
            }
            ExprKind::Lambda(params, body) => {
                self.bound
                    .extend(params.iter().map(|param| param.name.clone()));
                self.expr(body);
            }
            ExprKind::Match(scrutinee, arms) => {
                self.expr(scrutinee);
                for arm in arms {
                    let arm_mark = self.bound.len();
                    Interpreter::collect_pattern_binding_names(&arm.pat, &mut self.bound);
                    if let Some(guard) = &arm.guard {
                        self.expr(guard);
                    }
                    self.expr(&arm.body);
                    self.bound.truncate(arm_mark);
                }
            }
            ExprKind::Block(statements) => {
                for statement in statements {
                    if let Stmt::Bind(pattern, _, value) = statement {
                        self.expr(value);
                        Interpreter::collect_pattern_binding_names(pattern, &mut self.bound);
                    } else {
                        self.statement(statement);
                    }
                }
            }
            _ => visit_ast_expr_children(expr, &mut |child| self.child(child)),
        }
        self.bound.truncate(mark);
    }
}
