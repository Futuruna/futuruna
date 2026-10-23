//! Clause reachability for `runa audit`.
//!
//! A rule family (scope, name, arity) dispatches its candidates in priority
//! order: exceptions, guarded defaults (`under`), ordinary clauses, then
//! unguarded defaults; inside a tier, the first applicable candidate in source
//! order wins. For every family declared in the audited source this module
//! asks Z3, through the `runa verify` SMT encoding of rule groups, whether a
//! candidate can ever be selected and whether two candidates of one tier can
//! both apply with different results. Guards outside that encoding are
//! reported as not analyzed, never approximated.
use super::*;
use std::collections::HashMap;

/// Per-query solver budget. A query that exceeds it is reported as
/// undecided, never as a finding.
const SOLVER_TIMEOUT_MS: u32 = 10_000;
/// The receiver and argument names the generated rule functions use.
const RECEIVER: &str = "__runa_rule_self";

fn argument_symbol(index: usize) -> String {
    format!("__runa_rule_arg_{}", index)
}

#[derive(Clone)]
pub(super) struct ClauseLocation {
    file: String,
    line: usize,
    tier: RuleDispatchTier,
    text: String,
}

impl ClauseLocation {
    fn display(&self) -> String {
        format!("{}:{}", self.file, self.line)
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "file": self.file,
            "line": self.line,
            "tier": tier_json_name(self.tier),
            "source": self.text,
        })
    }
}

fn tier_json_name(tier: RuleDispatchTier) -> &'static str {
    match tier {
        RuleDispatchTier::Exception => "exception",
        RuleDispatchTier::ConditionalDefault => "guarded_default",
        RuleDispatchTier::Clause => "clause",
        RuleDispatchTier::UnconditionalDefault => "unguarded_default",
    }
}

fn tier_label(tier: RuleDispatchTier) -> &'static str {
    match tier {
        RuleDispatchTier::Exception => "exception",
        RuleDispatchTier::ConditionalDefault => "guarded default",
        RuleDispatchTier::Clause => "ordinary clause",
        RuleDispatchTier::UnconditionalDefault => "unguarded default",
    }
}

pub(super) enum ClauseFindingKind {
    /// Whenever the guard holds, a higher-priority candidate applies first.
    /// `by` lists those candidates (an unsat core); it is empty when the
    /// solver could not name them.
    Shadowed { by: Vec<ClauseLocation> },
    /// The guard can never hold.
    UnsatisfiableGuard,
    /// Both candidates of one tier apply to some input and select different
    /// results; source order selects `with`.
    Overlap {
        with: ClauseLocation,
        witness: Option<Vec<(String, String)>>,
    },
}

pub(super) struct ClauseFinding {
    family: String,
    clause: ClauseLocation,
    kind: ClauseFindingKind,
}

pub(super) struct SkippedClause {
    family: String,
    clause: ClauseLocation,
    reason: String,
}

pub(super) enum ClauseReachabilityReport {
    NotRun {
        reason: String,
    },
    Ran {
        families: usize,
        clauses: usize,
        findings: Vec<ClauseFinding>,
        /// Clauses whose reachability was not decided.
        not_analyzed: Vec<SkippedClause>,
        /// Clauses whose reachability was decided but whose overlap with an
        /// earlier same-tier clause could not be checked.
        overlap_not_checked: Vec<SkippedClause>,
    },
}

impl ClauseReachabilityReport {
    pub(super) fn unreachable_count(&self) -> usize {
        self.count(|kind| !matches!(kind, ClauseFindingKind::Overlap { .. }))
    }

    pub(super) fn overlap_count(&self) -> usize {
        self.count(|kind| matches!(kind, ClauseFindingKind::Overlap { .. }))
    }

    fn count(&self, predicate: impl Fn(&ClauseFindingKind) -> bool) -> usize {
        match self {
            Self::NotRun { .. } => 0,
            Self::Ran { findings, .. } => findings
                .iter()
                .filter(|finding| predicate(&finding.kind))
                .count(),
        }
    }

