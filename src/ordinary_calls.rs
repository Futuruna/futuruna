//! Ordinary calls follow the nearest lexical binder. These contracts diagnose
//! source mistakes and grant no purity, totality, or proof authority.

use super::*;

#[derive(Clone)]
pub(super) enum DeclaredCallable {
    Function(Vec<Param>),
    Rule(BTreeSet<usize>),
}

#[derive(Clone, Default)]
pub(super) struct CallableBinding {
    declaration: Option<DeclaredCallable>,
    pub(super) is_value: bool,
}

impl CallableBinding {
    pub(super) fn declaration(declaration: DeclaredCallable) -> Self {
        Self {
            declaration: Some(declaration),
            is_value: false,
        }
    }
}

impl TypeChecker {
    pub(super) fn define_rule_callable(&mut self, name: &str, arity: usize) {
        let existing = self
            .ordinary_callable_scopes
            .last()
            .and_then(|scope| scope.get(name))
            .and_then(|binding| binding.declaration.clone());
        self.define_var(name);
        let declaration = match existing {
            // Ordinary functions take precedence over same-named rule families.
            Some(function @ DeclaredCallable::Function(_)) => function,
            Some(DeclaredCallable::Rule(mut arities)) => {
                arities.insert(arity);
                DeclaredCallable::Rule(arities)
            }
            None => DeclaredCallable::Rule(BTreeSet::from([arity])),
        };
        self.ordinary_callable_scopes
            .last_mut()
            .unwrap()
            .insert(name.into(), CallableBinding::declaration(declaration));
    }

    /// Some(None) is a value binder: it hides outer callable declarations even
    /// when its type is unknown. None means that no lexical binder was found.
    fn ordinary_callable(
        &self,
        name: &str,
        call_arity: Option<usize>,
    ) -> Option<Option<DeclaredCallable>> {
        for (index, names) in self.scopes.iter().enumerate().rev() {
            if index == 0 {
                if let Some(scope) = self.active_rule_scope_inference.as_ref() {
                    if let Some(arity) = self
                        .rule_scope_methods
                        .get(scope)
                        .and_then(|methods| methods.get(name))
                        .filter(|arity| call_arity.is_none_or(|requested| requested == **arity))
                    {
                        return Some(Some(DeclaredCallable::Rule(BTreeSet::from([*arity]))));
                    }
                    if self
                        .rule_scope_value_methods
                        .get(scope)
                        .is_some_and(|methods| methods.contains_key(name))
                    {
                        // The existing RuleScope method checker owns this call.
                        return None;
                    }
                }
            }
            if names.contains(name) {
                return Some(
                    self.ordinary_callable_scopes
                        .get(index)
                        .and_then(|scope| scope.get(name))
                        .filter(|binding| !binding.is_value)
                        .and_then(|binding| binding.declaration.clone()),
                );
            }
        }
        None
    }

