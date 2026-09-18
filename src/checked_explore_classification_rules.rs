//! Checked rule families normalized to acyclic calls and ordered conditionals.
//!
//! No rule is selected from a sample. Every retained candidate is lowered in
//! the checked dispatch order. Clause false-backtracking differs from an
//! exception/default returning false, and an unproved miss stays residual.

use super::*;
use crate::{CheckedBinderKind, CheckedRuleCandidateResolution, RuleDispatchKey, RuleDispatchTier};

const MAX_RULE_CANDIDATES: usize = 256;
const MAX_ACTIVE_RULE_FAMILIES: usize = 128;

impl<'program, 'query> CheckedClassificationProducer<'program, 'query> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_rule_application(
        &mut self,
        site: &ExprSiteId,
        arguments: &[crate::Expr],
        resolution: &CheckedExpressionResolution,
        family: &RuleDispatchKey,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        environment: &BinderEnvironment,
    ) -> LoweringResult {
        if family.scope.is_some() || family.arity != arguments.len() {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::DynamicDispatch,
                [],
            ));
        }
        let callable_id = self.ensure_rule_family(family, site)?;
        self.lower_prepared_call(
            site,
            arguments,
            resolution,
            ty,
            scalar,
            environment,
            callable_id,
        )
    }

    fn ensure_rule_family(
        &mut self,
        key: &RuleDispatchKey,
        site: &ExprSiteId,
    ) -> Result<ClassificationCallableId, LoweringError> {
        match self.rule_states.get(key).cloned() {
            Some(CallableLoweringState::Lowered(id)) => return Ok(id),
            Some(CallableLoweringState::Residual(failure)) => {
                return Err(LoweringError::Residual(failure));
            }
            Some(CallableLoweringState::Visiting) => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::RecursiveCall,
                    [],
                ));
            }
            None => {}
        }
        if self
            .rule_states
            .values()
            .filter(|state| matches!(state, CallableLoweringState::Visiting))
            .count()
            >= MAX_ACTIVE_RULE_FAMILIES
        {
            return Err(self.residual_error(site, ClassificationResidualReason::RecursiveCall, []));
        }
        let digest = checked_explore_semantic_dependency_root_digest(
            &self.index,
            self.resolutions,
            &self.semantic_binders,
            CheckedExploreSemanticDependency::RuleFamily(key.clone()),
        )
        .map_err(|_| {
            self.residual_error(site, ClassificationResidualReason::DynamicDispatch, [])
        })?;
        // Distinguish the normalization contract from ordinary function calls.
        let mut hasher = Sha256::new();
        hasher.update(b"futuruna.checked-classification-rule-dispatch.v1\0");
        hasher.update(digest);
        let callable_id =
            ClassificationCallableId::from_checked_callable_digest(hasher.finalize().into());
        self.rule_states
            .insert(key.clone(), CallableLoweringState::Visiting);
        match self.lower_rule_definition(key, callable_id, site) {
            Ok(definition) => {
                self.callable_definitions.insert(callable_id, definition);
                self.rule_states
                    .insert(key.clone(), CallableLoweringState::Lowered(callable_id));
                Ok(callable_id)
            }
            Err(LoweringError::Residual(failure)) => {
                if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                    eprintln!(
                        "Explore classification rule residual: family={key:?}; reason={:?}",
                        failure.reason
                    );
                }
                let failure = failure
                    .without_node_dependencies()
                    .with_dependency(ClassificationResidualDependency::RuleFamily(digest));
                self.rule_states.insert(
                    key.clone(),
                    CallableLoweringState::Residual(failure.clone()),
                );
                Err(LoweringError::Residual(failure))
            }
            Err(error) => Err(error),
        }
    }

    fn lower_rule_definition(
        &mut self,
        key: &RuleDispatchKey,
        callable_id: ClassificationCallableId,
        site: &ExprSiteId,
    ) -> Result<ClassificationCallableDefinition, LoweringError> {
        let family = self
            .resolutions
            .rule_families
            .get(key)
            .cloned()
            .ok_or_else(|| {
                self.residual_error(site, ClassificationResidualReason::DynamicDispatch, [])
            })?;
        let contract = self
            .resolutions
            .rule_dispatch_type_contracts
            .get(key)
            .cloned()
            .ok_or_else(|| {
                self.residual_error(site, ClassificationResidualReason::UnsupportedType, [])
            })?;
        if &family.key != key
            || contract.parameter_types.len() != key.arity
            || family.candidates.is_empty()
            || family.candidates.len() > MAX_RULE_CANDIDATES
            || family.candidates.windows(2).any(|pair| {
                (pair[0].tier, pair[0].source_order) >= (pair[1].tier, pair[1].source_order)
            })
        {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::DynamicDispatch,
                [],
            ));
        }
        let (return_type, scalar) =
            self.classification_type(&contract.result_type)
                .ok_or_else(|| {
                    self.residual_error(site, ClassificationResidualReason::UnsupportedType, [])
                })?;
        let mut parameters = Vec::with_capacity(key.arity);
        for (ordinal, parameter) in contract.parameter_types.iter().enumerate() {
            let (ty, scalar) = parameter
                .as_ref()
                .and_then(|ty| self.classification_type(ty))
                .ok_or_else(|| {
                    self.residual_error(site, ClassificationResidualReason::UnsupportedType, [])
                })?;
            parameters.push(self.intern(
                ty,
                scalar,
                ClassificationNodeKind::CallableParameter {
                    callable_id,
                    ordinal: u32::try_from(ordinal).map_err(|_| {
                        self.residual_error(site, ClassificationResidualReason::UnsupportedType, [])
                    })?,
                },
            )?);
        }

        let boolean = scalar == Some(ScalarKind::Boolean);
        let mut candidates = Vec::new();
        for candidate in &family.candidates {
            let mut environment = BinderEnvironment::new();
            let (mut guard, mut irrefutable) =
                self.lower_rule_head(candidate, &parameters, &mut environment)?;
            if let Some(condition_site) = &candidate.condition_site {
                let condition = self.lower_expression(condition_site, &environment)?;
                guard = self.boolean_and(condition_site, guard, condition)?;
                irrefutable = false;
            }
            let value = if let Some(value_site) = &candidate.value_site {
                self.lower_expression(value_site, &environment)?
            } else if candidate.tier == RuleDispatchTier::Clause {
                self.boolean_constant(site, true)?
            } else {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    [],
                ));
            };
            if value.ty != return_type {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::UnsupportedType,
                    [],
                ));
            }
            let backtracks = boolean && candidate.tier == RuleDispatchTier::Clause;
            candidates.push((guard, value, irrefutable, backtracks));
            if irrefutable && !backtracks {
                // Later candidates are unreachable; unsupported dead bodies
                // need not poison an otherwise checked total dispatch.
                break;
            }
        }
        let mut tail = if boolean
            && self
                .resolutions
                .rule_dispatch_boolean_miss_safe_keys
                .contains(key)
        {
            Some(self.boolean_constant(site, false)?)
        } else {
            None
        };
        for (guard, value, irrefutable, backtracks) in candidates.into_iter().rev() {
            let selected = if backtracks {
                if let Some(next) = tail {
                    let yes = self.boolean_constant(site, true)?;
                    self.rule_if(return_type, scalar, value, yes, next)?
                } else if irrefutable {
                    // A last, always-matching Bool clause supplies its own
                    // false fallback even without a family-wide miss theorem.
                    value
                } else {
                    return Err(self.residual_error(
                        site,
                        ClassificationResidualReason::DynamicDispatch,
                        [],
                    ));
                }
            } else {
                value
            };
            tail = Some(if irrefutable {
                selected
            } else if let Some(next) = tail {
                self.rule_if(return_type, scalar, guard, selected, next)?
            } else {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    [],
                ));
            });
        }
        Ok(ClassificationCallableDefinition {
            callable_id,
            parameter_types: parameters.iter().map(|parameter| parameter.ty).collect(),
            return_type,
            body: tail
                .ok_or_else(|| {
                    self.residual_error(site, ClassificationResidualReason::DynamicDispatch, [])
                })?
                .node,
        })
    }

    fn rule_if(
        &mut self,
        ty: ClassificationTypeId,
        scalar: Option<ScalarKind>,
        condition: LoweredValue,
        yes: LoweredValue,
        no: LoweredValue,
    ) -> LoweringResult {
        self.intern(
            ty,
            scalar,
            ClassificationNodeKind::If {
                condition: condition.node,
                then_node: yes.node,
                else_node: no.node,
            },
        )
    }

    fn lower_rule_head(
        &mut self,
        candidate: &CheckedRuleCandidateResolution,
        parameters: &[LoweredValue],
        environment: &mut BinderEnvironment,
    ) -> Result<(LoweredValue, bool), LoweringError> {
        let site = &candidate.head_site;
        let expression = self.index.expression(site).cloned().ok_or_else(|| {
            self.residual_error(site, ClassificationResidualReason::DynamicDispatch, [])
        })?;
        let argument_count = match expression.kind {
            ExprKind::App(_, arguments) => arguments.len(),
            ExprKind::Var(_) if parameters.is_empty() => 0,
            _ => {
                return Err(self.residual_error(
                    site,
                    ClassificationResidualReason::DynamicDispatch,
                    [],
                ))
            }
        };
        if argument_count != parameters.len() {
            return Err(self.residual_error(
                site,
                ClassificationResidualReason::DynamicDispatch,
                [],
            ));
        }
        let mut guard = self.boolean_constant(site, true)?;
        let mut irrefutable = true;
        for (ordinal, parameter) in parameters.iter().copied().enumerate() {
            let mut argument_site = child_site(site, ordinal + 1);
            let mut argument = self
                .index
                .expression(&argument_site)
                .cloned()
                .ok_or_else(|| {
                    self.residual_error(site, ClassificationResidualReason::DynamicDispatch, [])
                })?;
            if let Some((_, annotation)) = crate::typed_rule_head_argument(&argument) {
                let matches = crate::parse_type_annotation(annotation)
                    .ok()
                    .and_then(|ty| self.classification_type(&ty))
                    .is_some_and(|(ty, _)| ty == parameter.ty);
                if !matches {
                    return Err(self.residual_error(
                        &argument_site,
                        ClassificationResidualReason::UnsupportedType,
                        [],
                    ));
                }
                argument_site = child_site(&argument_site, 1);
                argument = self
                    .index
                    .expression(&argument_site)
                    .cloned()
                    .ok_or_else(|| {
                        self.residual_error(site, ClassificationResidualReason::DynamicDispatch, [])
                    })?;
            }
            if self
                .resolutions
                .unsupported_sites
                .contains_key(&argument_site)
            {
                return Err(self.residual_error(
                    &argument_site,
                    ClassificationResidualReason::UnsupportedExpression,
                    [],
                ));
            }
            match &argument.kind {
                ExprKind::Var(name) if name == "_" => {}
                ExprKind::Var(_) => {
                    let resolution = self
                        .resolutions
                        .expressions
                        .get(&argument_site)
                        .cloned()
                        .ok_or_else(|| {
                            self.residual_error(
                                site,
                                ClassificationResidualReason::DynamicDispatch,
                                [],
                            )
                        })?;
                    match resolution.value_binding {
                        Some(CheckedValueBinding::Binder {
                            kind: CheckedBinderKind::RuleHead,
                            site: binder,
                        }) => {
                            // Runtime rule variables bind in head order; they
                            // are not Prolog equality constraints on repeats.
                            environment.insert(binder, BinderValue::Lowered(parameter));
                        }
                        _ => {
                            return Err(self.residual_error(
                                &argument_site,
                                ClassificationResidualReason::MatchNormalizationRequired,
                                [],
                            ))
                        }
                    }
                }
                ExprKind::Lit(literal) => {
                    let test = self.lower_pattern_literal(&argument_site, literal, parameter)?;
                    guard = self.boolean_and(&argument_site, guard, test)?;
                    irrefutable = false;
                }
                _ => {
                    return Err(self.residual_error(
                        &argument_site,
                        ClassificationResidualReason::MatchNormalizationRequired,
                        [],
                    ))
                }
            }
        }
        Ok((guard, irrefutable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::relational_classification_capsule::ClassificationLaneStatus;
    use crate::{Lexer, Parser, TypeChecker};

    fn graph(definition: &str) -> std::sync::Arc<FrozenClassificationProgram> {
        let source = format!("{definition}\n? explore checked_rules {{\n    from {{\n        vary before in range(1, 301)\n        given context = ()\n    }}\n    transition after = before + 1\n    find cases = matches of q(before) > 0\n}}\n");
        let statements = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap();
        let artifacts = TypeChecker::check_with_explore_artifacts(&statements, None, &source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        artifacts
            .checked_exploration_query(0)
            .unwrap()
            .classification_program()
    }

    #[test]
    fn checked_rule_dispatch_keeps_partial_recursive_and_captured_calls_residual() {
        for definition in [
            "| q(x: Int) -> x under x > 0",
            "| q(x: Int) -> if x <= 0 { 0 } else { q(x - 1) }",
            "= captured = 2\n| q(x: Int) -> x + captured",
        ] {
            let graph = graph(definition);
            assert!(
                graph.lane_manifest().iter().any(|entry| matches!(
                    entry.lane,
                    ClassificationSemanticLane::Find(_)
                ) && entry.status
                    == ClassificationLaneStatus::Residual),
                "unsupported family was silently lowered: {definition}"
            );
            assert!(graph.validate_identity());
        }
    }

    #[test]
    fn checked_rule_dispatch_normalization_is_name_independent_and_order_sensitive() {
        let original =
            graph("| q(x: Int) -> 0\n| q(x: Int) -> 7 under x > 7\n| q(x: Int) -> 8 under x > 8");
        let renamed = graph("| q(value: Int) -> 0\n| q(value: Int) -> 7 under value > 7\n| q(value: Int) -> 8 under value > 8");
        let reversed =
            graph("| q(x: Int) -> 0\n| q(x: Int) -> 8 under x > 8\n| q(x: Int) -> 7 under x > 7");
        assert!(original
            .lane_manifest()
            .iter()
            .all(|entry| entry.status == ClassificationLaneStatus::Lowered));
        assert_eq!(original.graph_root(), renamed.graph_root());
        assert_ne!(original.graph_root(), reversed.graph_root());
    }

    #[test]
    fn checked_rule_dispatch_local_blocks_preserve_lexical_binding_identity() {
        let original = graph("| q(x: Int) -> { = y = x + 1; = y = y * 2; y - x }");
        let renamed = graph("| q(a: Int) -> { = b = a + 1; = c = b * 2; c - a }");
        assert!(original
            .lane_manifest()
            .iter()
            .all(|entry| entry.status == ClassificationLaneStatus::Lowered));
        assert_eq!(original.graph_root(), renamed.graph_root());
        assert!(original.validate_identity());
    }
}