    pub(super) fn print(&self) {
        println!("── Clause Reachability ──");
        println!();
        let (families, clauses, findings, not_analyzed, overlap_not_checked) = match self {
            Self::NotRun { reason } => {
                println!("  Not run: {}", reason);
                println!(
                    "  No clause was analyzed; unreachable clauses and overlapping guards are unknown."
                );
                println!();
                return;
            }
            Self::Ran {
                families,
                clauses,
                findings,
                not_analyzed,
                overlap_not_checked,
            } => (
                families,
                clauses,
                findings,
                not_analyzed,
                overlap_not_checked,
            ),
        };
        println!(
            "  {} rule famil{}, {} clause{}: {} analyzed, {} not analyzed",
            families,
            if *families == 1 { "y" } else { "ies" },
            clauses,
            if *clauses == 1 { "" } else { "s" },
            clauses - not_analyzed.len(),
            not_analyzed.len()
        );
        println!(
            "  Findings: {} unreachable clause, {} overlapping guards",
            self.unreachable_count(),
            self.overlap_count()
        );
        println!();
        for finding in findings {
            let clause = &finding.clause;
            match &finding.kind {
                ClauseFindingKind::Shadowed { by } => {
                    status_println!(
                        "  \x1b[33m!! Unreachable clause in {}: {} never fires\x1b[0m",
                        finding.family,
                        clause.display()
                    );
                    println!("     {}  {}", clause.display(), clause.text);
                    if by.is_empty() {
                        println!("     Every input satisfying its guard is taken by an earlier-priority clause.");
                    } else {
                        println!("     Every input satisfying its guard is taken first by:");
                        for shadow in by {
                            println!(
                                "       {} ({})  {}",
                                shadow.display(),
                                tier_label(shadow.tier),
                                shadow.text
                            );
                        }
                    }
                }
                ClauseFindingKind::UnsatisfiableGuard => {
                    status_println!(
                        "  \x1b[33m!! Unreachable clause in {}: {} never fires\x1b[0m",
                        finding.family,
                        clause.display()
                    );
                    println!("     {}  {}", clause.display(), clause.text);
                    println!("     Its guard is unsatisfiable: no input makes it hold.");
                }
                ClauseFindingKind::Overlap { with, witness } => {
                    status_println!(
                        "  \x1b[35m~~ Overlapping guards in {}: {} and {}\x1b[0m",
                        finding.family,
                        with.display(),
                        clause.display()
                    );
                    println!("     {}  {}", with.display(), with.text);
                    println!("     {}  {}", clause.display(), clause.text);
                    println!(
                        "     Both {}s apply to some input with different results; source order selects {}.",
                        tier_label(clause.tier),
                        with.display()
                    );
                    match witness {
                        Some(values) if values.is_empty() => {
                            println!("     Witness: every call")
                        }
                        Some(values) => println!(
                            "     Witness: {}",
                            values
                                .iter()
                                .map(|(name, value)| format!("{} = {}", name, value))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        None => println!("     Witness: unavailable"),
                    }
                }
            }
            println!();
        }
        if !not_analyzed.is_empty() {
            println!("  Not analyzed (reachability unknown):");
            for skipped in not_analyzed {
                println!(
                    "    - {} in {}: {}",
                    skipped.clause.display(),
                    skipped.family,
                    skipped.reason
                );
            }
            println!();
        }
        if !overlap_not_checked.is_empty() {
            println!("  Overlap with earlier same-tier clauses not checked:");
            for skipped in overlap_not_checked {
                println!(
                    "    - {} in {}: {}",
                    skipped.clause.display(),
                    skipped.family,
                    skipped.reason
                );
            }
            println!();
        }
        if findings.is_empty() {
            println!("  No unreachable clauses or overlapping guards among the analyzed clauses.");
            println!();
        }
    }

    pub(super) fn json(&self) -> serde_json::Value {
        let skipped_json = |skipped: &SkippedClause| {
            serde_json::json!({
                "family": skipped.family,
                "clause": skipped.clause.json(),
                "reason": skipped.reason,
            })
        };
        match self {
            Self::NotRun { reason } => serde_json::json!({
                "status": "not_run",
                "reason": reason,
            }),
            Self::Ran {
                families,
                clauses,
                findings,
                not_analyzed,
                overlap_not_checked,
            } => serde_json::json!({
                "status": "ran",
                "solver": "z3",
                "summary": {
                    "families": families,
                    "clauses": clauses,
                    "analyzed": clauses - not_analyzed.len(),
                    "not_analyzed": not_analyzed.len(),
                    "overlap_not_checked": overlap_not_checked.len(),
                    "unreachable_clauses": self.unreachable_count(),
                    "overlapping_guards": self.overlap_count(),
                },
                "findings": findings.iter().map(|finding| match &finding.kind {
                    ClauseFindingKind::Shadowed { by } => serde_json::json!({
                        "kind": "unreachable_clause",
                        "cause": "shadowed",
                        "family": finding.family,
                        "clause": finding.clause.json(),
                        "shadowed_by": by.iter().map(ClauseLocation::json).collect::<Vec<_>>(),
                    }),
                    ClauseFindingKind::UnsatisfiableGuard => serde_json::json!({
                        "kind": "unreachable_clause",
                        "cause": "unsatisfiable_guard",
                        "family": finding.family,
                        "clause": finding.clause.json(),
                        "shadowed_by": [],
                    }),
                    ClauseFindingKind::Overlap { with, witness } => serde_json::json!({
                        "kind": "overlapping_guards",
                        "family": finding.family,
                        "clause": finding.clause.json(),
                        "overlaps": with.json(),
                        "witness": witness.as_ref().map(|values| values
                            .iter()
                            .map(|(name, value)| (name.clone(), serde_json::Value::String(value.clone())))
                            .collect::<serde_json::Map<_, _>>()),
                    }),
                }).collect::<Vec<_>>(),
                "not_analyzed": not_analyzed.iter().map(skipped_json).collect::<Vec<_>>(),
                "overlap_not_checked": overlap_not_checked.iter().map(skipped_json).collect::<Vec<_>>(),
            }),
        }
    }
}

/// Line lookup for the files that contribute rule candidates.
struct SourceLines {
    /// Character offset at which each line starts.
    starts: Vec<usize>,
    lines: Vec<String>,
}

impl SourceLines {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        for (offset, character) in text.chars().enumerate() {
            if character == '\n' {
                starts.push(offset + 1);
            }
        }
        Self {
            starts,
            lines: text.lines().map(str::to_string).collect(),
        }
    }

    /// One-based line of a character offset and that line's text.
    fn locate(&self, offset: usize) -> (usize, String) {
        let index = self.starts.partition_point(|start| *start <= offset).max(1) - 1;
        let text = self
            .lines
            .get(index)
            .map(|line| line.trim().to_string())
            .unwrap_or_default();
        (index + 1, text)
    }
}

/// What one lowered expression needs from the SMT program.
struct Requirements {
    variables: Vec<String>,
    functions: BTreeSet<String>,
}

struct Encoder<'a, 'program, 'registry> {
    lowerer: &'a SmtRuleLowerer<'program, 'registry>,
    declarations: &'a SmtProgramDeclarations,
    function_map: BTreeMap<String, (Vec<Param>, Option<Ty>, Expr)>,
    fn_names: BTreeSet<String>,
    ctor_names: BTreeSet<String>,
    adts_by_name: BTreeMap<&'a str, &'a [Variant]>,
    /// Definedness predicate per function (`None` when always defined).
    function_guards: std::cell::RefCell<BTreeMap<String, Result<Option<String>, String>>>,
}

impl<'a, 'program, 'registry> Encoder<'a, 'program, 'registry> {
    fn new(
        lowerer: &'a SmtRuleLowerer<'program, 'registry>,
        declarations: &'a SmtProgramDeclarations,
    ) -> Self {
        let function_map: BTreeMap<String, (Vec<Param>, Option<Ty>, Expr)> = declarations
            .functions
            .iter()
            .map(|(name, params, ret_ty, body)| {
                (name.clone(), (params.clone(), ret_ty.clone(), body.clone()))
            })
            .collect();
        Self {
            lowerer,
            declarations,
            fn_names: function_map.keys().cloned().collect(),
            function_map,
            ctor_names: declarations.ctor_to_type.keys().cloned().collect(),
            adts_by_name: declarations
                .adts
                .iter()
                .map(|(name, variants)| (name.as_str(), variants.as_slice()))
                .collect(),
            function_guards: Default::default(),
        }
    }