    pub(super) fn check_ordinary_call(
        &mut self,
        call: &Expr,
        name: &str,
        arguments: &[Expr],
    ) -> bool {
        let Some(mut target) = self.ordinary_callable(name, Some(arguments.len())) else {
            return false;
        };
        if target.is_none() {
            let ty = self
                .scopes
                .iter()
                .zip(&self.var_types)
                .rev()
                .find(|(names, _)| names.contains(name))
                .and_then(|(_, types)| types.get(name))
                .and_then(|ty| parse_type_annotation(ty).ok());
            if ty.as_ref().is_some_and(Self::ordinary_non_callable_type) {
                // Existing dispatch falls back to a declared function/rule when
                // a same-named binding is scalar (closures still shadow it).
                target = self
                    .ordinary_callable_scopes
                    .iter()
                    .rev()
                    .filter_map(|scope| scope.get(name))
                    .find_map(|binding| binding.declaration.clone());
                if target.is_none() && !self.functions.contains_key(name) {
                    self.error_at_expr(call, format!("value `{name}` is not callable"));
                }
            }
        }
        match target {
            Some(DeclaredCallable::Rule(arities)) if !arities.contains(&arguments.len()) => {
                let arities = arities
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                self.error_at_expr(
                    call,
                    format!(
                        "rule `{name}` has no {}-argument form; declared arities: {arities}",
                        arguments.len()
                    ),
                );
            }
            Some(DeclaredCallable::Function(parameters)) if parameters.len() != arguments.len() => {
                let dispatch_note = if self
                    .function_arities
                    .get(name)
                    .is_some_and(|arities| arities.len() > 1)
                {
                    "; ordinary runtime functions resolve by bare name, not by overloaded arity"
                } else {
                    ""
                };
                self.error_at_expr(
                    call,
                    format!(
                        "`{name}` expects {} argument{} but got {}{dispatch_note}",
                        parameters.len(),
                        if parameters.len() == 1 { "" } else { "s" },
                        arguments.len()
                    ),
                );
            }
            Some(DeclaredCallable::Function(parameters)) => {
                let names = parameters
                    .iter()
                    .map(|p| Some(p.name.clone()))
                    .collect::<Vec<_>>();
                if let Some(ordered) = Self::canonical_ordered_arguments(arguments, &names) {
                    let mut substitutions = BTreeMap::new();
                    for (parameter, argument) in parameters.iter().zip(ordered) {
                        let Some(expected) = parameter.ty.as_ref() else {
                            continue;
                        };
                        let Some(actual) = self.ordinary_expression_type(argument) else {
                            continue;
                        };
                        let Ok(actual_ty) = parse_type_annotation(&actual) else {
                            continue;
                        };
                        if !Self::ordinary_argument_matches(
                            &actual_ty,
                            expected,
                            &mut substitutions,
                        ) {
                            self.error_at_expr(
                                argument,
                                format!(
                                    "argument `{}` to `{name}` expects `{}`, got `{actual}`{}",
                                    parameter.name,
                                    Self::canonical_explore_ty_name(expected),
                                    Self::numeric_conversion_hint(&actual_ty, expected)
                                ),
                            );
                        }
                    }
                }
            }
            Some(DeclaredCallable::Rule(_)) => self.check_rule_call_argument_types(name, arguments),
            _ => {}
        }
        true
    }

    /// Every clause of a rule family declares one type per typed parameter
    /// position, so an argument of another type matches no clause.
    fn check_rule_call_argument_types(&mut self, name: &str, arguments: &[Expr]) {
        if has_named_args(arguments)
            || self
                .active_rule_scope_inference
                .as_ref()
                .is_some_and(|scope| {
                    self.rule_scope_methods
                        .get(scope)
                        .is_some_and(|methods| methods.contains_key(name))
                })
        {
            return;
        }
        let key = (name.to_string(), arguments.len());
        let Some(parameter_types) = self.rule_param_types_by_arity.get(&key).cloned() else {
            return;
        };
        let parameter_names = self.function_params_by_arity.get(&key).cloned();
        let mut substitutions = BTreeMap::new();
        for (index, (expected, argument)) in parameter_types.iter().zip(arguments).enumerate() {
            let Some(expected) = expected else {
                continue;
            };
            let Some(actual) = self.ordinary_expression_type(argument) else {
                continue;
            };
            let Ok(actual_ty) = parse_type_annotation(&actual) else {
                continue;
            };
            if Self::ordinary_argument_matches(&actual_ty, expected, &mut substitutions) {
                continue;
            }
            let parameter = parameter_names
                .as_ref()
                .and_then(|names| names.get(index))
                .map(|parameter| format!("`{parameter}`"))
                .unwrap_or_else(|| (index + 1).to_string());
            self.error_at_expr(
                argument,
                format!(
                    "argument {parameter} to `{name}` expects `{}`, got `{actual}`{}",
                    Self::canonical_explore_ty_name(expected),
                    Self::numeric_conversion_hint(&actual_ty, expected)
                ),
            );
        }
    }

