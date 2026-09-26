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
                                    "argument `{}` to `{name}` expects `{}`, got `{actual}`",
                                    parameter.name,
                                    Self::canonical_explore_ty_name(expected)
                                ),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        true
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
            (Ty::Name(a), Ty::Name(e)) if a == "Int" && e == "Float" => true,
            _ => {
                Self::canonical_explore_ty_name(actual) == Self::canonical_explore_ty_name(expected)
            }
        }
    }
}
