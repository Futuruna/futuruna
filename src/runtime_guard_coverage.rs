//! A narrow runtime dispatch judgment, separate from total-value proofs.
//! Complementary guards over stable inputs and complete nullary-enum cases
//! cannot all miss.
//! Guard expressions containing calls, arithmetic, indexing or floats, and
//! refutable heads, stay unsupported. Matching caller guards can also discharge
//! a direct helper call's runtime miss obligation at that exact call site.

use super::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Root {
    Argument(usize, String),
    Capture(String, String),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    Int(i64),
    Bool(bool),
    Field(Root, Vec<String>),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Atom {
    True,
    Boolean(Value),
    Less(Value, Value),
    LessEqual(Value, Value),
    Equal(Value, Value),
    Variant(Value, String, String),
}

/// Runtime-only permissions bound to the exact checked candidate and call AST.
/// The private sets cannot be populated by a backend or proof consumer.
#[derive(Clone, Default)]
pub struct RuntimeGuardedRuleCalls {
    candidates: BTreeMap<String, RuntimeGuardedCallSet>,
}

#[derive(Clone, Default)]
pub struct RuntimeGuardedCallSet {
    calls: BTreeSet<(RuleDispatchKey, String)>,
}

impl RuntimeGuardedRuleCalls {
    pub fn for_candidate(&self, rule: &Rule) -> RuntimeGuardedCallSet {
        self.candidates
            .get(&format!("{rule:?}"))
            .cloned()
            .unwrap_or_default()
    }
}

impl RuntimeGuardedCallSet {
    pub fn permits(&self, key: &RuleDispatchKey, function: &Expr, arguments: &[Expr]) -> bool {
        self.calls
            .contains(&(key.clone(), format!("{function:?}:{arguments:?}")))
    }
}

struct GuardContext<'a> {
    locals: BTreeMap<String, String>,
    roots: BTreeMap<String, Root>,
    condition: &'a Expr,
}

impl TypeChecker {
    pub(super) fn runtime_guarded_calls_for_groups(
        &self,
        groups: &BTreeMap<RuleDispatchKey, (BTreeMap<String, String>, Vec<&Rule>)>,
    ) -> RuntimeGuardedRuleCalls {
        let mut checked = RuntimeGuardedRuleCalls::default();
        if self.rule_dispatch_has_opaque_runtime_graph {
            return checked;
        }
        for (caller, (captures, rules)) in groups {
            // This first bounded judgment covers global rules only. It never
            // transports conditions through dynamic receivers or scope captures.
            if caller.scope.is_some()
                || !self.rule_dispatch_backend_return_types.contains_key(caller)
            {
                continue;
            }
            for rule in rules {
                let Some(context) = self.stable_rule_guard_context(None, captures, rule) else {
                    continue;
                };
                let Some(guard) =
                    self.stable_guard_atom(context.condition, &context.locals, &context.roots)
                else {
                    continue;
                };
                let mut value = match rule {
                    Rule::Default { value, .. } | Rule::Exception { value, .. } => value,
                    _ => continue,
                };
                // A direct call and its field projections have no preceding
                // statements, rebinding, callbacks or effects that can invalidate
                // the caller's stable input. Other expression forms stay closed.
                while let ExprKind::Field(base, _) = &value.kind {
                    value = base;
                }
                let ExprKind::App(function, arguments) = &value.kind else {
                    continue;
                };
                let ExprKind::Var(name) = &function.kind else {
                    continue;
                };
                let key = RuleDispatchKey {
                    scope: None,
                    name: name.clone(),
                    arity: arguments.len(),
                };
                if context.locals.contains_key(name)
                    || self
                        .explore_top_level_binding_initializers
                        .contains_key(name)
                    || self.explore_unsupported_top_level_bindings.contains(name)
                    || self
                        .explore_function_definitions_by_arity
                        .contains_key(&(name.clone(), arguments.len()))
                    || self.constructor_signatures.contains_key(name)
                    || !self.rule_dispatch_backend_return_types.contains_key(&key)
                    || self.rule_dispatch_runtime_irrefutable_keys.contains(&key)
                {
                    continue;
                }
                let Some((callee_captures, candidates)) = groups.get(&key) else {
                    continue;
                };
                if !arguments.iter().all(|argument| matches!(&argument.kind,
                    ExprKind::Var(name) if matches!(context.roots.get(name), Some(Root::Argument(..))))) {
                    continue;
                }
                let mut matches_guard = false;
                let mut stable = true;
                for candidate in candidates {
                    let Some(mut callee) =
                        self.stable_rule_guard_context(None, callee_captures, candidate)
                    else {
                        stable = false;
                        break;
                    };
                    for root in callee.roots.values_mut() {
                        let Root::Argument(index, expected) = root else {
                            stable = false;
                            break;
                        };
                        let Some(Expr {
                            kind: ExprKind::Var(argument),
                            ..
                        }) = arguments.get(*index)
                        else {
                            stable = false;
                            break;
                        };
                        let Some(actual @ Root::Argument(_, actual_type)) =
                            context.roots.get(argument)
                        else {
                            stable = false;
                            break;
                        };
                        if Self::canonical_explore_type_name(expected)
                            != Self::canonical_explore_type_name(actual_type)
                        {
                            stable = false;
                            break;
                        }
                        *root = actual.clone();
                    }
                    if !stable {
                        break;
                    }
                    let Some(callee_guard) =
                        self.stable_guard_atom(callee.condition, &callee.locals, &callee.roots)
                    else {
                        stable = false;
                        break;
                    };
                    matches_guard |= guard == callee_guard;
                }
                if stable && matches_guard {
                    checked
                        .candidates
                        .entry(format!("{rule:?}"))
                        .or_default()
                        .calls
                        .insert((key, format!("{function:?}:{arguments:?}")));
                }
            }
        }
        checked
    }

