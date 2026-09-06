//! Checked-interpreter adapter for relational mechanism replay.
//!
//! Every uncached endpoint command constructs a fresh ground evaluator, closes
//! the immutable bindings needed by the checked observer callable, and
//! evaluates that callable under the interpreter's exact-exploration contract.
//! One complete immutable proposal may be reused for an identical checked
//! observation/state/context tuple; mutable evaluator state is never reused.
//! The raw interpreter trace retains producer-owned checked identities; this
//! module only projects those identities into the relational replay ABI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use super::relational_mechanism_executor::{
    RelationalIfDecisionOutcome, RelationalMechanismActivationPathId,
    RelationalMechanismActivationPathNode, RelationalMechanismActivationStep,
    RelationalMechanismCalleeId, RelationalMechanismEndpointReplayProgress,
    RelationalMechanismEndpointReplayRequest, RelationalMechanismEndpointTraceProposal,
    RelationalMechanismEventKind, RelationalMechanismEventOutcome, RelationalMechanismOccurrenceId,
    RelationalMechanismOccurrenceProposal, RelationalMechanismPermanentUnavailable,
    RelationalMechanismReplayError, RelationalMechanismReplayObservationId,
    RelationalMechanismReplayRuntime, RelationalMechanismSiteId, RelationalRuleAttemptOutcome,
    RelationalRuleSelectionOutcome, RelationalShortCircuitOutcome,
};
use super::transition::canonical_explore_value_digest;
use super::{
    append_required_binding, collect_ground_bindings_inner, expression_query_dependencies,
    runtime_value_from_explore_value, ExploreRuntimeGroundEvaluator, GroundDefinitions,
    MechanismRequestId, RelationalEndpointTotalityCertificateId, EXPLORE_GROUND_COLLECTION_LIMIT,
};
use crate::{
    AnalysisProgramId, CheckedAnalysisProgram, CheckedCallableId, CheckedExploreAnalysisIdentity,
    CheckedExploreQueryView, CheckedInterpreterIfDecisionOutcome,
    CheckedInterpreterMechanismActivation, CheckedInterpreterMechanismCallee,
    CheckedInterpreterMechanismCatalog, CheckedInterpreterMechanismEvaluationError,
    CheckedInterpreterMechanismEventKind, CheckedInterpreterMechanismEventOutcome,
    CheckedInterpreterMechanismEventSite, CheckedInterpreterMechanismTrace,
    CheckedInterpreterMechanismTraceError, CheckedInterpreterRuleAttemptOutcome,
    CheckedInterpreterRuleSelectionOutcome, CheckedInterpreterShortCircuitOutcome,
    CheckedMechanismRuleMemoPlan, ExploreRuntimeFailure, ModuleId, SourcedImportKind, SourcedStmt,
    Stmt, TypeCheckArtifacts,
};