    /// Positional constructor arguments have the declared field types. An
    /// `Int` is not a `Float`: numeric conversion is explicit.
    pub(super) fn check_positional_constructor_arguments(
        &mut self,
        name: &str,
        arguments: &[Expr],
    ) {
        let Some(signature) = self.constructor_signature_for_args(name, arguments) else {
            return;
        };
        let mut substitutions = BTreeMap::new();
        for ((field, field_ty), argument) in signature
            .fields
            .iter()
            .zip(&signature.field_tys)
            .zip(arguments)
        {
            let Some(expected) = field_ty
                .as_deref()
                .and_then(|ty| parse_type_annotation(ty).ok())
            else {
                continue;
            };
            let Some(actual) = self.ordinary_expression_type(argument) else {
                continue;
            };
            let Ok(actual_ty) = parse_type_annotation(&actual) else {
                continue;
            };
            if !Self::ordinary_argument_matches(&actual_ty, &expected, &mut substitutions) {
                self.error_at_expr(
                    argument,
                    format!(
                        "constructor `{name}` field `{field}` expects `{}`, got `{actual}`{}",
                        Self::canonical_explore_ty_name(&expected),
                        Self::numeric_conversion_hint(&actual_ty, &expected)
                    ),
                );
            }
        }
    }

    fn numeric_conversion_hint(actual: &Ty, expected: &Ty) -> &'static str {
        match (actual, expected) {
            (Ty::Name(actual), Ty::Name(expected)) if actual == "Int" && expected == "Float" => {
                "; write a Float literal such as `25.0` or convert with `to_float`"
            }
            _ => "",
        }
    }

    pub(super) fn ordinary_expression_type(&self, expression: &Expr) -> Option<String> {
        let mut locals = BTreeMap::new();
        for (names, types) in self.scopes.iter().zip(&self.var_types).skip(1) {
            for name in names {
                locals.insert(
                    name.clone(),
                    types
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.into()),
                );
            }
        }
        self.infer_expr_type_name_with_locals(expression, &locals)
            .or_else(|| Self::is_polymorphic_empty_list_expr(expression).then(|| "List(_)".into()))
    }

    pub(super) fn check_ordinary_field(&mut self, expression: &Expr, base: &Expr, field: &str) {
        let Some(type_name) = self.ordinary_expression_type(base) else {
            // An unknown receiver is checked at runtime; field spelling alone
            // cannot identify a nominal type or constrain a generic parameter.
            return;
        };
        let owner = Self::canonical_nominal_owner(&type_name);
        if owner
            .as_deref()
            .is_some_and(|owner| self.check_sum_type_field(expression, base, owner, field))
        {
            return;
        }
        let tuple_size = parse_type_annotation(&type_name).ok().and_then(|ty| match ty {
            Ty::App(head, arguments) if matches!(head.as_ref(), Ty::Name(name) if name == "Tuple") => {
                Some(arguments.len())
            }
            _ => None,
        });
        let present = if let Some(size) = tuple_size {
            let index = match field {
                "fst" => Some(0),
                "snd" => Some(1),
                "trd" => Some(2),
                _ => field.parse::<usize>().ok(),
            };
            Some(index.is_some_and(|index| index < size))
        } else if let Some(owner) = owner.as_deref() {
            if self
                .type_fields
                .get(owner)
                .is_some_and(|fields| fields.contains(field))
                || self
                    .rule_scope_methods
                    .get(owner)
                    .is_some_and(|methods| methods.contains_key(field))
                || self
                    .rule_scope_value_methods
                    .get(owner)
                    .is_some_and(|methods| methods.contains_key(field))
            {
                Some(true)
            } else if self.type_has_scoped_members(owner) {
                // RuleScope values expose bindings and methods by name, not
                // the constructor's numeric argument positions.
                Some(false)
            } else if self.type_variants.contains_key(owner) || self.type_fields.contains_key(owner)
            {
                // Positional projections are supported on records as well as
                // tuples. Keep ambiguous nominal owners conservative here.
                Some(field.parse::<usize>().ok().is_some_and(|index| {
                    self.constructor_signatures
                        .values()
                        .flatten()
                        .any(|signature| signature.parent == owner && index < signature.arity())
                }))
            } else if matches!(
                owner,
                "Int" | "Float" | "Bool" | "String" | "Char" | "Unit" | "List" | "Map" | "Set"
            ) {
                Some(false)
            } else {
                None
            }
        } else if type_name == "()" {
            Some(false)
        } else {
            None
        };
        if present != Some(false) {
            return;
        }
        // The interpreter also binds authored functions/closures as methods.
        // Builtins and rule families are not part of that fallback. Tuples
        // have only positional projections in the existing runtime contract.
        if tuple_size.is_none() && self.ordinary_bound_method_available(field) {
            return;
        }
        self.error_at_expr(
            expression,
            format!("type `{type_name}` has no field `{field}` or method with that name"),
        );
    }

    /// A field of a sum type is readable when every variant declares it with
    /// the same type, or when a match arm has refined the value to a variant
    /// that declares it. Returns true when the field was judged here.
    fn check_sum_type_field(
        &mut self,
        expression: &Expr,
        base: &Expr,
        owner: &str,
        field: &str,
    ) -> bool {
        let Some(variants) = self.type_variants.get(owner).cloned() else {
            return false;
        };
        let carriers = variants
            .iter()
            .filter_map(|variant| {
                let signature = self.constructor_signature_for_parent(variant, Some(owner))?;
                if signature.parent != owner {
                    return None;
                }
                let index = signature.fields.iter().position(|name| name == field)?;
                Some((
                    variant.clone(),
                    signature.field_tys.get(index).cloned().flatten(),
                ))
            })
            .collect::<Vec<_>>();
        if carriers.is_empty() {
            return false;
        }
        if let Some(variant) = self.refined_variant(base) {
            if !carriers.iter().any(|(carrier, _)| carrier == &variant) {
                self.error_at_expr(
                    expression,
                    format!(
                        "variant `{variant}` of `{owner}` has no field `{field}`; this match arm has refined the value to `{variant}`"
                    ),
                );
            }
            return true;
        }
        let uniform = carriers.iter().all(|(_, ty)| ty == &carriers[0].1);
        if carriers.len() == variants.len() && uniform {
            return true;
        }
        if carriers.len() == variants.len() {
            let types = carriers
                .iter()
                .map(|(variant, ty)| format!("`{}` in `{variant}`", ty.as_deref().unwrap_or("_")))
                .collect::<Vec<_>>()
                .join(", ");
            self.error_at_expr(
                expression,
                format!(
                    "field `{field}` has different types in the variants of `{owner}` ({types}); match on the variant before reading it"
                ),
            );
        } else {
            let names = carriers
                .iter()
                .map(|(variant, _)| format!("`{variant}`"))
                .collect::<Vec<_>>()
                .join(", ");
            self.error_at_expr(
                expression,
                format!(
                    "field `{field}` exists only on variant{} {names} of `{owner}`; match on the variant first, e.g. `| {} -> ...`",
                    if carriers.len() == 1 { "" } else { "s" },
                    carriers[0].0
                ),
            );
        }
        true
    }

    /// The variant a named value is known to be: refined by an enclosing match
    /// arm (unless rebound inside the arm) or bound directly to a constructor
    /// call (`= c = Circle(5.0)`).
    pub(super) fn refined_variant(&self, base: &Expr) -> Option<String> {
        let ExprKind::Var(name) = &base.kind else {
            return None;
        };
        let (depth, _, variant, by_binding) = self
            .variant_refinements
            .iter()
            .rev()
            .find(|(_, subject, _, _)| subject == name)?;
        let binder = self.scopes.iter().rposition(|names| names.contains(name))?;
        let valid = if *by_binding {
            binder == *depth
        } else {
            binder < *depth
        };
        valid.then(|| variant.clone())
    }

    pub(super) fn record_binding_variant_refinement(&mut self, name: &str, value: &Expr) {
        let depth = self.scopes.len().saturating_sub(1);
        self.variant_refinements
            .retain(|(scope, subject, _, _)| !(*scope == depth && subject == name));
        let variant_parent = match &value.kind {
            ExprKind::App(function, arguments) => match &function.kind {
                ExprKind::Var(constructor) if !self.var_defined(constructor) => self
                    .constructor_parent_for_args(constructor, arguments)
                    .map(|parent| (constructor.clone(), parent)),
                _ => None,
            },
            ExprKind::Var(constructor) if !self.var_defined(constructor) => self
                .nullary_constructor_parent(constructor)
                .map(|parent| (constructor.clone(), parent)),
            _ => None,
        };
        if let Some((variant, parent)) = variant_parent {
            if self.type_variants.contains_key(&parent) {
                self.variant_refinements
                    .push((depth, name.to_string(), variant, true));
            }
        }
    }

    pub(super) fn pattern_variant_name(pattern: &Pat) -> Option<&str> {
        match pattern {
            Pat::Con(name, _) | Pat::NamedCon(name, _) => Some(name),
            Pat::As(inner, _) => Self::pattern_variant_name(inner),
            Pat::Var(_) | Pat::Wild | Pat::Lit(_) => None,
        }
    }

    fn ordinary_bound_method_available(&self, name: &str) -> bool {
        match self.ordinary_callable(name, None) {
            Some(Some(DeclaredCallable::Function(_))) => true,
            Some(Some(DeclaredCallable::Rule(_))) => false,
            Some(None) => {
                // Unknown lexical values may be callbacks; known scalars may
                // still fall back to an authored method declaration.
                let ty = self
                    .scopes
                    .iter()
                    .zip(&self.var_types)
                    .rev()
                    .find(|(names, _)| names.contains(name))
                    .and_then(|(_, types)| types.get(name))
                    .and_then(|ty| parse_type_annotation(ty).ok());
                !ty.as_ref().is_some_and(Self::ordinary_non_callable_type)
                    || self.user_functions.contains(name)
            }
            None => {
                self.function_params.contains_key(name)
                    && !self.rule_arities.iter().any(|(rule, _)| rule == name)
            }
        }
    }

    fn ordinary_non_callable_type(ty: &Ty) -> bool {
        match ty {
            Ty::Name(name) => matches!(
                name.as_str(),
                "Int" | "Float" | "Bool" | "String" | "Char" | "Unit"
            ),
            Ty::Unit => true,
            Ty::App(head, _) => {
                matches!(head.as_ref(), Ty::Name(name) if matches!(name.as_str(), "List" | "Map" | "Set" | "Tuple"))
            }
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
                Self::ordinary_non_callable_type(inner)
            }
            _ => false,
        }
    }

    fn ordinary_argument_matches(
        actual: &Ty,
        expected: &Ty,
        substitutions: &mut BTreeMap<String, Ty>,
    ) -> bool {
        match (actual, expected) {
            // T? is the same constructor application as Option(T), including
            // contextual None and generic constraints shared across arguments.
            (Ty::Optional(inner), _) => Self::ordinary_argument_matches(
                &Ty::App(Box::new(Ty::Name("Option".into())), vec![(**inner).clone()]),
                expected,
                substitutions,
            ),
            (_, Ty::Optional(inner)) => Self::ordinary_argument_matches(
                actual,
                &Ty::App(Box::new(Ty::Name("Option".into())), vec![(**inner).clone()]),
                substitutions,
            ),
            (Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner), _) => {
                Self::ordinary_argument_matches(inner, expected, substitutions)
            }
            (_, Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner)) => {
                Self::ordinary_argument_matches(actual, inner, substitutions)
            }
            (Ty::Hole | Ty::Var(_), _) | (_, Ty::Hole) => true,
            (_, Ty::Var(name)) if name == "_" => true,
            (_, Ty::Var(name)) => {
                if let Some(known) = substitutions.get(name).cloned() {
                    Self::ordinary_argument_matches(actual, &known, substitutions)
                } else {
                    substitutions.insert(name.clone(), actual.clone());
                    true
                }
            }
            (Ty::App(ac, aa), Ty::App(ec, ea)) => {
                aa.len() == ea.len()
                    && Self::ordinary_argument_matches(ac, ec, substitutions)
                    && aa
                        .iter()
                        .zip(ea)
                        .all(|(a, e)| Self::ordinary_argument_matches(a, e, substitutions))
            }
            (Ty::Arrow(ai, ao), Ty::Arrow(ei, eo)) => {
                Self::ordinary_argument_matches(ai, ei, substitutions)
                    && Self::ordinary_argument_matches(ao, eo, substitutions)
            }
            // A nominal result may have unknown generic arguments in ordinary
            // inference. Absence of evidence does not establish a mismatch.
            (Ty::Name(a), Ty::App(e, _)) => matches!(e.as_ref(), Ty::Name(e) if a == e),
            _ => {
                Self::canonical_explore_ty_name(actual) == Self::canonical_explore_ty_name(expected)
            }
        }
    }
}
