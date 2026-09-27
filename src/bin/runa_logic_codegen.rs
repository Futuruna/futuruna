use super::runa_logic_search::{Clause, Goal, Program, Term};
use super::*;

struct Variable {
    index: usize,
    ty: Option<FirTy>,
}

struct Evaluation {
    expression: Expr,
    parameters: Vec<(String, FirTy)>,
    result_type: FirTy,
    entry_scope: bool,
    functions: BTreeSet<String>,
}

#[derive(Default)]
struct Builder {
    variables: BTreeMap<String, Variable>,
    next_variable: usize,
    projection: Option<String>,
    pending: BTreeSet<(String, usize)>,
    evaluations: Vec<Evaluation>,
}

impl Builder {
    fn variable(&mut self, name: &str, ty: Option<FirTy>) -> Result<Term, String> {
        if name != "_" {
            if let Some(variable) = self.variables.get_mut(name) {
                if let (Some(previous), Some(next)) = (&variable.ty, &ty) {
                    if previous != next {
                        return Err(format!(
                            "native logic variable `{name}` has conflicting types"
                        ));
                    }
                }
                if variable.ty.is_none() {
                    variable.ty = ty;
                }
                return Ok(Term::Variable(variable.index));
            }
        }
        let index = self.next_variable;
        self.next_variable += 1;
        if name != "_" {
            self.variables.insert(name.into(), Variable { index, ty });
        }
        Ok(Term::Variable(index))
    }

    fn parameter_types(
        cg: &RustCodegen,
        name: &str,
        arity: usize,
    ) -> Result<Vec<Option<FirTy>>, String> {
        let types = cg
            .native_logic_parameter_types
            .get(&(name.into(), arity))
            .ok_or_else(|| format!("native query `{name}` has no argument schema"))?;
        Ok(types
            .iter()
            .map(|ty| ty.as_deref().map(RustCodegen::rust_type_to_fir))
            .collect())
    }

    fn argument_types(
        &self,
        cg: &RustCodegen,
        name: &str,
        arguments: &[Expr],
        entry: bool,
    ) -> Result<Vec<Option<FirTy>>, String> {
        let mut types = Self::parameter_types(cg, name, arguments.len())?;
        for (argument, ty) in arguments.iter().zip(&mut types) {
            if ty.is_some() {
                continue;
            }
            *ty = match &argument.kind {
                ExprKind::Lit(literal) => Some(RustCodegen::rust_type_to_fir(
                    RustCodegen::literal_rust_type(literal),
                )),
                ExprKind::Tuple(items) if items.is_empty() => Some(FirTy::Unit),
                ExprKind::Var(name) if name != "_" => self
                    .variables
                    .get(name)
                    .and_then(|variable| variable.ty.clone())
                    .or_else(|| {
                        (entry
                            && self.projection.as_deref() != Some(name)
                            && (cg.local_bindings.contains(name)
                                || cg.var_fir_types.contains_key(name)
                                || cg.binary_global_binding_types.contains_key(name)
                                || cg.types.literal_bindings.contains_key(name)))
                        .then(|| cg.infer_expr_fir_ty(argument))
                        .filter(|ty| !matches!(ty, FirTy::Unknown | FirTy::Var(_)))
                    }),
                _ => None,
            };
        }
        let rules = cg.types.prolog_rule_groups[name]
            .iter()
            .filter(|rule| RustCodegen::rule_exact_arity(rule) == Some(arguments.len()))
            .collect::<Vec<_>>();
        if let Some(heads) = RustCodegen::unconstrained_rule_fact_heads(&rules) {
            let known = types.clone();
            for (index, ty) in types.iter_mut().enumerate() {
                if ty.is_some() {
                    continue;
                }
                // Only equalities guaranteed by every clause can supply the
                // projection's type. Do not specialize the declaration from
                // one caller or from one alternative's bindings.
                let mut evidence = known.iter().enumerate().filter_map(|(other, ty)| {
                    heads
                        .iter()
                        .all(|head| head[index] != "_" && head[index] == head[other])
                        .then_some(ty.as_ref())
                        .flatten()
                });
                if let Some(first) = evidence.next() {
                    if evidence.all(|next| next == first) {
                        *ty = Some(first.clone());
                    }
                }
            }
        }
        Ok(types)
    }