    fn stable_guard_enum_variants(&self, owner: &str) -> Option<&Vec<String>> {
        let variants = self.type_variants.get(owner)?;
        if variants.is_empty() || !variants.iter().all(|variant| {
            self.constructor_signatures.get(variant).is_some_and(|signatures| {
                matches!(signatures.as_slice(), [signature] if signature.parent == owner && signature.fields.is_empty())
            })
        }) {
            return None;
        }
        Some(variants)
    }

    fn stable_guard_enum_comparison(
        &self,
        expression: &Expr,
        locals: &BTreeMap<String, String>,
        roots: &BTreeMap<String, Root>,
    ) -> Option<(Atom, bool)> {
        let ExprKind::BinOp(operator, left, right) = &expression.kind else {
            return None;
        };
        let positive = match operator.as_str() {
            "==" => true,
            "!=" => false,
            _ => return None,
        };
        for (value, constructor) in [(left, right), (right, left)] {
            let Some(value_path) = self.stable_guard_access(value, locals, roots) else {
                continue;
            };
            let Some(ty) = self.infer_expr_type_name_with_locals(value, locals) else {
                continue;
            };
            let Ok(Ty::Name(owner)) = parse_type_annotation(&ty) else {
                continue;
            };
            let Some(variants) = self.stable_guard_enum_variants(&owner) else {
                continue;
            };
            let ExprKind::Var(name) = &constructor.kind else {
                continue;
            };
            if !variants.contains(name)
                || locals.contains_key(name)
                || self
                    .explore_top_level_binding_initializers
                    .contains_key(name)
                || self.explore_unsupported_top_level_bindings.contains(name)
                || self
                    .explore_function_definitions_by_arity
                    .contains_key(&(name.clone(), 0))
                || self
                    .rule_dispatch_keys
                    .iter()
                    .any(|key| key.name == *name && key.arity == 0)
            {
                continue;
            }
            return Some((Atom::Variant(value_path, owner, name.clone()), positive));
        }
        None
    }