// A fixed capability of replay ABI v3, not a caller-selected slice/resource
// budget. Crossing it is a typed unavailable terminal: calling the same ABI
// again at the same ceiling would otherwise strand the stream forever.
const RELATIONAL_INTERPRETER_MECHANISM_STEP_LIMIT: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RelationalInterpreterEndpointCacheKey {
    certificate_id: RelationalEndpointTotalityCertificateId,
    observation_id: RelationalMechanismReplayObservationId,
    state_value_digest: [u8; 32],
    context_value_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalEndpointReplayAuthorization {
    observation_id: RelationalMechanismReplayObservationId,
    certificate_id: RelationalEndpointTotalityCertificateId,
}

/// Production fresh-replay adapter backed by one coherent checked program.
///
/// The catalog and declaration graph are immutable and may be shared between
/// commands. Interpreter state, top-level binding values, endpoint arguments,
/// budgets, output buffers, and trace stacks are deliberately not shared. The
/// one-entry proposal cache retains only completed projected evidence.
pub(crate) struct RelationalInterpreterMechanismReplayRuntime {
    catalog: Arc<CheckedInterpreterMechanismCatalog>,
    definitions: Arc<GroundDefinitions>,
    mechanism_memo_plan: Option<CheckedMechanismRuleMemoPlan>,
    endpoint_totality_authorizations:
        BTreeMap<MechanismRequestId, RelationalEndpointReplayAuthorization>,
    required_binding_orders: BTreeMap<CheckedCallableId, Box<[String]>>,
    /// Exactly one immutable proposal, sufficient for adjacent grid edges:
    /// one edge's After is the next edge's Before. This is deliberately not a
    /// population-sized memo and never retains mutable evaluator state.
    last_complete_endpoint: Option<(
        RelationalInterpreterEndpointCacheKey,
        RelationalMechanismEndpointTraceProposal,
    )>,
    step_limit: usize,
    collection_limit: usize,
}

impl RelationalInterpreterMechanismReplayRuntime {
    /// Build from the artifact-owned runtime snapshot, never from a caller's
    /// later mutable statement buffer.
    pub(crate) fn from_checked_artifacts(
        artifacts: &TypeCheckArtifacts,
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalInterpreterMechanismReplayError> {
        artifacts
            .checked_runtime_root_program_v1()
            .map_err(RelationalInterpreterMechanismReplayError::CheckedRuntimeSnapshot)?;
        let mut definitions = checked_ground_definitions(&artifacts.analysis_program)?;
        definitions.rule_dispatch_return_types =
            artifacts.rule_dispatch_backend_return_types.clone();
        definitions.rule_dispatch_return_issues =
            artifacts.rule_dispatch_backend_return_issues.clone();
        definitions.rule_dispatch_boolean_miss_safe_keys =
            artifacts.rule_dispatch_boolean_miss_safe_keys.clone();
        let mechanism_memo_plan = artifacts.checked_mechanism_rule_memo_plan(checked);
        Self::from_checked_definitions(
            artifacts,
            checked,
            Arc::new(definitions),
            mechanism_memo_plan,
        )
    }

    /// Parent-module integration seam for callers that already hold the exact
    /// collected declaration graph used by relational elaboration.
    pub(super) fn from_checked_definitions(
        artifacts: &TypeCheckArtifacts,
        checked: &CheckedExploreQueryView<'_>,
        definitions: Arc<GroundDefinitions>,
        mechanism_memo_plan: Option<CheckedMechanismRuleMemoPlan>,
    ) -> Result<Self, RelationalInterpreterMechanismReplayError> {
        if checked.artifact.identity.analysis_program != artifacts.analysis_program.id {
            return Err(
                RelationalInterpreterMechanismReplayError::CheckedTraceCatalog(
                    CheckedInterpreterMechanismTraceError::CatalogScopeMismatch,
                ),
            );
        }
        let mut endpoint_totality_authorizations = BTreeMap::new();
        for (_, identity) in checked.analysis_nodes() {
            let CheckedExploreAnalysisIdentity::Mechanisms {
                request_id,
                observation,
                endpoint_totality,
            } = identity
            else {
                continue;
            };
            endpoint_totality.validate_identity().map_err(|error| {
                RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(
                    error.to_string(),
                )
            })?;
            if endpoint_totality.request_id() != *request_id
                || endpoint_totality.relation_id() != checked.relation_id()
            {
                return Err(
                    RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(
                        "endpoint-totality certificate is outside the checked request or relation"
                            .into(),
                    ),
                );
            }
            let observation_id = RelationalMechanismReplayObservationId::derive_checked(
                observation,
            )
            .map_err(|error| {
                RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(format!(
                    "mechanism request {request_id:?} has an invalid checked observation: {error}"
                ))
            })?;
            let authorization = RelationalEndpointReplayAuthorization {
                observation_id,
                certificate_id: endpoint_totality.certificate_id(),
            };
            match endpoint_totality_authorizations.insert(*request_id, authorization) {
                None => {}
                Some(existing) if existing == authorization => {}
                Some(_) => {
                    return Err(
                        RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(
                            "one mechanism request resolves to conflicting totality evidence"
                                .into(),
                        ),
                    )
                }
            }
        }
        let roots = checked
            .analysis_nodes()
            .filter_map(|(_, identity)| match identity {
                CheckedExploreAnalysisIdentity::Mechanisms { observation, .. } => {
                    Some((&observation.template_site, &observation.endpoint_template))
                }
                CheckedExploreAnalysisIdentity::View { .. } => None,
            });
        let catalog = CheckedInterpreterMechanismCatalog::from_checked(
            &artifacts.analysis_program,
            &artifacts.checked_resolutions,
            roots,
        )
        .map_err(RelationalInterpreterMechanismReplayError::CheckedTraceCatalog)?;
        Ok(Self {
            catalog: Arc::new(catalog),
            definitions,
            mechanism_memo_plan,
            endpoint_totality_authorizations,
            required_binding_orders: BTreeMap::new(),
            last_complete_endpoint: None,
            step_limit: RELATIONAL_INTERPRETER_MECHANISM_STEP_LIMIT,
            collection_limit: EXPLORE_GROUND_COLLECTION_LIMIT as usize,
        })
    }

    fn required_binding_order(
        &mut self,
        callable: &CheckedCallableId,
    ) -> Result<Box<[String]>, RelationalInterpreterMechanismReplayError> {
        if let Some(order) = self.required_binding_orders.get(callable) {
            return Ok(order.clone());
        }
        let checked = self.catalog.callable(callable).ok_or_else(|| {
            RelationalInterpreterMechanismReplayError::CheckedTraceCatalog(
                CheckedInterpreterMechanismTraceError::CheckedCallableMissing(callable.clone()),
            )
        })?;
        let names = self
            .definitions
            .bindings
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let roots = expression_query_dependencies(checked.body(), &names, &self.definitions);
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        for root in roots {
            append_required_binding(
                &root,
                &names,
                &self.definitions,
                &mut visiting,
                &mut visited,
                &mut order,
            )
            .map_err(RelationalInterpreterMechanismReplayError::GroundBinding)?;
        }
        let order = order.into_boxed_slice();
        self.required_binding_orders
            .insert(callable.clone(), order.clone());
        Ok(order)
    }

    fn replay_checked_endpoint(
        &mut self,
        request: RelationalMechanismEndpointReplayRequest<'_>,
    ) -> Result<RelationalMechanismEndpointReplayProgress, RelationalInterpreterMechanismReplayError>
    {
        let observation = request.observation();
        let authorization = *self
            .endpoint_totality_authorizations
            .get(&request.scope().request_id())
            .ok_or_else(|| {
                RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(
                    "mechanism endpoint replay has no request-scoped totality certificate".into(),
                )
            })?;
        let replay_observation_id = RelationalMechanismReplayObservationId::derive_checked(
            observation,
        )
        .map_err(|error| {
            RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(format!(
                "mechanism endpoint replay has an invalid observation: {error}"
            ))
        })?;
        require_endpoint_totality_authorization(
            authorization,
            request.endpoint_totality_certificate_id(),
            request.observation_id(),
            replay_observation_id,
        )?;
        let cache_key = RelationalInterpreterEndpointCacheKey {
            certificate_id: request.endpoint_totality_certificate_id(),
            observation_id: replay_observation_id,
            state_value_digest: canonical_explore_value_digest(request.state()),
            context_value_digest: canonical_explore_value_digest(request.context()),
        };
        if let Some((retained_key, retained)) = &self.last_complete_endpoint {
            if *retained_key == cache_key {
                return Ok(RelationalMechanismEndpointReplayProgress::Complete(
                    retained.clone(),
                ));
            }
        }
        let binding_order = self.required_binding_order(&observation.endpoint_template)?;

        // Fresh construction is the isolation boundary. In particular, Before
        // cannot leave memoized rule values, mutable scope state, output, or a
        // partial trace for After (or for another case).
        let mut evaluator = ExploreRuntimeGroundEvaluator::new(&self.definitions);
        if let Some(plan) = self.mechanism_memo_plan.clone() {
            evaluator
                .interpreter
                .install_checked_mechanism_rule_memo_plan(plan)
                .map_err(RelationalInterpreterMechanismReplayError::CheckedMemoPlan)?;
        }
        evaluator
            .evaluate_required_bindings(&binding_order)
            .map_err(RelationalInterpreterMechanismReplayError::GroundBinding)?;
        let base_env = evaluator.base_env.clone();
        let evaluation = evaluator.interpreter.eval_checked_mechanism_endpoint(
            Arc::clone(&self.catalog),
            &observation.template_site,
            &observation.endpoint_template,
            runtime_value_from_explore_value(request.state()),
            runtime_value_from_explore_value(request.context()),
            request.state_type(),
            request.context_type(),
            request.observation_type(),
            &base_env,
            self.step_limit,
            self.collection_limit,
        );
        let progress = match evaluation {
            Ok(trace) => project_checked_trace(&observation.template_site.analysis_program, trace)
                .map(RelationalMechanismEndpointReplayProgress::Complete),
            Err(error) => match permanent_unavailability(&error) {
                Some(reason) => {
                    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                        eprintln!(
                            "Explore mechanism replay unavailable: endpoint={:?}; case={:?}; error={error}",
                            request.endpoint(),
                            request.case_id(),
                        );
                    }
                    Ok(RelationalMechanismEndpointReplayProgress::PermanentlyUnavailable(reason))
                }
                None => {
                    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                        eprintln!(
                            "Explore certified mechanism endpoint integrity failure: endpoint={:?}; case={:?}; error={error}",
                            request.endpoint(),
                            request.case_id(),
                        );
                    }
                    Err(
                        RelationalInterpreterMechanismReplayError::CertifiedEndpointIntegrity(
                            error,
                        ),
                    )
                }
            },
        }?;
        if let RelationalMechanismEndpointReplayProgress::Complete(proposal) = &progress {
            self.last_complete_endpoint = Some((cache_key, proposal.clone()));
        }
        Ok(progress)
    }
}

