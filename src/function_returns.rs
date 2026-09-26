//! Ordinary return annotations constrain values, including each branch of a
//! body. This is deliberately separate from Explore's closed-world admission:
//! unknown external results stay unknown; generic parameters inside a function
//! remain rigid, while a called function's parameters are instantiated.

use super::*;

#[derive(Clone, Default)]
struct ReturnLocals<'a> {
    types: BTreeMap<String, String>,
    unresolved: BTreeMap<String, Rc<ReturnBinding<'a>>>,
    native_mutable_lists: BTreeSet<String>,
}

struct ReturnBinding<'a> {
    expression: &'a Expr,
    locals: ReturnLocals<'a>,
}

impl TypeChecker {
    pub(super) fn check_function_body(
        &mut self,
        name: &str,
        params: &[Param],
        result: Option<&Ty>,
        body: &Expr,
        receiver: Option<&Ty>,
    ) {
        self.check_expr(body, Some(name));
        let Some(result) = result else { return };
        let mut locals = ReturnLocals::default();
        for (scope, types) in self.scopes.iter().zip(&self.var_types) {
            for name in scope {
                // Function declarations are also value names in the legacy
                // scope table. Their signatures are resolved separately below.
                if self.user_functions.contains(name)
                    && !self
                        .reference_symbols
                        .get(name)
                        .is_some_and(|symbols| symbols.contains(&ProgramSymbolKind::Binding))
                    && !types.contains_key(name)
                {
                    locals.types.remove(name);
                    continue;
                }
                locals.types.insert(
                    name.clone(),
                    CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string(),
                );
            }
            locals.types.extend(types.clone());
        }
        for (index, param) in params.iter().enumerate() {
            let native_list = match param.ty.as_ref() {
                Some(Ty::Shared(inner)) => inner.as_ref(),
                Some(ty) => ty,
                None => &Ty::Hole,
            };
            if param.inout
                && matches!(native_list, Ty::App(constructor, _) if matches!(constructor.as_ref(), Ty::Name(name) if name == "List"))
            {
                locals.native_mutable_lists.insert(param.name.clone());
            }
            let ty = param
                .ty
                .as_ref()
                .or_else(|| (index == 0).then_some(receiver).flatten());
            if let Some(ty) = ty {
                locals
                    .types
                    .insert(param.name.clone(), Self::canonical_explore_ty_name(ty));
            } else {
                // Parameters shadow earlier value and function declarations.
                if param.name != "self" || !locals.types.contains_key("self") {
                    locals.types.insert(
                        param.name.clone(),
                        CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string(),
                    );
                }
            }
        }
        let result = match receiver {
            Some(receiver) => Self::canonical_substitute_type_parameters(
                result,
                &BTreeMap::from([("Self".into(), receiver.clone())]),
            ),
            None => result.clone(),
        };
        self.check_return_expression(name, &result, body, &locals);
    }

    fn return_type_mismatch(&mut self, name: &str, expected: &Ty, actual: &str, expr: &Expr) {
        self.error_at_expr(
            expr,
            format!(
            "return type mismatch: function `{name}` expects `{}` but expression has `{actual}`",
            Self::canonical_explore_ty_name(expected),
        ),
        );
    }

    // A missing inferred argument is a hole, not a fresh rigid variable. Bare
    // constructor parents are partial inference (e.g. None), not proof of a
    // particular type argument. The contextual constructor walk checks fields.
    fn return_types_compatible(actual: &Ty, expected: &Ty) -> bool {
        let is_hole =
            |ty: &Ty| matches!(ty, Ty::Hole) || matches!(ty, Ty::Var(name) if name == "_");
        if is_hole(actual) || is_hole(expected) {
            return true;
        }
        match (actual, expected) {
            (Ty::App(ac, aa), Ty::App(ec, ea)) => {
                Self::return_types_compatible(ac, ec)
                    && aa.len() == ea.len()
                    && aa
                        .iter()
                        .zip(ea)
                        .all(|(a, e)| Self::return_types_compatible(a, e))
            }
            (Ty::Name(a), Ty::App(e, _)) => matches!(e.as_ref(), Ty::Name(e) if a == e),
            (Ty::Arrow(ai, ao), Ty::Arrow(ei, eo)) => {
                Self::return_types_compatible(ai, ei) && Self::return_types_compatible(ao, eo)
            }
            _ => {
                Self::canonical_explore_ty_name(actual) == Self::canonical_explore_ty_name(expected)
            }
        }
    }