    fn stable_guard_access(
        &self,
        expression: &Expr,
        locals: &BTreeMap<String, String>,
        roots: &BTreeMap<String, Root>,
    ) -> Option<Value> {
        match &expression.kind {
            ExprKind::Var(name) => Some(Value::Field(roots.get(name)?.clone(), Vec::new())),
            ExprKind::Field(base, field) => {
                let Value::Field(root, mut path) = self.stable_guard_access(base, locals, roots)?
                else {
                    return None;
                };
                let base_type = self.infer_expr_type_name_with_locals(base, locals)?;
                let owner = Self::canonical_nominal_owner(&base_type)?;
                // Legacy type names erase wrappers. Inspect the declared field
                // type so optional or shared/reference paths cannot be certified.
                let ty = self.type_field_tys.get(&owner)?.get(field)?;
                if !matches!(ty, Ty::Name(_) | Ty::App(_, _)) {
                    return None;
                }
                path.push(field.clone());
                Some(Value::Field(root, path))
            }
            _ => None,
        }
    }

    fn stable_guard_value(
        &self,
        expression: &Expr,
        locals: &BTreeMap<String, String>,
        roots: &BTreeMap<String, Root>,
    ) -> Option<(Value, &'static str)> {
        match &expression.kind {
            ExprKind::Lit(Literal::Int(value)) => Some((Value::Int(*value), "Int")),
            ExprKind::Lit(Literal::Bool(value)) => Some((Value::Bool(*value), "Bool")),
            ExprKind::UnOp(operator, inner) if operator == "-" => {
                let (Value::Int(value), _) = self.stable_guard_value(inner, locals, roots)? else {
                    return None;
                };
                Some((Value::Int(value.checked_neg()?), "Int"))
            }
            _ => {
                let value = self.stable_guard_access(expression, locals, roots)?;
                let ty = self.infer_expr_type_name_with_locals(expression, locals)?;
                match Self::canonical_explore_type_name(&ty).as_str() {
                    "Int" => Some((value, "Int")),
                    "Bool" => Some((value, "Bool")),
                    _ => None,
                }
            }
        }
    }

    fn stable_guard_atom(
        &self,
        expression: &Expr,
        locals: &BTreeMap<String, String>,
        roots: &BTreeMap<String, Root>,
    ) -> Option<(Atom, bool)> {
        if let ExprKind::UnOp(operator, inner) = &expression.kind {
            if operator == "!" {
                let (atom, positive) = self.stable_guard_atom(inner, locals, roots)?;
                return Some((atom, !positive));
            }
        }
        if let Some((value, "Bool")) = self.stable_guard_value(expression, locals, roots) {
            return Some(match value {
                Value::Bool(value) => (Atom::True, value),
                value => (Atom::Boolean(value), true),
            });
        }
        if let Some(atom) = self.stable_guard_enum_comparison(expression, locals, roots) {
            return Some(atom);
        }
        let ExprKind::BinOp(operator, left, right) = &expression.kind else {
            return None;
        };
        let (mut left, left_type) = self.stable_guard_value(left, locals, roots)?;
        let (mut right, right_type) = self.stable_guard_value(right, locals, roots)?;
        if left_type != right_type {
            return None;
        }
        let mut operator = operator.as_str();
        if left > right {
            std::mem::swap(&mut left, &mut right);
            operator = match operator {
                "<" => ">",
                "<=" => ">=",
                ">" => "<",
                ">=" => "<=",
                "==" => "==",
                "!=" => "!=",
                _ => return None,
            };
        }
        if left_type == "Bool" {
            let equal = match operator {
                "==" => true,
                "!=" => false,
                _ => return None,
            };
            return match (left, right) {
                (Value::Bool(a), Value::Bool(b)) => Some((Atom::True, (a == b) == equal)),
                (Value::Bool(value), other) => Some((Atom::Boolean(other), value == equal)),
                (left, right) => Some((Atom::Equal(left, right), equal)),
            };
        }
        Some(match operator {
            "<" => (Atom::Less(left, right), true),
            ">=" => (Atom::Less(left, right), false),
            "<=" => (Atom::LessEqual(left, right), true),
            ">" => (Atom::LessEqual(left, right), false),
            "==" => (Atom::Equal(left, right), true),
            "!=" => (Atom::Equal(left, right), false),
            _ => return None,
        })
    }