    fn evaluation(
        &mut self,
        cg: &RustCodegen,
        expression: &Expr,
        result_type: FirTy,
        result: Term,
        entry_scope: bool,
    ) -> Result<Goal, String> {
        let mut references = BTreeSet::new();
        collect_true_free_vars(expression, &mut references, &BTreeSet::new());
        let mut parameters = Vec::new();
        let mut arguments = Vec::new();
        let mut functions = BTreeSet::new();
        for name in references {
            if let Some(variable) = self.variables.get(&name) {
                let ty = variable
                    .ty
                    .clone()
                    .ok_or_else(|| format!("native predicate needs the type of `{name}`"))?;
                parameters.push((name, ty));
                arguments.push(Term::Variable(variable.index));
            } else if !entry_scope {
                if cg.binary_global_binding_types.contains_key(&name)
                    || cg.types.literal_bindings.contains_key(&name)
                    || cg.binary_global_env_fns.contains(&name)
                {
                    return Err(format!(
                        "native query needs a checked declaration environment for `{name}`"
                    ));
                }
                if cg
                    .ordinary_function_arities
                    .iter()
                    .any(|(function, _)| function == &name)
                {
                    functions.insert(sanitize_name(&name));
                }
                for ((function, _), emitted) in &cg.rule_emitted_names {
                    if function == &name {
                        functions.insert(emitted.clone());
                    }
                }
            }
        }
        let id = self.evaluations.len();
        self.evaluations.push(Evaluation {
            expression: expression.clone(),
            parameters,
            result_type,
            entry_scope,
            functions,
        });
        Ok(Goal::Evaluate {
            expression: id,
            arguments,
            result,
        })
    }

    fn term(
        &mut self,
        cg: &RustCodegen,
        expression: &Expr,
        ty: Option<FirTy>,
        entry: bool,
        setup: &mut Vec<Goal>,
    ) -> Result<Term, String> {
        let expression = RustCodegen::rule_head_arg_expr(expression);
        match &expression.kind {
            ExprKind::Lit(literal) => Ok(match literal {
                Literal::Int(value) => Term::Int(*value),
                Literal::Float(value) => Term::Float(*value),
                Literal::Bool(value) => Term::Bool(*value),
                Literal::Char(value) => Term::Char(*value),
                Literal::Str(value) => Term::String(value.clone()),
            }),
            ExprKind::Var(name) if !RustCodegen::is_uppercase_ident(name) => {
                let outer = cg.local_bindings.contains(name)
                    || cg.var_fir_types.contains_key(name)
                    || cg.binary_global_binding_types.contains_key(name)
                    || cg.types.literal_bindings.contains_key(name);
                if entry && name != "_" && self.projection.as_deref() != Some(name) && outer {
                    let result = self.variable("_", ty.clone())?;
                    let ty = ty.ok_or_else(|| format!("outer query input `{name}` has no type"))?;
                    setup.push(self.evaluation(cg, expression, ty, result.clone(), entry)?);
                    return Ok(result);
                }
                if !entry
                    && !self.variables.contains_key(name)
                    && (cg.binary_global_binding_types.contains_key(name)
                        || cg.types.literal_bindings.contains_key(name))
                {
                    return Err("existential query capture of an outer value has no checked native binding contract".into());
                }
                self.variable(name, ty)
            }
            ExprKind::Tuple(items) if items.is_empty() => Ok(Term::Unit),
            ExprKind::Tuple(items) | ExprKind::List(items) => {
                let list = matches!(&expression.kind, ExprKind::List(_));
                let mut fields = Vec::new();
                for (index, item) in items.iter().enumerate() {
                    let item_type = match &ty {
                        Some(FirTy::List(inner)) => Some((**inner).clone()),
                        Some(FirTy::Tuple(types)) => types.get(index).cloned(),
                        _ => None,
                    };
                    fields.push(self.term(cg, item, item_type, entry, setup)?);
                }
                Ok(Term::Compound(
                    if list { "$list" } else { "$tuple" }.into(),
                    fields,
                ))
            }
            _ => {
                if let Some(value) = rule_head_numeric_value(expression) {
                    return match value {
                        Value::Int(value) => Ok(Term::Int(value)),
                        Value::Float(value) => Ok(Term::Float(value)),
                        _ => Err("unsupported numeric logic term".into()),
                    };
                }
                let result_type =
                    ty.ok_or_else(|| "computed query argument has no type".to_string())?;
                let result = self.variable("_", Some(result_type.clone()))?;
                setup.push(self.evaluation(cg, expression, result_type, result.clone(), entry)?);
                Ok(result)
            }
        }
    }