    fn infer_return_type(&self, expr: &Expr, locals: &ReturnLocals<'_>) -> Option<String> {
        // Native inout List parameters lower to &mut Vec, where push mutates
        // the receiver and returns unit. Ordinary persistent push returns a
        // new list. Retain this established native contract without changing
        // the interpreter's separate inout support or certifying Explore code.
        if let ExprKind::App(function, arguments) = &expr.kind {
            if matches!(&function.kind, ExprKind::Var(callee)
                if builtin_canonical(callee) == "push"
                    && !locals.types.contains_key(callee)
                    && !self.explore_contextual_intrinsic_is_shadowed(callee, arguments.len()))
                && arguments.len() == 2
                && matches!(&arguments[0].kind, ExprKind::Var(target) if locals.native_mutable_lists.contains(target))
            {
                return Some("()".into());
            }
        }

        if let ExprKind::App(callee, arguments) = &expr.kind {
            if let ExprKind::Var(name) = &callee.kind {
                if let Some(local) = locals.types.get(name) {
                    return match parse_type_annotation(local).ok()? {
                        Ty::Arrow(_, result) => Some(Self::canonical_explore_ty_name(&result)),
                        _ => None,
                    };
                }
                if let Some(definitions) = self
                    .explore_function_definitions_by_arity
                    .get(&(name.clone(), arguments.len()))
                {
                    let [(parameters, Some(result), _)] = definitions.as_slice() else {
                        return None;
                    };
                    let names = parameters
                        .iter()
                        .map(|param| Some(param.name.clone()))
                        .collect::<Vec<_>>();
                    let ordered = Self::canonical_ordered_arguments(arguments, &names)?;
                    let mut substitutions = BTreeMap::new();
                    for (argument, parameter) in ordered.iter().zip(parameters) {
                        if let (Some(actual), Some(expected)) = (
                            self.infer_return_type(argument, locals),
                            parameter.ty.as_ref(),
                        ) {
                            // Only use a consistent substitution. This utility
                            // does not require closed schemas or prove totality.
                            if !self.canonical_schema_accepts_type(
                                &actual,
                                expected,
                                &mut substitutions,
                            ) {
                                return None;
                            }
                        }
                    }
                    fn instantiate(ty: &Ty, substitutions: &BTreeMap<String, String>) -> Ty {
                        match ty {
                            Ty::Var(name) => substitutions
                                .get(name)
                                .and_then(|s| parse_type_annotation(s).ok())
                                .unwrap_or(Ty::Hole),
                            Ty::App(c, args) => Ty::App(
                                c.clone(),
                                args.iter().map(|a| instantiate(a, substitutions)).collect(),
                            ),
                            Ty::Arrow(a, b) => Ty::Arrow(
                                Box::new(instantiate(a, substitutions)),
                                Box::new(instantiate(b, substitutions)),
                            ),
                            Ty::Optional(t) => {
                                Ty::Optional(Box::new(instantiate(t, substitutions)))
                            }
                            Ty::Ref(t) => Ty::Ref(Box::new(instantiate(t, substitutions))),
                            Ty::MutRef(t) => Ty::MutRef(Box::new(instantiate(t, substitutions))),
                            Ty::Shared(t) => Ty::Shared(Box::new(instantiate(t, substitutions))),
                            _ => ty.clone(),
                        }
                    }
                    return Some(Self::canonical_explore_ty_name(&instantiate(
                        result,
                        &substitutions,
                    )));
                }
            }
        }
        self.infer_expr_type_name_with_locals(expr, &locals.types)
    }