fn require_endpoint_totality_authorization(
    authorization: RelationalEndpointReplayAuthorization,
    requested_certificate_id: RelationalEndpointTotalityCertificateId,
    requested_observation_id: RelationalMechanismReplayObservationId,
    replay_observation_id: RelationalMechanismReplayObservationId,
) -> Result<(), RelationalInterpreterMechanismReplayError> {
    if replay_observation_id != requested_observation_id
        || authorization.observation_id != replay_observation_id
        || authorization.certificate_id != requested_certificate_id
    {
        return Err(
            RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(
                "mechanism endpoint replay observation or certificate is not authorized by its checked request"
                    .into(),
            ),
        );
    }
    Ok(())
}

/// Reconstruct the interpreter declaration graph from the exact syntax owned
/// by Phase A. Plain/hash imports are already flattened into this sequence;
/// no replay-time filesystem import is permitted to replace a checked body.
pub(super) fn checked_ground_definitions(
    program: &CheckedAnalysisProgram,
) -> Result<GroundDefinitions, RelationalInterpreterMechanismReplayError> {
    let root_modules = program
        .declarations
        .iter()
        .filter(|declaration| matches!(&declaration.import_kind, SourcedImportKind::Root))
        .map(|declaration| declaration.id.module.clone())
        .collect::<BTreeSet<_>>();
    if root_modules.len() != 1 {
        return Err(
            RelationalInterpreterMechanismReplayError::CheckedRuntimeSnapshot(
                "checked analysis does not identify exactly one root module".into(),
            ),
        );
    }
    let root_module = root_modules.iter().next().expect("checked one root module");

    let mut groups = Vec::<(String, Vec<&Stmt>)>::new();
    for declaration in program.declarations.iter() {
        let origin = checked_declaration_origin(declaration, root_module);
        if let Some((active_origin, statements)) = groups.last_mut() {
            if active_origin == &origin {
                statements.push(declaration.statement.as_ref());
                continue;
            }
        }
        groups.push((origin, vec![declaration.statement.as_ref()]));
    }

    let mut definitions = GroundDefinitions::default();
    let mut visited = BTreeSet::new();
    let mut errors = Vec::new();
    for (origin, statements) in groups {
        collect_ground_bindings_inner(
            &statements,
            None,
            &origin,
            &mut visited,
            &mut definitions,
            &mut errors,
        );
    }
    if errors.is_empty() {
        Ok(definitions)
    } else {
        Err(RelationalInterpreterMechanismReplayError::GroundDefinitions(errors.into_boxed_slice()))
    }
}

fn checked_declaration_origin(declaration: &SourcedStmt, root_module: &ModuleId) -> String {
    if &declaration.id.module == root_module {
        return "<root>".into();
    }
    let import_kind = match &declaration.import_kind {
        SourcedImportKind::Root => "root",
        SourcedImportKind::PlainImport => "plain",
        SourcedImportKind::HashImport { selected_hash } => selected_hash.as_ref(),
        SourcedImportKind::QualifiedImport { module_name } => module_name.as_ref(),
    };
    let internal_path = declaration.id.module.internal_path.join("/");
    format!(
        "<checked:{import_kind}:{}:{internal_path}>",
        declaration.id.module.content_hash
    )
}

impl RelationalMechanismReplayRuntime for RelationalInterpreterMechanismReplayRuntime {
    type Error = RelationalInterpreterMechanismReplayError;

    fn replay_fresh_endpoint(
        &mut self,
        request: RelationalMechanismEndpointReplayRequest<'_>,
    ) -> Result<RelationalMechanismEndpointReplayProgress, Self::Error> {
        self.replay_checked_endpoint(request)
    }
}

fn permanent_unavailability(
    error: &CheckedInterpreterMechanismEvaluationError,
) -> Option<RelationalMechanismPermanentUnavailable> {
    match error {
        CheckedInterpreterMechanismEvaluationError::Runtime(
            ExploreRuntimeFailure::OperationalLimit { .. },
        )
        | CheckedInterpreterMechanismEvaluationError::Trace(
            CheckedInterpreterMechanismTraceError::TraceCapacity { .. },
        ) => Some(RelationalMechanismPermanentUnavailable::ReplayAbiCapacityExceeded),
        CheckedInterpreterMechanismEvaluationError::Trace(
            CheckedInterpreterMechanismTraceError::RuntimeExpressionMissing { .. }
            | CheckedInterpreterMechanismTraceError::AmbiguousRuntimeExpression { .. }
            | CheckedInterpreterMechanismTraceError::BoundCallableTargetMissing { .. }
            | CheckedInterpreterMechanismTraceError::AmbiguousBoundCallableTarget { .. },
        ) => Some(RelationalMechanismPermanentUnavailable::ObservationInstrumentationUnsupported),
        // Every semantic endpoint failure below this boundary contradicts the
        // request-scoped totality certificate. Surface it as an integrity
        // error; never mint a durable "unavailable" fact for a semantic miss.
        _ => None,
    }
}