    fn goal(&mut self, cg: &RustCodegen, expression: &Expr, entry: bool) -> Result<Goal, String> {
        match &expression.kind {
            ExprKind::Lit(Literal::Bool(true)) => Ok(Goal::Succeed),
            ExprKind::Lit(Literal::Bool(false)) => Ok(Goal::Fail),
            ExprKind::Conjunction(goals) | ExprKind::Disjunction(goals) => {
                let lowered = goals
                    .iter()
                    .map(|goal| self.goal(cg, goal, entry))
                    .collect::<Result<_, _>>()?;
                Ok(if matches!(&expression.kind, ExprKind::Conjunction(_)) {
                    Goal::All(lowered)
                } else {
                    Goal::Any(lowered)
                })
            }
            ExprKind::App(function, arguments) => {
                let ExprKind::Var(name) = &function.kind else {
                    return Err("native query needs a statically resolved local predicate".into());
                };
                if name == "not"
                    && arguments.len() == 1
                    && !cg.types.user_functions.contains(name)
                    && !self.variables.contains_key(name)
                    && (!entry || !cg.local_bindings.contains(name))
                {
                    return Ok(Goal::Not(Box::new(self.goal(cg, &arguments[0], entry)?)));
                }
                if cg.types.prolog_rule_groups.contains_key(name)
                    && !cg
                        .ordinary_function_arities
                        .contains(&(name.clone(), arguments.len()))
                    && !cg.builtin_registry.contains_key(name)
                    && (!entry
                        || !cg.local_bindings.contains(name)
                        || cg.static_call_bypasses_non_callable_local(name, arguments.len()))
                {
                    let types = self.argument_types(cg, name, arguments, entry)?;
                    let mut setup = Vec::new();
                    let terms = arguments
                        .iter()
                        .zip(types)
                        .map(|(argument, ty)| self.term(cg, argument, ty, entry, &mut setup))
                        .collect::<Result<Vec<_>, _>>()?;
                    let binders = arguments.iter().zip(&terms).filter_map(|(argument, term)| {
                        matches!(&argument.kind, ExprKind::Var(name) if name != "_" && !RustCodegen::is_uppercase_ident(name)).then(|| term.clone())
                    }).collect();
                    self.pending.insert((name.clone(), arguments.len()));
                    setup.push(Goal::Call(name.clone(), terms, binders));
                    return Ok(Goal::All(setup));
                }
                if !cg.types.user_functions.contains(name)
                    && !cg.builtin_registry.contains_key(name)
                    && !cg.local_bindings.contains(name)
                    && !self.variables.contains_key(name)
                {
                    return Err(format!(
                        "native query predicate `{name}` has no resolved declaration"
                    ));
                }
                self.evaluation(cg, expression, FirTy::Bool, Term::Bool(true), entry)
            }
            _ => self.evaluation(cg, expression, FirTy::Bool, Term::Bool(true), entry),
        }
    }