    fn stable_guard_alternatives(
        &self,
        expression: &Expr,
        locals: &BTreeMap<String, String>,
        roots: &BTreeMap<String, Root>,
    ) -> Option<BTreeSet<(Atom, bool)>> {
        if let ExprKind::BinOp(operator, left, right) = &expression.kind {
            if operator == "||" {
                let mut alternatives = self.stable_guard_alternatives(left, locals, roots)?;
                alternatives.extend(self.stable_guard_alternatives(right, locals, roots)?);
                return Some(alternatives);
            }
        }
        Some(BTreeSet::from([
            self.stable_guard_atom(expression, locals, roots)?
        ]))
    }

    fn stable_rule_guard_context<'a>(
        &self,
        scope: Option<&str>,
        captures: &BTreeMap<String, String>,
        rule: &'a Rule,
    ) -> Option<GuardContext<'a>> {
        let (head, condition) = match rule {
            Rule::Default {
                head,
                condition: Some(condition),
                ..
            }
            | Rule::Exception {
                head,
                condition: Some(condition),
                ..
            } => (head, condition),
            _ => return None,
        };
        if !rule_head_is_irrefutable(rule) {
            return None;
        }
        let mut locals = captures.clone();
        locals.extend(self.rule_head_local_types(head));
        let mut roots = captures
            .keys()
            .filter_map(|name| {
                let ty = self.type_field_tys.get(scope?)?.get(name)?;
                match ty {
                    Ty::Name(_) | Ty::App(_, _) => {
                        Some((name.clone(), Root::Capture(name.clone(), ty.to_string())))
                    }
                    _ => None,
                }
            })
            .collect::<BTreeMap<_, _>>();
        let arguments = match &head.kind {
            ExprKind::App(_, arguments) => arguments.as_slice(),
            ExprKind::Var(_) => &[],
            _ => return None,
        };
        let mut bound = BTreeSet::new();
        for (index, argument) in arguments.iter().enumerate() {
            let annotation = typed_rule_head_argument(argument);
            let argument = annotation.map(|(inner, _)| inner).unwrap_or(argument);
            let ExprKind::Var(name) = &argument.kind else {
                return None;
            };
            roots.remove(name);
            if name == "_" {
                continue;
            }
            if !bound.insert(name) {
                return None;
            }
            // An untyped head binder must not inherit a capture's type
            // merely because it shadows that capture's spelling.
            let Some((_, ty)) = annotation else {
                return None;
            };
            if !matches!(parse_type_annotation(ty), Ok(Ty::Name(_) | Ty::App(_, _))) {
                return None;
            }
            roots.insert(name.clone(), Root::Argument(index, ty.to_owned()));
        }
        Some(GuardContext {
            locals,
            roots,
            condition,
        })
    }

    pub(super) fn runtime_complementary_guards_cover(
        &self,
        scope: Option<&str>,
        captures: &BTreeMap<String, String>,
        rules: &[&Rule],
    ) -> bool {
        let mut guards = BTreeSet::new();
        for rule in rules {
            let Some(GuardContext {
                locals,
                roots,
                condition,
            }) = self.stable_rule_guard_context(scope, captures, rule)
            else {
                return false;
            };
            // Every OR operand must be stable: no unsupported expression may
            // be evaluated before a covered alternative is reached. AND is not
            // a union and deliberately stays outside this coverage judgment.
            let Some(alternatives) = self.stable_guard_alternatives(condition, &locals, &roots)
            else {
                return false;
            };
            guards.extend(alternatives);
        }
        guards.contains(&(Atom::True, true))
            || guards
                .iter()
                .any(|(atom, positive)| guards.contains(&(atom.clone(), !positive)))
            || guards.iter().any(|(atom, positive)| {
                let Atom::Variant(value, owner, _) = atom else {
                    return false;
                };
                *positive
                    && self
                        .stable_guard_enum_variants(owner)
                        .is_some_and(|variants| {
                            variants.iter().all(|variant| {
                                guards.contains(&(
                                    Atom::Variant(value.clone(), owner.clone(), variant.clone()),
                                    true,
                                ))
                            })
                        })
            })
    }
}
