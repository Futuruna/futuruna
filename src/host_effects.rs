//! Host effects: builtins that act on the machine running Futuruna (console
//! input and output, files, the environment, clocks, network, processes).
//!
//! Compile-time evaluation (`@ comptime`), `@ calculate` entries and
//! `runa audit` never perform host effects. The checker rejects host effects
//! reachable from those roots through local functions and rules, and the
//! interpreter refuses them at run time for anything the static walk cannot
//! see (for example imported helpers).

use super::*;

/// Canonical names of host-effect builtins, whether called as `name(...)`
/// or performed as `@ name(...)`.
pub fn is_host_effect_builtin(name: &str) -> bool {
    matches!(
        builtin_canonical(name),
        "print"
            | "input"
            | "write_file"
            | "append_file"
            | "read_file"
            | "file_exists"
            | "read_lines"
            | "env_var"
            | "time"
            | "now"
            | "random"
            | "sleep"
            | "http_get"
            | "http_post"
            | "http_serve"
            | "http_respond"
            | "http_request_path"
            | "http_request_method"
            | "http_request_body"
            | "process_run"
    )
}

/// The first effect reached from a root expression.
pub(crate) struct ReachedEffect {
    /// Canonical builtin name, or the `@` operation name.
    pub name: String,
    pub span: Span,
    /// The local function or rule that performs it, when not the root itself.
    pub via: Option<String>,
}

/// Local callables of a program (top-level functions and rules), excluding
/// the injected prelude, each summarized once: the first effect it performs
/// directly and the local callables it refers to.
pub(crate) struct LocalCallables<'a> {
    summaries: BTreeMap<&'a str, Summary<'a>>,
}

#[derive(Default)]
struct Summary<'a> {
    /// First host effect performed directly.
    host: Option<(String, Span)>,
    /// First `@` effect of any kind, or host call, performed directly.
    any: Option<(String, Span)>,
    callees: BTreeSet<&'a str>,
}

impl<'a> Summary<'a> {
    fn effect(&self, any_at_effect: bool) -> Option<&(String, Span)> {
        if any_at_effect {
            self.any.as_ref()
        } else {
            self.host.as_ref()
        }
    }

    fn record(&mut self, child: AstChild<'a>, known: &impl Fn(&str) -> Option<&'a str>) {
        let mut visit = |child: AstChild<'a>| {
            let AstChild::Expr(expr) = child else {
                return;
            };
            match &expr.kind {
                ExprKind::Effect(name, _) => {
                    let effect = (builtin_canonical(name).to_string(), expr.span);
                    if is_host_effect_builtin(name) && self.host.is_none() {
                        self.host = Some(effect.clone());
                    }
                    if self.any.is_none() {
                        self.any = Some(effect);
                    }
                }
                ExprKind::Var(name) => {
                    if let Some(callee) = known(name) {
                        self.callees.insert(callee);
                    }
                }
                // Only calls: a parameter such as `input` is not the builtin.
                ExprKind::App(function, _) => {
                    if let ExprKind::Var(name) = &function.kind {
                        if known(name).is_none() && is_host_effect_builtin(name) {
                            let effect = (builtin_canonical(name).to_string(), expr.span);
                            if self.host.is_none() {
                                self.host = Some(effect.clone());
                            }
                            if self.any.is_none() {
                                self.any = Some(effect);
                            }
                        }
                    }
                }
                _ => {}
            }
        };
        match child {
            AstChild::Expr(expr) => walk_ast_expr(expr, &mut visit),
            AstChild::Stmt(stmt) => walk_ast_stmt(stmt, &mut visit),
        }
    }
}

impl<'a> LocalCallables<'a> {
    pub(crate) fn new(stmts: &'a [Stmt]) -> Self {
        let mut bodies: BTreeMap<&'a str, Vec<&'a Stmt>> = BTreeMap::new();
        for stmt in user_statements(stmts) {
            let name = match stmt {
                Stmt::Defn(Defn::Fn { name, .. }) => Some(name.as_str()),
                Stmt::Rule(
                    Rule::Clause { head, .. }
                    | Rule::Default { head, .. }
                    | Rule::Exception { head, .. },
                ) => head_name(head),
                _ => None,
            };
            if let Some(name) = name {
                bodies.entry(name).or_default().push(stmt);
            }
        }
        let known = |name: &str| bodies.get_key_value(name).map(|(name, _)| *name);
        let summaries = bodies
            .iter()
            .map(|(name, statements)| {
                let mut summary = Summary::default();
                for statement in statements {
                    summary.record(AstChild::Stmt(statement), &known);
                }
                (*name, summary)
            })
            .collect();
        Self { summaries }
    }