fn project_checked_trace(
    analysis_program: &AnalysisProgramId,
    trace: CheckedInterpreterMechanismTrace,
) -> Result<RelationalMechanismEndpointTraceProposal, RelationalInterpreterMechanismReplayError> {
    let activation_paths = trace
        .activation_paths
        .iter()
        .map(|path| {
            let parent = path
                .parent
                .map(|parent| {
                    RelationalMechanismActivationPathId::from_index(parent.index())
                        .map_err(RelationalInterpreterMechanismReplayError::Projection)
                })
                .transpose()?;
            Ok(RelationalMechanismActivationPathNode::new(
                parent,
                project_activation(analysis_program, &path.activation)?,
            ))
        })
        .collect::<Result<Vec<_>, RelationalInterpreterMechanismReplayError>>()?;

    let occurrences = trace
        .events
        .iter()
        .map(|event| {
            let dependencies = event
                .dependencies
                .iter()
                .map(|index| {
                    RelationalMechanismOccurrenceId::from_index(*index)
                        .map_err(RelationalInterpreterMechanismReplayError::Projection)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(RelationalMechanismOccurrenceProposal::new(
                event.root_index,
                RelationalMechanismActivationPathId::from_index(event.activation_path.index())
                    .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
                project_event_site(analysis_program, &event.site)?,
                project_event_kind(event.kind),
                event.visit_ordinal,
                project_outcome(analysis_program, &event.outcome)?,
                dependencies.into_boxed_slice(),
            ))
        })
        .collect::<Result<Vec<_>, RelationalInterpreterMechanismReplayError>>()?;

    let roots = trace
        .roots
        .iter()
        .map(|index| {
            RelationalMechanismOccurrenceId::from_index(*index)
                .map_err(RelationalInterpreterMechanismReplayError::Projection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelationalMechanismEndpointTraceProposal::new(
        activation_paths.into_boxed_slice(),
        roots.into_boxed_slice(),
        occurrences.into_boxed_slice(),
    ))
}

fn project_activation(
    analysis_program: &AnalysisProgramId,
    activation: &CheckedInterpreterMechanismActivation,
) -> Result<RelationalMechanismActivationStep, RelationalInterpreterMechanismReplayError> {
    let call_site = RelationalMechanismSiteId::from_checked_expression(&activation.call_site)
        .map_err(RelationalInterpreterMechanismReplayError::Projection)?;
    let callee = match &activation.callee {
        CheckedInterpreterMechanismCallee::Function(callable) => {
            RelationalMechanismCalleeId::function(
                RelationalMechanismSiteId::from_checked_callable(analysis_program, callable)
                    .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
            )
        }
        CheckedInterpreterMechanismCallee::RuleFamily(family) => {
            RelationalMechanismCalleeId::rule_family(
                RelationalMechanismSiteId::from_checked_rule_family(analysis_program, family)
                    .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
            )
        }
    }
    .map_err(RelationalInterpreterMechanismReplayError::Projection)?;
    RelationalMechanismActivationStep::new(call_site, callee, activation.invocation_ordinal)
        .map_err(RelationalInterpreterMechanismReplayError::Projection)
}

fn project_event_site(
    analysis_program: &AnalysisProgramId,
    site: &CheckedInterpreterMechanismEventSite,
) -> Result<RelationalMechanismSiteId, RelationalInterpreterMechanismReplayError> {
    match site {
        CheckedInterpreterMechanismEventSite::Expression(site) => {
            RelationalMechanismSiteId::from_checked_expression(site)
        }
        CheckedInterpreterMechanismEventSite::RuleFamily(family) => {
            RelationalMechanismSiteId::from_checked_rule_family(analysis_program, family)
        }
        CheckedInterpreterMechanismEventSite::RuleCandidate(candidate) => {
            RelationalMechanismSiteId::from_checked_rule_candidate(analysis_program, candidate)
        }
    }
    .map_err(RelationalInterpreterMechanismReplayError::Projection)
}

const fn project_event_kind(
    kind: CheckedInterpreterMechanismEventKind,
) -> RelationalMechanismEventKind {
    match kind {
        CheckedInterpreterMechanismEventKind::RuleAttempt => {
            RelationalMechanismEventKind::RuleAttempt
        }
        CheckedInterpreterMechanismEventKind::RuleSelection => {
            RelationalMechanismEventKind::RuleSelection
        }
        CheckedInterpreterMechanismEventKind::IfDecision => {
            RelationalMechanismEventKind::IfDecision
        }
        CheckedInterpreterMechanismEventKind::MatchDecision => {
            RelationalMechanismEventKind::MatchDecision
        }
        CheckedInterpreterMechanismEventKind::ShortCircuitAnd => {
            RelationalMechanismEventKind::ShortCircuitAnd
        }
        CheckedInterpreterMechanismEventKind::ShortCircuitOr => {
            RelationalMechanismEventKind::ShortCircuitOr
        }
    }
}

fn project_outcome(
    analysis_program: &AnalysisProgramId,
    outcome: &CheckedInterpreterMechanismEventOutcome,
) -> Result<RelationalMechanismEventOutcome, RelationalInterpreterMechanismReplayError> {
    Ok(match outcome {
        CheckedInterpreterMechanismEventOutcome::RuleAttempt(outcome) => {
            RelationalMechanismEventOutcome::RuleAttempt(match outcome {
                CheckedInterpreterRuleAttemptOutcome::HeadMismatch => {
                    RelationalRuleAttemptOutcome::HeadMismatch
                }
                CheckedInterpreterRuleAttemptOutcome::GuardFalse => {
                    RelationalRuleAttemptOutcome::GuardFalse
                }
                CheckedInterpreterRuleAttemptOutcome::BodyFalse => {
                    RelationalRuleAttemptOutcome::BodyFalse
                }
                CheckedInterpreterRuleAttemptOutcome::Applicable => {
                    RelationalRuleAttemptOutcome::Applicable
                }
            })
        }
        CheckedInterpreterMechanismEventOutcome::RuleSelection(outcome) => {
            RelationalMechanismEventOutcome::RuleSelection(match outcome {
                CheckedInterpreterRuleSelectionOutcome::NoApplicableRule => {
                    RelationalRuleSelectionOutcome::NoApplicableRule
                }
                CheckedInterpreterRuleSelectionOutcome::Selected(candidate) => {
                    RelationalRuleSelectionOutcome::Selected(
                        RelationalMechanismSiteId::from_checked_rule_candidate(
                            analysis_program,
                            candidate,
                        )
                        .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
                    )
                }
            })
        }
        CheckedInterpreterMechanismEventOutcome::IfDecision(outcome) => {
            RelationalMechanismEventOutcome::IfDecision(match outcome {
                CheckedInterpreterIfDecisionOutcome::Then => RelationalIfDecisionOutcome::Then,
                CheckedInterpreterIfDecisionOutcome::Else => RelationalIfDecisionOutcome::Else,
            })
        }
        CheckedInterpreterMechanismEventOutcome::MatchDecision { arm_index } => {
            RelationalMechanismEventOutcome::MatchDecision {
                arm_index: *arm_index,
            }
        }
        CheckedInterpreterMechanismEventOutcome::ShortCircuit(outcome) => {
            RelationalMechanismEventOutcome::ShortCircuit(match outcome {
                CheckedInterpreterShortCircuitOutcome::SkippedRight { result } => {
                    RelationalShortCircuitOutcome::SkippedRight { result: *result }
                }
                CheckedInterpreterShortCircuitOutcome::EvaluatedRight { result } => {
                    RelationalShortCircuitOutcome::EvaluatedRight { result: *result }
                }
            })
        }
    })
}

#[derive(Debug)]
pub(crate) enum RelationalInterpreterMechanismReplayError {
    CheckedRuntimeSnapshot(String),
    CheckedMemoPlan(String),
    GroundDefinitions(Box<[String]>),
    CheckedTraceCatalog(CheckedInterpreterMechanismTraceError),
    GroundBinding(String),
    EndpointTotalityAuthorization(String),
    CertifiedEndpointIntegrity(CheckedInterpreterMechanismEvaluationError),
    Projection(RelationalMechanismReplayError),
    TraceIndex {
        relation: &'static str,
        event_index: usize,
        referenced_index: usize,
        event_count: usize,
    },
}

impl fmt::Display for RelationalInterpreterMechanismReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckedRuntimeSnapshot(error) => write!(
                formatter,
                "checked mechanism runtime snapshot is unavailable: {error}"
            ),
            Self::CheckedMemoPlan(error) => write!(
                formatter,
                "checked mechanism rule memo plan is unavailable: {error}"
            ),
            Self::GroundDefinitions(errors) => write!(
                formatter,
                "checked mechanism declaration graph is not executable: {}",
                errors.join("; ")
            ),
            Self::CheckedTraceCatalog(error) => {
                write!(
                    formatter,
                    "checked mechanism trace catalog is invalid: {error}"
                )
            }
            Self::GroundBinding(error) => write!(
                formatter,
                "checked mechanism immutable binding evaluation failed: {error}"
            ),
            Self::EndpointTotalityAuthorization(error) => write!(
                formatter,
                "checked mechanism endpoint totality authorization failed: {error}"
            ),
            Self::CertifiedEndpointIntegrity(error) => {
                write!(
                    formatter,
                    "certified mechanism endpoint integrity failure: {error}"
                )
            }
            Self::Projection(error) => {
                write!(
                    formatter,
                    "checked mechanism trace projection failed: {error}"
                )
            }
            Self::TraceIndex {
                relation,
                event_index,
                referenced_index,
                event_count,
            } => write!(
                formatter,
                "checked mechanism event {event_index} has {relation} index {referenced_index}, but the trace has {event_count} events"
            ),
        }
    }
}

impl std::error::Error for RelationalInterpreterMechanismReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CheckedTraceCatalog(error) => Some(error),
            Self::CertifiedEndpointIntegrity(error) => Some(error),
            Self::CheckedRuntimeSnapshot(_)
            | Self::CheckedMemoPlan(_)
            | Self::GroundDefinitions(_)
            | Self::GroundBinding(_)
            | Self::EndpointTotalityAuthorization(_)
            | Self::Projection(_)
            | Self::TraceIndex { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::mechanism::MechanismObservationIr;
    use super::*;
    use crate::{
        CheckedCallTarget, CheckedInterpreterMechanismEvent, Lexer, Parser, Rule, RuleDispatchTier,
        TypeChecker, Value,
    };

    #[test]
    fn endpoint_totality_replay_requires_the_driver_certificate() {
        let observation_id =
            RelationalMechanismReplayObservationId::from_journal_codec_bytes([0x21; 32]);
        let authorized_certificate =
            RelationalEndpointTotalityCertificateId::from_canonical_bytes([0x31; 32]);
        let authorization = RelationalEndpointReplayAuthorization {
            observation_id,
            certificate_id: authorized_certificate,
        };
        require_endpoint_totality_authorization(
            authorization,
            authorized_certificate,
            observation_id,
            observation_id,
        )
        .expect("matching checked authorization");

        let different_certificate =
            RelationalEndpointTotalityCertificateId::from_canonical_bytes([0x32; 32]);
        assert!(matches!(
            require_endpoint_totality_authorization(
                authorization,
                different_certificate,
                observation_id,
                observation_id,
            ),
            Err(RelationalInterpreterMechanismReplayError::EndpointTotalityAuthorization(_))
        ));
    }

    #[test]
    fn endpoint_totality_certified_failures_distinguish_instrumentation_from_semantics() {
        let semantic_failure = CheckedInterpreterMechanismEvaluationError::Runtime(
            ExploreRuntimeFailure::UnsupportedCapability {
                message: "effectful observer escaped totality certification".into(),
            },
        );
        let instrumentation_failure = CheckedInterpreterMechanismEvaluationError::Trace(
            CheckedInterpreterMechanismTraceError::BoundCallableTargetMissing { arity: 1 },
        );
        assert_eq!(
            (
                permanent_unavailability(&semantic_failure),
                permanent_unavailability(&instrumentation_failure),
            ),
            (
                None,
                Some(
                    RelationalMechanismPermanentUnavailable::ObservationInstrumentationUnsupported,
                ),
            ),
        );
    }

    fn checked_mechanism_trace_artifacts(
        source: &str,
        rewrite: impl FnOnce(&mut [Stmt]),
    ) -> TypeCheckArtifacts {
        let mut lexer = Lexer::new(source);
        let mut statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse checked mechanism trace fixture");
        rewrite(&mut statements);
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "checked mechanism trace fixture diagnostics: {:?}; rule returns: {:?}; rule issues: {:?}",
            artifacts.diagnostics,
            artifacts.rule_dispatch_return_types,
            artifacts.rule_dispatch_return_issues,
        );
        artifacts
    }

    fn raw_checked_interpreter_mechanism_trace(
        source: &str,
        state: i64,
    ) -> CheckedInterpreterMechanismTrace {
        raw_checked_interpreter_mechanism_trace_after_parse(source, state, |_| {})
    }

    fn raw_checked_interpreter_mechanism_trace_after_parse(
        source: &str,
        state: i64,
        rewrite: impl FnOnce(&mut [Stmt]),
    ) -> CheckedInterpreterMechanismTrace {
        let artifacts = checked_mechanism_trace_artifacts(source, rewrite);
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("join checked mechanism trace Explore query");
        let observation = checked
            .analysis_nodes()
            .find_map(|(_, identity)| match identity {
                CheckedExploreAnalysisIdentity::Mechanisms { observation, .. } => {
                    Some(observation.clone())
                }
                CheckedExploreAnalysisIdentity::View { .. } => None,
            })
            .expect("fixture declares one mechanism observer");

        let mut runtime = RelationalInterpreterMechanismReplayRuntime::from_checked_artifacts(
            &artifacts, &checked,
        )
        .expect("build checked interpreter mechanism runtime");

        evaluate_checked_interpreter_mechanism_endpoint(&mut runtime, &observation, state)
            .expect("evaluate one fresh checked mechanism endpoint")
    }

    fn evaluate_checked_interpreter_mechanism_endpoint(
        runtime: &mut RelationalInterpreterMechanismReplayRuntime,
        observation: &MechanismObservationIr,
        state: i64,
    ) -> Result<CheckedInterpreterMechanismTrace, CheckedInterpreterMechanismEvaluationError> {
        let binding_order = runtime
            .required_binding_order(&observation.endpoint_template)
            .expect("derive observer binding order");
        let mut evaluator = ExploreRuntimeGroundEvaluator::new(&runtime.definitions);
        if let Some(plan) = runtime.mechanism_memo_plan.clone() {
            evaluator
                .interpreter
                .install_checked_mechanism_rule_memo_plan(plan)
                .expect("install checked mechanism memo plan");
        }
        evaluator
            .evaluate_required_bindings(&binding_order)
            .expect("evaluate observer bindings");
        let base_env = evaluator.base_env.clone();
        evaluator.interpreter.eval_checked_mechanism_endpoint(
            Arc::clone(&runtime.catalog),
            &observation.template_site,
            &observation.endpoint_template,
            Value::Int(state),
            Value::Unit,
            &observation.state_type,
            &observation.context_type,
            &observation.observation_type,
            &base_env,
            runtime.step_limit,
            runtime.collection_limit,
        )
    }

    #[test]
    fn flat_map_summary_certificate_allows_deterministic_cold_endpoint_replay() {
        let source = r#"
> observe_optional_items(state: Int, context: Unit) -> Int {
    length(flat_map(
        filter([state, state + 1], |item: Int| item > 0),
        |item: Int| if item > 1 { [item, item + 1] } else { [] }
    ))
}
? explore optional_endpoint_items {
    from {
        vary before in range(0, 3)
        given context = ()
    }
    transition after = before + 1
    find cases = all
    mechanisms paths from find cases using observe_optional_items
}
"#;
        // Each call reconstructs both the checked proof and a fresh runtime.
        // Cover all Before/After endpoint values, including the empty result.
        for state in 0..=3 {
            let cold = raw_checked_interpreter_mechanism_trace(source, state);
            assert!(!cold.events.is_empty());
            assert_eq!(cold, raw_checked_interpreter_mechanism_trace(source, state));
        }
    }

    fn rewrite_clause_as_unconditional_default(statement: &mut Stmt) {
        let Stmt::Rule(Rule::Clause {
            head,
            body: Some(value),
        }) = statement
        else {
            panic!("unconditional-default fixture must start as one valued clause");
        };
        *statement = Stmt::Rule(Rule::Default {
            head: head.clone(),
            value: value.clone(),
            condition: None,
        });
    }

    fn event_rule_family<'a>(
        trace: &'a CheckedInterpreterMechanismTrace,
        event: &CheckedInterpreterMechanismEvent,
    ) -> Option<&'a crate::RuleDispatchKey> {
        let activation = &trace.activation_paths[event.activation_path.index()].activation;
        match &activation.callee {
            CheckedInterpreterMechanismCallee::RuleFamily(family) => Some(family),
            CheckedInterpreterMechanismCallee::Function(_) => None,
        }
    }

    #[test]
    fn checked_rule_trace_preserves_authoritative_tier_order_and_selected_prefix() {
        let trace = raw_checked_interpreter_mechanism_trace_after_parse(
            r#"
| cascade(value: Int) -> False
| cascade(value: Int) -> True
| cascade(value: Int) -> True
| cascade(value: Int) -> True under False
| exception literal_miss cascade(99) -> True

> observe_cascade(state: Int, context: Unit) -> Bool {
    cascade(state)
}

? explore trace_cascade {
    from {
        given before = 7
        given context = ()
    }
    transition after = before
    find all_cases = all
    mechanisms paths from find all_cases using observe_cascade
}
"#,
            7,
            |statements| {
                // The parser currently has no spelling for this AST tier. Keep
                // the dispatch contract covered without inventing syntax.
                rewrite_clause_as_unconditional_default(&mut statements[1]);
                rewrite_clause_as_unconditional_default(&mut statements[2]);
            },
        );

        let cascade_events = trace
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                event_rule_family(&trace, event)
                    .is_some_and(|family| family.scope.is_none() && family.name == "cascade")
            })
            .collect::<Vec<_>>();
        assert_eq!(cascade_events.len(), 5);

        let expected_attempts = [
            (
                RuleDispatchTier::Exception,
                4,
                CheckedInterpreterRuleAttemptOutcome::HeadMismatch,
            ),
            (
                RuleDispatchTier::ConditionalDefault,
                3,
                CheckedInterpreterRuleAttemptOutcome::GuardFalse,
            ),
            (
                RuleDispatchTier::Clause,
                0,
                CheckedInterpreterRuleAttemptOutcome::BodyFalse,
            ),
            (
                RuleDispatchTier::UnconditionalDefault,
                1,
                CheckedInterpreterRuleAttemptOutcome::Applicable,
            ),
        ];
        for ((event_index, event), (tier, source_order, outcome)) in cascade_events
            .iter()
            .take(expected_attempts.len())
            .zip(expected_attempts)
        {
            let CheckedInterpreterMechanismEventSite::RuleCandidate(candidate) = &event.site else {
                panic!("event {event_index} is not a rule attempt candidate");
            };
            assert_eq!(
                (candidate.tier, candidate.source_order),
                (tier, source_order)
            );
            assert_eq!(
                event.outcome,
                CheckedInterpreterMechanismEventOutcome::RuleAttempt(outcome)
            );
        }

        let (selection_index, selection) = cascade_events[4];
        let CheckedInterpreterMechanismEventOutcome::RuleSelection(
            CheckedInterpreterRuleSelectionOutcome::Selected(selected),
        ) = &selection.outcome
        else {
            panic!("event {selection_index} is not a selected-rule closure");
        };
        let CheckedInterpreterMechanismEventSite::RuleCandidate(applicable) =
            &cascade_events[3].1.site
        else {
            unreachable!("fourth reached event is the applicable candidate")
        };
        assert_eq!(selected, applicable);
        assert_eq!(
            (selected.tier, selected.source_order),
            (RuleDispatchTier::UnconditionalDefault, 1)
        );
        assert_eq!(selection.dependencies.as_ref(), &[0, 1, 2, 3]);
        assert_eq!(trace.roots.as_ref(), &[selection_index]);
        assert!(trace.events.iter().all(|event| !matches!(
            &event.site,
            CheckedInterpreterMechanismEventSite::RuleCandidate(candidate)
                if candidate.source_order == 2
        )));
    }

    #[test]
    fn checked_rule_trace_closes_no_applicable_rule_after_complete_failed_prefix() {
        let trace = raw_checked_interpreter_mechanism_trace(
            r#"
| never_applies(value: Int) -> False
| never_applies(value: Int) -> True under False
| exception literal_miss never_applies(99) -> True

> observe_miss(state: Int, context: Unit) -> Bool {
    never_applies(state)
}

? explore trace_no_applicable {
    from {
        given before = 7
        given context = ()
    }
    transition after = before
    find all_cases = all
    mechanisms paths from find all_cases using observe_miss
}
"#,
            7,
        );

        let family_events = trace
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                event_rule_family(&trace, event)
                    .is_some_and(|family| family.scope.is_none() && family.name == "never_applies")
            })
            .collect::<Vec<_>>();
        assert_eq!(family_events.len(), 4);
        assert_eq!(
            family_events[..3]
                .iter()
                .map(|(_, event)| match &event.outcome {
                    CheckedInterpreterMechanismEventOutcome::RuleAttempt(outcome) => *outcome,
                    other => panic!("failed prefix contains non-attempt event: {other:?}"),
                })
                .collect::<Vec<_>>(),
            vec![
                CheckedInterpreterRuleAttemptOutcome::HeadMismatch,
                CheckedInterpreterRuleAttemptOutcome::GuardFalse,
                CheckedInterpreterRuleAttemptOutcome::BodyFalse,
            ]
        );
        let (selection_index, selection) = family_events[3];
        assert_eq!(
            selection.outcome,
            CheckedInterpreterMechanismEventOutcome::RuleSelection(
                CheckedInterpreterRuleSelectionOutcome::NoApplicableRule
            )
        );
        assert_eq!(selection.dependencies.as_ref(), &[0, 1, 2]);
        assert_eq!(trace.roots.as_ref(), &[selection_index]);
    }

    #[test]
    fn checked_rule_trace_rejects_an_incomplete_no_applicable_prefix() {
        let source = r#"
| never_applies(value: Int) -> False

> observe_miss(state: Int, context: Unit) -> Bool {
    never_applies(state)
}

? explore trace_incomplete_prefix {
    from {
        given before = 7
        given context = ()
    }
    transition after = before
    find all_cases = all
    mechanisms paths from find all_cases using observe_miss
}
"#;
        let artifacts = checked_mechanism_trace_artifacts(source, |_| {});
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("join incomplete-prefix fixture");
        let observation = checked
            .analysis_nodes()
            .find_map(|(_, identity)| match identity {
                CheckedExploreAnalysisIdentity::Mechanisms { observation, .. } => {
                    Some(observation.clone())
                }
                CheckedExploreAnalysisIdentity::View { .. } => None,
            })
            .expect("fixture declares one mechanism observer");
        let mut runtime = RelationalInterpreterMechanismReplayRuntime::from_checked_artifacts(
            &artifacts, &checked,
        )
        .expect("build incomplete-prefix checked runtime");
        let family = runtime
            .catalog
            .rule_family_orders
            .keys()
            .find(|family| family.scope.is_none() && family.name == "never_applies")
            .cloned()
            .expect("catalog retains the checked family roster");
        let catalog = Arc::get_mut(&mut runtime.catalog)
            .expect("test runtime has the only checked catalog reference");
        let order = catalog
            .rule_family_orders
            .get_mut(&family)
            .expect("checked family order remains present");
        let mut corrupted_order = order.to_vec();
        corrupted_order.push(usize::MAX);
        *order = corrupted_order.into_boxed_slice();

        let error = evaluate_checked_interpreter_mechanism_endpoint(&mut runtime, &observation, 7)
            .expect_err("NoApplicableRule cannot close an incomplete checked family prefix");
        assert_eq!(
            error,
            CheckedInterpreterMechanismEvaluationError::Trace(
                CheckedInterpreterMechanismTraceError::RuleDispatchTraceMismatch {
                    family,
                    reason: "selection does not close the exact checked candidate prefix",
                }
            )
        );
    }

    #[test]
    fn checked_rulescope_memo_reuse_has_fresh_scoped_activation_and_cold_provenance() {
        let trace = raw_checked_interpreter_mechanism_trace(
            r#"
# MemoScope() {
    | leaf() -> True
    | total() -> leaf() == leaf()
}

> observe_scope(state: Int, context: Unit) -> Bool {
    MemoScope().total()
}

? explore trace_scoped_memo {
    from {
        given before = 7
        given context = ()
    }
    transition after = before
    find all_cases = all
    mechanisms paths from find all_cases using observe_scope
}
"#,
            7,
        );

        let total_activations = trace
            .activation_paths
            .iter()
            .filter(|path| {
                matches!(
                    &path.activation.callee,
                    CheckedInterpreterMechanismCallee::RuleFamily(family)
                        if family.scope.as_deref() == Some("MemoScope")
                            && family.name == "total"
                            && family.arity == 0
                )
            })
            .count();
        assert_eq!(total_activations, 1);

        let leaf_selections = trace
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                event_rule_family(&trace, event).is_some_and(|family| {
                    family.scope.as_deref() == Some("MemoScope")
                        && family.name == "leaf"
                        && family.arity == 0
                }) && event.kind == CheckedInterpreterMechanismEventKind::RuleSelection
            })
            .collect::<Vec<_>>();
        assert_eq!(leaf_selections.len(), 2);
        let (cold_index, cold) = leaf_selections[0];
        let (hot_index, hot) = leaf_selections[1];

        assert_ne!(cold.activation_path, hot.activation_path);
        let cold_activation = &trace.activation_paths[cold.activation_path.index()];
        let hot_activation = &trace.activation_paths[hot.activation_path.index()];
        assert_eq!(cold_activation.parent, hot_activation.parent);
        assert_eq!(
            cold_activation.activation.callee,
            hot_activation.activation.callee
        );
        assert!(matches!(
            &cold_activation.activation.callee,
            CheckedInterpreterMechanismCallee::RuleFamily(family)
                if family.scope.as_deref() == Some("MemoScope")
                    && family.name == "leaf"
                    && family.arity == 0
        ));

        let CheckedInterpreterMechanismEventOutcome::RuleSelection(cold_outcome) = &cold.outcome
        else {
            unreachable!("filtered cold selection outcome")
        };
        let CheckedInterpreterMechanismEventOutcome::RuleSelection(hot_outcome) = &hot.outcome
        else {
            unreachable!("filtered hot selection outcome")
        };
        assert_eq!(hot_outcome, cold_outcome);
        assert_eq!(hot.dependencies.as_ref(), &[cold_index]);
        assert!(cold.dependencies.len() == 1 && cold.dependencies[0] < cold_index);
        assert!(trace.events.iter().enumerate().all(|(event_index, event)| {
            event_index == hot_index
                || event.activation_path != hot.activation_path
                || event.kind != CheckedInterpreterMechanismEventKind::RuleAttempt
        }));
    }

    #[test]
    fn checked_rulescope_target_cannot_fall_through_to_function_or_global_dispatch() {
        let source = r#"
# RuleOnlyScope(seed: Int) {
    | total() -> seed > 0
}