    fn extend_return_bindings(
        &self,
        locals: &mut ReturnLocals<'_>,
        pattern: &Pat,
        ty: Option<&str>,
    ) {
        let mut names = BTreeSet::new();
        collect_pattern_names(pattern, &mut names);
        for name in names {
            locals.unresolved.remove(&name);
            locals.native_mutable_lists.remove(&name);
            locals
                .types
                .insert(name, CHECKED_UNTYPED_SHADOW_TYPE_TOMBSTONE.to_string());
        }
        locals.types.extend(self.pattern_type_bindings(pattern, ty));
    }

    fn check_return_expression<'a>(
        &mut self,
        name: &str,
        expected: &Ty,
        expr: &'a Expr,
        locals: &ReturnLocals<'a>,
    ) {
        // Normalize Optional and Unit sugar before structural comparison.
        let expected = parse_type_annotation(&Self::canonical_explore_ty_name(expected))
            .unwrap_or_else(|_| expected.clone());
        if matches!(&expected, Ty::Hole) || matches!(&expected, Ty::Var(n) if n == "_") {
            return;
        }
        match &expr.kind {
            ExprKind::Var(binding) if locals.unresolved.contains_key(binding) => {
                let binding = &locals.unresolved[binding];
                self.check_return_expression(name, &expected, binding.expression, &binding.locals);
                return;
            }
            ExprKind::If(_, yes, no) => {
                self.check_return_expression(name, &expected, yes, locals);
                self.check_return_expression(name, &expected, no, locals);
                return;
            }
            ExprKind::Match(subject, arms) => {
                let subject_type = self.infer_return_type(subject, locals);
                for arm in arms {
                    let mut arm_locals = locals.clone();
                    self.extend_return_bindings(&mut arm_locals, &arm.pat, subject_type.as_deref());
                    self.check_return_expression(name, &expected, &arm.body, &arm_locals);
                }
                return;
            }
            ExprKind::Block(statements) => {
                let mut block_locals = locals.clone();
                for (index, statement) in statements.iter().enumerate() {
                    if index + 1 == statements.len() {
                        match statement {
                            Stmt::Expr(value) | Stmt::Bind(_, _, value) => {
                                self.check_return_expression(name, &expected, value, &block_locals);
                            }
                            // Other statement forms have independent execution
                            // semantics, including propagation and Rust blocks.
                            _ => {}
                        }
                        return;
                    }
                    if let Stmt::Bind(pattern, annotation, value) = statement {
                        let ty = annotation
                            .as_ref()
                            .map(Self::canonical_explore_ty_name)
                            .or_else(|| self.infer_return_type(value, &block_locals));
                        // Retain the lexical initializer when inference cannot
                        // choose one type (for example, a mixed-type branch).
                        // Rc keeps alias chains as a DAG without copying ASTs.
                        let pending = if ty.is_none() {
                            Some(Rc::new(ReturnBinding {
                                expression: value,
                                locals: block_locals.clone(),
                            }))
                        } else {
                            None
                        };
                        self.extend_return_bindings(&mut block_locals, pattern, ty.as_deref());
                        if let (Pat::Var(binding), Some(pending)) = (pattern, pending) {
                            block_locals.unresolved.insert(binding.clone(), pending);
                        }
                    } else if let Stmt::MonadicBind(pattern, annotation, value) = statement {
                        let ty = annotation.as_ref().map(Self::canonical_explore_ty_name).or_else(|| {
                            let wrapped = self.infer_return_type(value, &block_locals)?;
                            match parse_type_annotation(&wrapped).ok()? {
                                Ty::App(constructor, arguments) if matches!(constructor.as_ref(), Ty::Name(n) if n == "Option" || n == "Result") => arguments.first().map(Self::canonical_explore_ty_name),
                                _ => None,
                            }
                        });
                        self.extend_return_bindings(&mut block_locals, pattern, ty.as_deref());
                    }
                }
                if !Self::return_types_compatible(&Ty::Unit, &expected) {
                    self.return_type_mismatch(name, &expected, "()", expr);
                }
                return;
            }
            ExprKind::List(items) => {
                if let Ty::App(constructor, arguments) = &expected {
                    if matches!(constructor.as_ref(), Ty::Name(n) if n == "List")
                        && arguments.len() == 1
                    {
                        for item in items {
                            self.check_return_expression(name, &arguments[0], item, locals);
                        }
                        return;
                    }
                }
                self.return_type_mismatch(name, &expected, "List(_)", expr);
                return;
            }
            ExprKind::Lambda(parameters, body) => {
                if let Ty::Arrow(input, output) = &expected {
                    let inputs = match input.as_ref() {
                        Ty::Unit if parameters.is_empty() => vec![],
                        Ty::App(c, args) if matches!(c.as_ref(), Ty::Name(n) if n == "Tuple") => {
                            args.iter().collect()
                        }
                        input => vec![input],
                    };
                    if inputs.len() == parameters.len() {
                        let mut body_locals = locals.clone();
                        for (parameter, input) in parameters.iter().zip(inputs) {
                            body_locals.unresolved.remove(&parameter.name);
                            body_locals.native_mutable_lists.remove(&parameter.name);
                            if let Some(annotation) = &parameter.ty {
                                if !Self::return_types_compatible(annotation, input) {
                                    self.return_type_mismatch(
                                        name,
                                        input,
                                        &annotation.to_string(),
                                        expr,
                                    );
                                }
                            }
                            body_locals.types.insert(
                                parameter.name.clone(),
                                Self::canonical_explore_ty_name(input),
                            );
                        }
                        self.check_return_expression(name, output, body, &body_locals);
                        return;
                    }
                }
                self.return_type_mismatch(name, &expected, "closure", expr);
                return;
            }
            _ => {}
        }

        let constructor = match &expr.kind {
            ExprKind::Var(constructor) if !locals.types.contains_key(constructor) => {
                Some((constructor.as_str(), &[][..]))
            }
            ExprKind::App(callee, arguments) => match &callee.kind {
                ExprKind::Var(constructor) if !locals.types.contains_key(constructor) => {
                    Some((constructor.as_str(), arguments.as_slice()))
                }
                _ => None,
            },
            _ => None,
        };
        if let Some((constructor, arguments)) = constructor {
            let parent = Self::canonical_explore_ty_name(&expected);
            let owner = Self::canonical_nominal_owner(&parent);
            let signature = self
                .constructor_signatures
                .get(constructor)
                .and_then(|signatures| {
                    signatures
                        .iter()
                        .find(|signature| {
                            Some(signature.parent.as_str()) == owner.as_deref()
                                && signature.arity() == arguments.len()
                        })
                        .cloned()
                });
            if let Some(signature) = signature {
                for (position, argument) in arguments.iter().enumerate() {
                    let (index, value) = match named_arg_parts(argument) {
                        Some((field, value)) => match signature
                            .fields
                            .iter()
                            .position(|candidate| candidate == field)
                        {
                            Some(index) => (index, value),
                            None => continue, // The ordinary constructor check reports this.
                        },
                        None => (position, argument),
                    };
                    if let Some(field) = signature
                        .field_tys
                        .get(index)
                        .and_then(Option::as_ref)
                        .and_then(|s| parse_type_annotation(s).ok())
                    {
                        let field = self.canonical_instantiate_nominal_type(&parent, &field);
                        if let Ok(field) = parse_type_annotation(&field) {
                            self.check_return_expression(name, &field, value, locals);
                        }
                    }
                }
                return;
            }
        }
        if let Some(actual) = self.infer_return_type(expr, locals) {
            if let Ok(actual_ty) = parse_type_annotation(&actual) {
                if !Self::return_types_compatible(&actual_ty, &expected) {
                    self.return_type_mismatch(name, &expected, &actual, expr);
                }
            }
        }
    }
}