    /// Check that `expression` lies in the translatable subset and collect
    /// the values and functions it depends on. `sorts` holds the free
    /// argument values a family declares; `unsorted` explains arguments whose
    /// type has no SMT representation.
    fn requirements(
        &self,
        expression: &Expr,
        sorts: &BTreeMap<String, Ty>,
        unsorted: &BTreeMap<String, String>,
    ) -> Result<Requirements, String> {
        let declarations = self.declarations;
        let truth = smt_bool(true);
        let mut extra = BTreeSet::new();
        let (variables, functions) = loop {
            let variables = ordered_smt_free_vars_with_extra(
                expression,
                &truth,
                &declarations.bindings,
                &self.fn_names,
                &self.ctor_names,
                &extra,
            );
            let functions = transitive_smt_function_deps(
                expression,
                &truth,
                &variables,
                &declarations.bindings,
                &self.function_map,
            );
            let discovered =
                smt_function_free_vars(&functions, &self.function_map, &self.ctor_names);
            if discovered.is_subset(&extra) {
                break (variables, functions);
            }
            extra.extend(discovered);
        };
        for variable in &variables {
            if let Some(reason) = unsorted.get(variable) {
                return Err(reason.clone());
            }
            if !declarations.bindings.contains_key(variable) && !sorts.contains_key(variable) {
                return Err(format!("`{}` has no value in the SMT encoding", variable));
            }
            if let Some(reason) = declarations.binding_lowering_errors.get(variable) {
                return Err(format!("binding `{}` {}", variable, reason));
            }
        }
        if let Some(reason) = smt_function_lowering_error_for_invariant(
            expression,
            &truth,
            &variables,
            &declarations.bindings,
            &functions,
            &self.function_map,
            &declarations.function_lowering_errors,
        ) {
            return Err(reason);
        }
        if let Some(reason) = smt_skip_reason_for_invariant(
            expression,
            &truth,
            &variables,
            &declarations.bindings,
            &self.function_map,
            &self.ctor_names,
        ) {
            return Err(reason.trim_start_matches("predicate ").to_string());
        }
        let expressions = std::iter::once(expression)
            .chain(
                variables
                    .iter()
                    .filter_map(|name| declarations.bindings.get(name)),
            )
            .chain(
                functions
                    .iter()
                    .filter_map(|name| self.function_map.get(name).map(|(_, _, body)| body)),
            );
        for expression in expressions.clone() {
            runa_verification_arithmetic::check_float_expression(expression)?;
        }
        let types = variables
            .iter()
            .filter_map(|name| {
                sorts.get(name).or_else(|| {
                    declarations
                        .binding_types
                        .get(name)
                        .and_then(Option::as_ref)
                })
            })
            .chain(
                functions
                    .iter()
                    .filter_map(|name| self.function_map.get(name))
                    .flat_map(|(params, ret, _)| {
                        params
                            .iter()
                            .filter_map(|param| param.ty.as_ref())
                            .chain(ret.iter())
                    }),
            );
        for ty in types {
            runa_verification_arithmetic::check_float_type(ty, &declarations.adts)?;
        }
        let guarded = self.guarded_functions(&functions)?;
        runa_verification_arithmetic::definedness(expression, &guarded)?;
        // Datatypes are declared only for the types a query mentions, and
        // generic types not at all. A constructor of an undeclared type, or a
        // field whose type is not represented, is outside the encoding.
        let variable_set = variables.iter().cloned().collect::<BTreeSet<_>>();
        let declared = self
            .datatypes(sorts, &variable_set, &functions, &[expression])
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut fields = BTreeSet::new();
        let mut constructors = BTreeSet::new();
        for expression in expressions {
            walk_ast_expr(expression, &mut |node| {
                let AstChild::Expr(expression) = node else {
                    return;
                };
                match &expression.kind {
                    ExprKind::Field(_, field) => {
                        fields.insert(field.clone());
                    }
                    ExprKind::Var(name) if declarations.ctor_to_type.contains_key(name) => {
                        constructors.insert(name.clone());
                    }
                    ExprKind::Match(_, arms) => {
                        for arm in arms {
                            visit_pattern_constructor_names(&arm.pat, &mut constructors);
                        }
                    }
                    _ => {}
                }
            });
        }
        for constructor in constructors {
            let Some(owner) = declarations.ctor_to_type.get(&constructor) else {
                continue;
            };
            if !declared.contains(owner.as_str()) {
                return Err(format!(
                    "constructor `{}` of generic type `{}` is not represented by the SMT encoding",
                    constructor, owner
                ));
            }
        }
        for field in fields {
            for owner in &declared {
                for variant in self.adts_by_name[owner] {
                    for field_declaration in &variant.fields {
                        if field_declaration.name == field
                            && !self
                                .lowerer
                                .rule_type_is_smt_representable(&field_declaration.ty)
                        {
                            return Err(format!(
                                "field `{}` of `{}` has type `{}`, which the SMT encoding does not represent",
                                field, owner, field_declaration.ty
                            ));
                        }
                    }
                }
            }
        }
        Ok(Requirements {
            variables,
            functions,
        })
    }

    /// Functions (among `functions` and their callees) whose evaluation can
    /// fail, after computing each definedness predicate once.
    fn guarded_functions(&self, functions: &BTreeSet<String>) -> Result<BTreeSet<String>, String> {
        let order = ordered_smt_functions(functions, &self.function_map)?;
        let mut cache = self.function_guards.borrow_mut();
        for name in &order {
            if cache.contains_key(name) {
                continue;
            }
            let guarded: BTreeSet<String> = cache
                .iter()
                .filter(|(_, guard)| matches!(guard, Ok(Some(_))))
                .map(|(name, _)| name.clone())
                .collect();
            let (_, _, body) = &self.function_map[name];
            let guard = runa_verification_arithmetic::definedness(body, &guarded)
                .map(|guard| (guard != "true").then_some(guard));
            cache.insert(name.clone(), guard);
        }
        let mut guarded = BTreeSet::new();
        for name in &order {
            match &cache[name] {
                Ok(Some(_)) => {
                    guarded.insert(name.clone());
                }
                Ok(None) => {}
                Err(reason) => return Err(format!("function `{}` {}", name, reason)),
            }
        }
        Ok(guarded)
    }

    fn definedness(&self, expression: &Expr, functions: &BTreeSet<String>) -> String {
        let guarded = self
            .guarded_functions(functions)
            .expect("requirements checked function definedness");
        runa_verification_arithmetic::definedness(expression, &guarded)
            .expect("requirements checked expression definedness")
    }

    /// Datatypes the query mentions, dependencies first.
    fn datatypes(
        &self,
        sorts: &BTreeMap<String, Ty>,
        variables: &BTreeSet<String>,
        functions: &BTreeSet<String>,
        expressions: &[&Expr],
    ) -> Vec<&'a str> {
        let mut roots = BTreeSet::new();
        for ty in sorts.values() {
            visit_type_names(ty, &mut roots);
        }
        let mut constructor_sources: Vec<&Expr> = expressions.to_vec();
        for variable in variables {
            if let Some(Some(ty)) = self.declarations.binding_types.get(variable) {
                visit_type_names(ty, &mut roots);
            } else if let Some(type_name) = self.lowerer.binding_types.get(variable) {
                if let Ok(ty) = parse_type_annotation(type_name) {
                    visit_type_names(&ty, &mut roots);
                }
            }
            if let Some(value) = self.declarations.bindings.get(variable) {
                constructor_sources.push(value);
            }
        }
        for function in functions {
            if let Some((params, ret, body)) = self.function_map.get(function) {
                for ty in params
                    .iter()
                    .filter_map(|param| param.ty.as_ref())
                    .chain(ret)
                {
                    visit_type_names(ty, &mut roots);
                }
                constructor_sources.push(body);
            }
        }
        for expression in constructor_sources {
            walk_ast_expr(expression, &mut |node| {
                let AstChild::Expr(expression) = node else {
                    return;
                };
                match &expression.kind {
                    ExprKind::Var(name) => {
                        if let Some(owner) = self.declarations.ctor_to_type.get(name) {
                            roots.insert(owner.clone());
                        }
                    }
                    ExprKind::Match(_, arms) => {
                        for arm in arms {
                            visit_pattern_constructors(
                                &arm.pat,
                                &self.declarations.ctor_to_type,
                                &mut roots,
                            );
                        }
                    }
                    _ => {}
                }
            });
        }