    fn program(&mut self, cg: &RustCodegen) -> Result<Program, String> {
        let mut program = Program::default();
        while let Some(key) = self.pending.pop_first() {
            if program.relations.contains_key(&key) {
                continue;
            }
            let canonical = RuleDispatchKey {
                scope: None,
                name: key.0.clone(),
                arity: key.1,
            };
            if cg.canonical_rule_metadata_installed
                && (cg
                    .canonical_rule_return_types
                    .get(&canonical)
                    .map(String::as_str)
                    != Some("Bool")
                    || cg.canonical_rule_return_issues.contains_key(&canonical)
                    || cg.canonical_rule_parameter_issues.contains(&canonical))
            {
                return Err(format!(
                    "native query `{}` has no checked Boolean contract",
                    key.0
                ));
            }
            let types = Self::parameter_types(cg, &key.0, key.1)?;
            let rules = cg
                .types
                .prolog_rule_groups
                .get(&key.0)
                .ok_or_else(|| "query relation is missing".to_string())?;
            let mut clauses = Vec::new();
            for rule in rules {
                if RustCodegen::rule_exact_arity(rule) != Some(key.1) {
                    continue;
                }
                let Rule::Clause { head, body } = rule else {
                    return Err("native query priority needs a checked lowering".into());
                };
                let ExprKind::App(_, arguments) = &head.kind else {
                    return Err("native query head has no positional arguments".into());
                };
                self.variables.clear();
                self.next_variable = 0;
                // Heads introduce local binders even when a global has the same name.
                for (argument, ty) in arguments.iter().zip(&types) {
                    if let Some(name) = RustCodegen::rule_head_var_name(argument) {
                        self.variable(&name, ty.clone())?;
                    }
                }
                let mut setup = Vec::new();
                let head = arguments
                    .iter()
                    .zip(&types)
                    .map(|(argument, ty)| self.term(cg, argument, ty.clone(), false, &mut setup))
                    .collect::<Result<Vec<_>, _>>()?;
                if !setup.is_empty() {
                    return Err("native query head needs structural term lowering".into());
                }
                let body = body
                    .as_ref()
                    .map(|body| self.goal(cg, body, false))
                    .transpose()?
                    .unwrap_or(Goal::Succeed);
                clauses.push(Clause { head, body });
            }
            if clauses.is_empty() {
                return Err("native query has no clauses".into());
            }
            program.relations.insert(key, clauses);
        }
        Ok(program)
    }
}

fn quote_term(term: &Term, runtime: &str) -> String {
    let inner = match term {
        Term::Variable(index) => format!("Variable({index})"),
        Term::Int(value) => format!("Int({value}i64)"),
        Term::Float(value) => format!("Float(f64::from_bits({}))", value.to_bits()),
        Term::Bool(value) => format!("Bool({value})"),
        Term::Char(value) => format!("Char({value:?})"),
        Term::String(value) => format!("String({value:?}.to_string())"),
        Term::Unit => "Unit".into(),
        Term::Compound(name, fields) => format!(
            "Compound({name:?}.to_string(), vec![{}])",
            fields
                .iter()
                .map(|term| quote_term(term, runtime))
                .collect::<Vec<_>>()
                .join(",")
        ),
    };
    format!("{runtime}::Term::{inner}")
}