> observe_rule(state: Int, context: Unit) -> Bool {
    RuleOnlyScope(seed = state).total()
}

? explore trace_unresolved_scoped_member {
    from {
        given before = 7
        given context = ()
    }
    transition after = before
    find all_cases = all
    mechanisms paths from find all_cases using observe_rule
}
"#;
        let mut artifacts = checked_mechanism_trace_artifacts(source, |_| {});
        let owned_checked = artifacts
            .checked_exploration_query(0)
            .expect("join unresolved-member fixture before corrupting its checked target")
            .to_owned_checked_query();
        let (scoped_site, owner_type, member, arity, family) = artifacts
            .checked_resolutions
            .expressions
            .iter()
            .find_map(|(site, resolution)| match &resolution.call_target {
                Some(CheckedCallTarget::ScopedMember {
                    owner_type,
                    member,
                    arity,
                    rule_family: Some(family),
                }) if owner_type.as_ref() == "RuleOnlyScope"
                    && member.as_ref() == "total"
                    && *arity == 0 =>
                {
                    Some((
                        site.clone(),
                        owner_type.clone(),
                        member.clone(),
                        *arity,
                        family.clone(),
                    ))
                }
                _ => None,
            })
            .expect("checked fixture resolves total() to its exact scoped rule family");
        assert_eq!(family.scope.as_deref(), Some("RuleOnlyScope"));
        assert_eq!((family.name.as_str(), family.arity), ("total", 0));
        artifacts
            .checked_resolutions
            .expressions
            .get_mut(&scoped_site)
            .expect("scoped rule resolution remains present")
            .call_target = Some(CheckedCallTarget::ScopedMember {
            owner_type: owner_type.clone(),
            member: member.clone(),
            arity,
            rule_family: None,
        });

        let checked = owned_checked.view();
        let observation = checked
            .analysis_nodes()
            .find_map(|(_, identity)| match identity {
                CheckedExploreAnalysisIdentity::Mechanisms { observation, .. } => {
                    Some(observation.clone())
                }
                CheckedExploreAnalysisIdentity::View { .. } => None,
            })
            .expect("fixture declares one mechanism observer");
        let mut definitions = checked_ground_definitions(&artifacts.analysis_program)
            .expect("rebuild checked unresolved-member definitions");
        definitions.rule_dispatch_return_types =
            artifacts.rule_dispatch_backend_return_types.clone();
        definitions.rule_dispatch_return_issues =
            artifacts.rule_dispatch_backend_return_issues.clone();
        definitions.rule_dispatch_boolean_miss_safe_keys =
            artifacts.rule_dispatch_boolean_miss_safe_keys.clone();
        let mut runtime = RelationalInterpreterMechanismReplayRuntime::from_checked_definitions(
            &artifacts,
            &checked,
            Arc::new(definitions),
            None,
        )
        .expect("build catalog with deliberately unresolved scoped callable target");

        let error = evaluate_checked_interpreter_mechanism_endpoint(&mut runtime, &observation, 7)
            .expect_err("a scoped rule cannot fall through to another callable namespace");
        assert_eq!(
            error,
            CheckedInterpreterMechanismEvaluationError::Trace(
                CheckedInterpreterMechanismTraceError::ScopedCallableMissing {
                    owner_type: owner_type.into(),
                    member: member.into(),
                    arity,
                }
            )
        );
    }
}