        let mut ordered = Vec::new();
        let mut visited = BTreeSet::new();
        for root in roots {
            self.order_datatype(&root, &mut visited, &mut ordered);
        }
        ordered
    }

    fn order_datatype(
        &self,
        name: &str,
        visited: &mut BTreeSet<String>,
        output: &mut Vec<&'a str>,
    ) {
        let Some((&owner, variants)) = self.adts_by_name.get_key_value(name) else {
            return;
        };
        if self
            .lowerer
            .type_parameters_by_owner
            .get(name)
            .is_some_and(|parameters| !parameters.is_empty())
            || !visited.insert(name.to_string())
        {
            return;
        }
        let mut dependencies = BTreeSet::new();
        for variant in variants.iter() {
            for field in &variant.fields {
                visit_type_names(&field.ty, &mut dependencies);
            }
        }
        for dependency in dependencies {
            self.order_datatype(&dependency, visited, output);
        }
        output.push(owner);
    }

    /// Constraints that keep every Int inside a value in the 64-bit range.
    fn value_domain(&self, ty: &Ty, value: &str, depth: usize) -> String {
        match ty {
            Ty::Name(name) if name == "Int" => runa_verification_arithmetic::int_domain(value),
            Ty::Name(name) if depth < 4 => {
                let Some(variants) = self.adts_by_name.get(name.as_str()) else {
                    return "true".into();
                };
                let mut constraints = Vec::new();
                for variant in variants.iter() {
                    let fields = variant
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            let selector = if variant.positional {
                                format!("{}_f{}", variant.name, index)
                            } else {
                                field.name.clone()
                            };
                            self.value_domain(
                                &field.ty,
                                &format!("({} {})", smt_symbol(&selector), value),
                                depth + 1,
                            )
                        })
                        .collect::<Vec<_>>();
                    let fields = runa_verification_arithmetic::and(fields);
                    if fields != "true" {
                        constraints.push(if variants.len() == 1 {
                            fields
                        } else {
                            format!(
                                "(=> ((_ is {}) {}) {})",
                                smt_symbol(&variant.name),
                                value,
                                fields
                            )
                        });
                    }
                }
                runa_verification_arithmetic::and(constraints)
            }
            _ => "true".into(),
        }
    }

    fn binding_sort(&self, name: &str) -> String {
        let bound = &self.declarations.bindings[name];
        if let Some(Some(ty)) = self.declarations.binding_types.get(name) {
            ty_to_smt_sort(ty)
        } else if let Some(type_name) = self.lowerer.binding_types.get(name) {
            smt_type_name_to_sort(type_name)
        } else {
            infer_smt_sort_adts(bound, &self.declarations.ctor_to_type)
        }
    }

    /// Declarations shared by every query of one family.
    fn preamble(
        &self,
        sorts: &BTreeMap<String, Ty>,
        variables: &BTreeSet<String>,
        functions: &BTreeSet<String>,
        expressions: &[&Expr],
    ) -> String {
        let mut smt = String::new();
        smt.push_str("; Auto-generated by runa audit (clause reachability)\n");
        smt.push_str("(set-option :produce-unsat-cores true)\n");
        smt.push_str(&format!("(set-option :timeout {})\n", SOLVER_TIMEOUT_MS));
        smt.push_str("(set-logic ALL)\n");
        for name in self.datatypes(sorts, variables, functions, expressions) {
            smt.push_str(&emit_z3_datatype(name, self.adts_by_name[name]));
            smt.push('\n');
        }
        for variable in variables {
            if let Some(ty) = sorts.get(variable) {
                let symbol = smt_value_symbol(variable);
                smt.push_str(&format!(
                    "(declare-const {} {})\n",
                    symbol,
                    ty_to_smt_sort(ty)
                ));
                let domain = self.value_domain(ty, &symbol, 0);
                if domain != "true" {
                    smt.push_str(&format!("(assert {})\n", domain));
                }
            } else {
                smt.push_str(&format!(
                    "(declare-const {} {})\n",
                    smt_value_symbol(variable),
                    self.binding_sort(variable)
                ));
            }
        }
        let order = ordered_smt_functions(functions, &self.function_map)
            .expect("requirements checked function order");
        let cache = self.function_guards.borrow();
        for name in order {
            let (params, ret_ty, body) = &self.function_map[&name];
            let parameters = params
                .iter()
                .map(|param| {
                    let sort = param
                        .ty
                        .as_ref()
                        .map(ty_to_smt_sort)
                        .unwrap_or_else(|| "Int".into());
                    format!("({} {})", smt_value_symbol(&param.name), sort)
                })
                .collect::<Vec<_>>()
                .join(" ");
            let ret_sort = ret_ty
                .as_ref()
                .map(ty_to_smt_sort)
                .unwrap_or_else(|| "Bool".into());
            smt.push_str(&format!(
                "(define-fun {} ({}) {} {})\n",
                smt_symbol(&name),
                parameters,
                ret_sort,
                expr_to_smt(body)
            ));
            if let Some(Ok(Some(guard))) = cache.get(&name) {
                smt.push_str(&format!(
                    "(define-fun {} ({}) Bool {})\n",
                    runa_verification_arithmetic::guard_name(&name),
                    parameters,
                    guard
                ));
            }
        }
        for variable in variables {
            if let Some(bound) = self.declarations.bindings.get(variable) {
                smt.push_str(&format!(
                    "(assert (= {} {}))\n",
                    smt_value_symbol(variable),
                    expr_to_smt(bound)
                ));
            }
        }
        smt
    }
}

fn visit_type_names(ty: &Ty, output: &mut BTreeSet<String>) {
    match ty {
        Ty::Name(name) => {
            output.insert(name.clone());
        }
        Ty::App(constructor, arguments) => {
            visit_type_names(constructor, output);
            for argument in arguments {
                visit_type_names(argument, output);
            }
        }
        Ty::Optional(inner) | Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
            visit_type_names(inner, output)
        }
        Ty::Arrow(left, right) => {
            visit_type_names(left, output);
            visit_type_names(right, output);
        }
        Ty::Unit | Ty::Var(_) | Ty::Hole => {}
    }
}

fn visit_pattern_constructors(
    pattern: &Pat,
    ctor_to_type: &BTreeMap<String, String>,
    output: &mut BTreeSet<String>,
) {
    match pattern {
        Pat::Con(name, patterns) => {
            if let Some(owner) = ctor_to_type.get(name) {
                output.insert(owner.clone());
            }
            for pattern in patterns {
                visit_pattern_constructors(pattern, ctor_to_type, output);
            }
        }
        Pat::NamedCon(name, patterns) => {
            if let Some(owner) = ctor_to_type.get(name) {
                output.insert(owner.clone());
            }
            for (_, pattern) in patterns {
                visit_pattern_constructors(pattern, ctor_to_type, output);
            }
        }
        Pat::As(inner, _) => visit_pattern_constructors(inner, ctor_to_type, output),
        Pat::Var(name) => {
            if let Some(owner) = ctor_to_type.get(name) {
                output.insert(owner.clone());
            }
        }
        Pat::Wild | Pat::Lit(_) => {}
    }
}