fn quote_goal(goal: &Goal, runtime: &str) -> String {
    let inner = match goal {
        Goal::Succeed => "Succeed".into(),
        Goal::Fail => "Fail".into(),
        Goal::Call(name, arguments, binders) => format!(
            "Call({name:?}.to_string(),vec![{}],vec![{}])",
            arguments
                .iter()
                .map(|term| quote_term(term, runtime))
                .collect::<Vec<_>>()
                .join(","),
            binders
                .iter()
                .map(|term| quote_term(term, runtime))
                .collect::<Vec<_>>()
                .join(","),
        ),
        Goal::All(goals) | Goal::Any(goals) => format!(
            "{}(vec![{}])",
            if matches!(goal, Goal::All(_)) {
                "All"
            } else {
                "Any"
            },
            goals
                .iter()
                .map(|goal| quote_goal(goal, runtime))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Goal::Not(goal) => format!("Not(Box::new({}))", quote_goal(goal, runtime)),
        Goal::Evaluate {
            expression,
            arguments,
            result,
        } => format!(
            "Evaluate {{ expression: {expression}, arguments: vec![{}], result: {} }}",
            arguments
                .iter()
                .map(|term| quote_term(term, runtime))
                .collect::<Vec<_>>()
                .join(","),
            quote_term(result, runtime)
        ),
    };
    format!("{runtime}::Goal::{inner}")
}

fn encode(value: &str, ty: &FirTy, runtime: &str) -> Result<String, String> {
    let variant = match ty {
        FirTy::Int => "Int",
        FirTy::Float => "Float",
        FirTy::Bool => "Bool",
        FirTy::Char => "Char",
        FirTy::String => "String",
        FirTy::Unit => return Ok(format!("{{ let _ = {value}; {runtime}::Term::Unit }}")),
        _ => return Err("native query expression has no value encoding".into()),
    };
    Ok(format!("{runtime}::Term::{variant}({value})"))
}

fn decode(value: &str, ty: &FirTy, runtime: &str) -> Result<String, String> {
    let variant = match ty {
        FirTy::Int => "Int", FirTy::Float => "Float", FirTy::Bool => "Bool",
        FirTy::Char => "Char", FirTy::String => "String",
        FirTy::Unit => return Ok(format!("match {value} {{ {runtime}::Term::Unit => (), _ => panic!(\"native logic value does not match its checked Unit type\") }}")),
        _ => return Err("native query result has no value decoding".into()),
    };
    Ok(format!("match {value} {{ {runtime}::Term::{variant}(value) => value.clone(), _ => panic!(\"native logic value does not match its checked {variant} type\") }}"))
}

impl RustCodegen {
    pub(super) fn unconstrained_rule_fact_heads(rules: &[&Rule]) -> Option<Vec<Vec<String>>> {
        let arity = Self::rule_arity(rules);
        if rules.is_empty() || arity == 0 {
            return None;
        }
        rules
            .iter()
            .map(|rule| {
                let Rule::Clause { head, body: None } = rule else {
                    return None;
                };
                let ExprKind::App(_, arguments) = &head.kind else {
                    return None;
                };
                if arguments.len() != arity {
                    return None;
                }
                arguments
                    .iter()
                    .map(|argument| match &argument.kind {
                        ExprKind::Var(name) if !Self::is_uppercase_ident(name) => {
                            Some(name.clone())
                        }
                        _ => None,
                    })
                    .collect()
            })
            .collect()
    }

    pub(super) fn emit_polymorphic_rule_facts(
        &mut self,
        fn_name: &str,
        emitted_name: &str,
        rules: &[&Rule],
    ) -> Option<String> {
        let heads = Self::unconstrained_rule_fact_heads(rules)?;
        let key = RuleDispatchKey {
            scope: self.current_rule_scope_name.clone(),
            name: fn_name.into(),
            arity: heads[0].len(),
        };
        // A proven parameter schema still owns the ABI. Generalization is for
        // unconstrained declarations, never a way around canonical evidence.
        if self.canonical_rule_metadata_installed
            && self.current_module_path.is_empty()
            && self
                .canonical_rule_parameter_types
                .get(&key)
                .is_some_and(|types| types.iter().any(Option::is_some))
        {
            return None;
        }
        let mut alternatives = Vec::new();
        let mut compared = BTreeSet::new();
        let runtime = &self.native_logic_runtime_name;
        for head in &heads {
            let mut bindings = BTreeMap::new();
            let mut comparisons = Vec::new();
            for (index, name) in head.iter().enumerate() {
                if name == "_" {
                    continue;
                }
                if let Some(previous) = bindings.get(name) {
                    compared.insert(*previous);
                    compared.insert(index);
                    comparisons.push(format!(
                        "{runtime}::LogicValue::logic_value(&__arg_{previous}).same_value(&{runtime}::LogicValue::logic_value(&__arg_{index}))"
                    ));
                } else {
                    bindings.insert(name, index);
                }
            }
            if comparisons.is_empty() {
                compared.clear();
                alternatives = vec!["true".to_string()];
                break;
            }
            alternatives.push(format!("({})", comparisons.join(" && ")));
        }
        let generics = (0..key.arity)
            .map(|index| {
                if compared.contains(&index) {
                    format!("__FutArg{index}: {runtime}::LogicValue")
                } else {
                    format!("__FutArg{index}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let parameters = (0..key.arity)
            .map(|index| format!("__arg_{index}: __FutArg{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let visibility = if self.rule_is_exported_in_current_namespace(fn_name) {
            "pub "
        } else {
            ""
        };
        self.native_logic_runtime_needed |= !compared.is_empty();
        Some(format!(
            "{visibility}fn {}<{generics}>({parameters}) -> bool {{ {} }}\n",
            sanitize_name(emitted_name),
            alternatives.join(" || ")
        ))
    }

    pub(super) fn emit_native_logic_search(
        &mut self,
        template: Option<&Expr>,
        goal: &Expr,
    ) -> Result<String, String> {
        if !self.current_module_path.is_empty() || self.current_rule_scope_name.is_some() {
            return Err("native derived query needs an owner-scoped plan".into());
        }
        let mut builder = Builder::default();
        if let Some(template) = template {
            let ExprKind::Var(name) = &template.kind else {
                return Err("logic query template must be a single variable".into());
            };
            let ExprKind::App(_, arguments) = &goal.kind else {
                return Err("logic query goal must be a named rule call".into());
            };
            if !arguments.iter().any(
                |argument| matches!(&argument.kind, ExprKind::Var(candidate) if candidate == name),
            ) {
                return Err("query template must occur as a direct goal argument".into());
            }
            builder.projection = Some(name.clone());
        }
        let query = builder.goal(self, goal, true)?;
        let projection = if let Some(name) = &builder.projection {
            let variable = builder
                .variables
                .get(name)
                .ok_or_else(|| "query template is not bound by its goal".to_string())?;
            Some((
                Term::Variable(variable.index),
                variable
                    .ty
                    .clone()
                    .ok_or_else(|| "query template has no checked type".to_string())?,
            ))
        } else {
            None
        };
        let program = builder.program(self)?;
        let runtime_name = self.native_logic_runtime_name.clone();
        let runtime = runtime_name.as_str();
        let mut used = BTreeSet::new();
        collect_generated_rust_expr_names(goal, &mut used);
        for evaluation in &builder.evaluations {
            collect_generated_rust_expr_names(&evaluation.expression, &mut used);
            used.extend(evaluation.parameters.iter().map(|(name, _)| name.clone()));
        }
        used.extend(self.local_bindings.iter().cloned());
        let mut used = used.into_iter().map(|name| sanitize_name(&name)).collect();
        let program_name = fresh_generated_rust_name("__program", &mut used);
        let search_name = fresh_generated_rust_name("__search", &mut used);
        let expression_name = fresh_generated_rust_name("__expression", &mut used);
        let arguments_name = fresh_generated_rust_name("__arguments", &mut used);
        let value_name = fresh_generated_rust_name("__value", &mut used);
        let mut output = format!("{{ let mut {program_name} = {runtime}::Program::default();");
        for ((name, arity), clauses) in program.relations {
            let clauses = clauses
                .iter()
                .map(|clause| {
                    format!(
                        "{runtime}::Clause {{ head: vec![{}], body: {} }}",
                        clause
                            .head
                            .iter()
                            .map(|term| quote_term(term, runtime))
                            .collect::<Vec<_>>()
                            .join(","),
                        quote_goal(&clause.body, runtime)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            output.push_str(&format!(
                "{program_name}.relations.insert(({name:?}.to_string(),{arity}),vec![{clauses}]);"
            ));
        }
        output.push_str(&format!("let mut {search_name} = {runtime}::Search::new(&{program_name}, |{expression_name}, {arguments_name}: &[{runtime}::Term]| -> Result<{runtime}::Term, String> {{ match {expression_name} {{"));
        for (index, evaluation) in builder.evaluations.iter().enumerate() {
            // Rule expressions belong to the declaration, not the query's
            // caller. Only their explicit logical inputs enter this scope.
            let caller = (!evaluation.entry_scope).then(|| {
                (
                    std::mem::take(&mut self.local_bindings),
                    std::mem::take(&mut self.var_types),
                    std::mem::take(&mut self.var_fir_types),
                    std::mem::take(&mut self.current_borrow_params),
                    std::mem::take(&mut self.ref_match_bindings),
                    std::mem::take(&mut self.types.rule_clone_params),
                )
            });
            let context = RustRuleHeadApplicability {
                subject: String::new(),
                pattern: String::new(),
                nested_patterns: Vec::new(),
                guards: Vec::new(),
                bindings: evaluation
                    .parameters
                    .iter()
                    .map(|(name, ty)| RustRuleHeadBinding {
                        source_name: name.clone(),
                        temporary_name: String::new(),
                        ty: ty.clone(),
                        materialization: RustRuleHeadBindingMaterialization::Direct,
                    })
                    .collect(),
            };
            let expression =
                self.with_rule_head_binding_context(&context, &[&evaluation.expression], |cg| {
                    let old_iter = std::mem::replace(&mut cg.in_iter_closure, true);
                    let old_params = std::mem::replace(
                        &mut cg.closure_params,
                        evaluation
                            .parameters
                            .iter()
                            .map(|(name, _)| name.clone())
                            .collect(),
                    );
                    let expression = cg.emit_expr_with_expected_ty(
                        &evaluation.expression,
                        &evaluation.result_type,
                    );
                    cg.in_iter_closure = old_iter;
                    cg.closure_params = old_params;
                    expression
                });
            if let Some((locals, types, fir_types, borrowed, referenced, cloned)) = caller {
                self.local_bindings = locals;
                self.var_types = types;
                self.var_fir_types = fir_types;
                self.current_borrow_params = borrowed;
                self.ref_match_bindings = referenced;
                self.types.rule_clone_params = cloned;
            }
            let encoded = encode(&expression, &evaluation.result_type, runtime)?;
            output.push_str(&format!("{index} => {{"));
            for function in &evaluation.functions {
                output.push_str(&format!("use self::{function};"));
            }
            for (position, (name, ty)) in evaluation.parameters.iter().enumerate() {
                let value = decode(&format!("&{arguments_name}[{position}]"), ty, runtime)?;
                output.push_str(&format!("let {} = {value};", sanitize_name(name)));
            }
            output.push_str(&format!("Ok({encoded}) }},"));
        }
        output.push_str("_ => Err(\"unknown compiled logic expression\".to_string()) } });");
        let query = quote_goal(&query, runtime);
        if let Some((projection, ty)) = projection {
            let projection = quote_term(&projection, runtime);
            let value = decode(&value_name, &ty, runtime)?;
            output.push_str(&format!("{search_name}.findall(&{query}, &{projection}).unwrap_or_else(|error| panic!(\"{{}}\",error)).iter().map(|{value_name}| {value}).collect::<Vec<_>>()"));
        } else {
            output.push_str(&format!(
                "{search_name}.exists(&{query}).unwrap_or_else(|error| panic!(\"{{}}\",error))"
            ));
        }
        output.push('}');
        self.native_logic_runtime_needed = true;
        Ok(output)
    }
}