    /// Find a host effect (or, with `any_at_effect`, any `@` effect) reachable
    /// from `roots` through calls to local callables.
    pub(crate) fn first_reachable_effect(
        &self,
        roots: &[AstChild<'a>],
        any_at_effect: bool,
    ) -> Option<ReachedEffect> {
        let known = |name: &str| self.summaries.get_key_value(name).map(|(name, _)| *name);
        let mut root = Summary::default();
        for child in roots {
            root.record(clone_child(child), &known);
        }
        if let Some((name, span)) = root.effect(any_at_effect) {
            return Some(ReachedEffect {
                name: name.clone(),
                span: *span,
                via: None,
            });
        }
        let mut visited: BTreeSet<&'a str> = BTreeSet::new();
        let mut queue: Vec<&'a str> = root.callees.into_iter().collect();
        while let Some(callee) = queue.pop() {
            if !visited.insert(callee) {
                continue;
            }
            let summary = &self.summaries[callee];
            if let Some((name, span)) = summary.effect(any_at_effect) {
                return Some(ReachedEffect {
                    name: name.clone(),
                    span: *span,
                    via: Some(callee.to_string()),
                });
            }
            queue.extend(summary.callees.iter().copied());
        }
        None
    }
}

fn user_statements(stmts: &[Stmt]) -> &[Stmt] {
    let user_start = stmts
        .iter()
        .position(|stmt| matches!(stmt, Stmt::PreludeBoundary))
        .map_or(0, |index| index + 1);
    &stmts[user_start..]
}

fn clone_child<'a>(child: &AstChild<'a>) -> AstChild<'a> {
    match child {
        AstChild::Expr(expr) => AstChild::Expr(expr),
        AstChild::Stmt(stmt) => AstChild::Stmt(stmt),
    }
}

fn head_name(head: &Expr) -> Option<&str> {
    match &head.kind {
        ExprKind::App(function, _) => head_name(function),
        ExprKind::Var(name) => Some(name.as_str()),
        _ => None,
    }
}

/// Describe where a reached effect is performed, for diagnostics.
pub(crate) fn reached_effect_location(effect: &ReachedEffect) -> String {
    match &effect.via {
        Some(via) => format!("`{}` (called through `{}`)", effect.name, via),
        None => format!("`{}`", effect.name),
    }
}

/// Located errors for `@ comptime` declarations that reach a host effect.
pub(crate) fn comptime_host_effect_diagnostics(stmts: &[Stmt]) -> Vec<Diagnostic> {
    let callables = LocalCallables::new(stmts);
    let mut diagnostics = Vec::new();
    let mut pending_comptime = false;
    for stmt in stmts {
        match stmt {
            Stmt::Annot(name, arguments) if name == "comptime" => {
                let roots: Vec<AstChild> = arguments.iter().map(AstChild::Expr).collect();
                if let Some(effect) = callables.first_reachable_effect(&roots, false) {
                    diagnostics.push(comptime_diagnostic(&effect));
                }
                pending_comptime = arguments.is_empty();
            }
            Stmt::Bind(_, _, expression) if pending_comptime => {
                pending_comptime = false;
                if let Some(effect) =
                    callables.first_reachable_effect(&[AstChild::Expr(expression)], false)
                {
                    diagnostics.push(comptime_diagnostic(&effect));
                }
            }
            _ => pending_comptime = false,
        }
    }
    diagnostics
}

fn comptime_diagnostic(effect: &ReachedEffect) -> Diagnostic {
    Diagnostic::error_at(
        effect.span,
        format!(
            "compile-time evaluation cannot perform the host effect {}; `@ comptime` values must be computed from the program alone",
            reached_effect_location(effect)
        ),
    )
}

/// Located errors for top-level bindings and rules that `runa audit` would
/// evaluate but that reach a host effect.
pub fn audit_host_effect_diagnostics(stmts: &[Stmt]) -> Vec<Diagnostic> {
    let callables = LocalCallables::new(stmts);
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut reported = BTreeSet::new();
    for stmt in user_statements(stmts) {
        let root = match stmt {
            Stmt::Rule(_) => AstChild::Stmt(stmt),
            Stmt::Bind(_, _, expression) | Stmt::MonadicBind(_, _, expression) => {
                AstChild::Expr(expression)
            }
            _ => continue,
        };
        if let Some(effect) = callables.first_reachable_effect(&[root], false) {
            if reported.insert((effect.span.start, effect.span.end)) {
                diagnostics.push(Diagnostic::error_at(
                    effect.span,
                    format!(
                        "`runa audit` cannot perform the host effect {}; an audit evaluates rules and bindings without running effects",
                        reached_effect_location(&effect)
                    ),
                ));
            }
        }
    }
    diagnostics
}