/// Names a pattern may test as constructors (nullary ones appear as `Var`).
fn visit_pattern_constructor_names(pattern: &Pat, output: &mut BTreeSet<String>) {
    match pattern {
        Pat::Con(name, patterns) => {
            output.insert(name.clone());
            for pattern in patterns {
                visit_pattern_constructor_names(pattern, output);
            }
        }
        Pat::NamedCon(name, patterns) => {
            output.insert(name.clone());
            for (_, pattern) in patterns {
                visit_pattern_constructor_names(pattern, output);
            }
        }
        Pat::As(inner, _) => visit_pattern_constructor_names(inner, output),
        Pat::Var(name) => {
            output.insert(name.clone());
        }
        Pat::Wild | Pat::Lit(_) => {}
    }
}

/// A candidate after lowering for the audit.
struct Candidate {
    location: ClauseLocation,
    tier: RuleDispatchTier,
    /// Lowered guard with its requirements, or why it is not translatable.
    guard: Result<(Expr, Requirements), String>,
    /// Lowered selected value with its requirements, or why not.
    value: Result<(Expr, Requirements), String>,
}

#[derive(Clone, Copy)]
enum Answer {
    Sat,
    Unsat,
    Unknown,
}

enum FamilyError {
    SolverMissing(String),
}

/// Ask each assertion in its own `push`/`pop` scope after `prefix`. The inner
/// error is a solver failure of this script.
fn decide(prefix: &str, assertions: &[String]) -> Result<Result<Vec<Answer>, String>, FamilyError> {
    let mut script = String::from(prefix);
    for assertion in assertions {
        script.push_str("(push 1)\n(assert ");
        script.push_str(assertion);
        script.push_str(")\n(check-sat)\n(pop 1)\n");
    }
    Ok(run_solver(&script)?.and_then(|output| {
        let answers = output
            .split_whitespace()
            .map(|answer| match answer {
                "sat" => Some(Answer::Sat),
                "unsat" => Some(Answer::Unsat),
                "unknown" => Some(Answer::Unknown),
                _ => None,
            })
            .collect::<Option<Vec<_>>>();
        match answers {
            Some(answers) if answers.len() == assertions.len() => Ok(answers),
            _ => Err(format!("Z3 returned unexpected output: {}", output.trim())),
        }
    }))
}

/// SMT names of one family's per-candidate definitions.
fn guard_name(index: usize) -> String {
    format!("runa_guard_{}", index)
}

fn guard_defined_name(index: usize) -> String {
    format!("runa_guard_defined_{}", index)
}

/// Every translatable guard before the candidate is false.
fn none_before_name(index: usize) -> String {
    format!("runa_none_before_{}", index)
}

/// Dispatch evaluates the candidate and selects it: every earlier guard is
/// defined and false, and its own guard is defined and true.
fn fires_name(index: usize) -> String {
    format!("runa_fires_{}", index)
}

/// Analyze every rule family with a candidate declared in `filename`.
pub(super) fn analyze_clause_reachability(
    statements: &[Stmt],
    filename: &str,
    source: &str,
) -> ClauseReachabilityReport {
    let not_run = |reason: String| ClauseReachabilityReport::NotRun { reason };
    let resolved = match resolve_smt_program(statements, filename) {
        Ok(program) => program,
        Err(error) => return not_run(error),
    };
    let symbol_index = match SmtSymbolIndex::from_program(&resolved) {
        Ok(index) => index,
        Err(error) => return not_run(error),
    };
    let (all_stmts, origins) = match lower_smt_namespaces(&resolved, &symbol_index) {
        Ok(lowered) => lowered,
        Err(error) => return not_run(error),
    };
    let type_artifacts = TypeChecker::check_with_artifacts(&all_stmts, None, "");
    if let Some(diagnostic) = type_artifacts.diagnostics.first() {
        return not_run(format!(
            "the SMT encoding of this program does not type-check: {}",
            diagnostic.message
        ));
    }
    let registry = RuleDispatchRegistry::from_statements(&all_stmts, &type_artifacts);
    let lowerer = SmtRuleLowerer::new(&all_stmts, &registry, symbol_index.rule_identities.clone());
    let declarations = SmtProgramDeclarations::collect(&all_stmts, &lowerer);
    let encoder = Encoder::new(&lowerer, &declarations);

    let mut rule_origins: HashMap<*const Rule, &Rc<str>> = HashMap::new();
    for (statement, origin) in all_stmts.iter().zip(&origins) {
        match statement {
            Stmt::Rule(rule) => {
                rule_origins.insert(rule as *const Rule, origin);
            }
            Stmt::TypeDecl(TypeDecl::RuleScope { body, .. }) => {
                for scoped in body {
                    if let Stmt::Rule(rule) = scoped {
                        rule_origins.insert(rule as *const Rule, origin);
                    }
                }
            }
            _ => {}
        }
    }
    let mut sources: BTreeMap<String, Option<SourceLines>> = BTreeMap::new();
    sources.insert(filename.to_string(), Some(SourceLines::new(source)));

    let mut families = 0;
    let mut clauses = 0;
    let mut findings = Vec::new();
    let mut not_analyzed = Vec::new();
    let mut overlap_not_checked = Vec::new();
    for group in registry.groups.values() {
        let candidates = group.plan.candidates().collect::<Vec<_>>();
        let from_audited_source = candidates.iter().any(|candidate| {
            rule_origins
                .get(&(candidate.rule as *const Rule))
                .is_some_and(|origin| origin.as_ref() == filename)
        });
        if !from_audited_source {
            continue;
        }
        let family = family_name(&group.key);
        let mut locate = |candidate: &RuleDispatchCandidate| -> ClauseLocation {
            let file = rule_origins
                .get(&(candidate.rule as *const Rule))
                .map(|origin| origin.to_string())
                .unwrap_or_else(|| filename.to_string());
            let lines = sources.entry(file.clone()).or_insert_with(|| {
                std::fs::read_to_string(&file)
                    .ok()
                    .map(|text| SourceLines::new(&text))
            });
            let (line, text) = lines
                .as_ref()
                .map(|lines| lines.locate(candidate.source_span.start))
                .unwrap_or((0, String::new()));
            ClauseLocation {
                file,
                line,
                tier: candidate
                    .rule
                    .dispatch_tier()
                    .unwrap_or(RuleDispatchTier::Clause),
                text,
            }
        };
        let signature = family_signature(&encoder, group);
        let lowered = lower_family(&encoder, group, &signature, &candidates, &mut locate);
        if lowered.is_empty() {
            continue;
        }
        families += 1;
        clauses += lowered.len();
        match analyze_family(&family, &signature, &encoder, &lowered) {
            Ok(outcome) => {
                findings.extend(outcome.findings);
                not_analyzed.extend(outcome.not_analyzed);
                overlap_not_checked.extend(outcome.overlap_not_checked);
            }
            Err(FamilyError::SolverMissing(reason)) => return not_run(reason),
        }
    }
    findings.sort_by(|left, right| {
        (&left.clause.file, left.clause.line).cmp(&(&right.clause.file, right.clause.line))
    });
    ClauseReachabilityReport::Ran {
        families,
        clauses,
        findings,
        not_analyzed,
        overlap_not_checked,
    }
}

fn family_name(key: &RuleDispatchKey) -> String {
    match &key.scope {
        Some(scope) => format!("{}.{}/{}", scope, key.name, key.arity),
        None => format!("{}/{}", key.name, key.arity),
    }
}

/// The argument values a family's queries range over.
struct FamilySignature {
    arguments: Vec<Expr>,
    receiver: Option<Expr>,
    sorts: BTreeMap<String, Ty>,
    unsorted: BTreeMap<String, String>,
    /// Display name of each declared value, in witness order.
    witness_names: Vec<(String, String)>,
}

fn family_signature(encoder: &Encoder, group: &RuleDispatchGroup) -> FamilySignature {
    let mut sorts = BTreeMap::new();
    let mut unsorted = BTreeMap::new();
    let mut witness_names = Vec::new();
    let receiver = group.key.scope.as_ref().map(|scope| {
        sorts.insert(RECEIVER.to_string(), Ty::Name(scope.clone()));
        witness_names.push((RECEIVER.to_string(), "self".to_string()));
        Expr::unspanned(ExprKind::Var(RECEIVER.to_string()))
    });
    let arguments = group
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let symbol = argument_symbol(index);
            let display = parameter
                .name
                .clone()
                .unwrap_or_else(|| format!("argument {}", index + 1));
            match parameter.ty.as_deref().map(parse_type_annotation) {
                Some(Ok(ty)) if encoder.lowerer.rule_type_is_smt_representable(&ty) => {
                    sorts.insert(symbol.clone(), ty);
                    witness_names.push((symbol.clone(), display));
                }
                Some(Ok(ty)) => {
                    unsorted.insert(
                        symbol.clone(),
                        format!(
                            "parameter `{}` has type `{}`, which the SMT encoding does not represent",
                            display, ty
                        ),
                    );
                }
                Some(Err(error)) => {
                    unsorted.insert(
                        symbol.clone(),
                        format!("parameter `{}` has an invalid type: {}", display, error),
                    );
                }
                None => {
                    unsorted.insert(
                        symbol.clone(),
                        format!("parameter `{}` has no inferred type", display),
                    );
                }
            }
            Expr::unspanned(ExprKind::Var(symbol))
        })
        .collect();
    FamilySignature {
        arguments,
        receiver,
        sorts,
        unsorted,
        witness_names,
    }
}

/// Lower each candidate of a family. Boolean clauses whose body is the
/// literal `false` only mark a matched head (so the predicate answers false
/// instead of missing); they select nothing and are not audited.
fn lower_family(
    encoder: &Encoder,
    group: &RuleDispatchGroup,
    signature: &FamilySignature,
    candidates: &[RuleDispatchCandidate],
    locate: &mut impl FnMut(&RuleDispatchCandidate) -> ClauseLocation,
) -> Vec<Candidate> {
    let environment = SmtLoweringEnv {
        types: signature
            .sorts
            .iter()
            .map(|(name, ty)| (name.clone(), ty.to_string()))
            .collect(),
        rule_calls_as_symbols: true,
        ..SmtLoweringEnv::default()
    };
    let return_is_bool = group.return_type.as_deref() == Some("Bool");
    let mut lowered = Vec::new();
    for candidate in candidates {
        if return_is_bool
            && matches!(
                candidate.rule,
                Rule::Clause {
                    body: Some(Expr {
                        kind: ExprKind::Lit(Literal::Bool(false)),
                        ..
                    }),
                    ..
                }
            )
        {
            continue;
        }
        let Some(tier) = candidate.rule.dispatch_tier() else {
            continue;
        };
        let applicability = encoder.lowerer.lower_rule_candidate_applicability(
            group,
            candidate.rule,
            &signature.arguments,
            signature.receiver.as_ref(),
            &environment,
            &mut Vec::new(),
        );
        let (guard, value) = match applicability {
            Ok(None) => continue,
            Ok(Some(lowered_candidate)) => {
                let guard = encoder
                    .requirements(
                        &lowered_candidate.applicability,
                        &signature.sorts,
                        &signature.unsorted,
                    )
                    .map(|requirements| (lowered_candidate.applicability.clone(), requirements));
                let value = encoder
                    .lowerer
                    .lower_rule_candidate_value(&lowered_candidate, &mut Vec::new())
                    .and_then(|value| {
                        encoder
                            .requirements(&value, &signature.sorts, &signature.unsorted)
                            .map(|requirements| (value, requirements))
                    });
                (guard, value)
            }
            Err(error) => (Err(error.clone()), Err(error)),
        };
        lowered.push(Candidate {
            location: locate(candidate),
            tier,
            guard,
            value,
        });
    }
    lowered
}

struct FamilyOutcome {
    findings: Vec<ClauseFinding>,
    not_analyzed: Vec<SkippedClause>,
    overlap_not_checked: Vec<SkippedClause>,
}

fn analyze_family(
    family: &str,
    signature: &FamilySignature,
    encoder: &Encoder,
    candidates: &[Candidate],
) -> Result<FamilyOutcome, FamilyError> {
    let mut outcome = FamilyOutcome {
        findings: Vec::new(),
        not_analyzed: Vec::new(),
        overlap_not_checked: Vec::new(),
    };
    let skip = |index: usize, reason: String| SkippedClause {
        family: family.to_string(),
        clause: candidates[index].location.clone(),
        reason,
    };
    let count = candidates.len();
    let guards: Vec<Option<&(Expr, Requirements)>> = candidates
        .iter()
        .map(|candidate| candidate.guard.as_ref().ok())
        .collect();
    for (index, candidate) in candidates.iter().enumerate() {
        if let Err(reason) = &candidate.guard {
            outcome
                .not_analyzed
                .push(skip(index, format!("guard not translatable: {}", reason)));
        }
    }
    let analyzed: Vec<usize> = (0..count)
        .filter(|index| guards[*index].is_some())
        .collect();
    if analyzed.is_empty() {
        return Ok(outcome);
    }

    // Everything the queries mention, declared once per family. Arguments are
    // declared even when no guard mentions them, so a witness names each one.
    let mut variables = BTreeSet::new();
    let mut functions = BTreeSet::new();
    let mut expressions = Vec::new();
    for candidate in candidates {
        for (expression, requirements) in [candidate.guard.as_ref(), candidate.value.as_ref()]
            .into_iter()
            .flatten()
        {
            variables.extend(requirements.variables.iter().cloned());
            functions.extend(requirements.functions.iter().cloned());
            expressions.push(expression);
        }
    }
    variables.extend(signature.sorts.keys().cloned());
    let mut prefix = encoder.preamble(&signature.sorts, &variables, &functions, &expressions);

    // One definition per candidate, in dispatch order, so every question
    // below is a constant-size formula over these names.
    let mut none_before = String::from("true");
    let mut reach = Some(String::from("true"));
    let mut fires_defined = vec![false; count];
    for index in 0..count {
        prefix.push_str(&format!(
            "(define-fun {} () Bool {})\n",
            none_before_name(index),
            none_before
        ));
        none_before = none_before_name(index);
        let Some((expression, requirements)) = guards[index] else {
            // Dispatch evaluates this guard before every later candidate;
            // without it, whether a later candidate is selected is unknown.
            reach = None;
            continue;
        };
        let guard = guard_name(index);
        let defined = guard_defined_name(index);
        prefix.push_str(&format!(
            "(define-fun {} () Bool {})\n(define-fun {} () Bool {})\n",
            guard,
            expr_to_smt(expression),
            defined,
            encoder.definedness(expression, &requirements.functions)
        ));
        if let Some(reached) = &reach {
            let next = format!("runa_reach_{}", index + 1);
            prefix.push_str(&format!(
                "(define-fun {} () Bool (and {} {} {}))\n(define-fun {} () Bool (and {} {} (not {})))\n",
                fires_name(index),
                reached,
                defined,
                guard,
                next,
                reached,
                defined,
                guard
            ));
            fires_defined[index] = true;
            reach = Some(next);
        }
        none_before = format!("(and {} (not {}))", none_before_name(index), guard);
    }
    let values: Vec<Option<(String, String)>> = candidates
        .iter()
        .map(|candidate| {
            candidate
                .value
                .as_ref()
                .ok()
                .map(|(expression, requirements)| {
                    (
                        expr_to_smt(expression),
                        encoder.definedness(expression, &requirements.functions),
                    )
                })
        })
        .collect();
    let applies =
        |index: usize| -> String { format!("{} {}", guard_defined_name(index), guard_name(index)) };
    // `earlier` is selected while `later` applies and selects another value.
    let overlap_assertion = |earlier: usize, later: usize| -> String {
        let (earlier_value, earlier_defined) = values[earlier].as_ref().expect("checked value");
        let (later_value, later_defined) = values[later].as_ref().expect("checked value");
        format!(
            "(and {} {} {} {} (not (= {} {})))",
            fires_name(earlier),
            earlier_defined,
            applies(later),
            later_defined,
            earlier_value,
            later_value
        )
    };

    // Which earlier candidates of the same tier each candidate can be
    // compared with; anything else is recorded as not checked.
    let mut comparable: Vec<Vec<usize>> = vec![Vec::new(); count];
    for &later in &analyzed {
        let mut reason = None;
        for earlier in 0..later {
            if candidates[earlier].tier != candidates[later].tier {
                continue;
            }
            let unchecked = if !fires_defined[earlier] {
                let blocker = (0..=earlier)
                    .find(|k| guards[*k].is_none())
                    .expect("an untranslatable guard precedes");
                Some(format!(
                    "the guard of {} is not translatable",
                    candidates[blocker].location.display()
                ))
            } else if let Err(error) = &candidates[earlier].value {
                Some(format!(
                    "the result of {} is not translatable: {}",
                    candidates[earlier].location.display(),
                    error
                ))
            } else if let Err(error) = &candidates[later].value {
                Some(format!("its result is not translatable: {}", error))
            } else {
                None
            };
            match unchecked {
                Some(why) => {
                    reason.get_or_insert(why);
                }
                None => comparable[later].push(earlier),
            }
        }
        if let Some(reason) = reason {
            outcome.overlap_not_checked.push(skip(later, reason));
        }
    }

    // Phase 1: can each candidate fire, can its guard hold, and can it apply
    // where an earlier candidate of its tier selects a different value?
    let mut assertions = Vec::new();
    let mut fires_at = vec![usize::MAX; count];
    let mut holds_at = vec![usize::MAX; count];
    let mut overlaps_at = vec![None; count];
    for &index in &analyzed {
        fires_at[index] = assertions.len();
        assertions.push(format!(
            "(and {} {})",
            applies(index),
            none_before_name(index)
        ));
        holds_at[index] = assertions.len();
        assertions.push(format!("(and {})", applies(index)));
        if !comparable[index].is_empty() {
            overlaps_at[index] = Some(assertions.len());
            assertions.push(format!(
                "(or {})",
                comparable[index]
                    .iter()
                    .map(|earlier| overlap_assertion(*earlier, index))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }
    let answers = match decide(&prefix, &assertions)? {
        Ok(answers) => answers,
        Err(error) => {
            record_solver_failure(&mut outcome, &analyzed, &skip, &error);
            return Ok(outcome);
        }
    };

    let mut unreachable = vec![false; count];
    let mut shadowed = Vec::new();
    for &index in &analyzed {
        match (answers[fires_at[index]], answers[holds_at[index]]) {
            (Answer::Unsat, Answer::Unsat) => {
                unreachable[index] = true;
                outcome.findings.push(ClauseFinding {
                    family: family.to_string(),
                    clause: candidates[index].location.clone(),
                    kind: ClauseFindingKind::UnsatisfiableGuard,
                });
            }
            (Answer::Unsat, _) => {
                unreachable[index] = true;
                shadowed.push(index);
            }
            (Answer::Sat, _) => {}
            (Answer::Unknown, _) => outcome.not_analyzed.push(skip(
                index,
                "Z3 could not decide whether it fires (unknown or timeout)".into(),
            )),
        }
    }

    // Phase 2: for each candidate that overlaps some earlier one, find which
    // ones, and drop pairs where the earlier candidate is a special case of
    // the later (specific before general is ordinary first-match refinement).
    let mut pairs = Vec::new();
    for &later in &analyzed {
        let Some(position) = overlaps_at[later] else {
            continue;
        };
        if unreachable[later] {
            continue;
        }
        match answers[position] {
            Answer::Sat => pairs.extend(
                comparable[later]
                    .iter()
                    .filter(|earlier| !unreachable[**earlier])
                    .map(|earlier| (*earlier, later)),
            ),
            Answer::Unsat => {}
            Answer::Unknown => outcome.overlap_not_checked.push(skip(
                later,
                "Z3 could not decide whether it overlaps an earlier clause (unknown or timeout)"
                    .into(),
            )),
        }
    }
    let mut overlaps = Vec::new();
    if !pairs.is_empty() {
        let mut pair_assertions = Vec::with_capacity(pairs.len() * 2);
        for &(earlier, later) in &pairs {
            pair_assertions.push(overlap_assertion(earlier, later));
            pair_assertions.push(format!(
                "(and {} {} (not {}))",
                fires_name(earlier),
                guard_defined_name(later),
                guard_name(later)
            ));
        }
        match decide(&prefix, &pair_assertions)? {
            Ok(answers) => {
                for (pair, verdict) in pairs.iter().zip(answers.chunks(2)) {
                    let (earlier, later) = *pair;
                    match (verdict[0], verdict[1]) {
                        (Answer::Sat, Answer::Sat) => overlaps.push(*pair),
                        (Answer::Unsat, _) | (Answer::Sat, Answer::Unsat) => {}
                        (Answer::Unknown, _) | (_, Answer::Unknown) => {
                            outcome.overlap_not_checked.push(skip(
                                later,
                                format!(
                                    "Z3 could not decide its overlap with {} (unknown or timeout)",
                                    candidates[earlier].location.display()
                                ),
                            ))
                        }
                    }
                }
            }
            Err(error) => {
                for &(_, later) in &pairs {
                    outcome.overlap_not_checked.push(skip(
                        later,
                        format!(
                            "Z3 rejected the overlap query: {}",
                            solver_error_summary(&error)
                        ),
                    ));
                }
            }
        }
    }

    // Explanations: which candidates shadow each unreachable one, and a
    // witness input for each overlap. Every query here is known satisfiable or
    // unsatisfiable, so the solver can answer each follow-up request.
    let mut explanation = prefix;
    for &index in &shadowed {
        explanation.push_str(&format!(
            "(push 1)\n(assert (! (and {}) :named runa_target))\n",
            applies(index)
        ));
        for earlier in 0..index {
            if guards[earlier].is_some() {
                explanation.push_str(&format!(
                    "(assert (! (not {}) :named runa_candidate_{}))\n",
                    guard_name(earlier),
                    earlier
                ));
            }
        }
        explanation.push_str("(check-sat)\n(get-unsat-core)\n(pop 1)\n");
    }
    for &(earlier, later) in &overlaps {
        explanation.push_str(&format!(
            "(push 1)\n(assert {})\n(check-sat)\n",
            overlap_assertion(earlier, later)
        ));
        for (symbol, _) in &signature.witness_names {
            explanation.push_str(&format!("(get-value ({}))\n", smt_value_symbol(symbol)));
        }
        explanation.push_str("(pop 1)\n");
    }
    let explained = if shadowed.is_empty() && overlaps.is_empty() {
        None
    } else {
        run_solver(&explanation)?
            .ok()
            .and_then(|output| parse_explanations(&output, &shadowed, &overlaps, signature))
    };
    let (cores, witnesses) = match explained {
        Some((cores, witnesses)) => (
            cores.into_iter().map(Some).collect(),
            witnesses.into_iter().map(Some).collect(),
        ),
        None => (vec![None; shadowed.len()], vec![None; overlaps.len()]),
    };
    for (&index, core) in shadowed.iter().zip::<Vec<Option<Vec<usize>>>>(cores) {
        outcome.findings.push(ClauseFinding {
            family: family.to_string(),
            clause: candidates[index].location.clone(),
            kind: ClauseFindingKind::Shadowed {
                by: core
                    .unwrap_or_default()
                    .into_iter()
                    .map(|earlier| candidates[earlier].location.clone())
                    .collect(),
            },
        });
    }
    for (&(earlier, later), witness) in overlaps
        .iter()
        .zip::<Vec<Option<Vec<(String, String)>>>>(witnesses)
    {
        outcome.findings.push(ClauseFinding {
            family: family.to_string(),
            clause: candidates[later].location.clone(),
            kind: ClauseFindingKind::Overlap {
                with: candidates[earlier].location.clone(),
                witness,
            },
        });
    }
    // One entry per clause: the first reason its overlaps went unchecked.
    let mut listed = BTreeSet::new();
    outcome
        .overlap_not_checked
        .retain(|skipped| listed.insert((skipped.clause.file.clone(), skipped.clause.line)));
    Ok(outcome)
}

/// The solver's own error line when there is one.
fn solver_error_summary(error: &str) -> String {
    error
        .lines()
        .find(|line| line.trim_start().starts_with("(error"))
        .or_else(|| error.lines().next())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn record_solver_failure(
    outcome: &mut FamilyOutcome,
    analyzed: &[usize],
    skip: &impl Fn(usize, String) -> SkippedClause,
    error: &str,
) {
    let summary = solver_error_summary(error);
    for &index in analyzed {
        outcome
            .not_analyzed
            .push(skip(index, format!("Z3 rejected the query: {}", summary)));
    }
}

/// Run one script. The outer error means no solver is available, which stops
/// the whole analysis; the inner error is a failure of this script.
fn run_solver(script: &str) -> Result<Result<String, String>, FamilyError> {
    match run_z3_script(script) {
        Err(error) if error.starts_with("Z3 not found") => Err(FamilyError::SolverMissing(error)),
        result => Ok(result),
    }
}

/// Split solver output into its top-level responses.
fn smt_responses(output: &str) -> Vec<String> {
    let mut responses = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut characters = output.chars().peekable();
    while let Some(character) = characters.next() {
        if in_string {
            current.push(character);
            if character == '"' {
                if characters.peek() == Some(&'"') {
                    current.push(characters.next().expect("peeked quote"));
                } else {
                    in_string = false;
                }
            }
            continue;
        }
        match character {
            '"' => {
                in_string = true;
                current.push(character);
            }
            '(' => {
                depth += 1;
                current.push(character);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(character);
                if depth == 0 {
                    responses.push(std::mem::take(&mut current).trim().to_string());
                }
            }
            whitespace if whitespace.is_whitespace() && depth == 0 => {
                if !current.trim().is_empty() {
                    responses.push(std::mem::take(&mut current).trim().to_string());
                }
                current.clear();
            }
            other => current.push(other),
        }
    }
    if !current.trim().is_empty() {
        responses.push(current.trim().to_string());
    }
    responses
}

type Explanations = (Vec<Vec<usize>>, Vec<Vec<(String, String)>>);

fn parse_explanations(
    output: &str,
    shadowed: &[usize],
    overlaps: &[(usize, usize)],
    signature: &FamilySignature,
) -> Option<Explanations> {
    let mut responses = smt_responses(output).into_iter();
    let mut cores = Vec::new();
    for _ in shadowed {
        if responses.next()? != "unsat" {
            return None;
        }
        let core = responses.next()?;
        let mut members = core
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split_whitespace()
            .filter_map(|name| name.strip_prefix("runa_candidate_"))
            .map(|index| index.parse::<usize>().ok())
            .collect::<Option<Vec<_>>>()?;
        members.sort_unstable();
        cores.push(members);
    }
    let mut witnesses = Vec::new();
    for _ in overlaps {
        if responses.next()? != "sat" {
            return None;
        }
        let mut witness = Vec::new();
        for (symbol, display) in &signature.witness_names {
            let response = responses.next()?;
            let prefix = format!("(({} ", smt_value_symbol(symbol));
            let value = response.strip_prefix(&prefix)?.strip_suffix("))")?;
            witness.push((display.clone(), readable_smt_value(value)));
        }
        witnesses.push(witness);
    }
    Some((cores, witnesses))
}

/// Present a model value in source spelling where the encoding allows:
/// negative integers and hex-encoded constructor names.
fn readable_smt_value(value: &str) -> String {
    let value = value.trim();
    if let Some(magnitude) = value
        .strip_prefix("(- ")
        .and_then(|rest| rest.strip_suffix(')'))
        .filter(|digits| digits.chars().all(|c| c.is_ascii_digit()))
    {
        return format!("-{}", magnitude);
    }
    let mut readable = String::new();
    let mut rest = value;
    while let Some(position) = rest.find("runa_s_") {
        readable.push_str(&rest[..position]);
        let encoded = &rest[position + "runa_s_".len()..];
        let length = encoded
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(encoded.len());
        let bytes = (0..length / 2 * 2)
            .step_by(2)
            .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>();
        match bytes.ok().and_then(|bytes| String::from_utf8(bytes).ok()) {
            Some(decoded) if length % 2 == 0 => readable.push_str(&decoded),
            _ => readable.push_str(&rest[position..position + "runa_s_".len() + length]),
        }
        rest = &encoded[length..];
    }
    readable.push_str(rest);
    readable
}
